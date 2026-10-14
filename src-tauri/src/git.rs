//! Git reads (status, diffs, history, file contents) and network actions (fetch, pull, push).

use crate::run;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Files bigger than this are not shown in the viewer.
const MAX_VIEW_BYTES: u64 = 5 * 1024 * 1024;
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    /// added | modified | deleted | renamed | untracked | conflict | copied | typechange
    pub kind: String,
    /// True when some of the change is staged.
    pub staged: bool,
    /// True when some of the change is not staged yet.
    pub unstaged: bool,
    /// Unix seconds when the file last changed on disk (working-tree changes only).
    pub mtime: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub sha: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub time: i64,
    pub refs: String,
    /// Parent commit ids. Two or more means a merge.
    pub parents: Vec<String>,
    pub email: String,
    /// The message after the subject line, cut to 800 characters.
    pub body: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub path: String,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    pub changes: Vec<Change>,
    pub last: Option<Commit>,
    pub remote: Option<String>,
    /// github | azdo | other | none
    pub provider: String,
    /// Unix seconds of the last fetch, when known.
    pub fetched_at: Option<i64>,
    /// Unix seconds of the newest change: a changed file saved on disk, or the last commit.
    pub activity: Option<i64>,
    /// Changes whenever a branch, tag, or HEAD moves (commit, pull, fetch, checkout). Unix milliseconds.
    pub refs_stamp: Option<i64>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    pub text: Option<String>,
    pub binary: bool,
    pub too_large: bool,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub old: FileText,
    pub new: FileText,
}

fn p(repo: &str) -> &Path {
    Path::new(repo)
}

/// Joins a repo-relative path and refuses paths that leave the repo.
pub fn safe_join(repo: &str, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!("path is outside the repo: {rel}"));
    }
    Ok(Path::new(repo).join(rel_path))
}

pub fn provider_of(url: &str) -> &'static str {
    let u = url.to_lowercase();
    if u.contains("github.com") {
        "github"
    } else if u.contains("dev.azure.com") || u.contains("visualstudio.com") {
        "azdo"
    } else {
        "other"
    }
}

fn to_text(bytes: Vec<u8>, size: u64) -> FileText {
    let head = &bytes[..bytes.len().min(8000)];
    if head.contains(&0) {
        return FileText { text: None, binary: true, too_large: false, size };
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if text.starts_with('\u{feff}') {
        text.remove(0);
    }
    FileText { text: Some(text), binary: false, too_large: false, size }
}

fn missing() -> FileText {
    FileText { text: Some(String::new()), ..Default::default() }
}

pub fn read_file(repo: &str, rel: &str) -> Result<FileText, String> {
    let full = safe_join(repo, rel)?;
    let meta = fs::metadata(&full).map_err(|e| format!("{rel}: {e}"))?;
    if meta.len() > MAX_VIEW_BYTES {
        return Ok(FileText { text: None, binary: false, too_large: true, size: meta.len() });
    }
    let bytes = fs::read(&full).map_err(|e| format!("{rel}: {e}"))?;
    Ok(to_text(bytes, meta.len()))
}

pub fn write_file(repo: &str, rel: &str, text: &str) -> Result<(), String> {
    let full = safe_join(repo, rel)?;
    // Keep the file's existing line endings when it uses CRLF.
    let crlf = fs::read(&full).map(|b| b.windows(2).any(|w| w == b"\r\n")).unwrap_or(false);
    let normalized = text.replace("\r\n", "\n");
    let out = if crlf { normalized.replace('\n', "\r\n") } else { normalized };
    fs::write(&full, out).map_err(|e| format!("{rel}: {e}"))
}

/// Reads a file as stored in a commit. A missing file gives empty text.
pub fn blob(repo: &str, rev: &str, rel: &str) -> FileText {
    let spec = format!("{rev}:{rel}");
    let size = run::git(p(repo), &["cat-file", "-s", &spec])
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    match size {
        None => missing(),
        Some(n) if n > MAX_VIEW_BYTES => FileText { text: None, binary: false, too_large: true, size: n },
        Some(n) => match run::git_bytes(p(repo), &["cat-file", "blob", &spec]) {
            Ok(bytes) => to_text(bytes, n),
            Err(_) => missing(),
        },
    }
}

fn has_head(repo: &str) -> bool {
    run::git(p(repo), &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok()
}

fn kind_of(x: char, y: char) -> &'static str {
    if x == 'R' || y == 'R' {
        "renamed"
    } else if x == 'C' || y == 'C' {
        "copied"
    } else if x == 'A' {
        "added"
    } else if x == 'D' || y == 'D' {
        "deleted"
    } else if x == 'T' || y == 'T' {
        "typechange"
    } else {
        "modified"
    }
}

/// Parses `git status --porcelain=v2 --branch -z`.
fn parse_status(text: &str, s: &mut Summary) {
    let mut fields = text.split('\0');
    while let Some(line) = fields.next() {
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            if let Some(b) = rest.strip_prefix("branch.head ") {
                s.branch = (b != "(detached)").then(|| b.to_string());
            } else if let Some(u) = rest.strip_prefix("branch.upstream ") {
                s.upstream = Some(u.to_string());
            } else if let Some(ab) = rest.strip_prefix("branch.ab ") {
                let mut parts = ab.split(' ');
                s.ahead = parts.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
                s.behind = parts.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
            }
            continue;
        }
        let tag = line.as_bytes()[0];
        match tag {
            b'1' | b'2' => {
                let n = if tag == b'1' { 9 } else { 10 };
                let parts: Vec<&str> = line.splitn(n, ' ').collect();
                if parts.len() < n {
                    continue;
                }
                let xy: Vec<char> = parts[1].chars().collect();
                let old_path = if tag == b'2' { fields.next().map(|o| o.to_string()) } else { None };
                s.changes.push(Change {
                    path: parts[n - 1].to_string(),
                    old_path,
                    kind: kind_of(xy[0], xy[1]).to_string(),
                    staged: xy[0] != '.',
                    unstaged: xy[1] != '.',
                    mtime: None,
                });
            }
            b'u' => {
                let parts: Vec<&str> = line.splitn(11, ' ').collect();
                if let Some(path) = parts.get(10) {
                    s.changes.push(Change {
                        path: path.to_string(),
                        old_path: None,
                        kind: "conflict".into(),
                        staged: false,
                        unstaged: true,
                        mtime: None,
                    });
                }
            }
            b'?' => s.changes.push(Change {
                path: line[2..].to_string(),
                old_path: None,
                kind: "untracked".into(),
                staged: false,
                unstaged: true,
                mtime: None,
            }),
            _ => {}
        }
    }
}

/// The repo's own git folder and the shared one. They differ only for a linked worktree.
fn git_dirs(repo: &str) -> Option<(PathBuf, PathBuf)> {
    let dot = Path::new(repo).join(".git");
    if dot.is_dir() {
        return Some((dot.clone(), dot));
    }
    let text = fs::read_to_string(&dot).ok()?;
    let gitdir = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
    let gitdir = if gitdir.is_absolute() { gitdir } else { Path::new(repo).join(gitdir) };
    let common = match fs::read_to_string(gitdir.join("commondir")) {
        Ok(c) => {
            let c = PathBuf::from(c.trim());
            if c.is_absolute() { c } else { gitdir.join(c) }
        }
        Err(_) => gitdir.clone(),
    };
    Some((gitdir, common))
}

/// Finds the shared git folder, where FETCH_HEAD lives, for a repo or a linked worktree.
fn common_git_dir(repo: &str) -> Option<PathBuf> {
    git_dirs(repo).map(|(_, common)| common)
}

fn mtime_ms(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn newest_in(dir: &Path, depth: u32, best: &mut i64) {
    *best = (*best).max(mtime_ms(dir));
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                newest_in(&p, depth - 1, best);
            }
        } else {
            *best = (*best).max(mtime_ms(&p));
        }
    }
}

/// The newest change time of git's branch files. Reading it starts no git program.
fn refs_stamp(repo: &str) -> Option<i64> {
    let (own, common) = git_dirs(repo)?;
    let mut best = 0;
    for f in ["HEAD", "packed-refs", "FETCH_HEAD", "ORIG_HEAD"] {
        best = best.max(mtime_ms(&common.join(f)));
    }
    best = best.max(mtime_ms(&own.join("HEAD")));
    newest_in(&common.join("refs"), 8, &mut best);
    Some(best)
}

fn mtime_of(path: &Path) -> Option<i64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64)
}

fn fetched_at(repo: &str) -> Option<i64> {
    let meta = fs::metadata(common_git_dir(repo)?.join("FETCH_HEAD")).ok()?;
    let t = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(t.as_secs() as i64)
}

fn parse_log(text: &str) -> Vec<Commit> {
    text.split('\x1e')
        .filter_map(|rec| {
            let f: Vec<&str> = rec.trim_start_matches('\n').split('\x1f').collect();
            if f.len() < 6 {
                return None;
            }
            let body = f.get(8).map(|b| b.trim()).unwrap_or("");
            let body: String = body.chars().take(800).collect();
            Some(Commit {
                sha: f[0].to_string(),
                short: f[1].to_string(),
                subject: f[2].to_string(),
                author: f[3].to_string(),
                time: f[4].parse().unwrap_or(0),
                refs: f[5].trim().to_string(),
                parents: f.get(6).map(|p| p.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                email: f.get(7).unwrap_or(&"").to_string(),
                body,
            })
        })
        .collect()
}

pub const LOG_FORMAT: &str = "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ct%x1f%D%x1f%P%x1f%ae%x1f%b%x1e";

pub fn parse_log_pub(text: &str) -> Vec<Commit> {
    parse_log(text)
}

pub fn summary(repo: &str) -> Summary {
    let mut s = Summary { path: repo.to_string(), provider: "none".into(), ..Default::default() };
    match run::git(p(repo), &["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"]) {
        Ok(text) => parse_status(&text, &mut s),
        Err(e) => {
            s.error = Some(e);
            return s;
        }
    }
    if let Ok(text) = run::git(p(repo), &["log", "-1", LOG_FORMAT]) {
        s.last = parse_log(&text).into_iter().next();
    }
    // Changed files are few, so a stat per file is cheap. This is how agent activity is seen.
    for c in s.changes.iter_mut() {
        c.mtime = mtime_of(&Path::new(repo).join(&c.path));
    }
    s.activity = s.changes.iter().filter_map(|c| c.mtime).chain(s.last.as_ref().map(|c| c.time)).max();
    let remote = run::git(p(repo), &["config", "--get", "remote.origin.url"])
        .ok()
        .or_else(|| {
            let names = run::git(p(repo), &["remote"]).ok()?;
            let first = names.lines().next()?.to_string();
            run::git(p(repo), &["config", "--get", &format!("remote.{first}.url")]).ok()
        })
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    if let Some(url) = &remote {
        s.provider = provider_of(url).into();
    }
    s.remote = remote;
    s.fetched_at = fetched_at(repo);
    s.refs_stamp = refs_stamp(repo);
    s
}

/// Runs status for many repos side by side, 8 at a time.
pub fn summaries(repos: Vec<String>) -> Vec<Summary> {
    let mut out = Vec::with_capacity(repos.len());
    for chunk in repos.chunks(8) {
        let part: Vec<Summary> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk.iter().map(|r| sc.spawn(move || summary(r))).collect();
            hs.into_iter().map(|h| h.join().unwrap_or_default()).collect()
        });
        out.extend(part);
    }
    out
}

/// All files git knows about or would add (tracked + untracked, minus ignored).
pub fn list_files(repo: &str) -> Result<Vec<String>, String> {
    let text = run::git(p(repo), &["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--deduplicate"])?;
    let mut files: Vec<String> = text.split('\0').filter(|f| !f.is_empty()).map(String::from).collect();
    files.sort_unstable();
    files.dedup();
    Ok(files)
}

/// Old (HEAD) and new (working tree) text for one changed file.
pub fn worktree_diff(repo: &str, rel: &str, old_rel: Option<&str>) -> Result<FileDiff, String> {
    let old = if has_head(repo) { blob(repo, "HEAD", old_rel.unwrap_or(rel)) } else { missing() };
    let new = match safe_join(repo, rel)?.exists() {
        true => read_file(repo, rel)?,
        false => missing(),
    };
    Ok(FileDiff { old, new })
}

/// Old and new text for one file between two commits.
pub fn rev_diff(repo: &str, base: &str, head: &str, rel: &str, old_rel: Option<&str>) -> FileDiff {
    FileDiff { old: blob(repo, base, old_rel.unwrap_or(rel)), new: blob(repo, head, rel) }
}

/// Files changed between two commits, from `git diff --name-status -z -M`.
pub fn rev_files(repo: &str, base: &str, head: &str) -> Result<Vec<Change>, String> {
    let text = run::git(p(repo), &["diff", "--name-status", "-z", "-M", base, head])?;
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let Some(status) = fields.next() {
        let code = status.chars().next().unwrap_or('M');
        let (old_path, path) = if code == 'R' || code == 'C' {
            let old = fields.next().map(String::from);
            (old, fields.next().map(String::from))
        } else {
            (None, fields.next().map(String::from))
        };
        let Some(path) = path else { break };
        let kind = match code {
            'A' => "added",
            'D' => "deleted",
            'R' => "renamed",
            'C' => "copied",
            'T' => "typechange",
            _ => "modified",
        };
        out.push(Change { path, old_path, kind: kind.into(), staged: false, unstaged: false, mtime: None });
    }
    Ok(out)
}

/// The parent of a commit, or the empty tree for the first commit.
pub fn parent_of(repo: &str, sha: &str) -> String {
    run::git(p(repo), &["rev-list", "--parents", "-n", "1", sha])
        .ok()
        .and_then(|l| l.split_whitespace().nth(1).map(String::from))
        .unwrap_or_else(|| EMPTY_TREE.to_string())
}

/// History, newest first, in an order where children always come before parents (the graph needs this).
/// With `all`, local branches, remote branches, and tags are included, not only the current branch.
/// The history list, and how the branch stands against its remote branch.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub commits: Vec<Commit>,
    pub branch: Option<String>,
    /// For example `origin/main`. None when the branch has no remote branch.
    pub upstream: Option<String>,
    pub upstream_sha: String,
    /// Where the branch and its remote branch split. Same as HEAD when a pull is a fast-forward.
    pub base: String,
    pub behind: usize,
    pub ahead: usize,
    /// Commits on the remote branch only (a pull brings them), newest first, up to 5000.
    pub incoming: Vec<String>,
    /// Commits on this branch only (a push sends them), up to 5000.
    pub outgoing: Vec<String>,
}

fn rev_list(repo: &str, range: &str) -> Vec<String> {
    run::git(p(repo), &["rev-list", "-n", "5000", range])
        .map(|t| t.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Without `all`, the list is this branch plus the commits a pull would bring, placed above it.
/// Those come first, so a long list of new remote commits cannot push your own history out.
pub fn log(repo: &str, count: u32, all: bool) -> Result<History, String> {
    let n = count.to_string();
    let mut h = History::default();
    h.branch = run::git(p(repo), &["rev-parse", "--abbrev-ref", "HEAD"]).ok().map(|b| b.trim().to_string()).filter(|b| b != "HEAD");
    h.upstream = run::git(p(repo), &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok().map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    if h.upstream.is_some() {
        h.upstream_sha = run::git(p(repo), &["rev-parse", "@{upstream}"]).map(|s| s.trim().to_string()).unwrap_or_default();
        h.base = run::git(p(repo), &["merge-base", "HEAD", "@{upstream}"]).map(|s| s.trim().to_string()).unwrap_or_default();
        if let Ok(t) = run::git(p(repo), &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]) {
            let mut it = t.split_whitespace().map(|x| x.parse::<usize>().unwrap_or(0));
            h.ahead = it.next().unwrap_or(0);
            h.behind = it.next().unwrap_or(0);
        }
        if h.behind > 0 {
            h.incoming = rev_list(repo, "HEAD..@{upstream}");
        }
        if h.ahead > 0 {
            h.outgoing = rev_list(repo, "@{upstream}..HEAD");
        }
    }
    if all {
        h.commits = parse_log(&run::git(p(repo), &["log", "-n", &n, "--date-order", LOG_FORMAT, "--branches", "--remotes", "--tags"])?);
        return Ok(h);
    }
    if h.behind > 0 {
        h.commits = parse_log(&run::git(p(repo), &["log", "-n", &n, "--date-order", LOG_FORMAT, "HEAD..@{upstream}"])?);
    }
    h.commits.extend(parse_log(&run::git(p(repo), &["log", "-n", &n, "--date-order", LOG_FORMAT, "HEAD"])?));
    Ok(h)
}

/// Commits a pull would bring in (`incoming`) or a push would send (`outgoing`), newest first.
/// With no upstream, `outgoing` lists the commits that are on no remote branch.
pub fn pending(repo: &str, incoming: bool) -> Result<Vec<Commit>, String> {
    let has_upstream = run::git(p(repo), &["rev-parse", "--abbrev-ref", "@{upstream}"]).is_ok();
    let args: Vec<&str> = match (incoming, has_upstream) {
        (true, true) => vec!["log", "-n", "50", LOG_FORMAT, "HEAD..@{upstream}"],
        (false, true) => vec!["log", "-n", "50", LOG_FORMAT, "@{upstream}..HEAD"],
        (true, false) => return Ok(Vec::new()),
        (false, false) => vec!["log", "-n", "50", LOG_FORMAT, "HEAD", "--not", "--remotes"],
    };
    Ok(parse_log(&run::git(p(repo), &args)?))
}

/// A unified diff of the working tree against HEAD, including untracked files.
pub fn worktree_patch(repo: &str, paths: &[String], untracked: &[String]) -> Result<String, String> {
    let mut out = String::new();
    let tracked: Vec<&String> = paths.iter().filter(|x| !untracked.contains(x)).collect();
    if !tracked.is_empty() || paths.is_empty() {
        let mut args: Vec<&str> = vec!["diff"];
        if has_head(repo) {
            args.push("HEAD");
        } else {
            args.push("--cached");
        }
        args.push("--");
        args.extend(tracked.iter().map(|s| s.as_str()));
        out.push_str(&run::git(p(repo), &args)?);
    }
    for u in untracked {
        out.push_str(&run::git_allow(p(repo), &["diff", "--no-index", "--", "/dev/null", u], &[1])?);
    }
    Ok(out)
}

/// A unified diff between two commits, optionally limited to some paths.
pub fn rev_patch(repo: &str, base: &str, head: &str, paths: &[String]) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["diff", "-M", base, head, "--"];
    args.extend(paths.iter().map(|s| s.as_str()));
    run::git(p(repo), &args)
}

/// Everything a branch or worktree changed since it split from its base branch.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BranchReview {
    /// The base it is compared with, for example "main" or "origin/main".
    pub base_ref: String,
    /// The commit where the branch split from the base.
    pub merge_base: String,
    pub head: String,
    pub branch: Option<String>,
    /// Commits on the branch that the base does not have, newest first.
    pub commits: Vec<Commit>,
    /// Files that differ between the split point and the files on disk now.
    pub files: Vec<Change>,
    /// Other bases that exist in this repo.
    pub candidates: Vec<String>,
}

fn ref_exists(repo: &str, name: &str) -> bool {
    run::git(p(repo), &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")]).is_ok()
}

/// Likely base branches, best first: the remote's default branch as a local branch, then common names.
pub fn base_candidates(repo: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Ok(head) = run::git(p(repo), &["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"]) {
        if let Some(b) = head.trim().strip_prefix("refs/remotes/origin/") {
            names.push(b.to_string());
            names.push(format!("origin/{b}"));
        }
    }
    for b in ["main", "master", "develop", "dev", "trunk"] {
        names.push(b.to_string());
        names.push(format!("origin/{b}"));
    }
    let mut out = Vec::new();
    for n in names {
        if !out.contains(&n) && ref_exists(repo, &n) {
            out.push(n);
        }
    }
    out
}

/// The base that split from HEAD most recently (fewest commits on the branch since then).
/// A stale local `main` can be far behind `origin/main`; this picks the one that matches the work.
fn closest_base(repo: &str, candidates: &[String]) -> Option<String> {
    let mut best: Option<(u64, String)> = None;
    for c in candidates {
        let Ok(mb) = run::git(p(repo), &["merge-base", c, "HEAD"]) else { continue };
        let range = format!("{}..HEAD", mb.trim());
        let Some(n) = run::git(p(repo), &["rev-list", "--count", &range]).ok().and_then(|t| t.trim().parse::<u64>().ok()) else { continue };
        if best.as_ref().map(|(b, _)| n < *b).unwrap_or(true) {
            best = Some((n, c.clone()));
        }
    }
    best.map(|(_, c)| c).or_else(|| candidates.first().cloned())
}

pub fn branch_review(repo: &str, base: Option<&str>) -> Result<BranchReview, String> {
    let candidates = base_candidates(repo);
    let base_ref = match base.filter(|b| !b.is_empty()) {
        Some(b) if ref_exists(repo, b) => b.to_string(),
        Some(b) => return Err(format!("base not found: {b}")),
        None => closest_base(repo, &candidates).ok_or("Tern could not find a main, master, or develop branch to compare with.")?,
    };
    let head = run::git(p(repo), &["rev-parse", "HEAD"])?.trim().to_string();
    let merge_base = run::git(p(repo), &["merge-base", &base_ref, "HEAD"])?.trim().to_string();
    let range = format!("{merge_base}..HEAD");
    let commits = parse_log(&run::git(p(repo), &["log", "-n", "200", LOG_FORMAT, &range])?);
    // Without a second commit, git diff compares with the files on disk, so uncommitted work is included.
    let mut files = rev_files_to_worktree(repo, &merge_base)?;
    let untracked = run::git(p(repo), &["ls-files", "-z", "--others", "--exclude-standard"])?;
    for f in untracked.split('\0').filter(|f| !f.is_empty()) {
        files.push(Change { path: f.to_string(), old_path: None, kind: "untracked".into(), staged: false, unstaged: true, mtime: None });
    }
    for c in files.iter_mut() {
        c.mtime = mtime_of(&Path::new(repo).join(&c.path));
    }
    let branch = run::git(p(repo), &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok().map(|b| b.trim().to_string());
    Ok(BranchReview { base_ref, merge_base, head, branch, commits, files, candidates })
}

fn rev_files_to_worktree(repo: &str, base: &str) -> Result<Vec<Change>, String> {
    let text = run::git(p(repo), &["diff", "--name-status", "-z", "-M", base])?;
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let Some(status) = fields.next() {
        let code = status.chars().next().unwrap_or('M');
        let (old_path, path) = if code == 'R' || code == 'C' {
            (fields.next().map(String::from), fields.next().map(String::from))
        } else {
            (None, fields.next().map(String::from))
        };
        let Some(path) = path else { break };
        let kind = match code { 'A' => "added", 'D' => "deleted", 'R' => "renamed", 'C' => "copied", 'T' => "typechange", _ => "modified" };
        out.push(Change { path, old_path, kind: kind.into(), staged: false, unstaged: false, mtime: None });
    }
    Ok(out)
}

/// Old text at the split point and new text on disk, for one file in a branch review.
pub fn base_diff(repo: &str, base: &str, rel: &str, old_rel: Option<&str>) -> Result<FileDiff, String> {
    let old = blob(repo, base, old_rel.unwrap_or(rel));
    let new = match safe_join(repo, rel)?.exists() {
        true => read_file(repo, rel)?,
        false => missing(),
    };
    Ok(FileDiff { old, new })
}

/// A unified diff from the split point to the files on disk, including new files.
pub fn base_patch(repo: &str, base: &str, paths: &[String], untracked: &[String]) -> Result<String, String> {
    let tracked: Vec<&String> = paths.iter().filter(|x| !untracked.contains(x)).collect();
    let mut out = String::new();
    if !tracked.is_empty() || paths.is_empty() {
        let mut args: Vec<&str> = vec!["diff", "-M", base, "--"];
        args.extend(tracked.iter().map(|s| s.as_str()));
        out.push_str(&run::git(p(repo), &args)?);
    }
    for u in untracked {
        out.push_str(&run::git_allow(p(repo), &["diff", "--no-index", "--", "/dev/null", u], &[1])?);
    }
    Ok(out)
}

/// Stages files (all changes when `paths` is empty).
pub fn stage(repo: &str, paths: &[String]) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(|s| s.as_str()));
    run::git(p(repo), &args).map(|_| ())
}

/// Unstages files (everything when `paths` is empty). The files on disk do not change.
pub fn unstage(repo: &str, paths: &[String]) -> Result<(), String> {
    let mut args: Vec<&str> = if has_head(repo) {
        vec!["restore", "--staged", "--"]
    } else {
        vec!["rm", "--cached", "-r", "-q", "--"]
    };
    if paths.is_empty() {
        args.push(".");
    }
    args.extend(paths.iter().map(|s| s.as_str()));
    run::git(p(repo), &args).map(|_| ())
}

/// True when HEAD is already on a remote branch (pushed), so rewriting it would change shared history.
pub fn head_is_pushed(repo: &str) -> bool {
    run::git(p(repo), &["branch", "-r", "--contains", "HEAD"]).map(|t| !t.trim().is_empty()).unwrap_or(false)
}

/// Commits the staged changes. With `stage_all`, stages everything first. With `push`, pushes after.
/// With `amend`, the staged changes and the message replace the last commit (only when it is not pushed).
pub fn commit(repo: &str, message: &str, stage_all: bool, push: bool, amend: bool) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Write a commit message first.".into());
    }
    if amend && head_is_pushed(repo) {
        return Err("The last commit is already pushed, so Tern will not change it.".into());
    }
    if stage_all {
        stage(repo, &[])?;
    }
    let args: &[&str] = if amend { &["commit", "--amend", "-F", "-"] } else { &["commit", "-F", "-"] };
    let mut out = run::git_input(p(repo), args, message)?;
    if push {
        let pushed = action(repo, "push").map_err(|e| format!("Committed, but the push failed: {e}"))?;
        out.push('\n');
        out.push_str(&pushed);
    }
    Ok(out)
}

pub fn action(repo: &str, what: &str) -> Result<String, String> {
    match what {
        "fetch" => run::git_verbose(p(repo), &["fetch", "--prune"]),
        "pull" => run::git_verbose(p(repo), &["pull", "--ff-only"]),
        "push" => match run::git_verbose(p(repo), &["push"]) {
            Err(e) if e.contains("no upstream") || e.contains("has no upstream") => {
                run::git_verbose(p(repo), &["push", "-u", "origin", "HEAD"])
            }
            other => other,
        },
        _ => Err(format!("unknown action: {what}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_v2() {
        let text = "# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -3\0\
1 .M N... 100644 100644 100644 aaa bbb src/app.ts\0\
2 R. N... 100644 100644 100644 aaa bbb R100 src/new name.ts\0src/old.ts\0\
u UU N... 100644 100644 100644 100644 a b c conflict.txt\0\
? notes.md\0";
        let mut s = Summary::default();
        parse_status(text, &mut s);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!((s.ahead, s.behind), (2, 3));
        assert_eq!(s.changes.len(), 4);
        assert_eq!(s.changes[0].kind, "modified");
        assert!(!s.changes[0].staged);
        assert_eq!(s.changes[1].path, "src/new name.ts");
        assert_eq!(s.changes[1].old_path.as_deref(), Some("src/old.ts"));
        assert_eq!(s.changes[2].kind, "conflict");
        assert_eq!(s.changes[3].kind, "untracked");
    }

    /// Runs the real git functions against a throwaway repo.
    #[test]
    fn works_on_a_real_repo() {
        let dir = std::env::temp_dir().join(format!("tern-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src")).unwrap();
        let repo = dir.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&dir, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        fs::write(dir.join("src/a.txt"), "one\ntwo\n").unwrap();
        fs::write(dir.join("old.txt"), "keep me\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);
        let first = g(&["rev-parse", "HEAD"]).trim().to_string();

        fs::write(dir.join("src/a.txt"), "one\nTWO\nthree\n").unwrap();
        fs::write(dir.join("new file.txt"), "hello\n").unwrap();
        g(&["mv", "old.txt", "renamed.txt"]);

        let s = summary(&repo);
        assert_eq!(s.error, None);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.last.as_ref().map(|c| c.subject.as_str()), Some("first"));
        let kinds: Vec<(&str, &str)> = s.changes.iter().map(|c| (c.path.as_str(), c.kind.as_str())).collect();
        assert!(kinds.contains(&("src/a.txt", "modified")), "{kinds:?}");
        assert!(kinds.contains(&("new file.txt", "untracked")), "{kinds:?}");
        assert!(kinds.contains(&("renamed.txt", "renamed")), "{kinds:?}");

        let d = worktree_diff(&repo, "src/a.txt", None).unwrap();
        assert_eq!(d.old.text.as_deref(), Some("one\ntwo\n"));
        assert_eq!(d.new.text.as_deref(), Some("one\nTWO\nthree\n"));
        let r = worktree_diff(&repo, "renamed.txt", Some("old.txt")).unwrap();
        assert_eq!(r.old.text.as_deref(), Some("keep me\n"));

        let patch = worktree_patch(&repo, &[], &["new file.txt".to_string()]).unwrap();
        assert!(patch.contains("+TWO"), "{patch}");
        assert!(patch.contains("+hello"), "{patch}");

        let files = list_files(&repo).unwrap();
        assert!(files.contains(&"new file.txt".to_string()));

        assert!(s.activity.is_some());
        assert!(s.changes.iter().find(|c| c.path == "src/a.txt").unwrap().mtime.is_some());

        stage(&repo, &["src/a.txt".to_string()]).unwrap();
        let a = summary(&repo).changes.into_iter().find(|c| c.path == "src/a.txt").unwrap();
        assert!(a.staged && !a.unstaged);
        unstage(&repo, &["src/a.txt".to_string()]).unwrap();
        let a = summary(&repo).changes.into_iter().find(|c| c.path == "src/a.txt").unwrap();
        assert!(!a.staged && a.unstaged);
        assert!(commit(&repo, "  ", true, false, false).is_err());

        let stamp_before = summary(&repo).refs_stamp;
        std::thread::sleep(std::time::Duration::from_millis(30));
        commit(&repo, "second\n\nWith a \"quoted\" body line.", true, false, false).unwrap();
        // Amend: a new message, and a staged change folded into the same commit.
        let before = g(&["rev-list", "--count", "HEAD"]);
        fs::write(dir.join("amended.txt"), "x\n").unwrap();
        stage(&repo, &["amended.txt".to_string()]).unwrap();
        commit(&repo, "second, amended\n\nWith a \"quoted\" body line.", false, false, true).unwrap();
        assert_eq!(g(&["rev-list", "--count", "HEAD"]), before, "amend must not add a commit");
        assert!(g(&["show", "--name-only", "--format=%s", "HEAD"]).contains("amended.txt"));
        assert!(g(&["log", "-1", "--format=%s"]).contains("second, amended"));
        assert_ne!(summary(&repo).refs_stamp, stamp_before, "a commit must change the branch stamp");
        assert!(summary(&repo).changes.is_empty());
        let body = g(&["log", "-1", "--format=%B"]);
        assert!(body.contains("\"quoted\""), "{body}");
        let head = g(&["rev-parse", "HEAD"]).trim().to_string();
        assert_eq!(parent_of(&repo, &head), first);
        assert_eq!(parent_of(&repo, &first), EMPTY_TREE);
        let changed = rev_files(&repo, &first, &head).unwrap();
        assert_eq!(changed.len(), 4, "{changed:?}"); // 3 from the commit, 1 added by the amend
        let root_files = rev_files(&repo, EMPTY_TREE, &first).unwrap();
        assert_eq!(root_files.len(), 2);
        let history = log(&repo, 10, false).unwrap().commits;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].parents, vec![first.clone()]);
        assert!(history[1].parents.is_empty());
        assert!(history[0].body.contains("quoted"), "{:?}", history[0].body);
        assert_eq!(log(&repo, 10, true).unwrap().commits.len(), 2);
        assert!(rev_patch(&repo, &first, &head, &[]).unwrap().contains("+three"));
        // No upstream: nothing comes in, and every local commit would go out.
        assert!(pending(&repo, true).unwrap().is_empty());
        assert_eq!(pending(&repo, false).unwrap().len(), 2);

        // Behind the remote branch: the history shows the new commits first, then this branch.
        g(&["switch", "-q", "-c", "up"]);
        fs::write(dir.join("up.txt"), "u\n").unwrap();
        g(&["add", "up.txt"]);
        g(&["commit", "-q", "-m", "upstream work"]);
        g(&["switch", "-q", "main"]);
        g(&["branch", "-q", "-u", "up"]);
        let h = log(&repo, 10, false).unwrap();
        assert_eq!((h.behind, h.ahead), (1, 0));
        assert_eq!(h.incoming.len(), 1);
        assert_eq!(h.base, head);
        assert_eq!(h.commits.len(), 3);
        assert_eq!(h.commits[0].subject, "upstream work");
        assert_eq!(h.commits[0].sha, h.upstream_sha);
        g(&["branch", "-q", "--unset-upstream"]);
        g(&["branch", "-q", "-D", "up"]);
        assert_eq!(log(&repo, 10, false).unwrap().upstream, None);

        // A branch review: one commit on the branch, one uncommitted edit, one new file.
        g(&["switch", "-q", "-c", "feature"]);
        fs::write(dir.join("feature.txt"), "f\n").unwrap();
        g(&["add", "feature.txt"]);
        g(&["commit", "-q", "-m", "feature work"]);
        fs::write(dir.join("renamed.txt"), "keep me\nplus\n").unwrap();
        fs::write(dir.join("brand new.txt"), "n\n").unwrap();
        let r = branch_review(&repo, None).unwrap();
        assert_eq!(r.base_ref, "main");
        assert_eq!(r.merge_base, head);
        assert_eq!(r.commits.len(), 1);
        assert_eq!(r.commits[0].subject, "feature work");
        let paths: Vec<&str> = r.files.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"feature.txt") && paths.contains(&"renamed.txt") && paths.contains(&"brand new.txt"), "{paths:?}");
        let d = base_diff(&repo, &r.merge_base, "renamed.txt", None).unwrap();
        assert_eq!(d.new.text.as_deref(), Some("keep me\nplus\n"));
        let patch = base_patch(&repo, &r.merge_base, &[], &["brand new.txt".to_string()]).unwrap();
        assert!(patch.contains("+plus") && patch.contains("+f") && patch.contains("+n"), "{patch}");
        assert!(branch_review(&repo, Some("nope")).is_err());
        g(&["switch", "-q", "main"]);

        write_file(&repo, "src/a.txt", "edited\n").unwrap();
        assert_eq!(read_file(&repo, "src/a.txt").unwrap().text.as_deref(), Some("edited\n"));
        assert!(read_file(&repo, "../outside.txt").is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Prints a branch review of real repos: `SCOUT_REPOS="C:\a;C:\b" cargo test print_review -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_review() {
        for repo in std::env::var("SCOUT_REPOS").unwrap_or_default().split(';').filter(|r| !r.is_empty()) {
            match branch_review(repo, None) {
                Ok(r) => {
                    println!("== {repo}
  {:?} since {} (split {}), bases {:?}", r.branch, r.base_ref, &r.merge_base[..7], r.candidates);
                    println!("  {} commits: {:?}", r.commits.len(), r.commits.iter().take(3).map(|c| c.subject.as_str()).collect::<Vec<_>>());
                    println!("  {} files: {:?}", r.files.len(), r.files.iter().take(5).map(|c| format!("{} {}", c.kind, c.path)).collect::<Vec<_>>());
                }
                Err(e) => println!("== {repo}
  error: {e}"),
            }
        }
    }

    #[test]
    fn refuses_paths_outside_repo() {
        assert!(safe_join("C:\\repo", "..\\secret").is_err());
        assert!(safe_join("C:\\repo", "src/app.ts").is_ok());
    }

    #[test]
    fn detects_providers() {
        assert_eq!(provider_of("https://github.com/a/b.git"), "github");
        assert_eq!(provider_of("https://org@dev.azure.com/org/proj/_git/repo"), "azdo");
        assert_eq!(provider_of("https://org.visualstudio.com/proj/_git/repo"), "azdo");
    }
}
