//! User settings, stored as one small JSON file in the app config folder.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A named group of repos, like "Work" or "Personal". Only one space shows at a time.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub color: String,
    /// A repo joins this space when its remote URL contains one of these (case does not matter).
    pub remotes: Vec<String>,
    /// A repo joins this space when its path starts with one of these folders.
    pub folders: Vec<String>,
    /// Tabs shown for repos in this space, in order. Empty means the default set.
    pub tabs: Vec<String>,
}

/// A command the user saved for one repo.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomTask {
    pub name: String,
    pub command: String,
    /// Folder relative to the repo. Empty for the repo root.
    pub cwd: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Folders that are scanned for git repos.
    pub roots: Vec<String>,
    /// Repos added by hand (anywhere on disk).
    pub extra: Vec<String>,
    /// Repos that must not show up.
    pub hidden: Vec<String>,
    /// Repos shown at the top of the list.
    pub pinned: Vec<String>,
    /// Main repos in the order you dragged them. Repos not in it follow, by name.
    pub order: Vec<String>,
    /// How many folder levels below a root are scanned.
    pub scan_depth: u32,
    /// Seconds between status refreshes while the window has focus.
    pub poll_seconds: u32,
    pub spaces: Vec<Space>,
    /// Repo path to space id, set by hand. Wins over the rules.
    pub assign: BTreeMap<String, String>,
    /// Space for repos that match no rule. None means they show only in "All".
    pub default_space: Option<String>,
    /// The space on screen. None means all spaces.
    pub active_space: Option<String>,
    /// Repo path to the commands saved for it.
    pub tasks: BTreeMap<String, Vec<CustomTask>>,
    /// Repo path to the ids of its pinned tasks, in pin order.
    pub pinned_tasks: BTreeMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        let mut roots = Vec::new();
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            for name in ["Coding projects", "source\\repos", "code", "projects", "dev"] {
                let p = Path::new(&home).join(name);
                if p.is_dir() {
                    roots.push(p.to_string_lossy().into_owned());
                }
            }
        }
        Config {
            roots,
            extra: Vec::new(),
            hidden: Vec::new(),
            pinned: Vec::new(),
            order: Vec::new(),
            scan_depth: 3,
            poll_seconds: 8,
            spaces: vec![
                Space {
                    id: "work".into(),
                    name: "Work".into(),
                    color: "#6fa8ff".into(),
                    remotes: Vec::new(),
                    folders: Vec::new(),
                    tabs: ["changes", "files", "history", "ci", "tasks"].map(String::from).to_vec(),
                },
                Space {
                    id: "personal".into(),
                    name: "Personal".into(),
                    color: "#f0b54a".into(),
                    remotes: Vec::new(),
                    folders: Vec::new(),
                    tabs: ["changes", "files", "prs", "issues", "history", "ci", "tasks"].map(String::from).to_vec(),
                },
            ],
            assign: BTreeMap::new(),
            default_space: Some("personal".into()),
            active_space: None,
            tasks: BTreeMap::new(),
            pinned_tasks: BTreeMap::new(),
        }
    }
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join("config.json")
}

pub fn load(config_dir: &Path) -> Config {
    match fs::read_to_string(path(config_dir)) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(config_dir: &Path, config: &Config) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path(config_dir), text).map_err(|e| e.to_string())
}
