//! Finding the build, test, and dev commands a repo offers: npm scripts, VS Code tasks,
//! .NET, Gradle, Cargo, Make, Docker Compose, and commands the user saved.

use crate::config::CustomTask;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Stable id, for example "npm:web:dev".
    pub id: String,
    pub name: String,
    /// npm | vscode | dotnet | gradle | cargo | make | compose | custom
    pub source: String,
    /// The command line, run with `cmd /C`.
    pub command: String,
    /// Folder relative to the repo, with forward slashes. Empty for the repo root.
    pub cwd: String,
    /// Extra text shown under the name.
    pub detail: String,
}

const SKIP_DIRS: &[&str] = &["node_modules", "bin", "obj", "target", "dist", "build", "out", ".git", "packages"];

fn task(source: &str, cwd: &str, name: &str, command: String, detail: String) -> Task {
    Task {
        id: format!("{source}:{cwd}:{name}"),
        name: name.to_string(),
        source: source.to_string(),
        command,
        cwd: cwd.to_string(),
        detail,
    }
}

/// Quotes one argument for cmd when it has spaces or special characters.
pub fn quote(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".into();
    }
    if arg.chars().any(|c| c.is_whitespace() || "&|<>^()".contains(c)) && !arg.starts_with('"') {
        format!("\"{arg}\"")
    } else {
        arg.to_string()
    }
}

/// Removes comments and trailing commas so VS Code's JSON-with-comments files can be read.
pub fn strip_jsonc(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
        } else if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        } else if c == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        } else if c == ',' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_whitespace() {
                j += 1;
            }
            if j < b.len() && (b[j] == '}' || b[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn rel(repo: &Path, dir: &Path) -> String {
    dir.strip_prefix(repo).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default()
}

/// The repo root plus its direct sub-folders, for monorepos like `web/` and `cdk/`.
fn package_dirs(repo: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![repo.to_path_buf()];
    if let Ok(entries) = fs::read_dir(repo) {
        let mut subs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                !n.starts_with('.') && !SKIP_DIRS.contains(&n.as_str())
            })
            .map(|e| e.path())
            .collect();
        subs.sort();
        dirs.extend(subs);
    }
    dirs
}

fn package_manager(repo: &Path, dir: &Path) -> &'static str {
    for d in [dir, repo] {
        if d.join("pnpm-lock.yaml").exists() {
            return "pnpm";
        }
        if d.join("yarn.lock").exists() {
            return "yarn";
        }
        if d.join("bun.lockb").exists() || d.join("bun.lock").exists() {
            return "bun";
        }
    }
    "npm"
}

fn npm(repo: &Path, out: &mut Vec<Task>) {
    for dir in package_dirs(repo) {
        let Ok(text) = fs::read_to_string(dir.join("package.json")) else { continue };
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        let Some(scripts) = json["scripts"].as_object() else { continue };
        let pm = package_manager(repo, &dir);
        let cwd = rel(repo, &dir);
        for (name, body) in scripts {
            if name.starts_with("pre") && scripts.contains_key(&name[3..]) || name.starts_with("post") && scripts.contains_key(&name[4..]) {
                continue; // npm runs these by itself around the main script
            }
            let command = if pm == "npm" { format!("npm run {name}") } else { format!("{pm} run {name}") };
            out.push(task("npm", &cwd, name, command, body.as_str().unwrap_or("").to_string()));
        }
    }
}

fn expand(s: &str, repo: &Path) -> String {
    let root = repo.to_string_lossy();
    let base = repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut out = s
        .replace("${workspaceFolder}", &root)
        .replace("${workspaceRoot}", &root)
        .replace("${workspaceFolderBasename}", &base)
        .replace("${cwd}", &root)
        .replace("${pathSeparator}", "\\");
    while let Some(start) = out.find("${env:") {
        let Some(end) = out[start..].find('}') else { break };
        let name = &out[start + 6..start + end];
        let value = std::env::var(name).unwrap_or_default();
        out.replace_range(start..start + end + 1, &value);
    }
    out
}

fn arg_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("value").and_then(|x| x.as_str()).map(String::from),
        _ => None,
    }
}

fn vscode(repo: &Path, out: &mut Vec<Task>) {
    let Ok(text) = fs::read_to_string(repo.join(".vscode").join("tasks.json")) else { return };
    let Ok(json) = serde_json::from_str::<Value>(&strip_jsonc(&text)) else { return };
    let Some(list) = json["tasks"].as_array() else { return };
    for t in list {
        // A "windows" block overrides the general fields on this platform.
        let mut t = t.clone();
        if let Some(win) = t.get("windows").and_then(|w| w.as_object()).cloned() {
            for (k, v) in win {
                t[k.as_str()] = v;
            }
        }
        let kind = t["type"].as_str().unwrap_or("process");
        let mut cwd_abs = t["options"]["cwd"].as_str().map(|c| expand(c, repo));
        let command = if kind == "npm" {
            let Some(script) = t["script"].as_str() else { continue };
            if let Some(p) = t["path"].as_str() {
                cwd_abs = Some(repo.join(p).to_string_lossy().into_owned());
            }
            let dir = cwd_abs.as_deref().map(PathBuf::from).unwrap_or_else(|| repo.to_path_buf());
            format!("{} run {script}", package_manager(repo, &dir))
        } else {
            let Some(cmd) = t["command"].as_str().map(|c| expand(c, repo)) else { continue };
            let args: Vec<String> = t["args"]
                .as_array()
                .map(|a| a.iter().filter_map(arg_text).map(|x| quote(&expand(&x, repo))).collect())
                .unwrap_or_default();
            let head = if kind == "shell" { cmd } else { quote(&cmd) };
            std::iter::once(head).chain(args).collect::<Vec<_>>().join(" ")
        };
        let label = t["label"]
            .as_str()
            .map(String::from)
            .or_else(|| t["script"].as_str().map(|s| format!("npm: {s}")))
            .unwrap_or_else(|| command.clone());
        let cwd = cwd_abs.map(|c| rel(repo, Path::new(&c))).unwrap_or_default();
        let depends = t["dependsOn"].is_string() || t["dependsOn"].is_array();
        let detail = if depends { format!("{command}  (its dependsOn tasks do not run here)") } else { command.clone() };
        out.push(task("vscode", &cwd, &label, command, detail));
    }
}

fn find_files(dir: &Path, ext: &str, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if depth > 0 && !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                find_files(&p, ext, depth - 1, out);
            }
        } else if name.to_lowercase().ends_with(ext) {
            out.push(p);
        }
    }
}

fn dotnet(repo: &Path, out: &mut Vec<Task>) {
    let mut slns = Vec::new();
    find_files(repo, ".sln", 1, &mut slns);
    let mut slnx = Vec::new();
    find_files(repo, ".slnx", 1, &mut slnx);
    slns.extend(slnx);
    slns.sort();
    for sln in &slns {
        let r = rel(repo, sln);
        out.push(task("dotnet", "", &format!("build {r}"), format!("dotnet build {}", quote(&r)), String::new()));
        out.push(task("dotnet", "", &format!("test {r}"), format!("dotnet test {}", quote(&r)), String::new()));
    }
    let mut projs = Vec::new();
    find_files(repo, ".csproj", 4, &mut projs);
    projs.sort();
    let mut runnable = 0;
    for proj in projs {
        let Ok(text) = fs::read_to_string(&proj) else { continue };
        let name = proj.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let is_test = name.ends_with("Tests") || name.ends_with("Test") || text.contains("<IsTestProject>true");
        let is_app = text.contains("Microsoft.NET.Sdk.Web") || text.contains("<OutputType>Exe</OutputType>");
        if !is_app || is_test || runnable >= 8 {
            continue;
        }
        runnable += 1;
        let r = rel(repo, &proj);
        out.push(task("dotnet", "", &format!("run {name}"), format!("dotnet run --project {}", quote(&r)), r.clone()));
        out.push(task("dotnet", "", &format!("watch {name}"), format!("dotnet watch run --project {}", quote(&r)), r));
    }
    if slns.is_empty() && runnable == 0 && repo.join("global.json").exists() {
        out.push(task("dotnet", "", "build", "dotnet build".into(), String::new()));
    }
}

fn gradle(repo: &Path, out: &mut Vec<Task>) {
    let wrapper = if cfg!(windows) { "gradlew.bat" } else { "./gradlew" };
    let has_wrapper = repo.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" }).exists();
    let has_build = repo.join("build.gradle").exists() || repo.join("build.gradle.kts").exists();
    if !has_build && !has_wrapper {
        return;
    }
    let tool = if has_wrapper { wrapper } else { "gradle" };
    for t in ["build", "test", "run", "clean"] {
        out.push(task("gradle", "", t, format!("{tool} {t}"), String::new()));
    }
}

fn cargo(repo: &Path, out: &mut Vec<Task>) {
    if !repo.join("Cargo.toml").exists() {
        return;
    }
    for t in ["build", "test", "run", "check"] {
        out.push(task("cargo", "", t, format!("cargo {t}"), String::new()));
    }
}

fn make(repo: &Path, out: &mut Vec<Task>) {
    let Ok(text) = fs::read_to_string(repo.join("Makefile")) else { return };
    for line in text.lines() {
        if line.starts_with(|c: char| c.is_whitespace() || c == '.' || c == '#') {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else { continue };
        if rest.starts_with('=') || name.contains(['$', '%', ' ', '=']) || name.is_empty() {
            continue;
        }
        out.push(task("make", "", name, format!("make {name}"), String::new()));
    }
}

fn compose(repo: &Path, out: &mut Vec<Task>) {
    let found = ["compose.yaml", "compose.yml", "docker-compose.yml", "docker-compose.yaml"]
        .iter()
        .any(|f| repo.join(f).exists());
    if found {
        out.push(task("compose", "", "up", "docker compose up".into(), String::new()));
        out.push(task("compose", "", "down", "docker compose down".into(), String::new()));
    }
}

pub fn discover(repo: &str, custom: &[CustomTask]) -> Vec<Task> {
    let root = Path::new(repo);
    let mut out = Vec::new();
    for (i, c) in custom.iter().enumerate() {
        let mut t = task("custom", &c.cwd, &c.name, c.command.clone(), c.command.clone());
        t.id = format!("custom:{i}:{}", c.name);
        out.push(t);
    }
    vscode(root, &mut out);
    npm(root, &mut out);
    dotnet(root, &mut out);
    gradle(root, &mut out);
    cargo(root, &mut out);
    make(root, &mut out);
    compose(root, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints what Tern finds in real repos: `TERN_REPOS="C:\a;C:\b" cargo test print_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_real() {
        for repo in std::env::var("TERN_REPOS").unwrap_or_default().split(';').filter(|r| !r.is_empty()) {
            println!("== {repo}");
            for t in discover(repo, &[]) {
                println!("  [{}] {:<28} cwd={:<8} {}", t.source, t.name, t.cwd, t.command);
            }
        }
    }

    #[test]
    fn strips_comments_and_trailing_commas() {
        let text = r#"{
            // a comment
            "a": "http://x/*not a comment*/", /* block */
            "b": [1, 2,],
        }"#;
        let v: Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
        assert_eq!(v["a"], "http://x/*not a comment*/");
        assert_eq!(v["b"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn finds_tasks_in_a_folder() {
        let dir = std::env::temp_dir().join(format!("tern-tasks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".vscode")).unwrap();
        fs::create_dir_all(dir.join("web")).unwrap();
        fs::create_dir_all(dir.join("Api")).unwrap();
        fs::write(dir.join("package.json"), r#"{"scripts":{"predev":"x","dev":"vite","build":"vite build"}}"#).unwrap();
        fs::write(dir.join("web/package.json"), r#"{"scripts":{"start":"ng serve"}}"#).unwrap();
        fs::write(dir.join("web/yarn.lock"), "").unwrap();
        fs::write(dir.join("App.sln"), "").unwrap();
        fs::write(dir.join("Api/Api.csproj"), r#"<Project Sdk="Microsoft.NET.Sdk.Web"></Project>"#).unwrap();
        fs::write(
            dir.join(".vscode/tasks.json"),
            r#"{ "tasks": [
                { "label": "build", "type": "process", "command": "dotnet",
                  "args": ["build", "${workspaceFolder}/App.sln"], },
                { "type": "npm", "script": "start", "path": "web", "label": "npm: start" },
            ] }"#,
        )
        .unwrap();
        let custom = vec![CustomTask { name: "seed".into(), command: "node seed.js".into(), cwd: String::new() }];
        let tasks = discover(&dir.to_string_lossy(), &custom);
        let find = |id: &str| tasks.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("{id} missing: {tasks:#?}"));

        assert_eq!(find("custom:0:seed").command, "node seed.js");
        assert_eq!(find("npm::dev").command, "npm run dev");
        assert!(!tasks.iter().any(|t| t.name == "predev"));
        assert_eq!(find("npm:web:start").command, "yarn run start");
        assert_eq!(find("dotnet::build App.sln").command, "dotnet build App.sln");
        assert_eq!(find("dotnet::run Api").command, "dotnet run --project Api/Api.csproj");
        let vs = find("vscode::build");
        assert!(vs.command.starts_with("dotnet build "), "{}", vs.command);
        assert!(vs.command.contains("App.sln"));
        assert_eq!(find("vscode:web:npm: start").command, "yarn run start");
        let _ = fs::remove_dir_all(&dir);
    }
}
