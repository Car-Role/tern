//! Branches (list, switch, create, delete) and worktrees (add, remove, clean up).

use crate::git;
use crate::run;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Short name, for example "feature/x" or "origin/feature/x".
    pub name: String,
    pub remote: bool,
    pub upstream: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    /// The upstream branch was deleted on the remote.
    pub gone: bool,
    pub sha: String,
    pub time: i64,
    pub author: String,
    pub subject: String,
    pub current: bool,
    /// The worktree folder that has this branch checked out, if any.
    pub worktree: Option<String>,
    /// All of its commits are in the base branch.
    pub merged: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BranchList {
    pub branches: Vec<Branch>,
    /// The branch "merged" is measured against, for example "origin/main".
    pub base: Option<String>,
}

fn p(repo: &str) -> &Path {
    Path::new(repo)
}

fn parse_track(t: &str) -> (i64, i64, bool) {
    let t = t.trim().trim_start_matches('[').trim_end_matches(']');
    if t == "gone" {
        return (0, 0, true);
    }
    let mut ahead = 0;
    let mut behind = 0;
    for part in t.split(',') {
        let part = part.trim();
        if let Some(n) = part.strip_prefix("ahead ") {
            ahead = n.parse().unwrap_or(0);
        } else if let Some(n) = part.strip_prefix("behind ") {
            behind = n.parse().unwrap_or(0);
        }
    }
    (ahead, behind, false)
}

/// The base for "merged" marks: the remote default branch when there is one.
fn merge_base_ref(repo: &str) -> Option<String> {
    let c = git::base_candidates(repo);
    c.iter().find(|n| n.starts_with("origin/")).or(c.first()).cloned()
}

pub fn list(repo: &str) -> Result<BranchList, String> {
    let format = "--format=%(refname)%1f%(refname:short)%1f%(upstream:short)%1f%(upstream:track)%1f%(objectname:short)%1f%(committerdate:unix)%1f%(authorname)%1f%(contents:subject)%1f%(HEAD)%1f%(worktreepath)";
    let text = run::git(p(repo), &["for-each-ref", "--sort=-committerdate", "--count=600", format, "refs/heads", "refs/remotes"])?;
    let base = merge_base_ref(repo);
    let merged: HashSet<String> = match &base {
        Some(b) => run::git(p(repo), &["for-each-ref", "--merged", b, "--format=%(refname)", "refs/heads", "refs/remotes"])
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect(),
        None => HashSet::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() < 10 || f[0].ends_with("/HEAD") {
            continue;
        }
        let (ahead, behind, gone) = parse_track(f[3]);
        let remote = f[0].starts_with("refs/remotes/");
        out.push(Branch {
            name: f[1].to_string(),
            remote,
            upstream: Some(f[2].to_string()).filter(|u| !u.is_empty()),
            ahead,
            behind,
            gone,
            sha: f[4].to_string(),
            time: f[5].parse().unwrap_or(0),
            author: f[6].to_string(),
            subject: f[7].to_string(),
            current: f[8] == "*",
            worktree: Some(git_path(f[9])).filter(|w| !w.is_empty()),
            // The base itself is not "merged into" itself.
            merged: merged.contains(f[0]) && base.as_deref() != Some(f[1]),
        });
    }
    Ok(BranchList { branches: out, base })
}

fn git_path(s: &str) -> String {
    if cfg!(windows) { s.replace('/', "\\") } else { s.to_string() }
}

fn check_name(repo: &str, name: &str) -> Result<(), String> {
    run::git(p(repo), &["check-ref-format", "--branch", name])
        .map(|_| ())
        .map_err(|_| format!("\"{name}\" is not a valid branch name"))
}

/// Switches to a branch. A remote branch like "origin/x" becomes a local "x" that tracks it.
pub fn switch(repo: &str, name: &str, remote: bool) -> Result<String, String> {
    if remote {
        run::git_verbose(p(repo), &["switch", "--track", name])
    } else {
        run::git_verbose(p(repo), &["switch", name])
    }
}

pub fn create(repo: &str, name: &str, from: Option<&str>) -> Result<String, String> {
    check_name(repo, name)?;
    let mut args = vec!["switch", "-c", name];
    if let Some(f) = from.filter(|f| !f.is_empty()) {
        args.push(f);
    }
    run::git_verbose(p(repo), &args)
}

/// Deletes a local branch. Git refuses when its commits are not merged, so no work is lost.
pub fn delete(repo: &str, name: &str) -> Result<String, String> {
    run::git_verbose(p(repo), &["branch", "-d", name])
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' }).collect();
    s.trim_matches('-').to_string()
}

/// Where a new worktree goes: next to the ones agents make (`.claude/worktrees`) when that folder
/// exists, otherwise in a `<repo>.worktrees` folder beside the repo.
pub fn suggest_path(repo: &str, branch: &str) -> String {
    let root = Path::new(repo);
    let agents = root.join(".claude").join("worktrees");
    let dir = if agents.is_dir() {
        agents
    } else {
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
        root.parent().map(|p| p.join(format!("{name}.worktrees"))).unwrap_or_else(|| root.join(".worktrees"))
    };
    dir.join(slug(branch)).to_string_lossy().into_owned()
}

/// Makes a worktree. `new_branch`: create `branch` from `base`. A remote `branch` like "origin/x"
/// gets a local branch "x" that tracks it.
pub fn worktree_add(repo: &str, path: &str, branch: &str, new_branch: bool, base: Option<&str>) -> Result<String, String> {
    if Path::new(path).exists() {
        return Err(format!("{path} already exists"));
    }
    if new_branch {
        check_name(repo, branch)?;
        let mut args = vec!["worktree", "add", "-b", branch, path];
        if let Some(b) = base.filter(|b| !b.is_empty()) {
            args.push(b);
        }
        return run::git_verbose(p(repo), &args);
    }
    let is_remote = run::git(p(repo), &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/{branch}")]).is_ok();
    if is_remote {
        let local = branch.split_once('/').map(|(_, rest)| rest).unwrap_or(branch);
        return run::git_verbose(p(repo), &["worktree", "add", "--track", "-b", local, path, branch]);
    }
    run::git_verbose(p(repo), &["worktree", "add", path, branch])
}

fn same(a: &str, b: &str) -> bool {
    let n = |s: &str| {
        let s = s.replace('/', "\\").trim_end_matches('\\').to_string();
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    if n(a) == n(b) {
        return true;
    }
    // The same folder can have two spellings, like a short 8.3 name on Windows
    // (C:\Users\RUNNER~1) and its long name. When both exist, ask the file system.
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// The linked worktrees git knows for this repo (not the main one), including missing ones.
fn listed_worktrees(repo: &str) -> Result<Vec<String>, String> {
    let text = run::git(p(repo), &["worktree", "list", "--porcelain"])?;
    let mut out: Vec<String> = text.lines().filter_map(|l| l.strip_prefix("worktree ")).map(git_path).collect();
    if !out.is_empty() {
        out.remove(0); // the first one is always the main worktree
    }
    Ok(out)
}

/// Removes a worktree of `repo` (the main repo). With `force`, uncommitted work in it is thrown away.
/// Git cannot remove worktrees that contain submodules, or broken ones. Those go to the Recycle Bin,
/// and git then forgets them.
pub fn worktree_remove(repo: &str, path: &str, force: bool) -> Result<String, String> {
    if !listed_worktrees(repo)?.iter().any(|w| same(w, path)) {
        return Err(format!("{path} is not a worktree of {repo}"));
    }
    let mut args = vec!["worktree", "remove"];
    if force {
        args.extend(["--force", "--force"]);
    }
    args.push(path);
    match run::git_verbose(p(repo), &args) {
        Ok(out) => Ok(out),
        Err(e) if force && (e.contains("submodule") || e.contains("not a git repository") || e.contains("is not a working tree")) => {
            if Path::new(path).exists() {
                trash::delete(PathBuf::from(path)).map_err(|t| format!("git could not remove it ({e}), and moving it to the Recycle Bin failed: {t}"))?;
            }
            run::git_verbose(p(repo), &["worktree", "prune", "-v"])?;
            Ok(format!("Moved {path} to the Recycle Bin, and git forgot the worktree."))
        }
        Err(e) => Err(e),
    }
}

/// Forgets worktrees whose folder is gone.
pub fn worktree_prune(repo: &str) -> Result<String, String> {
    let out = run::git_verbose(p(repo), &["worktree", "prune", "-v"])?;
    Ok(if out.trim().is_empty() { "Nothing to clean up.".into() } else { out })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn branches_and_worktrees_work() {
        let dir = std::env::temp_dir().join(format!("tern-branches-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let r = repo.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&repo, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        fs::write(repo.join("a.txt"), "a\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);

        create(&r, "feature/one", None).unwrap();
        assert!(create(&r, "bad name", None).is_err());
        fs::write(repo.join("b.txt"), "b\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "feature work"]);
        switch(&r, "main", false).unwrap();

        let l = list(&r).unwrap();
        let names: Vec<&str> = l.branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main") && names.contains(&"feature/one"), "{names:?}");
        assert!(l.branches.iter().find(|b| b.name == "main").unwrap().current);
        // No commits of feature/one are on main, so it is not merged, and git refuses to delete it.
        assert!(!l.branches.iter().find(|b| b.name == "feature/one").unwrap().merged || l.base.is_none());
        assert!(delete(&r, "feature/one").is_err());

        let wt = suggest_path(&r, "feature/one");
        assert!(wt.ends_with("repo.worktrees\\feature-one") || wt.ends_with("repo.worktrees/feature-one"), "{wt}");
        worktree_add(&r, &wt, "feature/one", false, None).unwrap();
        assert!(Path::new(&wt).join("b.txt").exists());
        let l = list(&r).unwrap();
        let f = l.branches.iter().find(|b| b.name == "feature/one").unwrap();
        assert!(f.worktree.as_deref().map(|w| same(w, &wt)).unwrap_or(false), "{:?}", f.worktree);

        let wt2 = suggest_path(&r, "two");
        worktree_add(&r, &wt2, "two", true, Some("main")).unwrap();
        assert!(worktree_add(&r, &wt2, "three", true, None).is_err(), "an existing folder must be refused");

        // Uncommitted work: git refuses without force.
        fs::write(Path::new(&wt2).join("dirty.txt"), "x").unwrap();
        assert!(worktree_remove(&r, &wt2, false).is_err());
        worktree_remove(&r, &wt2, true).unwrap();
        assert!(!Path::new(&wt2).exists());
        assert!(worktree_remove(&r, &r, true).is_err(), "the main worktree must never be removed");
        assert!(worktree_remove(&r, "C:\\elsewhere", true).is_err());

        // A worktree folder deleted by hand: prune forgets it.
        worktree_remove(&r, &wt, false).unwrap();
        worktree_prune(&r).unwrap();
        delete(&r, "two").unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_track_text() {
        assert_eq!(parse_track("[ahead 2, behind 1]"), (2, 1, false));
        assert_eq!(parse_track("[behind 3]"), (0, 3, false));
        assert_eq!(parse_track("[gone]"), (0, 0, true));
        assert_eq!(parse_track(""), (0, 0, false));
    }
}
