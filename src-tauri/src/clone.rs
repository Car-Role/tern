//! Listing repos on Azure DevOps (`az`) and GitHub (`gh`), and building the clone command.

use crate::run;
use crate::tasks::quote;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRepo {
    pub name: String,
    /// Azure DevOps project, or GitHub owner.
    pub group: String,
    pub url: String,
    pub description: String,
    pub size: u64,
    pub updated: String,
    pub private: bool,
    pub disabled: bool,
}

fn org_url(org: &str) -> String {
    let org = org.trim().trim_end_matches('/');
    if org.starts_with("https://") { org.to_string() } else { format!("https://dev.azure.com/{org}") }
}

fn home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(Into::into).unwrap_or_else(std::env::temp_dir)
}

pub fn azdo_projects(org: &str) -> Result<Vec<String>, String> {
    let text = run::tool(run::az_program(), &home(), &["devops", "project", "list", "--org", &org_url(org), "--top", "500", "-o", "json"])?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let list = v["value"].as_array().or_else(|| v.as_array()).cloned().unwrap_or_default();
    let mut names: Vec<String> = list.iter().filter_map(|p| p["name"].as_str().map(String::from)).collect();
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names)
}

pub fn azdo_repos(org: &str, project: &str) -> Result<Vec<RemoteRepo>, String> {
    let text = run::tool(run::az_program(), &home(), &["repos", "list", "--org", &org_url(org), "--project", project, "-o", "json"])?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out: Vec<RemoteRepo> = items
        .iter()
        .map(|r| RemoteRepo {
            name: r["name"].as_str().unwrap_or("").to_string(),
            group: r["project"]["name"].as_str().unwrap_or(project).to_string(),
            url: r["remoteUrl"].as_str().unwrap_or("").to_string(),
            description: String::new(),
            size: r["size"].as_u64().unwrap_or(0),
            updated: String::new(),
            private: true,
            disabled: r["isDisabled"].as_bool().unwrap_or(false),
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    Ok(out)
}

/// Repos of a GitHub user or organization. An empty owner means the signed-in user.
pub fn github_repos(owner: &str) -> Result<Vec<RemoteRepo>, String> {
    let mut args = vec!["repo", "list"];
    if !owner.trim().is_empty() {
        args.push(owner.trim());
    }
    args.extend(["--limit", "300", "--json", "name,owner,url,description,diskUsage,pushedAt,isPrivate,isArchived"]);
    let text = run::tool("gh", &home(), &args)?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items
        .iter()
        .map(|r| RemoteRepo {
            name: r["name"].as_str().unwrap_or("").to_string(),
            group: r["owner"]["login"].as_str().unwrap_or("").to_string(),
            url: format!("{}.git", r["url"].as_str().unwrap_or("")),
            description: r["description"].as_str().unwrap_or("").to_string(),
            size: r["diskUsage"].as_u64().unwrap_or(0) * 1024,
            updated: r["pushedAt"].as_str().unwrap_or("").to_string(),
            private: r["isPrivate"].as_bool().unwrap_or(false),
            disabled: r["isArchived"].as_bool().unwrap_or(false),
        })
        .collect())
}

/// Checks the target folder and returns the command line for the clone.
pub fn command(url: &str, parent: &str, name: &str) -> Result<String, String> {
    let bad = |c: char| "\\/:*?\"<>|".contains(c);
    if name.is_empty() || name.contains(bad) || name == "." || name == ".." {
        return Err(format!("not a valid folder name: {name}"));
    }
    if !url.starts_with("https://") && !url.starts_with("git@") && !url.starts_with("ssh://") {
        return Err(format!("not a clone address: {url}"));
    }
    if url.contains(['"', '&', '|', '<', '>', '^', '%']) && !url.contains("%20") {
        return Err(format!("the address has characters that are not safe in a command: {url}"));
    }
    let parent_path = Path::new(parent);
    if !parent_path.is_dir() {
        return Err(format!("folder not found: {parent}"));
    }
    if parent_path.join(name).exists() {
        return Err(format!("{} already exists", parent_path.join(name).display()));
    }
    // %20 in Azure DevOps addresses would be read by cmd as a variable, so it is written as a space.
    let url = url.replace("%20", " ");
    Ok(format!("git clone --progress {} {}", quote(&url), quote(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_safe_clone_command() {
        let dir = std::env::temp_dir();
        let cmd = command("https://acme@dev.azure.com/acme/Billing%20Team/_git/invoices-service", &dir.to_string_lossy(), "invoices-service-tern-test").unwrap();
        assert_eq!(cmd, "git clone --progress \"https://acme@dev.azure.com/acme/Billing Team/_git/invoices-service\" invoices-service-tern-test");
        assert!(command("https://x/y.git", &dir.to_string_lossy(), "a/b").is_err());
        assert!(command("file:///c:/x", &dir.to_string_lossy(), "x").is_err());
        assert!(command("https://x/y.git & del", &dir.to_string_lossy(), "x").is_err());
    }
}
