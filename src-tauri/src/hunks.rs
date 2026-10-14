//! Hunk staging: stage, unstage, or discard one block of changes in a file instead of the whole file.
//! The hunks come from `git diff`, and each one is applied on its own with `git apply`.

use crate::run;
use serde::Serialize;
use std::path::Path;

fn p(repo: &str) -> &Path {
    Path::new(repo)
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    /// The `@@ -12,6 +12,8 @@ fn name` line.
    pub header: String,
    /// The first changed line in the new file, for jumping there.
    pub new_start: u32,
    pub added: u32,
    pub removed: u32,
    /// The hunk's lines, each starting with "+", "-", " ", or "\".
    pub lines: Vec<String>,
    /// A complete patch with only this hunk, ready for `git apply`.
    pub patch: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileHunks {
    /// Changes in the working tree that are not staged.
    pub unstaged: Vec<Hunk>,
    /// Changes in the index (staged).
    pub staged: Vec<Hunk>,
    /// Git shows no lines for binary files.
    pub binary: bool,
}

/// Splits `git diff` output for one file into hunks, each with the file header in front.
pub fn parse(diff: &str) -> (Vec<Hunk>, bool) {
    let binary = diff.lines().any(|l| l.starts_with("Binary files ") || l == "GIT binary patch");
    let mut header = String::new();
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut cur: Option<(String, Vec<String>)> = None;
    let finish = |cur: &mut Option<(String, Vec<String>)>, hunks: &mut Vec<Hunk>, header: &str| {
        if let Some((h, lines)) = cur.take() {
            let new_start = h
                .split(' ')
                .find(|t| t.starts_with('+'))
                .and_then(|t| t[1..].split(',').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(1);
            let added = lines.iter().filter(|l| l.starts_with('+')).count() as u32;
            let removed = lines.iter().filter(|l| l.starts_with('-')).count() as u32;
            let mut patch = String::from(header);
            patch.push_str(&h);
            patch.push('\n');
            for l in &lines {
                patch.push_str(l);
                patch.push('\n');
            }
            hunks.push(Hunk { header: h, new_start, added, removed, lines, patch });
        }
    };
    for line in diff.lines() {
        if line.starts_with("@@") {
            finish(&mut cur, &mut hunks, &header);
            cur = Some((line.to_string(), Vec::new()));
        } else if let Some((_, lines)) = cur.as_mut() {
            if line.starts_with(['+', '-', ' ', '\\']) || line.is_empty() {
                // An empty line inside a hunk is an unchanged blank line whose leading space was trimmed.
                lines.push(if line.is_empty() { " ".to_string() } else { line.to_string() });
            }
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    finish(&mut cur, &mut hunks, &header);
    (hunks, binary)
}

pub fn file_hunks(repo: &str, rel: &str) -> Result<FileHunks, String> {
    let args = |cached: bool| {
        let mut a = vec!["diff", "--no-color", "--no-ext-diff", "-U3"];
        if cached {
            a.push("--cached");
        }
        a.extend(["--", rel]);
        a
    };
    let (unstaged, b1) = parse(&run::git(p(repo), &args(false))?);
    let (staged, b2) = parse(&run::git(p(repo), &args(true))?);
    Ok(FileHunks { unstaged, staged, binary: b1 || b2 })
}

/// stage: index gets the hunk. unstage: the hunk leaves the index. discard: the hunk leaves the working tree.
/// Discard returns a backup commit (from `git stash create`) so it can be undone.
pub fn apply(repo: &str, patch: &str, mode: &str) -> Result<String, String> {
    let backup = if mode == "discard" {
        run::git(p(repo), &["stash", "create"]).map(|s| s.trim().to_string()).unwrap_or_default()
    } else {
        String::new()
    };
    let args: &[&str] = match mode {
        "stage" => &["apply", "--cached", "--whitespace=nowarn", "-"],
        "unstage" => &["apply", "--cached", "--reverse", "--whitespace=nowarn", "-"],
        "discard" => &["apply", "--reverse", "--whitespace=nowarn", "-"],
        _ => return Err(format!("unknown hunk action: {mode}")),
    };
    run::git_input(p(repo), args, patch).map_err(|e| {
        if e.contains("does not apply") || e.contains("patch failed") {
            "The file changed since Tern read it. Refresh and try again.".to_string()
        } else {
            e
        }
    })?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn splits_a_diff_into_hunks() {
        let diff = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -10,2 +10,3 @@ fn x\n ten\n+ten and a half\n eleven\n";
        let (h, binary) = parse(diff);
        assert!(!binary);
        assert_eq!(h.len(), 2);
        assert_eq!((h[0].new_start, h[0].added, h[0].removed), (1, 1, 1));
        assert_eq!((h[1].new_start, h[1].added, h[1].removed), (10, 1, 0));
        assert!(h[1].patch.starts_with("diff --git a/a.txt b/a.txt\n"));
        assert!(h[1].patch.contains("@@ -10,2 +10,3 @@ fn x\n ten\n+ten and a half\n eleven\n"));
        assert!(!h[1].patch.contains("TWO"), "each patch holds one hunk only");
    }

    #[test]
    fn stages_unstages_and_discards_one_hunk() {
        let dir = std::env::temp_dir().join(format!("tern-hunks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&dir, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        let base: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        fs::write(dir.join("a.txt"), &base).unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);

        // Two separate changes, far apart: two hunks.
        let changed = base.replace("line 2\n", "line 2 changed\n").replace("line 28\n", "line 28 changed\n");
        fs::write(dir.join("a.txt"), &changed).unwrap();
        let h = file_hunks(&repo, "a.txt").unwrap();
        assert_eq!((h.unstaged.len(), h.staged.len()), (2, 0));

        // Stage only the second one.
        apply(&repo, &h.unstaged[1].patch, "stage").unwrap();
        let staged = g(&["diff", "--cached"]);
        assert!(staged.contains("line 28 changed") && !staged.contains("line 2 changed"), "{staged}");
        let h = file_hunks(&repo, "a.txt").unwrap();
        assert_eq!((h.unstaged.len(), h.staged.len()), (1, 1));

        // Unstage it again.
        apply(&repo, &h.staged[0].patch, "unstage").unwrap();
        assert!(g(&["diff", "--cached"]).is_empty());

        // Discard the first change only; the second stays on disk.
        let h = file_hunks(&repo, "a.txt").unwrap();
        let backup = apply(&repo, &h.unstaged[0].patch, "discard").unwrap();
        assert!(!backup.is_empty());
        // git may write Windows line endings (core.autocrlf), like any checkout does.
        let now = fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
        assert!(now.contains("line 2\n") && now.contains("line 28 changed"), "{now}");

        assert!(apply(&repo, "not a patch", "stage").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
