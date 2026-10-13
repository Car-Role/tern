//! Searching inside repos: file contents (`git grep`) and commit messages (`git log --grep`).

use crate::git::{self, Commit};
use crate::run;
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub path: String,
    pub line: u32,
    /// The matching line, cut to 300 characters.
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TextResult {
    pub hits: Vec<Hit>,
    /// More matches exist than were read.
    pub truncated: bool,
}

/// What to look for and where. `paths` limits the search to those files or folders (empty means the whole repo).
#[derive(serde::Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Opts {
    pub regex: bool,
    pub case_sensitive: bool,
    /// Match whole words only.
    pub word: bool,
    /// The pattern is a Perl-style regular expression (git grep -P), for word edges and `\s`.
    pub perl: bool,
    pub paths: Vec<String>,
}

/// Searches tracked and new (not ignored) files. Binary files are skipped. Stops after `limit` hits.
pub fn text(repo: &str, query: &str, regex: bool, case_sensitive: bool, limit: usize) -> Result<TextResult, String> {
    text_with(repo, query, &Opts { regex, case_sensitive, ..Default::default() }, limit)
}

pub fn text_with(repo: &str, query: &str, o: &Opts, limit: usize) -> Result<TextResult, String> {
    let (regex, case_sensitive) = (o.regex, o.case_sensitive);
    if query.is_empty() {
        return Ok(TextResult { hits: Vec::new(), truncated: false });
    }
    let mut cmd = run::command("git");
    cmd.arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=off", "-c", "color.ui=never", "-c", "grep.lineNumber=true"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["grep", "-z", "-n", "-I", "--untracked", "--max-count=20", "--no-color"]);
    if !case_sensitive {
        cmd.arg("-i");
    }
    cmd.arg(if o.perl { "-P" } else if regex { "-E" } else { "-F" });
    if o.word {
        cmd.arg("-w");
    }
    cmd.arg("-e").arg(query);
    if !o.paths.is_empty() {
        cmd.arg("--").args(&o.paths);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("could not start git: {e}"))?;
    let out = child.stdout.take().ok_or("no output")?;
    let mut hits = Vec::new();
    let mut truncated = false;
    // Lines look like "path\0line\0text".
    for raw in BufReader::new(out).split(b'\n') {
        let Ok(raw) = raw else { break };
        let mut parts = raw.splitn(3, |b| *b == 0);
        let (Some(path), Some(line), Some(text)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let text = String::from_utf8_lossy(text);
        let text: String = text.trim_end_matches('\r').chars().take(300).collect();
        hits.push(Hit {
            path: String::from_utf8_lossy(path).into_owned(),
            line: String::from_utf8_lossy(line).parse().unwrap_or(1),
            text,
        });
        if hits.len() >= limit {
            truncated = true;
            break;
        }
    }
    if truncated {
        let _ = child.kill();
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    // git grep exits with 1 when nothing matches.
    if !truncated && !status.success() && status.code() != Some(1) {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut err);
        }
        let err = err.trim();
        if !err.is_empty() {
            return Err(err.to_string());
        }
    }
    Ok(TextResult { hits, truncated })
}

/// Commits on any branch whose message matches.
pub fn commits(repo: &str, query: &str, regex: bool, case_sensitive: bool) -> Result<Vec<Commit>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let grep = format!("--grep={query}");
    let mut args = vec!["log", "--branches", "--remotes", "--tags", "-n", "30", "--date-order", git::LOG_FORMAT, &grep];
    if !case_sensitive {
        args.push("-i");
    }
    if !regex {
        args.push("--fixed-strings");
    } else {
        args.push("--extended-regexp");
    }
    let text = run::git(Path::new(repo), &args)?;
    Ok(git::parse_log_pub(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Times a search in real repos: `SCOUT_REPOS="C:\\a;C:\\b" cargo test print_search -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_search() {
        let q = std::env::var("SCOUT_QUERY").unwrap_or_else(|_| "TODO".into());
        for repo in std::env::var("SCOUT_REPOS").unwrap_or_default().split(';').filter(|r| !r.is_empty()) {
            let t = std::time::Instant::now();
            let r = text(repo, &q, false, false, 60);
            let ms = t.elapsed().as_millis();
            match r {
                Ok(r) => println!("{ms:>5} ms  {:>3} hits{}  {repo}", r.hits.len(), if r.truncated { "+" } else { " " }),
                Err(e) => println!("{ms:>5} ms  error {e}  {repo}"),
            }
        }
    }

    #[test]
    fn finds_text_and_commits() {
        let dir = std::env::temp_dir().join(format!("tern-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let r = dir.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&dir, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        fs::write(dir.join("src/a.ts"), "const x = 1;\nfunction loadSession() {}\n// loadsession again\n").unwrap();
        fs::write(dir.join(".gitignore"), "ignored/\n").unwrap();
        fs::create_dir_all(dir.join("ignored")).unwrap();
        fs::write(dir.join("ignored/b.ts"), "loadSession").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "Add loadSession helper"]);
        fs::write(dir.join("new file.ts"), "loadSession(); // [x]\n").unwrap();

        let res = text(&r, "loadSession", false, true, 100).unwrap();
        let found: Vec<(String, u32)> = res.hits.iter().map(|h| (h.path.clone(), h.line)).collect();
        assert!(found.contains(&("src/a.ts".into(), 2)), "{found:?}");
        assert!(found.contains(&("new file.ts".into(), 1)), "untracked files are searched: {found:?}");
        assert!(!found.iter().any(|(p, _)| p.starts_with("ignored/")), "ignored files are skipped");
        assert_eq!(found.iter().filter(|(p, _)| p == "src/a.ts").count(), 1, "case matters when asked");
        assert_eq!(text(&r, "loadsession", false, false, 100).unwrap().hits.iter().filter(|h| h.path == "src/a.ts").count(), 2);
        assert!(!text(&r, "[x]", false, true, 100).unwrap().hits.is_empty(), "plain text is not a pattern");
        assert!(text(&r, "load.*\\(", true, true, 100).unwrap().hits.len() >= 2);
        assert!(text(&r, "nothing-like-this", false, true, 100).unwrap().hits.is_empty());
        let one = text(&r, "loadSession", false, false, 1).unwrap();
        assert!(one.truncated && one.hits.len() == 1);

        // Whole words, and one file only.
        let w = Opts { word: true, ..Default::default() };
        assert!(text_with(&r, "loadSess", &w, 100).unwrap().hits.is_empty(), "part of a word is not a whole word");
        assert!(!text_with(&r, "loadSession", &w, 100).unwrap().hits.is_empty());
        let perl = Opts { perl: true, ..Default::default() };
        let defs = text_with(&r, r"\bfunction\s+loadSession\b", &perl, 100).unwrap().hits;
        assert_eq!(defs.len(), 1, "{defs:?}");
        let one_file = Opts { paths: vec!["src/a.ts".into()], ..Default::default() };
        let hits = text_with(&r, "loadSession", &one_file, 100).unwrap().hits;
        assert!(!hits.is_empty() && hits.iter().all(|h| h.path == "src/a.ts"), "{hits:?}");

        let c = commits(&r, "helper", false, false).unwrap();
        assert_eq!(c.len(), 1);
        assert!(commits(&r, "missing", false, false).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
