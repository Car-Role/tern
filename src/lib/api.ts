// Typed calls into the Rust back end. Outside Tauri (a plain browser), a mock answers instead,
// so the screen can be designed and tested with `npm run dev`.

import type {
  Config,
  FileDiff,
  FileText,
  GitAction,
  OpenTarget,
  PullRequest,
  RepoEntry,
  RevFiles,
  Summary,
  Commit,
  RunInfo,
  Task,
  Issue,
  IssueDetail,
  RemoteRepo,
  BranchReview,
  BranchList,
  TextResult,
  CiRun,
  CiStep,
  History,
  Ticket,
  TicketDetail,
  Stash,
  StashFiles,
  FileHunks,
  PrDraft,
  FileBytes,
  BigText,
  Tail,
} from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const { mock } = await import("./mock");
  return mock(cmd, args) as Promise<T>;
}

export const api = {
  getConfig: () => call<Config>("get_config"),
  setConfig: (config: Config) => call<void>("set_config", { config }),
  configPath: () => call<string>("config_path"),
  listRepos: () => call<RepoEntry[]>("list_repos"),
  summaries: (paths: string[]) => call<Summary[]>("summaries", { paths }),
  listFiles: (repo: string) => call<string[]>("list_files", { repo }),
  readFile: (repo: string, rel: string) => call<FileText>("read_file", { repo, rel }),
  writeFile: (repo: string, rel: string, text: string) => call<void>("write_file", { repo, rel, text }),
  worktreeDiff: (repo: string, rel: string, oldRel: string | null) =>
    call<FileDiff>("worktree_diff", { repo, rel, oldRel }),
  revDiff: (repo: string, base: string, head: string, rel: string, oldRel: string | null) =>
    call<FileDiff>("rev_diff", { repo, base, head, rel, oldRel }),
  commitFiles: (repo: string, sha: string) => call<RevFiles>("commit_files", { repo, sha }),
  log: (repo: string, count = 300, all = false) => call<History>("log", { repo, count, all }),
  rangeFiles: (repo: string, base: string, head: string) => call<RevFiles>("range_files", { repo, base, head }),
  searchText: (repo: string, query: string, regex: boolean, caseSensitive: boolean, limit = 60) =>
    call<TextResult>("search_text", { repo, query, regex, caseSensitive, limit }),
  searchTextWith: (
    repo: string,
    query: string,
    opts: { regex?: boolean; caseSensitive?: boolean; word?: boolean; perl?: boolean; paths?: string[] },
    limit = 60,
  ) => call<TextResult>("search_text_with", { repo, query, opts, limit }),
  searchCommits: (repo: string, query: string, regex: boolean, caseSensitive: boolean) =>
    call<Commit[]>("search_commits", { repo, query, regex, caseSensitive }),
  takeOpened: () => call<string[]>("take_opened"),
  ticketsAzdo: (org: string) => call<Ticket[]>("tickets_azdo", { org }),
  ticketsGithub: () => call<Ticket[]>("tickets_github"),
  ticketAzdoDetail: (org: string, project: string, id: number) => call<TicketDetail>("ticket_azdo_detail", { org, project, id }),
  /** An image from a ticket, downloaded with your az or gh sign-in, as a data: URL. */
  ticketImage: (url: string) => call<string>("ticket_image", { url }),
  ticketGithubDetail: (repo: string, number: number) => call<TicketDetail>("ticket_github_detail", { repo, number }),
  ciRuns: (repo: string) => call<CiRun[]>("ci_runs", { repo }),
  ciDetail: (repo: string, id: number) => call<CiStep[]>("ci_detail", { repo, id }),
  listBranches: (repo: string) => call<BranchList>("list_branches", { repo }),
  switchBranch: (repo: string, name: string, remote: boolean) => call<string>("switch_branch", { repo, name, remote }),
  createBranch: (repo: string, name: string, from: string | null) => call<string>("create_branch", { repo, name, from }),
  deleteBranch: (repo: string, name: string) => call<string>("delete_branch", { repo, name }),
  suggestWorktreePath: (repo: string, branch: string) => call<string>("suggest_worktree_path", { repo, branch }),
  worktreeAdd: (repo: string, path: string, branch: string, newBranch: boolean, base: string | null) =>
    call<string>("worktree_add", { repo, path, branch, newBranch, base }),
  worktreeRemove: (repo: string, path: string, force: boolean) => call<string>("worktree_remove", { repo, path, force }),
  worktreePrune: (repo: string) => call<string>("worktree_prune", { repo }),
  baseCandidates: (repo: string) => call<string[]>("base_candidates", { repo }),
  branchReview: (repo: string, base: string | null) => call<BranchReview>("branch_review", { repo, base }),
  baseDiff: (repo: string, base: string, rel: string, oldRel: string | null) => call<FileDiff>("base_diff", { repo, base, rel, oldRel }),
  basePatch: (repo: string, base: string, paths: string[], untracked: string[]) => call<string>("base_patch", { repo, base, paths, untracked }),
  listIssues: (repo: string, state: string) => call<Issue[]>("list_issues", { repo, state }),
  issueDetail: (repo: string, number: number) => call<IssueDetail>("issue_detail", { repo, number }),
  azdoProjects: (org: string) => call<string[]>("azdo_projects", { org }),
  azdoRepos: (org: string, project: string) => call<RemoteRepo[]>("azdo_repos", { org, project }),
  githubRepos: (owner: string) => call<RemoteRepo[]>("github_repos", { owner }),
  cloneRepo: (url: string, parent: string, name: string) => call<RunInfo>("clone_repo", { url, parent, name }),
  worktreePatch: (repo: string, paths: string[], untracked: string[]) =>
    call<string>("worktree_patch", { repo, paths, untracked }),
  revPatch: (repo: string, base: string, head: string, paths: string[]) =>
    call<string>("rev_patch", { repo, base, head, paths }),
  gitAction: (repo: string, action: GitAction) => call<string>("git_action", { repo, action }),
  pending: (repo: string, incoming: boolean) => call<Commit[]>("pending", { repo, incoming }),
  stage: (repo: string, paths: string[]) => call<void>("stage", { repo, paths }),
  stashList: (repo: string) => call<Stash[]>("stash_list", { repo }),
  stashPush: (repo: string, message: string, paths: string[]) => call<string>("stash_push", { repo, message, paths }),
  stashApply: (repo: string, sha: string, pop: boolean) => call<string>("stash_apply", { repo, sha, pop }),
  stashDrop: (repo: string, sha: string) => call<string>("stash_drop", { repo, sha }),
  stashFiles: (repo: string, sha: string) => call<StashFiles>("stash_files", { repo, sha }),
  /** Returns a backup commit of the changed files, for undo. */
  discard: (repo: string, tracked: string[], untracked: string[]) => call<string>("discard", { repo, tracked, untracked }),
  undoDiscard: (repo: string, backup: string, tracked: string[]) => call<void>("undo_discard", { repo, backup, tracked }),
  undoCommit: (repo: string) => call<string>("undo_commit", { repo }),
  unstage: (repo: string, paths: string[]) => call<void>("unstage", { repo, paths }),
  /** A file as a data URL: from disk (rev null) or from a commit. For images and PDFs. */
  fileBytes: (repo: string, rel: string, rev: string | null = null) => call<FileBytes>("file_bytes", { repo, rel, rev }),
  /** Text bigger than the code view takes, for tables and trees. */
  readText: (repo: string, rel: string, max: number) => call<BigText>("read_text", { repo, rel, max }),
  /** The new end of a growing file. Offset 0 starts over with the last `max` bytes. */
  readTail: (repo: string, rel: string, offset: number, max: number) => call<Tail>("read_tail", { repo, rel, offset, max }),
  fileHunks: (repo: string, rel: string) => call<FileHunks>("file_hunks", { repo, rel }),
  /** mode: stage | unstage | discard. Discard returns a backup commit, for undo. */
  applyHunk: (repo: string, patch: string, mode: "stage" | "unstage" | "discard") => call<string>("apply_hunk", { repo, patch, mode }),
  prDraft: (repo: string) => call<PrDraft>("pr_draft", { repo }),
  prCreate: (repo: string, title: string, body: string, base: string, draft: boolean) => call<string>("pr_create", { repo, title, body, base, draft }),
  commit: (repo: string, message: string, stageAll: boolean, push: boolean, amend = false) =>
    call<string>("commit", { repo, message, stageAll, push, amend }),
  listPrs: (repo: string) => call<PullRequest[]>("list_prs", { repo }),
  preparePr: (repo: string, pr: PullRequest) => call<RevFiles>("prepare_pr", { repo, pr }),
  listTasks: (repo: string) => call<Task[]>("list_tasks", { repo }),
  startTask: (repo: string, task: Task) => call<RunInfo>("start_task", { repo, task }),
  stopRun: (id: number) => call<void>("stop_run", { id }),
  runInput: (id: number, text: string) => call<void>("run_input", { id, text }),
  runResize: (id: number, cols: number, rows: number) => call<void>("run_resize", { id, cols, rows }),
  osBuild: () => call<number>("os_build"),
  runOutput: (id: number) => call<string>("run_output", { id }),
  listRuns: () => call<RunInfo[]>("list_runs"),
  forgetRun: (id: number) => call<void>("forget_run", { id }),
  ptyOpen: (cwd: string, cols: number, rows: number) => call<{ id: number; shell: string }>("pty_open", { cwd, cols, rows }),
  ptyWrite: (id: number, data: string) => call<void>("pty_write", { id, data }),
  ptyResize: (id: number, cols: number, rows: number) => call<void>("pty_resize", { id, cols, rows }),
  ptyClose: (id: number) => call<void>("pty_close", { id }),
  openIn: (repo: string, target: OpenTarget, rel?: string, line?: number) =>
    call<void>("open_in", { repo, target, rel: rel ?? null, line: line ?? null }),
};

/** Calls back for task output and task exits. Returns a function that stops listening. */
/** Calls back when Windows or macOS asks Tern to open files (a second launch). */
export async function onOpenFiles(run: () => void) {
  if (!inTauri) return;
  const { listen } = await import("@tauri-apps/api/event");
  await listen("open-files", run);
}

export async function onTaskEvents(
  onOutput: (id: number, data: string) => void,
  onExit: (info: RunInfo) => void,
): Promise<() => void> {
  if (!inTauri) {
    const { mockTaskEvents } = await import("./mock");
    return mockTaskEvents(onOutput, onExit);
  }
  const { listen } = await import("@tauri-apps/api/event");
  const a = await listen<{ id: number; data: string }>("task-output", (e) => onOutput(e.payload.id, e.payload.data));
  const b = await listen<RunInfo>("task-exit", (e) => onExit(e.payload));
  return () => {
    a();
    b();
  };
}

export async function copyText(text: string): Promise<void> {
  if (inTauri) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  } else {
    await navigator.clipboard.writeText(text);
  }
}

/** Tern's version, from the app itself (the browser preview reports "dev"). */
export async function appVersion(): Promise<string> {
  if (!inTauri) return "dev";
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

export type UpdateInfo = { version: string; notes: string; date: string | null; install: (onProgress: (done: number, total: number | null) => void) => Promise<void> };

/** Asks GitHub Releases for a newer signed version. Null when Tern is up to date. */
export async function findUpdate(): Promise<UpdateInfo | null> {
  if (!inTauri) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  const u = await check();
  if (!u) return null;
  return {
    version: u.version,
    notes: u.body ?? "",
    date: u.date ?? null,
    install: async (onProgress) => {
      let total: number | null = null;
      let done = 0;
      await u.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? null;
        else if (e.event === "Progress") onProgress((done += e.data.chunkLength), total);
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}

export async function readClipboard(): Promise<string> {
  if (inTauri) {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return (await readText()) ?? "";
  }
  return navigator.clipboard.readText();
}

export async function openUrl(url: string): Promise<void> {
  if (inTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank");
  }
}
