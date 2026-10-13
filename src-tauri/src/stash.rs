//! Stashes, discarding local changes, and undoing the last commit that is not pushed yet.

use crate::git::{rev_files, Change};
use crate::run;
use serde::Serialize;
use std::path::{Path, PathBuf};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn p(repo: &str) -> &Path {
    Path::new(repo)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stash {
    /// `stash@{0}` and so on. It changes when stashes are added or dropped, so actions use `sha`.
    pub name: String,
    pub sha: String,
    pub message: String,
    /// The branch it was made on.
    pub branch: String,
    /// Unix seconds.
    pub time: i64,
    /// Changed files that git tracks.
    pub files: usize,
    /// New files saved in the stash (stashed with --include-untracked).
    pub untracked: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StashFiles {
    pub base: String,
    pub head: String,
    pub files: Vec<Change>,
    /// New files live in a third commit of the stash. Their diff is EMPTY_TREE..untracked_head.
    pub untracked: Vec<String>,
    pub untracked_head: Option<String>,
    pub empty_tree: String,
}

/// "WIP on main: 1a2b3c subject" or "On main: my message" -> ("main", "my message").
fn parse_subject(s: &str) -> (String, String) {
    let rest = s.strip_prefix("WIP on ").or_else(|| s.strip_prefix("On ")).unwrap_or(s);
    match rest.split_once(": ") {
        Some((branch, msg)) => (branch.to_string(), msg.to_string()),
        None => (String::new(), s.to_string()),
    }
}

fn untracked_head(repo: &str, sha: &str) -> Option<String> {
    run::git(p(repo), &["rev-parse", "--verify", "-q", &format!("{sha}^3")]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

pub fn list(repo: &str) -> Result<Vec<Stash>, String> {
    let text = run::git(p(repo), &["stash", "list", "--format=%gd%x1f%H%x1f%ct%x1f%gs"])?;
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\x1f').collect();
        if f.len() < 4 {
            continue;
        }
        let (branch, message) = parse_subject(f[3]);
        let sha = f[1].to_string();
        let files = run::git(p(repo), &["diff", "--name-only", &format!("{sha}^1"), &sha]).map(|t| lines(&t).len()).unwrap_or(0);
        let untracked = match untracked_head(repo, &sha) {
            Some(u) => run::git(p(repo), &["ls-tree", "-r", "--name-only", &u]).map(|t| lines(&t)).unwrap_or_default(),
            None => Vec::new(),
        };
        out.push(Stash { name: f[0].to_string(), sha, message, branch, time: f[2].parse().unwrap_or(0), files, untracked });
    }
    Ok(out)
}

/// Stashes changes, new files included. With `paths`, only those files.
pub fn push(repo: &str, message: &str, paths: &[String]) -> Result<String, String> {
    let mut args: Vec<&str> = vec!["stash", "push", "--include-untracked"];
    if !message.trim().is_empty() {
        args.extend(["-m", message.trim()]);
    }
    if !paths.is_empty() {
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
    }
    let out = run::git_verbose(p(repo), &args)?;
    if out.contains("No local changes to save") {
        return Err("There are no changes to stash.".into());
    }
    Ok(out)
}

/// The current `stash@{n}` name of a stash, found by its commit (names shift as stashes come and go).
fn name_of(repo: &str, sha: &str) -> Result<String, String> {
    list(repo)?.into_iter().find(|s| s.sha == sha).map(|s| s.name).ok_or_else(|| "That stash is gone. Refresh the list.".into())
}

/// Puts a stash's changes back. With `pop`, also removes it from the list (only when it applied cleanly).
pub fn apply(repo: &str, sha: &str, pop: bool) -> Result<String, String> {
    let name = name_of(repo, sha)?;
    run::git_verbose(p(repo), &["stash", if pop { "pop" } else { "apply" }, &name])
}

pub fn drop(repo: &str, sha: &str) -> Result<String, String> {
    let name = name_of(repo, sha)?;
    run::git_verbose(p(repo), &["stash", "drop", &name])
}

pub fn files(repo: &str, sha: &str) -> Result<StashFiles, String> {
    let base = format!("{sha}^1");
    let base = run::git(p(repo), &["rev-parse", &base])?.trim().to_string();
    let files = rev_files(repo, &base, sha)?;
    let uh = untracked_head(repo, sha);
    let untracked = match &uh {
        Some(u) => lines(&run::git(p(repo), &["ls-tree", "-r", "--name-only", u])?),
        None => Vec::new(),
    };
    Ok(StashFiles { base, head: sha.to_string(), files, untracked, untracked_head: uh, empty_tree: EMPTY_TREE.into() })
}

fn has_head(repo: &str) -> bool {
    run::git(p(repo), &["rev-parse", "--verify", "-q", "HEAD"]).is_ok()
}

/// Throws away local changes. Changed files go back to the last commit; new files go to the Recycle Bin.
/// Returns a backup commit of the changed files (from `git stash create`), so the discard can be undone.
pub fn discard(repo: &str, tracked: &[String], untracked: &[String]) -> Result<String, String> {
    let backup = run::git(p(repo), &["stash", "create"]).map(|s| s.trim().to_string()).unwrap_or_default();
    if !tracked.is_empty() {
        let mut args: Vec<&str> = if has_head(repo) {
            vec!["restore", "--source=HEAD", "--staged", "--worktree", "--"]
        } else {
            vec!["rm", "--cached", "-r", "-q", "--"]
        };
        args.extend(tracked.iter().map(String::as_str));
        run::git(p(repo), &args)?;
        // A file that was only staged as new is gone from git now but may still be on disk.
        for rel in tracked {
            let full = crate::git::safe_join(repo, rel)?;
            if full.is_file() && run::git(p(repo), &["ls-files", "--error-unmatch", "--", rel]).is_err() {
                trash::delete(&full).map_err(|e| format!("{rel}: {e}"))?;
            }
        }
    }
    for rel in untracked {
        let full: PathBuf = crate::git::safe_join(repo, rel)?;
        if full.exists() {
            trash::delete(&full).map_err(|e| format!("{rel}: could not move it to the Recycle Bin: {e}"))?;
        }
    }
    Ok(backup)
}

/// Puts back what `discard` removed from changed files, from its backup commit. New files are in the Recycle Bin.
pub fn undo_discard(repo: &str, backup: &str, tracked: &[String]) -> Result<(), String> {
    if backup.is_empty() || tracked.is_empty() {
        return Ok(());
    }
    // Only the files that exist in the backup (a deleted file is restored by checking it out too).
    let mut args: Vec<&str> = vec!["checkout", backup, "--"];
    let known = lines(&run::git(p(repo), &["ls-tree", "-r", "--name-only", backup])?);
    let back: Vec<&String> = tracked.iter().filter(|t| known.contains(t)).collect();
    if back.is_empty() {
        return Ok(());
    }
    args.extend(back.iter().map(|s| s.as_str()));
    run::git(p(repo), &args)?;
    // checkout stages them; keep them as plain local changes, like before.
    let mut un: Vec<&str> = vec!["restore", "--staged", "--"];
    un.extend(back.iter().map(|s| s.as_str()));
    run::git(p(repo), &un).map(|_| ())
}

/// Undoes the last commit and keeps its changes staged. Refuses when the commit is already pushed.
pub fn undo_commit(repo: &str) -> Result<String, String> {
    let head = run::git(p(repo), &["rev-parse", "HEAD"])?.trim().to_string();
    if run::git(p(repo), &["rev-parse", "--verify", "-q", "HEAD~1"]).is_err() {
        return Err("This is the first commit of the repo, so there is nothing to go back to.".into());
    }
    if let Ok(remotes) = run::git(p(repo), &["branch", "-r", "--contains", &head]) {
        if !remotes.trim().is_empty() {
            return Err("This commit is already pushed. Undoing it would rewrite shared history.".into());
        }
    }
    run::git(p(repo), &["reset", "--soft", "HEAD~1"])?;
    Ok(head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn reads_stash_subjects() {
        assert_eq!(parse_subject("WIP on main: 1a2b3c Fix it"), ("main".into(), "1a2b3c Fix it".into()));
        assert_eq!(parse_subject("On feature/x: my note"), ("feature/x".into(), "my note".into()));
    }

    #[test]
    fn stashes_discards_and_undoes() {
        let dir = std::env::temp_dir().join(format!("tern-stash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&dir, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        fs::write(dir.join("a.txt"), "one\n").unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);

        // Stash a change and a new file, then look inside, then pop it back.
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        fs::write(dir.join("new.txt"), "n\n").unwrap();
        push(&repo, "my work", &[]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap().trim(), "one");
        assert!(!dir.join("new.txt").exists());
        let l = list(&repo).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].branch.as_str(), l[0].message.as_str(), l[0].files), ("main", "my work", 1));
        assert_eq!(l[0].untracked, vec!["new.txt".to_string()]);
        let f = files(&repo, &l[0].sha).unwrap();
        assert_eq!(f.files.len(), 1);
        assert_eq!(f.untracked, vec!["new.txt".to_string()]);
        assert!(push(&repo, "", &[]).is_err(), "nothing left to stash");
        apply(&repo, &l[0].sha, true).unwrap();
        assert!(list(&repo).unwrap().is_empty());
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap().trim(), "two");

        // Stash one file only, then drop that stash.
        push(&repo, "", &["a.txt".to_string()]).unwrap();
        assert!(dir.join("new.txt").exists(), "other files stay");
        let sha = list(&repo).unwrap()[0].sha.clone();
        drop(&repo, &sha).unwrap();
        assert!(list(&repo).unwrap().is_empty());

        // Discard a change, then undo it.
        fs::write(dir.join("a.txt"), "three\n").unwrap();
        let backup = discard(&repo, &["a.txt".to_string()], &[]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap().trim(), "one");
        undo_discard(&repo, &backup, &["a.txt".to_string()]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap().trim(), "three");

        // A staged new file is discarded too.
        fs::write(dir.join("staged.txt"), "s\n").unwrap();
        g(&["add", "staged.txt"]);
        discard(&repo, &["staged.txt".to_string()], &[]).unwrap();
        assert!(!dir.join("staged.txt").exists());

        // Undo a commit that is not pushed: its change comes back staged.
        g(&["commit", "-qam", "second"]);
        undo_commit(&repo).unwrap();
        assert_eq!(g(&["log", "--format=%s"]).trim(), "first");
        assert!(g(&["diff", "--cached", "--name-only"]).contains("a.txt"));

        let _ = fs::remove_dir_all(&dir);
    }
}
