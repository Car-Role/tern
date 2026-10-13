//! Tickets assigned to you: Azure DevOps work items (through `az boards`) and GitHub issues (through `gh`).
//! Read only. Tern shows tickets; it does not change them.

use crate::run;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    /// Unique across sources: `azdo:<org>:<id>` or `gh:<owner/repo>#<n>`.
    pub key: String,
    pub provider: String,
    /// The Azure DevOps organization, or the GitHub `owner/repo`.
    pub source: String,
    pub number: i64,
    pub title: String,
    pub state: String,
    /// Task, Bug, User Story, Feature, Issue, ...
    pub kind: String,
    /// The Azure DevOps project, or the GitHub repo name.
    pub project: String,
    pub area: String,
    /// The sprint, for example `2026.40`.
    pub iteration: String,
    pub priority: Option<i64>,
    pub tags: Vec<String>,
    pub changed: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    pub body: String,
    /// true: `body` is Markdown. false: HTML. Azure DevOps can store a field either way.
    pub markdown: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// commit | pr | branch | parent | child | related
    pub kind: String,
    pub label: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub author: String,
    pub created: String,
    pub body: String,
    pub markdown: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct TicketDetail {
    /// true: section bodies and comments are Markdown (GitHub). false: HTML (Azure DevOps).
    pub markdown: bool,
    pub sections: Vec<Section>,
    pub links: Vec<Link>,
    pub comments: Vec<Comment>,
    pub created_by: String,
    pub created: String,
    pub assigned_to: String,
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

fn person(v: &Value) -> String {
    v["displayName"].as_str().or_else(|| v.as_str()).unwrap_or("").to_string()
}

fn home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(Into::into).unwrap_or_else(|| ".".into())
}

/// The last part of a path like `Billing\2026.40`.
fn last_part(p: &str) -> String {
    p.rsplit('\\').next().unwrap_or(p).to_string()
}

const WIQL: &str = "SELECT [System.Id],[System.Title],[System.State],[System.WorkItemType],[System.TeamProject],\
[Microsoft.VSTS.Common.Priority],[System.ChangedDate],[System.IterationPath],[System.AreaPath],[System.Tags] \
FROM WorkItems WHERE [System.AssignedTo] = @Me AND [System.State] NOT IN ('Closed','Done','Removed','Resolved','Completed','Cut') \
ORDER BY [System.ChangedDate] DESC";

pub fn azdo(org: &str) -> Result<Vec<Ticket>, String> {
    let org_url = format!("https://dev.azure.com/{org}");
    let text = run::tool(run::az_program(), &home(), &["boards", "query", "--org", &org_url, "--wiql", WIQL, "-o", "json"])?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items.iter().map(|w| azdo_ticket(org, &w["fields"])).collect())
}

fn azdo_ticket(org: &str, f: &Value) -> Ticket {
    let id = f["System.Id"].as_i64().unwrap_or(0);
    let project = s(f, "System.TeamProject");
    Ticket {
        key: format!("azdo:{org}:{id}"),
        provider: "azdo".into(),
        source: org.to_string(),
        number: id,
        title: s(f, "System.Title"),
        state: s(f, "System.State"),
        kind: s(f, "System.WorkItemType"),
        url: format!("https://dev.azure.com/{org}/{}/_workitems/edit/{id}", project.replace(' ', "%20")),
        area: last_part(&s(f, "System.AreaPath")),
        iteration: last_part(&s(f, "System.IterationPath")),
        priority: f["Microsoft.VSTS.Common.Priority"].as_i64(),
        tags: s(f, "System.Tags").split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        changed: s(f, "System.ChangedDate"),
        project,
    }
}

/// Reads `vstfs:///Git/Commit/<project>%2F<repo>%2F<sha>` and the like into a short label.
fn artifact_label(url: &str, name: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("vstfs:///Git/")?;
    let (what, ids) = rest.split_once('/')?;
    let ids = ids.replace("%2F", "/").replace("%2f", "/");
    // `<project>/<repo>/<rest>`: the rest can hold slashes, like a branch name.
    let last = ids.splitn(3, '/').nth(2).unwrap_or("").to_string();
    match what {
        "Commit" => Some(("commit".into(), format!("{name} {}", &last[..last.len().min(7)]))),
        "PullRequestId" => Some(("pr".into(), format!("Pull request !{last}"))),
        "Ref" => Some(("branch".into(), format!("Branch {}", last.trim_start_matches("GB")))),
        _ => None,
    }
}

pub fn azdo_detail(org: &str, project: &str, id: i64) -> Result<TicketDetail, String> {
    let org_url = format!("https://dev.azure.com/{org}");
    let ids = id.to_string();
    let text = run::tool(run::az_program(), &home(), &["boards", "work-item", "show", "--org", &org_url, "--id", &ids, "--expand", "relations", "-o", "json"])?;
    let w: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let f = &w["fields"];
    let mut d = TicketDetail {
        markdown: false,
        created_by: person(&f["System.CreatedBy"]),
        created: s(f, "System.CreatedDate"),
        assigned_to: person(&f["System.AssignedTo"]),
        ..Default::default()
    };
    for (field, title) in [
        ("System.Description", "Description"),
        ("Microsoft.VSTS.TCM.ReproSteps", "Repro steps"),
        ("Custom.ExpectedResults", "Expected"),
        ("Custom.ActualResults", "Actual"),
        ("Microsoft.VSTS.Common.AcceptanceCriteria", "Acceptance criteria"),
        ("Microsoft.VSTS.TCM.SystemInfo", "System info"),
    ] {
        let body = s(f, field);
        if !body.trim().is_empty() {
            let markdown = w["multilineFieldsFormat"][field].as_str().is_some_and(|m| m.eq_ignore_ascii_case("markdown"));
            d.sections.push(Section { title: title.into(), body, markdown });
        }
    }
    let web = |wid: &str| format!("https://dev.azure.com/{org}/{}/_workitems/edit/{wid}", project.replace(' ', "%20"));
    for r in w["relations"].as_array().into_iter().flatten() {
        let rel = s(r, "rel");
        let url = s(r, "url");
        let name = r["attributes"]["name"].as_str().unwrap_or("").to_string();
        if rel == "ArtifactLink" {
            if let Some((kind, label)) = artifact_label(&url, &name) {
                d.links.push(Link { kind, label, url: String::new() });
            }
        } else if rel.starts_with("System.LinkTypes") {
            let wid = url.rsplit('/').next().unwrap_or("").to_string();
            let kind = match rel.as_str() {
                "System.LinkTypes.Hierarchy-Reverse" => "parent",
                "System.LinkTypes.Hierarchy-Forward" => "child",
                _ => "related",
            };
            d.links.push(Link { kind: kind.into(), label: format!("{name} #{wid}"), url: web(&wid) });
        }
    }
    // Comments are a separate call. A failure here should not hide the rest.
    if f["System.CommentCount"].as_i64().unwrap_or(0) > 0 {
        let route = format!("project={project}");
        let wi = format!("workItemId={id}");
        if let Ok(text) = run::tool(run::az_program(), &home(), &[
            "devops", "invoke", "--org", &org_url, "--area", "wit", "--resource", "comments",
            "--route-parameters", &route, &wi, "--api-version", "7.1-preview", "-o", "json",
        ]) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                for c in v["comments"].as_array().into_iter().flatten().take(20) {
                    let markdown = s(c, "format").eq_ignore_ascii_case("markdown");
                    d.comments.push(Comment { author: person(&c["createdBy"]), created: s(c, "createdDate"), body: s(c, "text"), markdown });
                }
            }
        }
    }
    Ok(d)
}

pub fn github() -> Result<Vec<Ticket>, String> {
    let text = run::tool("gh", &home(), &[
        "search", "issues", "--assignee", "@me", "--state", "open", "--limit", "100",
        "--json", "number,title,state,url,repository,updatedAt,labels",
    ])?;
    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(items
        .iter()
        .map(|i| {
            let repo = i["repository"]["nameWithOwner"].as_str().unwrap_or("").to_string();
            let n = i["number"].as_i64().unwrap_or(0);
            Ticket {
                key: format!("gh:{repo}#{n}"),
                provider: "github".into(),
                project: repo.rsplit('/').next().unwrap_or(&repo).to_string(),
                source: repo,
                number: n,
                title: s(i, "title"),
                state: "Open".into(),
                kind: "Issue".into(),
                tags: i["labels"].as_array().into_iter().flatten().filter_map(|l| l["name"].as_str().map(String::from)).collect(),
                changed: s(i, "updatedAt"),
                url: s(i, "url"),
                ..Default::default()
            }
        })
        .collect())
}

pub fn github_detail(repo: &str, number: i64) -> Result<TicketDetail, String> {
    let n = number.to_string();
    let text = run::tool("gh", &home(), &["issue", "view", &n, "-R", repo, "--json", "body,author,createdAt,assignees,comments"])?;
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut d = TicketDetail {
        markdown: true,
        created_by: v["author"]["login"].as_str().unwrap_or("").to_string(),
        created: s(&v, "createdAt"),
        assigned_to: v["assignees"].as_array().into_iter().flatten().filter_map(|a| a["login"].as_str()).collect::<Vec<_>>().join(", "),
        ..Default::default()
    };
    let body = s(&v, "body");
    if !body.trim().is_empty() {
        d.sections.push(Section { title: "Description".into(), body, markdown: true });
    }
    for c in v["comments"].as_array().into_iter().flatten().take(20) {
        d.comments.push(Comment { author: c["author"]["login"].as_str().unwrap_or("").to_string(), created: s(c, "createdAt"), body: s(c, "body"), markdown: true });
    }
    Ok(d)
}

// Images

/// The Azure DevOps sign-in, reused for 45 minutes (az tokens last about an hour).
static AZDO_TOKEN: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);

fn azdo_token() -> Result<String, String> {
    let mut cached = AZDO_TOKEN.lock().unwrap();
    if let Some((t, at)) = cached.as_ref() {
        if at.elapsed().as_secs() < 45 * 60 {
            return Ok(t.clone());
        }
    }
    // 499b84ac-... is Azure DevOps' fixed resource id.
    let t = run::tool(run::az_program(), &home(), &[
        "account", "get-access-token", "--resource", "499b84ac-1321-427f-aa17-267ca6975798", "--query", "accessToken", "-o", "tsv",
    ])?
    .trim()
    .to_string();
    *cached = Some((t.clone(), std::time::Instant::now()));
    Ok(t)
}

/// The host of an https URL, lowercased. Anything else is refused.
fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn is_azdo_host(host: &str) -> bool {
    host == "dev.azure.com" || host.ends_with(".visualstudio.com")
}

fn is_github_host(host: &str) -> bool {
    host == "github.com" || host == "api.github.com" || host.ends_with(".githubusercontent.com")
}

/// Which sign-in an image host gets. A token is only ever sent to its own service.
fn auth_for(host: &str) -> Result<Option<String>, String> {
    if is_azdo_host(host) {
        Ok(Some(azdo_token()?))
    } else if is_github_host(host) {
        Ok(run::tool("gh", &home(), &["auth", "token"]).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    } else {
        Ok(None)
    }
}

const MAX_IMAGE: u64 = 20 * 1024 * 1024;

/// One request with curl. The token goes in through stdin, so it never shows in a process list.
/// Returns (status, redirect target, content type, body).
fn curl_once(url: &str, token: Option<&str>) -> Result<(u32, String, String, Vec<u8>), String> {
    use std::io::Write;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let body_file = std::env::temp_dir().join(format!("tern-img-{}-{stamp}", std::process::id()));
    let mut cmd = run::command("curl");
    cmd.args(["-sS", "--max-time", "30", "--max-filesize", &MAX_IMAGE.to_string(), "-o"])
        .arg(&body_file)
        .args(["-w", "%{http_code}\n%{redirect_url}\n%{content_type}"]);
    if token.is_some() {
        cmd.args(["-H", "@-"]);
    }
    cmd.arg(url)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("curl: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(t) = token {
            let _ = writeln!(stdin, "Authorization: Bearer {t}");
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let body = std::fs::read(&body_file).unwrap_or_default();
    let _ = std::fs::remove_file(&body_file);
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let meta = String::from_utf8_lossy(&out.stdout).to_string();
    let mut lines = meta.lines();
    let code = lines.next().unwrap_or("0").trim().parse().unwrap_or(0);
    let redirect = lines.next().unwrap_or("").trim().to_string();
    let kind = lines.next().unwrap_or("").trim().to_string();
    Ok((code, redirect, kind, body))
}

/// Downloads an image in a ticket and returns it as a `data:` URL the window can show.
/// Redirects are followed without the token (GitHub sends images to signed storage links).
pub fn image(url: &str) -> Result<String, String> {
    let host = https_host(url).ok_or("only https images can be shown")?;
    let token = auth_for(&host)?;
    let (mut code, mut redirect, mut kind, mut body) = curl_once(url, token.as_deref())?;
    let mut hops = 0;
    while (300..400).contains(&code) && !redirect.is_empty() && hops < 3 {
        let next = redirect.clone();
        let next_host = https_host(&next).ok_or("the image moved to a non-https address")?;
        // Keep the token only while the redirect stays on the same host.
        let t = if next_host == host { token.as_deref() } else { None };
        (code, redirect, kind, body) = curl_once(&next, t)?;
        hops += 1;
    }
    if code != 200 {
        return Err(format!("the image server answered {code}"));
    }
    let mime = sniff(&body).map(String::from).unwrap_or_else(|| kind.split(';').next().unwrap_or("").trim().to_string());
    if !mime.starts_with("image/") {
        return Err("that link is not an image".into());
    }
    Ok(format!("data:{mime};base64,{}", base64(&body)))
}

/// The image type from its first bytes, which is more reliable than the server's label.
fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        let head = String::from_utf8_lossy(&b[..b.len().min(256)]).to_ascii_lowercase();
        head.contains("<svg").then_some("image/svg+xml")
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_work_item_fields() {
        let f: Value = serde_json::from_str(r#"{"System.Id":4812,"System.Title":"Sign-in issue","System.State":"To Do",
            "System.WorkItemType":"Task","System.TeamProject":"Billing Team","Microsoft.VSTS.Common.Priority":2,
            "System.IterationPath":"Billing Team\\2026.40","System.AreaPath":"Billing Team\\Accounts\\Mobile Sign-in",
            "System.Tags":"Mobile; Accounts","System.ChangedDate":"2026-09-28T14:44:12.69Z"}"#).unwrap();
        let t = azdo_ticket("acme", &f);
        assert_eq!(t.key, "azdo:acme:4812");
        assert_eq!(t.url, "https://dev.azure.com/acme/Billing%20Team/_workitems/edit/4812");
        assert_eq!(t.iteration, "2026.40");
        assert_eq!(t.area, "Mobile Sign-in");
        assert_eq!(t.tags, vec!["Mobile", "Accounts"]);
        assert_eq!(t.priority, Some(2));
    }

    #[test]
    fn reads_artifact_links() {
        let c = artifact_label("vstfs:///Git/Commit/6c47%2Fc358%2F374c66bba167", "Fixed in Commit").unwrap();
        assert_eq!(c, ("commit".into(), "Fixed in Commit 374c66b".into()));
        let p = artifact_label("vstfs:///Git/PullRequestId/6c47%2Fc358%2F812", "Pull Request").unwrap();
        assert_eq!(p.1, "Pull request !812");
        let b = artifact_label("vstfs:///Git/Ref/6c47%2Fc358%2FGBfeature%2Flogin", "Branch").unwrap();
        assert_eq!(b.1, "Branch feature/login");
    }

    #[test]
    fn sends_tokens_only_to_their_own_hosts() {
        assert_eq!(https_host("https://dev.azure.com/acme/x/_apis/wit/attachments/1?fileName=a.png").as_deref(), Some("dev.azure.com"));
        assert_eq!(https_host("https://user@Org.VisualStudio.com:443/x").as_deref(), Some("org.visualstudio.com"));
        assert_eq!(https_host("http://dev.azure.com/x"), None);
        assert!(!is_azdo_host("dev.azure.com.evil.example"));
        assert!(!is_github_host("github.com.evil.example"));
        assert!(is_github_host("private-user-images.githubusercontent.com"));
        assert!(auth_for("example.com").unwrap().is_none());
    }

    #[test]
    fn encodes_base64_and_sniffs_types() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), Some("image/png"));
        assert_eq!(sniff(b"<?xml version=\"1.0\"?><svg xmlns="), Some("image/svg+xml"));
        assert_eq!(sniff(b"<html>"), None);
    }
}
