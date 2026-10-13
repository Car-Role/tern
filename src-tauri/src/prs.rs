//! Pull requests from GitHub (through `gh`) and Azure DevOps (through `az repos`).
//! Diffs are made locally: fetch both branches, then compare the merge base with the PR head.

use crate::git::{self, Change};
use crate::run;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub source: String,
    pub target: String,
    pub url: String,
    pub draft: bool,
    pub updated: String,
    pub head_sha: Option<String>,
    /// approved | changes | waiting | none
    pub review: String,
    pub provider: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrDiff {
    pub base: String,
    pub head: String,
    pub files: Vec<Change>,
}

#[derive(Debug, PartialEq)]
pub struct AzdoRepo {
    pub org: String,
    pub project: String,
    pub repo: String,
}

fn decode(s: &str) -> String {
    // Enough for Azure DevOps names, which mostly escape spaces.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads org, project, and repo from any Azure DevOps remote URL form.
pub fn parse_azdo(url: &str) -> Option<AzdoRepo> {
    let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
    // SSH: git@ssh.dev.azure.com:v3/org/project/repo or org@vs-ssh.visualstudio.com:v3/org/project/repo
    if let Some(i) = url.find(":v3/") {
        let parts: Vec<&str> = url[i + 4..].split('/').collect();
        if parts.len() >= 3 {
            return Some(AzdoRepo { org: decode(parts[0]), project: decode(parts[1]), repo: decode(parts[2]) });
        }
        return None;
    }
    let rest = url.split("://").nth(1)?;
    let rest = rest.rsplit('@').next()?; // drop "user@"
    let mut segs = rest.split('/');
    let host = segs.next()?.to_lowercase();
    let segs: Vec<&str> = segs.filter(|s| !s.is_empty()).collect();
    let git_at = segs.iter().position(|s| *s == "_git")?;
    let repo = segs.get(git_at + 1)?;
    let (org, before): (String, &[&str]) = if host == "dev.azure.com" {
        (segs.first()?.to_string(), &segs[1..git_at])
    } else if let Some(org) = host.strip_suffix(".visualstudio.com") {
        (org.to_string(), &segs[..git_at])
    } else {
        return None;
    };
    // Project is the last segment before _git. It is missing when repo and project share a name.
    let project = before.iter().rev().find(|s| !s.eq_ignore_ascii_case("DefaultCollection"));
    let project = project.map(|s| s.to_string()).unwrap_or_else(|| repo.to_string());
    Some(AzdoRepo { org: decode(&org), project: decode(&project), repo: decode(repo) })
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> &'a str {
    let mut cur = v;
    for key in path {
        cur = &cur[*key];
    }
    cur.as_str().unwrap_or("")
}

fn github(repo: &str) -> Result<Vec<PullRequest>, String> {
    let text = run::tool(
        "gh",
        Path::new(repo),
        &[
            "pr", "list", "--state", "open", "--limit", "50", "--json",
            "number,title,author,headRefName,baseRefName,headRefOid,url,isDraft,updatedAt,reviewDecision",
        ],
    )?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items
        .iter()
        .map(|v| PullRequest {
            id: v["number"].as_i64().unwrap_or(0),
            title: str_at(v, &["title"]).into(),
            author: str_at(v, &["author", "login"]).into(),
            source: str_at(v, &["headRefName"]).into(),
            target: str_at(v, &["baseRefName"]).into(),
            url: str_at(v, &["url"]).into(),
            draft: v["isDraft"].as_bool().unwrap_or(false),
            updated: str_at(v, &["updatedAt"]).into(),
            head_sha: Some(str_at(v, &["headRefOid"]).to_string()).filter(|s| !s.is_empty()),
            review: match str_at(v, &["reviewDecision"]) {
                "APPROVED" => "approved",
                "CHANGES_REQUESTED" => "changes",
                "REVIEW_REQUIRED" => "waiting",
                _ => "none",
            }
            .into(),
            provider: "github".into(),
        })
        .collect())
}

fn azdo(repo: &str, remote: &str) -> Result<Vec<PullRequest>, String> {
    let r = parse_azdo(remote).ok_or_else(|| format!("could not read the Azure DevOps project from {remote}"))?;
    let org_url = format!("https://dev.azure.com/{}", r.org);
    let text = run::tool(
        run::az_program(),
        Path::new(repo),
        &[
            "repos", "pr", "list", "--org", &org_url, "--project", &r.project, "--repository", &r.repo,
            "--status", "active", "--top", "50", "-o", "json",
        ],
    )?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let web = format!("https://dev.azure.com/{}/{}/_git/{}", r.org, r.project, r.repo).replace(' ', "%20");
    Ok(items
        .iter()
        .map(|v| {
            let id = v["pullRequestId"].as_i64().unwrap_or(0);
            let votes: Vec<i64> = v["reviewers"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["vote"].as_i64()).collect())
                .unwrap_or_default();
            let review = if votes.iter().any(|&x| x == -10) {
                "changes"
            } else if votes.iter().any(|&x| x == -5) {
                "waiting"
            } else if votes.iter().any(|&x| x >= 5) {
                "approved"
            } else {
                "none"
            };
            PullRequest {
                id,
                title: str_at(v, &["title"]).into(),
                author: str_at(v, &["createdBy", "displayName"]).into(),
                source: str_at(v, &["sourceRefName"]).trim_start_matches("refs/heads/").into(),
                target: str_at(v, &["targetRefName"]).trim_start_matches("refs/heads/").into(),
                url: format!("{web}/pullrequest/{id}"),
                draft: v["isDraft"].as_bool().unwrap_or(false),
                updated: str_at(v, &["creationDate"]).into(),
                head_sha: Some(str_at(v, &["lastMergeSourceCommit", "commitId"]).to_string())
                    .filter(|s| !s.is_empty()),
                review: review.into(),
                provider: "azdo".into(),
            }
        })
        .collect())
}

pub fn list(repo: &str) -> Result<Vec<PullRequest>, String> {
    let s = git::summary(repo);
    let remote = s.remote.ok_or("this repo has no remote")?;
    match s.provider.as_str() {
        "github" => github(repo),
        "azdo" => azdo(repo, &remote),
        _ => Err("pull requests work for GitHub and Azure DevOps remotes only".into()),
    }
}

fn remote_name(repo: &str) -> String {
    let names = run::git(Path::new(repo), &["remote"]).unwrap_or_default();
    if names.lines().any(|l| l == "origin") {
        return "origin".into();
    }
    names.lines().next().unwrap_or("origin").to_string()
}

fn commit_exists(repo: &str, sha: &str) -> bool {
    run::git(Path::new(repo), &["cat-file", "-e", &format!("{sha}^{{commit}}")]).is_ok()
}

pub fn prepare(repo: &str, pr: &PullRequest) -> Result<PrDiff, String> {
    let remote = remote_name(repo);
    let target_ref = format!("refs/remotes/{remote}/{}", pr.target);
    let target_spec = format!("+refs/heads/{0}:refs/remotes/{remote}/{0}", pr.target);
    let source_ref = format!("refs/remotes/{remote}/{}", pr.source);
    let head_spec = if pr.provider == "github" {
        format!("refs/pull/{}/head", pr.id)
    } else {
        format!("+refs/heads/{0}:refs/remotes/{remote}/{0}", pr.source)
    };
    run::git_verbose(Path::new(repo), &["fetch", "--no-write-fetch-head", &remote, &head_spec, &target_spec])?;

    let head = match &pr.head_sha {
        Some(sha) if commit_exists(repo, sha) => sha.clone(),
        _ if pr.provider != "github" => run::git(Path::new(repo), &["rev-parse", &source_ref])?.trim().to_string(),
        _ => return Err("could not find the pull request head commit after fetch".into()),
    };
    let base = run::git(Path::new(repo), &["merge-base", &target_ref, &head])?.trim().to_string();
    let files = git::rev_files(repo, &base, &head)?;
    Ok(PrDiff { base, head, files })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(org: &str, project: &str, repo: &str) -> Option<AzdoRepo> {
        Some(AzdoRepo { org: org.into(), project: project.into(), repo: repo.into() })
    }

    #[test]
    fn parses_azdo_urls() {
        assert_eq!(parse_azdo("https://acme@dev.azure.com/acme/Web%20App/_git/site"), r("acme", "Web App", "site"));
        assert_eq!(parse_azdo("https://dev.azure.com/acme/proj/_git/repo"), r("acme", "proj", "repo"));
        assert_eq!(parse_azdo("https://acme.visualstudio.com/proj/_git/repo"), r("acme", "proj", "repo"));
        assert_eq!(
            parse_azdo("https://acme.visualstudio.com/DefaultCollection/proj/_git/repo"),
            r("acme", "proj", "repo")
        );
        assert_eq!(parse_azdo("git@ssh.dev.azure.com:v3/acme/proj/repo"), r("acme", "proj", "repo"));
        assert_eq!(parse_azdo("https://github.com/a/b.git"), None);
    }
}
