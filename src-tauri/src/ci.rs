//! CI runs: Azure DevOps pipelines (through `az`) and GitHub Actions (through `gh`), in one shape.

use crate::git;
use crate::prs::parse_azdo;
use crate::run;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: i64,
    /// Pipeline or workflow name.
    pub name: String,
    pub number: String,
    /// Commit message or run title, when known.
    pub title: String,
    /// queued | running | completed
    pub status: String,
    /// succeeded | partial | failed | canceled | skipped | "" (not finished)
    pub result: String,
    pub branch: String,
    pub sha: String,
    pub by: String,
    pub started: String,
    pub finished: String,
    pub url: String,
    pub provider: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// error | warning
    pub kind: String,
    pub message: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub name: String,
    /// stage | job | step
    pub kind: String,
    pub depth: u32,
    pub status: String,
    pub result: String,
    pub started: String,
    pub finished: String,
    pub issues: Vec<Issue>,
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

fn azdo_status(st: &str) -> &'static str {
    match st {
        "completed" => "completed",
        "inProgress" | "cancelling" => "running",
        _ => "queued",
    }
}

fn azdo_result(r: &str) -> &'static str {
    match r {
        "succeeded" => "succeeded",
        "partiallySucceeded" | "succeededWithIssues" => "partial",
        "failed" => "failed",
        "canceled" | "abandoned" => "canceled",
        "skipped" => "skipped",
        _ => "",
    }
}

fn gh_status(st: &str) -> &'static str {
    match st {
        "completed" => "completed",
        "in_progress" => "running",
        _ => "queued",
    }
}

fn gh_result(c: &str) -> &'static str {
    match c {
        "success" => "succeeded",
        "failure" | "timed_out" | "startup_failure" => "failed",
        "cancelled" => "canceled",
        "skipped" | "neutral" => "skipped",
        "action_required" => "partial",
        _ => "",
    }
}

fn azdo_ctx(repo: &str) -> Result<(String, String, String, String), String> {
    let remote = git::summary(repo).remote.ok_or("this repo has no remote")?;
    let r = parse_azdo(&remote).ok_or("not an Azure DevOps remote")?;
    let org_url = format!("https://dev.azure.com/{}", r.org);
    Ok((org_url, r.org, r.project, r.repo))
}

pub fn list(repo: &str) -> Result<Vec<Run>, String> {
    match git::summary(repo).provider.as_str() {
        "azdo" => azdo_list(repo),
        "github" => github_list(repo),
        _ => Err("CI works for Azure DevOps and GitHub repos.".into()),
    }
}

fn azdo_list(repo: &str) -> Result<Vec<Run>, String> {
    let (org_url, org, project, name) = azdo_ctx(repo)?;
    let cwd = Path::new(repo);
    let defs = run::tool(run::az_program(), cwd, &[
        "pipelines", "list", "--org", &org_url, "--project", &project, "--repository", &name, "--repository-type", "tfsgit", "-o", "json",
    ])?;
    let defs: Vec<Value> = serde_json::from_str(&defs).map_err(|e| e.to_string())?;
    let ids: Vec<String> = defs.iter().filter_map(|d| d["id"].as_i64()).map(|i| i.to_string()).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args = vec!["pipelines", "runs", "list", "--org", &org_url, "--project", &project, "--top", "30", "-o", "json", "--pipeline-ids"];
    args.extend(ids.iter().map(|s| s.as_str()));
    let text = run::tool(run::az_program(), cwd, &args)?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let web = format!("https://dev.azure.com/{}/{}/_build/results?buildId=", org, project.replace(' ', "%20"));
    Ok(items
        .iter()
        .map(|r| {
            let id = r["id"].as_i64().unwrap_or(0);
            Run {
                id,
                name: r["definition"]["name"].as_str().unwrap_or("").to_string(),
                number: s(r, "buildNumber"),
                title: r["triggerInfo"]["ci.message"].as_str().unwrap_or("").to_string(),
                status: azdo_status(r["status"].as_str().unwrap_or("")).into(),
                result: azdo_result(r["result"].as_str().unwrap_or("")).into(),
                branch: s(r, "sourceBranch").trim_start_matches("refs/heads/").to_string(),
                sha: s(r, "sourceVersion"),
                by: r["requestedFor"]["displayName"].as_str().unwrap_or("").to_string(),
                started: s(r, "startTime"),
                finished: s(r, "finishTime"),
                url: format!("{web}{id}"),
                provider: "azdo".into(),
            }
        })
        .collect())
}

fn github_list(repo: &str) -> Result<Vec<Run>, String> {
    let text = run::tool("gh", Path::new(repo), &[
        "run", "list", "--limit", "30", "--json",
        "databaseId,displayTitle,status,conclusion,headBranch,headSha,createdAt,updatedAt,url,workflowName,number,event",
    ])?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items
        .iter()
        .map(|r| {
            let status = gh_status(r["status"].as_str().unwrap_or(""));
            Run {
                id: r["databaseId"].as_i64().unwrap_or(0),
                name: s(r, "workflowName"),
                number: r["number"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
                title: s(r, "displayTitle"),
                status: status.into(),
                result: gh_result(r["conclusion"].as_str().unwrap_or("")).into(),
                branch: s(r, "headBranch"),
                sha: s(r, "headSha"),
                by: s(r, "event"),
                started: s(r, "createdAt"),
                finished: if status == "completed" { s(r, "updatedAt") } else { String::new() },
                url: s(r, "url"),
                provider: "github".into(),
            }
        })
        .collect())
}

pub fn detail(repo: &str, id: i64) -> Result<Vec<Step>, String> {
    match git::summary(repo).provider.as_str() {
        "azdo" => azdo_detail(repo, id),
        "github" => github_detail(repo, id),
        _ => Err("CI works for Azure DevOps and GitHub repos.".into()),
    }
}

fn azdo_detail(repo: &str, id: i64) -> Result<Vec<Step>, String> {
    let (org_url, _, project, _) = azdo_ctx(repo)?;
    let text = run::tool(run::az_program(), Path::new(repo), &[
        "devops", "invoke", "--org", &org_url, "--area", "build", "--resource", "timeline",
        "--route-parameters", &format!("project={project}"), &format!("buildId={id}"), "--api-version", "7.1", "-o", "json",
    ])?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let records = v["records"].as_array().cloned().unwrap_or_default();
    // Build the tree: Stage > Phase > Job > Task. Phases are folded into their jobs.
    let by_id: std::collections::HashMap<String, &Value> = records.iter().map(|r| (s(r, "id"), r)).collect();
    let kind_of = |r: &Value| match r["type"].as_str().unwrap_or("") {
        "Stage" => Some(("stage", 0)),
        "Job" => Some(("job", 1)),
        "Task" => Some(("step", 2)),
        _ => None,
    };
    let mut steps: Vec<(String, i64, Step)> = Vec::new();
    for r in &records {
        let Some((kind, depth)) = kind_of(r) else { continue };
        // Sort key: the order of the stage, then the job, then the task.
        let mut chain = vec![r["order"].as_i64().unwrap_or(0)];
        let mut cur = r;
        while let Some(parent) = cur["parentId"].as_str().and_then(|p| by_id.get(p)) {
            chain.push(parent["order"].as_i64().unwrap_or(0));
            cur = parent;
        }
        chain.reverse();
        let key = chain.iter().map(|o| format!("{o:05}")).collect::<Vec<_>>().join(".");
        let issues = r["issues"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|i| Issue { kind: s(i, "type").to_lowercase(), message: s(i, "message") })
                    .filter(|i| !i.message.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        steps.push((
            key,
            depth,
            Step {
                name: s(r, "name"),
                kind: kind.into(),
                depth: depth as u32,
                status: azdo_status(r["state"].as_str().unwrap_or("")).into(),
                result: azdo_result(r["result"].as_str().unwrap_or("")).into(),
                started: s(r, "startTime"),
                finished: s(r, "finishTime"),
                issues,
            },
        ));
    }
    steps.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(steps.into_iter().map(|(_, _, st)| st).collect())
}

fn github_detail(repo: &str, id: i64) -> Result<Vec<Step>, String> {
    let text = run::tool("gh", Path::new(repo), &["run", "view", &id.to_string(), "--json", "jobs"])?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for j in v["jobs"].as_array().cloned().unwrap_or_default() {
        out.push(Step {
            name: s(&j, "name"),
            kind: "job".into(),
            depth: 0,
            status: gh_status(j["status"].as_str().unwrap_or("")).into(),
            result: gh_result(j["conclusion"].as_str().unwrap_or("")).into(),
            started: s(&j, "startedAt"),
            finished: s(&j, "completedAt"),
            issues: Vec::new(),
        });
        for st in j["steps"].as_array().cloned().unwrap_or_default() {
            out.push(Step {
                name: s(&st, "name"),
                kind: "step".into(),
                depth: 1,
                status: gh_status(st["status"].as_str().unwrap_or("")).into(),
                result: gh_result(st["conclusion"].as_str().unwrap_or("")).into(),
                started: s(&st, "startedAt"),
                finished: s(&st, "completedAt"),
                issues: Vec::new(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_states_from_both_services() {
        assert_eq!((azdo_status("inProgress"), azdo_result("partiallySucceeded")), ("running", "partial"));
        assert_eq!((azdo_status("notStarted"), azdo_result("")), ("queued", ""));
        assert_eq!((gh_status("in_progress"), gh_result("failure")), ("running", "failed"));
        assert_eq!((gh_status("completed"), gh_result("success")), ("completed", "succeeded"));
        assert_eq!(gh_result("cancelled"), "canceled");
    }
}
