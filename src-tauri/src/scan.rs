//! Finding git repos under the root folders, plus their linked worktrees.

use crate::config::Config;
use crate::run;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub path: String,
    pub name: String,
    /// Set for a linked worktree: the path of its main repo.
    pub parent: Option<String>,
    pub pinned: bool,
}

/// Folders that never hold a repo we care about, and are slow to walk.
const SKIP: &[&str] = &[
    "node_modules", "target", "dist", "build", "out", "obj", "bin", "Library", "Temp",
    "venv", ".venv", "__pycache__", ".gradle", ".idea", ".vs", "packages", "vendor",
];

/// Makes paths comparable: native separators, no trailing separator.
pub fn norm(p: &str) -> String {
    let mut s = if cfg!(windows) { p.replace('/', "\\") } else { p.to_string() };
    while s.len() > 3 && (s.ends_with('\\') || s.ends_with('/')) {
        s.pop();
    }
    s
}

fn key(p: &str) -> String {
    if cfg!(windows) {
        norm(p).to_lowercase()
    } else {
        norm(p)
    }
}

fn name_of(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string())
}

fn walk(dir: &Path, depth: u32, found: &mut Vec<String>) {
    if dir.join(".git").exists() {
        found.push(norm(&dir.to_string_lossy()));
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() || ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP.contains(&name.as_str()) {
            continue;
        }
        walk(&entry.path(), depth - 1, found);
    }
}

/// Lists linked worktrees of a repo (not the main one). Agents often create these.
fn worktrees(repo: &str) -> Vec<String> {
    let Ok(text) = run::git(Path::new(repo), &["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let mut path = None;
        let mut skip = false;
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(norm(p));
            } else if line == "bare" || line.starts_with("prunable") {
                skip = true;
            }
        }
        if let (Some(p), false) = (path, skip) {
            if key(&p) != key(repo) && Path::new(&p).is_dir() {
                out.push(p);
            }
        }
    }
    out
}

pub fn list(config: &Config) -> Vec<RepoEntry> {
    let mut found = Vec::new();
    for root in &config.roots {
        walk(Path::new(root), config.scan_depth, &mut found);
    }
    for p in &config.extra {
        if Path::new(p).join(".git").exists() {
            found.push(norm(p));
        }
    }

    let pinned: HashSet<String> = config.pinned.iter().map(|p| key(p)).collect();
    let mut unique = HashSet::new();
    found.retain(|p| unique.insert(key(p)));

    // A real repo has a .git folder. A linked worktree has a .git file and belongs under its main repo.
    let (real, linked): (Vec<String>, Vec<String>) = found.into_iter().partition(|p| Path::new(p).join(".git").is_dir());

    // Worktree lookups run git once per repo, so run them side by side.
    let real_trees: Vec<Vec<String>> = std::thread::scope(|s| {
        let handles: Vec<_> = real.iter().map(|m| s.spawn(move || worktrees(m))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let tree_keys: HashSet<String> = real_trees.iter().flatten().map(|p| key(p)).collect();

    // Worktrees whose main repo is not scanned still show, on their own.
    let mut mains: Vec<(String, Vec<String>)> = real.into_iter().zip(real_trees).collect();
    for l in linked {
        if !tree_keys.contains(&key(&l)) {
            mains.push((l, Vec::new()));
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (main, children) in mains {
        if !seen.insert(key(&main)) {
            continue;
        }
        out.push(RepoEntry {
            name: name_of(&main),
            pinned: pinned.contains(&key(&main)),
            path: main.clone(),
            parent: None,
        });
        for child in children {
            if !seen.insert(key(&child)) {
                continue;
            }
            out.push(RepoEntry {
                name: name_of(&child),
                pinned: false,
                path: child,
                parent: Some(main.clone()),
            });
        }
    }
    out
}
