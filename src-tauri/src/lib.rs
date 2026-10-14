mod branches;
mod ci;
mod clone;
mod config;
mod git;
mod hunks;
mod issues;
mod prs;
mod pty;
mod run;
mod runner;
mod scan;
mod search;
mod stash;
mod tasks;
mod tickets;

use config::Config;
use git::{Change, Commit, FileDiff, FileText, Summary};
use prs::{PrDiff, PullRequest};
use scan::RepoEntry;
use serde::Serialize;
use std::path::{Path, PathBuf};
use runner::{RunInfo, Runner};
use std::sync::{Arc, Mutex};
use tasks::Task;
use tauri::{Emitter, Manager, State};

struct AppState {
    config_dir: PathBuf,
    config: Mutex<Config>,
    runner: Arc<Runner>,
    terminals: pty::Terminals,
    /** Files that Windows (or macOS) asked Tern to open, not yet shown. */
    opened: Mutex<Vec<String>>,
}

/// The files among command-line arguments, as full paths. Flags are skipped.
fn file_args(args: impl Iterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.filter(|a| !a.starts_with('-'))
        .map(|a| cwd.join(a))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string())
        .collect()
}

/// Hands the waiting files to the window (once).
#[tauri::command]
fn take_opened(state: State<AppState>) -> Vec<String> {
    std::mem::take(&mut *state.opened.lock().unwrap())
}

/// Keeps the files to open and tells the window to pick them up.
fn hand_over(app: &tauri::AppHandle, files: Vec<String>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if files.is_empty() {
        return;
    }
    app.state::<AppState>().opened.lock().unwrap().extend(files);
    let _ = app.emit("open-files", ());
}

type Res<T> = Result<T, String>;

/// Runs slow work (git, network) off the main thread so the window never freezes.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_config(state: State<AppState>, config: Config) -> Res<()> {
    config::save(&state.config_dir, &config)?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
fn config_path(state: State<AppState>) -> String {
    config::path(&state.config_dir).to_string_lossy().into_owned()
}

#[tauri::command]
async fn list_repos(state: State<'_, AppState>) -> Res<Vec<RepoEntry>> {
    let cfg = state.config.lock().unwrap().clone();
    blocking(move || Ok(scan::list(&cfg))).await
}

#[tauri::command]
async fn summaries(paths: Vec<String>) -> Res<Vec<Summary>> {
    blocking(move || Ok(git::summaries(paths))).await
}

#[tauri::command]
async fn list_files(repo: String) -> Res<Vec<String>> {
    blocking(move || git::list_files(&repo)).await
}

#[tauri::command]
async fn read_file(repo: String, rel: String) -> Res<FileText> {
    blocking(move || git::read_file(&repo, &rel)).await
}

#[tauri::command]
async fn write_file(repo: String, rel: String, text: String) -> Res<()> {
    blocking(move || git::write_file(&repo, &rel, &text)).await
}

#[tauri::command]
async fn worktree_diff(repo: String, rel: String, old_rel: Option<String>) -> Res<FileDiff> {
    blocking(move || git::worktree_diff(&repo, &rel, old_rel.as_deref())).await
}

#[tauri::command]
async fn rev_diff(repo: String, base: String, head: String, rel: String, old_rel: Option<String>) -> Res<FileDiff> {
    blocking(move || Ok(git::rev_diff(&repo, &base, &head, &rel, old_rel.as_deref()))).await
}

#[derive(Serialize)]
struct RevFiles {
    base: String,
    head: String,
    files: Vec<Change>,
}

#[tauri::command]
async fn commit_files(repo: String, sha: String) -> Res<RevFiles> {
    blocking(move || {
        let base = git::parent_of(&repo, &sha);
        let files = git::rev_files(&repo, &base, &sha)?;
        Ok(RevFiles { base, head: sha, files })
    })
    .await
}

#[tauri::command]
async fn log(repo: String, count: u32, all: bool) -> Res<git::History> {
    blocking(move || git::log(&repo, count, all)).await
}

/// The files changed between two commits, for example everything a pull would bring.
#[tauri::command]
async fn range_files(repo: String, base: String, head: String) -> Res<RevFiles> {
    blocking(move || {
        let files = git::rev_files(&repo, &base, &head)?;
        Ok(RevFiles { base, head, files })
    })
    .await
}

#[tauri::command]
async fn search_text(repo: String, query: String, regex: bool, case_sensitive: bool, limit: usize) -> Res<search::TextResult> {
    blocking(move || search::text(&repo, &query, regex, case_sensitive, limit)).await
}

#[tauri::command]
async fn search_text_with(repo: String, query: String, opts: search::Opts, limit: usize) -> Res<search::TextResult> {
    blocking(move || search::text_with(&repo, &query, &opts, limit)).await
}

#[tauri::command]
async fn search_commits(repo: String, query: String, regex: bool, case_sensitive: bool) -> Res<Vec<Commit>> {
    blocking(move || search::commits(&repo, &query, regex, case_sensitive)).await
}

#[tauri::command]
async fn ci_runs(repo: String) -> Res<Vec<ci::Run>> {
    blocking(move || ci::list(&repo)).await
}

#[tauri::command]
async fn ci_detail(repo: String, id: i64) -> Res<Vec<ci::Step>> {
    blocking(move || ci::detail(&repo, id)).await
}

#[tauri::command]
async fn list_branches(repo: String) -> Res<branches::BranchList> {
    blocking(move || branches::list(&repo)).await
}

#[tauri::command]
async fn switch_branch(repo: String, name: String, remote: bool) -> Res<String> {
    blocking(move || branches::switch(&repo, &name, remote)).await
}

#[tauri::command]
async fn create_branch(repo: String, name: String, from: Option<String>) -> Res<String> {
    blocking(move || branches::create(&repo, &name, from.as_deref())).await
}

#[tauri::command]
async fn delete_branch(repo: String, name: String) -> Res<String> {
    blocking(move || branches::delete(&repo, &name)).await
}

#[tauri::command]
fn suggest_worktree_path(repo: String, branch: String) -> String {
    branches::suggest_path(&repo, &branch)
}

#[tauri::command]
async fn worktree_add(repo: String, path: String, branch: String, new_branch: bool, base: Option<String>) -> Res<String> {
    blocking(move || branches::worktree_add(&repo, &path, &branch, new_branch, base.as_deref())).await
}

#[tauri::command]
async fn worktree_remove(repo: String, path: String, force: bool) -> Res<String> {
    blocking(move || branches::worktree_remove(&repo, &path, force)).await
}

#[tauri::command]
async fn worktree_prune(repo: String) -> Res<String> {
    blocking(move || branches::worktree_prune(&repo)).await
}

#[tauri::command]
async fn base_candidates(repo: String) -> Res<Vec<String>> {
    blocking(move || Ok(git::base_candidates(&repo))).await
}

#[tauri::command]
async fn branch_review(repo: String, base: Option<String>) -> Res<git::BranchReview> {
    blocking(move || git::branch_review(&repo, base.as_deref())).await
}

#[tauri::command]
async fn base_diff(repo: String, base: String, rel: String, old_rel: Option<String>) -> Res<FileDiff> {
    blocking(move || git::base_diff(&repo, &base, &rel, old_rel.as_deref())).await
}

#[tauri::command]
async fn base_patch(repo: String, base: String, paths: Vec<String>, untracked: Vec<String>) -> Res<String> {
    blocking(move || git::base_patch(&repo, &base, &paths, &untracked)).await
}

#[tauri::command]
async fn list_issues(repo: String, state: String) -> Res<Vec<issues::Issue>> {
    blocking(move || issues::list(&repo, &state)).await
}

#[tauri::command]
async fn issue_detail(repo: String, number: i64) -> Res<issues::IssueDetail> {
    blocking(move || issues::detail(&repo, number)).await
}

#[tauri::command]
async fn azdo_projects(org: String) -> Res<Vec<String>> {
    blocking(move || clone::azdo_projects(&org)).await
}

#[tauri::command]
async fn azdo_repos(org: String, project: String) -> Res<Vec<clone::RemoteRepo>> {
    blocking(move || clone::azdo_repos(&org, &project)).await
}

#[tauri::command]
async fn github_repos(owner: String) -> Res<Vec<clone::RemoteRepo>> {
    blocking(move || clone::github_repos(&owner)).await
}

/// Starts `git clone` as a task, so its progress shows in the panel.
#[tauri::command]
fn clone_repo(app: tauri::AppHandle, state: State<AppState>, url: String, parent: String, name: String) -> Res<RunInfo> {
    let command = clone::command(&url, &parent, &name)?;
    let (out_app, exit_app) = (app.clone(), app);
    state.runner.start(
        Arc::new(move |id, data| {
            let _ = out_app.emit("task-output", OutputEvent { id, data });
        }),
        Arc::new(move |info| {
            let _ = exit_app.emit("task-exit", info);
        }),
        &parent,
        &format!("clone:{name}"),
        &format!("clone {name}"),
        &command,
        "",
    )
}

#[tauri::command]
async fn worktree_patch(repo: String, paths: Vec<String>, untracked: Vec<String>) -> Res<String> {
    blocking(move || git::worktree_patch(&repo, &paths, &untracked)).await
}

#[tauri::command]
async fn rev_patch(repo: String, base: String, head: String, paths: Vec<String>) -> Res<String> {
    blocking(move || git::rev_patch(&repo, &base, &head, &paths)).await
}

#[tauri::command]
async fn git_action(repo: String, action: String) -> Res<String> {
    blocking(move || git::action(&repo, &action)).await
}

#[tauri::command]
async fn pending(repo: String, incoming: bool) -> Res<Vec<Commit>> {
    blocking(move || git::pending(&repo, incoming)).await
}

#[tauri::command]
async fn stage(repo: String, paths: Vec<String>) -> Res<()> {
    blocking(move || git::stage(&repo, &paths)).await
}

#[tauri::command]
async fn unstage(repo: String, paths: Vec<String>) -> Res<()> {
    blocking(move || git::unstage(&repo, &paths)).await
}

#[tauri::command]
async fn file_hunks(repo: String, rel: String) -> Res<hunks::FileHunks> {
    blocking(move || hunks::file_hunks(&repo, &rel)).await
}

#[tauri::command]
async fn apply_hunk(repo: String, patch: String, mode: String) -> Res<String> {
    blocking(move || hunks::apply(&repo, &patch, &mode)).await
}

#[tauri::command]
async fn pr_draft(repo: String) -> Res<prs::PrDraft> {
    blocking(move || prs::draft(&repo)).await
}

#[tauri::command]
async fn pr_create(repo: String, title: String, body: String, base: String, draft: bool) -> Res<String> {
    blocking(move || prs::create(&repo, &title, &body, &base, draft)).await
}

#[tauri::command]
async fn stash_list(repo: String) -> Res<Vec<stash::Stash>> {
    blocking(move || stash::list(&repo)).await
}

#[tauri::command]
async fn stash_push(repo: String, message: String, paths: Vec<String>) -> Res<String> {
    blocking(move || stash::push(&repo, &message, &paths)).await
}

#[tauri::command]
async fn stash_apply(repo: String, sha: String, pop: bool) -> Res<String> {
    blocking(move || stash::apply(&repo, &sha, pop)).await
}

#[tauri::command]
async fn stash_drop(repo: String, sha: String) -> Res<String> {
    blocking(move || stash::drop(&repo, &sha)).await
}

#[tauri::command]
async fn stash_files(repo: String, sha: String) -> Res<stash::StashFiles> {
    blocking(move || stash::files(&repo, &sha)).await
}

#[tauri::command]
async fn discard(repo: String, tracked: Vec<String>, untracked: Vec<String>) -> Res<String> {
    blocking(move || stash::discard(&repo, &tracked, &untracked)).await
}

#[tauri::command]
async fn undo_discard(repo: String, backup: String, tracked: Vec<String>) -> Res<()> {
    blocking(move || stash::undo_discard(&repo, &backup, &tracked)).await
}

#[tauri::command]
async fn undo_commit(repo: String) -> Res<String> {
    blocking(move || stash::undo_commit(&repo)).await
}

#[tauri::command]
async fn commit(repo: String, message: String, stage_all: bool, push: bool, amend: Option<bool>) -> Res<String> {
    blocking(move || git::commit(&repo, &message, stage_all, push, amend.unwrap_or(false))).await
}

#[tauri::command]
async fn list_prs(repo: String) -> Res<Vec<PullRequest>> {
    blocking(move || prs::list(&repo)).await
}

#[tauri::command]
async fn prepare_pr(repo: String, pr: PullRequest) -> Res<PrDiff> {
    blocking(move || prs::prepare(&repo, &pr)).await
}

#[tauri::command]
async fn list_tasks(state: State<'_, AppState>, repo: String) -> Res<Vec<Task>> {
    let custom = state.config.lock().unwrap().tasks.get(&repo).cloned().unwrap_or_default();
    blocking(move || Ok(tasks::discover(&repo, &custom))).await
}

#[derive(Serialize, Clone)]
struct OutputEvent<'a> {
    id: u64,
    data: &'a str,
}

#[tauri::command]
fn start_task(app: tauri::AppHandle, state: State<AppState>, repo: String, task: Task) -> Res<RunInfo> {
    let (out_app, exit_app) = (app.clone(), app);
    state.runner.start(
        Arc::new(move |id, data| {
            let _ = out_app.emit("task-output", OutputEvent { id, data });
        }),
        Arc::new(move |info| {
            let _ = exit_app.emit("task-exit", info);
        }),
        &repo,
        &task.id,
        &task.name,
        &task.command,
        &task.cwd,
    )
}

#[tauri::command]
fn stop_run(state: State<AppState>, id: u64) -> Res<()> {
    state.runner.stop(id)
}

#[tauri::command]
fn run_input(state: State<AppState>, id: u64, text: String) -> Res<()> {
    state.runner.input(id, &text)
}

#[tauri::command]
fn run_resize(state: State<AppState>, id: u64, cols: u16, rows: u16) -> Res<()> {
    state.runner.resize(id, cols, rows)
}

#[tauri::command]
async fn tickets_azdo(org: String) -> Res<Vec<tickets::Ticket>> {
    blocking(move || tickets::azdo(&org)).await
}

#[tauri::command]
async fn tickets_github() -> Res<Vec<tickets::Ticket>> {
    blocking(tickets::github).await
}

#[tauri::command]
async fn ticket_azdo_detail(org: String, project: String, id: i64) -> Res<tickets::TicketDetail> {
    blocking(move || tickets::azdo_detail(&org, &project, id)).await
}

#[tauri::command]
async fn ticket_image(url: String) -> Res<String> {
    blocking(move || tickets::image(&url)).await
}

#[tauri::command]
async fn ticket_github_detail(repo: String, number: i64) -> Res<tickets::TicketDetail> {
    blocking(move || tickets::github_detail(&repo, number)).await
}

/// The Windows build number (for example 26200), so the terminal view can match ConPTY. 0 elsewhere.
#[tauri::command]
fn os_build() -> u32 {
    #[cfg(windows)]
    {
        if let Ok(out) = run::command("cmd").args(["/C", "ver"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(v) = text.split("Version ").nth(1) {
                return v.split('.').nth(2).and_then(|b| b.trim_end_matches(']').trim().parse().ok()).unwrap_or(0);
            }
        }
    }
    0
}

#[tauri::command]
fn run_output(state: State<AppState>, id: u64) -> Res<String> {
    state.runner.output(id)
}

#[tauri::command]
fn list_runs(state: State<AppState>) -> Vec<RunInfo> {
    state.runner.list()
}

#[tauri::command]
fn forget_run(state: State<AppState>, id: u64) {
    state.runner.forget(id)
}

#[derive(Serialize, Clone)]
struct PtyExit {
    id: u64,
    code: Option<u32>,
}

#[tauri::command]
fn pty_open(app: tauri::AppHandle, state: State<AppState>, cwd: String, cols: u16, rows: u16) -> Res<pty::Opened> {
    let (data_app, exit_app) = (app.clone(), app);
    state.terminals.open(
        &cwd,
        cols,
        rows,
        Arc::new(move |id, data| {
            let _ = data_app.emit("pty-output", OutputEvent { id, data });
        }),
        Arc::new(move |id, code| {
            let _ = exit_app.emit("pty-exit", PtyExit { id, code });
        }),
    )
}

#[tauri::command]
fn pty_write(state: State<AppState>, id: u64, data: String) -> Res<()> {
    state.terminals.write(id, &data)
}

#[tauri::command]
fn pty_resize(state: State<AppState>, id: u64, cols: u16, rows: u16) -> Res<()> {
    state.terminals.resize(id, cols, rows)
}

#[tauri::command]
fn pty_close(state: State<AppState>, id: u64) {
    state.terminals.close(id)
}

/// Opens the repo (or a file in it) in another program. Uses plain commands so new windows can appear.
#[tauri::command]
fn open_in(repo: String, target: String, rel: Option<String>, line: Option<u32>) -> Res<()> {
    use std::process::Command;
    let file = match &rel {
        Some(r) => Some(git::safe_join(&repo, r)?),
        None => None,
    };
    let spawn = |mut c: Command| c.spawn().map(|_| ()).map_err(|e| e.to_string());
    match target.as_str() {
        #[cfg(target_os = "macos")]
        "terminal" | "agent" => {
            if target == "agent" {
                let script = format!(
                    "tell application \"Terminal\" to do script \"cd '{}' && claude\"",
                    repo.replace('\'', "'\\''")
                );
                let mut c = Command::new("osascript");
                c.args(["-e", &script, "-e", "tell application \"Terminal\" to activate"]);
                return spawn(c);
            }
            let mut c = Command::new("open");
            c.args(["-a", "Terminal"]).arg(&repo);
            spawn(c)
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        "terminal" | "agent" => {
            // Try the common terminals in turn. Each one takes the folder in its own way.
            let agent = target == "agent";
            let tries: [(&str, Vec<String>); 4] = [
                ("x-terminal-emulator", if agent { vec!["-e".into(), "claude".into()] } else { vec![] }),
                ("gnome-terminal", if agent { vec![format!("--working-directory={repo}"), "--".into(), "claude".into()] } else { vec![format!("--working-directory={repo}")] }),
                ("konsole", if agent { vec!["--workdir".into(), repo.clone(), "-e".into(), "claude".into()] } else { vec!["--workdir".into(), repo.clone()] }),
                ("xterm", if agent { vec!["-e".into(), "claude".into()] } else { vec![] }),
            ];
            for (exe, args) in tries {
                let mut c = Command::new(exe);
                c.current_dir(&repo).args(args);
                if spawn(c).is_ok() {
                    return Ok(());
                }
            }
            Err("No terminal app found. Install gnome-terminal, konsole, or xterm.".into())
        }
        #[cfg(windows)]
        "terminal" | "agent" => {
            let mut wt = Command::new("wt");
            wt.args(["-w", "0", "nt", "-d"]).arg(&repo);
            if target == "agent" {
                wt.arg("claude");
            }
            if spawn(wt).is_ok() {
                return Ok(());
            }
            let mut ps = Command::new("cmd");
            ps.current_dir(&repo).args(["/C", "start", "", "powershell", "-NoExit"]);
            if target == "agent" {
                ps.args(["-Command", "claude"]);
            }
            spawn(ps)
        }
        "editor" => {
            let mut c = Command::new(if cfg!(windows) { "code.cmd" } else { "code" });
            c.arg(&repo);
            if let Some(f) = &file {
                let spot = format!("{}:{}", f.to_string_lossy(), line.unwrap_or(1));
                c.arg("-g").arg(spot);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x0800_0000);
            }
            spawn(c)
        }
        "explorer" => {
            #[cfg(windows)]
            let c = {
                let mut c = Command::new("explorer");
                match &file {
                    Some(f) => c.arg(format!("/select,{}", f.to_string_lossy())),
                    None => c.arg(Path::new(&repo)),
                };
                c
            };
            #[cfg(target_os = "macos")]
            let c = {
                let mut c = Command::new("open");
                match &file {
                    Some(f) => c.arg("-R").arg(f),
                    None => c.arg(&repo),
                };
                c
            };
            // Linux file managers cannot all select a file, so open its folder.
            #[cfg(all(unix, not(target_os = "macos")))]
            let c = {
                let mut c = Command::new("xdg-open");
                match file.as_ref().and_then(|f| f.parent()) {
                    Some(dir) => c.arg(dir),
                    None => c.arg(&repo),
                };
                c
            };
            spawn(c)
        }
        _ => Err(format!("unknown target: {target}")),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // A second launch (for example a double-clicked .md file) goes to the open window.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            hand_over(app, file_args(argv.into_iter().skip(1), Path::new(&cwd)));
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let config = config::load(&config_dir);
            let cwd = std::env::current_dir().unwrap_or_default();
            let opened = Mutex::new(file_args(std::env::args().skip(1), &cwd));
            // Tern draws its own title bar. Windows keeps the window shadow and rounded corners;
            // macOS keeps its traffic-light buttons, drawn over Tern's bar.
            if let Some(w) = app.get_webview_window("main") {
                #[cfg(windows)]
                let _ = w.set_shadow(true);
                #[cfg(target_os = "macos")]
                let _ = w.set_decorations(true);
            }
            app.manage(AppState { config_dir, config: Mutex::new(config), runner: Arc::new(Runner::default()), terminals: pty::Terminals::default(), opened });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            config_path,
            list_repos,
            summaries,
            list_files,
            read_file,
            write_file,
            worktree_diff,
            rev_diff,
            commit_files,
            log,
            worktree_patch,
            rev_patch,
            git_action,
            pending,
            stage,
            unstage,
            commit,
            file_hunks,
            apply_hunk,
            pr_draft,
            pr_create,
            stash_list,
            stash_push,
            stash_apply,
            stash_drop,
            stash_files,
            discard,
            undo_discard,
            undo_commit,
            list_prs,
            prepare_pr,
            open_in,
            list_tasks,
            start_task,
            stop_run,
            run_input,
            run_output,
            run_resize,
            tickets_azdo,
            tickets_github,
            ticket_azdo_detail,
            ticket_github_detail,
            ticket_image,
            os_build,
            list_runs,
            forget_run,
            list_issues,
            issue_detail,
            branch_review,
            list_branches,
            ci_runs,
            ci_detail,
            take_opened,
            range_files,
            search_text,
            search_text_with,
            search_commits,
            switch_branch,
            create_branch,
            delete_branch,
            suggest_worktree_path,
            worktree_add,
            worktree_remove,
            worktree_prune,
            base_candidates,
            base_diff,
            base_patch,
            azdo_projects,
            azdo_repos,
            github_repos,
            clone_repo,
            pty_open,
            pty_write,
            pty_resize,
            pty_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // macOS sends opened files as an event, not as arguments.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                let files = urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().to_string()).collect();
                hand_over(app, files);
            }
            // Closing Tern stops every task it started.
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                state.runner.stop_all();
                state.terminals.close_all();
            }
        });
}
