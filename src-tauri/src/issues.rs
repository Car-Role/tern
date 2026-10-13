//! GitHub issues through `gh`.

use crate::git;
use crate::run;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    /// Hex color without "#", as GitHub stores it.
    pub color: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub author: String,
    pub state: String,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
    pub comments: usize,
    pub updated: String,
    pub created: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub created: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    pub issue: Issue,
    pub body: String,
    pub comments: Vec<Comment>,
}

fn s(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn to_issue(v: &Value) -> Issue {
    Issue {
        number: v["number"].as_i64().unwrap_or(0),
        title: s(v, "title"),
        author: v["author"]["login"].as_str().unwrap_or("").to_string(),
        state: s(v, "state").to_lowercase(),
        labels: v["labels"]
            .as_array()
            .map(|a| a.iter().map(|l| Label { name: s(l, "name"), color: s(l, "color") }).collect())
            .unwrap_or_default(),
        assignees: v["assignees"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["login"].as_str().map(String::from)).collect())
            .unwrap_or_default(),
        comments: v["comments"].as_array().map(|c| c.len()).or_else(|| v["comments"]["totalCount"].as_u64().map(|n| n as usize)).unwrap_or(0),
        updated: s(v, "updatedAt"),
        created: s(v, "createdAt"),
        url: s(v, "url"),
    }
}

fn check_github(repo: &str) -> Result<(), String> {
    let summary = git::summary(repo);
    if summary.provider != "github" {
        return Err("Issues work for GitHub repos. This repo's remote is not on GitHub.".into());
    }
    Ok(())
}

const FIELDS: &str = "number,title,author,state,labels,assignees,comments,updatedAt,createdAt,url";

/// `state` is open, closed, or all.
pub fn list(repo: &str, state: &str) -> Result<Vec<Issue>, String> {
    check_github(repo)?;
    let state = match state {
        "closed" | "all" => state,
        _ => "open",
    };
    let text = run::tool("gh", Path::new(repo), &["issue", "list", "--state", state, "--limit", "100", "--json", FIELDS])?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items.iter().map(to_issue).collect())
}

pub fn detail(repo: &str, number: i64) -> Result<IssueDetail, String> {
    check_github(repo)?;
    let fields = format!("{FIELDS},body");
    let text = run::tool("gh", Path::new(repo), &["issue", "view", &number.to_string(), "--json", &fields])?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let comments = v["comments"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| Comment {
                    author: c["author"]["login"].as_str().unwrap_or("").to_string(),
                    body: s(c, "body"),
                    created: s(c, "createdAt"),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(IssueDetail { issue: to_issue(&v), body: s(&v, "body"), comments })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gh_issue_json() {
        let v: Value = serde_json::from_str(
            r#"{"number":87,"title":"Search","author":{"login":"jt"},"state":"OPEN",
                "labels":[{"name":"bug","color":"d73a4a"}],"assignees":[{"login":"cb"}],
                "comments":[{"body":"x"},{"body":"y"}],"updatedAt":"2026-07-20T09:45:01Z","url":"u"}"#,
        )
        .unwrap();
        let i = to_issue(&v);
        assert_eq!((i.number, i.state.as_str(), i.comments), (87, "open", 2));
        assert_eq!(i.labels[0].color, "d73a4a");
        assert_eq!(i.assignees, vec!["cb"]);
    }
}
