// All app state and actions. Components read from `app` and call its methods.

import { api, appVersion, copyText, findUpdate, onTaskEvents, openUrl, onOpenFiles, type UpdateInfo } from "./api";
import { formatPaths, type CopyFormat, type LineRange, ago, baseName, duration } from "./format";
import { Term } from "./term";
import { closeShell, disposeRun, feedRun, focusShell, isInsideTerminal, openShell, retheme, runText, writeToShell } from "./shells";

/** One place you have been: project, tab, open file or diff, and review. Back and forward move between them. */
type Nav = { current: string | null; tab: TabId; view: View | null; review: Review | null; tickets: boolean; data: boolean };

/** A built-in terminal tab in the bottom panel. */
export type ShellTab = { id: number; repo: string; title: string; alive: boolean; created: number };
import type {
  DbConn,
  DbSchema,
  Change,
  ChangeKind,
  Commit,
  Config,
  GitAction,
  OpenTarget,
  PullRequest,
  RepoEntry,
  Issue,
  RunInfo,
  CiRun,
  Stash,
  Hunk,
  FileHunks,
  PrDraft,
  History,
  Ticket,
  TicketDetail,
  Space,
  Summary,
  TabId,
  Task,
} from "./types";
import type { HoverData } from "./hover";

/** A right-click menu item. "sep" draws a line. */
export type MenuItem = { label: string; hint?: string; run: () => void; danger?: boolean; disabled?: boolean } | "sep";

export type Tab = TabId;

/** Every tab, in its default order. A space can show fewer. */
export const ALL_TABS: TabId[] = ["changes", "files", "prs", "issues", "history", "ci", "tasks"];

/** A list of files from a commit or pull request, shown in place of the working-tree changes. */
export type Review = {
  /** "branch": everything a branch or worktree changed since it split from its base, including uncommitted work. */
  kind: "commit" | "pr" | "branch" | "stash";
  commits?: Commit[];
  baseRef?: string;
  candidates?: string[];
  title: string;
  subtitle: string;
  base: string;
  head: string;
  files: Change[];
  pr?: PullRequest;
  /** The stash being looked at (kind "stash"). */
  stash?: Stash;
};

export type View =
  | {
      kind: "file";
      repo: string;
      rel: string;
      line?: number;
      /** What to select on that line, for example the search words. */
      find?: { text: string; regex: boolean; matchCase: boolean };
      /** New for every jump, so picking another match in the same file still scrolls there. */
      jump?: number;
      /** A file outside every project (opened from Windows). `repo` is then its folder. */
      loose?: boolean;
    }
  | {
      kind: "diff";
      repo: string;
      rel: string;
      oldRel: string | null;
      changeKind: ChangeKind;
      rev: { base: string; head: string } | null;
    }
  | { kind: "issue"; repo: string; number: number; rel: "" }
  | { kind: "run"; repo: string; run: CiRun; rel: "" };

export type Overlay = null | "palette" | "copy" | "help" | "settings" | "spaces" | "move" | "confirm" | "clone" | "branches" | "search" | "newpr";

/** What the confirm dialog shows. `resolve` is called once with the answer. */
export type ConfirmRequest = {
  title: string;
  body: string;
  /** A command line, shown in code style. */
  code?: string;
  commits?: Commit[];
  commitsLabel?: string;
  /** One line per project, for actions on many projects. */
  list?: { name: string; detail: string; tone?: "warn" | "skip" }[];
  listLabel?: string;
  warnings?: string[];
  confirm: string;
  /** Destructive (discard, drop, delete, remove): the main button is red. */
  danger?: boolean;
  /** False when the action cannot work (for example, nothing to push). */
  allowed?: boolean;
  resolve: (ok: boolean) => void;
};

/** Tasks whose name or command matches this change things outside your machine, so they ask first. */
const RISKY_TASK = /deploy|publish|release|destroy|teardown|\bprod\b|[:_-]prod|prod[:_-]|\bdrop\b|db:reset|migrate|compose down/i;

/** Pushing straight to one of these gets an extra warning. */
const MAIN_BRANCHES = ["main", "master", "develop", "dev", "release"];

/** A repo counts as "working" while its files changed less than this many seconds ago. */
export const WORKING_SECONDS = 120;
export type PaletteMode = "projects" | "files" | "commands" | "tasks";

type PrState = { loading: boolean; error: string | null; items: PullRequest[] };

const saved = (key: string) => {
  try {
    return localStorage.getItem("tern." + key);
  } catch {
    return null;
  }
};
const save = (key: string, value: string) => {
  try {
    localStorage.setItem("tern." + key, value);
  } catch {
    /* storage can be blocked; the app works without it */
  }
};

const loadJson = <T>(key: string, fallback: T): T => {
  try {
    return JSON.parse(saved(key) ?? "") as T;
  } catch {
    return fallback;
  }
};

const nowSeconds = () => Math.floor(Date.now() / 1000);
const slash = (p: string) => p.replace(/\\/g, "/").toLowerCase();

const firstLine = (s: string) => String(s).split("\n").find((l) => l.trim()) ?? String(s);

class Store {
  config = $state<Config | null>(null);
  repos = $state<RepoEntry[]>([]);
  sums = $state<Record<string, Summary>>({});
  loadingRepos = $state(true);

  current = $state<string | null>(null);
  tab = $state<Tab>((saved("tab") as Tab) || "changes");
  files = $state<Record<string, string[]>>({});
  prs = $state<Record<string, PrState>>({});
  logs = $state<Record<string, Commit[]>>({});
  /** How each repo's branch stands against its remote branch, from the last history load. */
  logSync = $state<Record<string, Omit<History, "commits">>>({});

  review = $state<Review | null>(null);
  reviewLoading = $state<string | null>(null);
  view = $state<View | null>(null);
  selected = $state<string[]>([]);
  lines = $state<LineRange | null>(null);
  diffMode = $state<"split" | "unified">((saved("diffMode") as "split" | "unified") || "unified");
  editing = $state(false);
  /** Goes up when files on disk may have changed (window focus, F5). The viewer reloads on change. */
  stamp = $state(0);

  busy = $state<Record<string, GitAction>>({});
  toast = $state<{ text: string; kind: "ok" | "err" | "info"; id: number } | null>(null);
  /** Every message Tern showed this session, newest first, for the notification list. */
  notices = $state<{ id: number; text: string; kind: "ok" | "err" | "info"; at: number; repo: string | null }[]>([]);
  /** When the notification list was last opened. Newer messages count as unread. */
  noticesSeen = $state(Date.now());
  noticesOpen = $state(false);
  /** An open overlay can return true to stop Esc from closing it (for example, to ask about unsaved changes). */
  overlayGuard: (() => boolean) | null = null;
  /** The sidebar is folded away (Ctrl \). */
  sidebarCollapsed = $state(saved("sidebar") === "0");
  overlay = $state<Overlay>(null);
  paletteMode = $state<PaletteMode>("projects");
  lastCopy = $state<CopyFormat>((saved("lastCopy") as CopyFormat) || "relative");

  /** Ticks every few seconds so "working" and "5m ago" stay current. */
  now = $state(nowSeconds());
  /** Repo path to the time you last looked at it. Newer activity counts as unseen. */
  lastSeen = $state<Record<string, number>>(loadJson("lastSeen", {}));
  private startedAt = nowSeconds();
  private wasWorking = new Set<string>();

  confirmReq = $state<ConfirmRequest | null>(null);
  /** The right-click menu on screen. */
  menu = $state<{ x: number; y: number; title: string; items: MenuItem[] } | null>(null);
  /** The hover card on screen, and the element it belongs to. */
  hoverCard = $state<{ rect: { left: number; top: number; right: number; bottom: number }; data: HoverData } | null>(null);
  private hoverOwner: HTMLElement | null = null;
  logAll = $state(saved("logAll") === "1");
  issues = $state<Record<string, { loading: boolean; error: string | null; items: Issue[] }>>({});
  ci = $state<Record<string, { loading: boolean; error: string | null; items: CiRun[]; at: number }>>({});
  /** Stashes per repo, and the branch-files stamp they were read at. */
  stashes = $state<Record<string, Stash[]>>({});
  private stashStamp: Record<string, number | null | undefined> = {};
  version = $state("");
  /** The hunk panel beside a working-tree diff. */
  hunksOpen = $state(saved("hunks") === "1");
  hunks = $state<{ repo: string; rel: string; loading: boolean; error: string | null; untracked: boolean; data: FileHunks | null } | null>(null);
  hunkBusy = $state<string | null>(null);
  /** Asks the diff view to scroll to a line of the new file. */
  diffJump = $state<{ line: number; at: number } | null>(null);
  /** Amend the last commit instead of making a new one, per repo. */
  amend = $state<Record<string, boolean>>({});
  /** The new pull request dialog. */
  prDraft = $state<{ loading: boolean; error: string | null; data: PrDraft | null } | null>(null);
  prCreating = $state(false);
  /** A newer version found on GitHub, and how its install is going. */
  update = $state<{ info: UpdateInfo; state: "ready" | "downloading" | "failed"; progress: number } | null>(null);
  updateCheck = $state<{ state: "idle" | "checking" | "current" | "failed"; at: number; error?: string }>({ state: "idle", at: 0 });
  /** The last discard, so it can be undone for a few minutes. */
  lastDiscard = $state<{ repo: string; backup: string; tracked: string[]; untracked: number; count: number; at: number } | null>(null);
  issueState = $state<"open" | "closed" | "all">((saved("issueState") as "open" | "closed" | "all") || "open");
  /** The repos that "Move to a space" acts on. Empty means the open project. */
  moveTargets = $state<string[]>([]);
  theme = $state(saved("theme") || "system");
  /** How the search window should open next time: scope, words, or a definition lookup. */
  searchPreset = $state<{ query?: string; scope?: string; word?: boolean; kind?: string; defsFor?: string } | null>(null);
  /** The word at the cursor in the code viewer. The viewer sets this. */
  wordAtCursor: () => string = () => "";
  /** Which tab the branch picker opens on. */
  branchTab = $state<"branches" | "worktrees">("branches");
  private navBack: Nav[] = [];
  private navFwd: Nav[] = [];
  private restoring = false;
  canBack = $state(false);
  canForward = $state(false);

  /** Repos picked with Ctrl+click or Shift+click in the project list. */
  repoSel = $state<string[]>([]);
  private repoAnchor: string | null = null;
  /** The branch stamp each history was loaded at. A new stamp means new commits. */
  private logStamp: Record<string, string> = {};
  /** The base each repo's branch review compares with, when you picked one. */
  private reviewBase: Record<string, string> = loadJson("reviewBase", {});
  private branchSig = "";
  /** When each pull request and issue list was loaded (ms). */
  private listAt: Record<string, number> = {};
  showHidden = $state(saved("showHidden") === "1");

  tasks = $state<Record<string, Task[] | undefined>>({});
  runs = $state<RunInfo[]>([]);
  activeRun = $state<number | null>(null);
  shells = $state<ShellTab[]>([]);
  /** The tab on screen in the bottom panel: "run:<id>" or "shell:<id>". */
  activeTab = $state<string | null>(null);
  panelOpen = $state(saved("panelOpen") === "1");
  panelHeight = $state(Number(saved("panelHeight")) || 280);
  /** Goes up (at most once a frame) when any task output changes. The panel redraws on it. */
  termTick = $state(0);
  private terms = new Map<number, Term>();
  private tickQueued = false;

  commitMsg = $state<Record<string, string>>({});
  pushAfterCommit = $state(saved("pushAfterCommit") === "1");
  committing = $state(false);
  sortRecent = $state(saved("sortRecent") === "1");

  spaces = $derived<Space[]>(this.config?.spaces ?? []);
  activeSpace = $derived(this.spaces.find((x) => x.id === this.config?.activeSpace) ?? null);

  /** Which space a repo belongs to: a manual choice, then its main repo, then the rules, then the default. */
  spaceOf(repo: RepoEntry): string | null {
    const cfg = this.config;
    if (!cfg) return null;
    const manual = cfg.assign[repo.path];
    if (manual && cfg.spaces.some((x) => x.id === manual)) return manual;
    if (repo.parent) {
      const main = this.repos.find((r) => r.path === repo.parent);
      if (main) return this.spaceOf(main);
    }
    const path = slash(repo.path);
    const remote = (this.sums[repo.path]?.remote ?? "").toLowerCase();
    // Folder rules win over remote rules, and the longest matching folder wins.
    let best: { id: string; len: number } | null = null;
    for (const sp of cfg.spaces) {
      for (const f of sp.folders) {
        const dir = slash(f.trim()).replace(/\/$/, "");
        if (dir && (path + "/").startsWith(dir + "/") && (!best || dir.length > best.len)) best = { id: sp.id, len: dir.length };
      }
    }
    if (best) return best.id;
    for (const sp of cfg.spaces) {
      if (remote && sp.remotes.some((r) => r.trim() && remote.includes(r.trim().toLowerCase()))) return sp.id;
    }
    return cfg.defaultSpace && cfg.spaces.some((x) => x.id === cfg.defaultSpace) ? cfg.defaultSpace : null;
  }

  private hiddenKeys = $derived(new Set((this.config?.hidden ?? []).map(slash)));

  /** Hidden by you. A worktree is hidden when its main repo is. */
  isHidden(r: RepoEntry) {
    return this.hiddenKeys.has(slash(r.path)) || (!!r.parent && this.hiddenKeys.has(slash(r.parent)));
  }

  /** Repos in the space on screen, without hidden ones unless "show hidden" is on. */
  visible = $derived.by(() => {
    const id = this.config?.activeSpace ?? null;
    return this.repos.filter((r) => (!id || this.spaceOf(r) === id) && (this.showHidden || !this.isHidden(r)));
  });

  /** How many repos in this space are hidden. */
  hiddenCount = $derived.by(() => {
    const id = this.config?.activeSpace ?? null;
    return this.repos.filter((r) => !r.parent && (!id || this.spaceOf(r) === id) && this.isHidden(r)).length;
  });

  /** Main repos first (pinned, then your dragged order, then by name). Each worktree follows its main repo. */
  ordered = $derived.by(() => this.sortRepos(this.visible));

  /** The rail and overview show these groups. With one space on screen there is one group. */
  groups = $derived.by(() => {
    if (this.activeSpace) return [{ space: this.activeSpace as Space | null, repos: this.ordered }];
    const out: { space: Space | null; repos: RepoEntry[] }[] = [];
    for (const sp of this.spaces) {
      const repos = this.ordered.filter((r) => this.spaceOf(r) === sp.id);
      if (repos.length) out.push({ space: sp, repos });
    }
    const rest = this.ordered.filter((r) => this.spaceOf(r) === null);
    if (rest.length) out.push({ space: null, repos: rest });
    return out;
  });

  private sortRepos(list: RepoEntry[]) {
    const byName = (a: RepoEntry, b: RepoEntry) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    const mains = list.filter((r) => !r.parent || !list.some((m) => m.path === r.parent));
    const at = new Map((this.config?.order ?? []).map((p, i) => [slash(p), i]));
    const rank = (r: RepoEntry) => at.get(slash(r.path)) ?? Infinity;
    mains.sort((a, b) => Number(b.pinned) - Number(a.pinned) || rank(a) - rank(b) || byName(a, b));
    const out: RepoEntry[] = [];
    for (const m of mains) {
      out.push(m);
      out.push(...list.filter((r) => r.parent === m.path).sort(byName));
    }
    return out;
  }

  /** Folder shown next to a repo name when two repos share that name. */
  nameHint = $derived.by(() => {
    const count = new Map<string, number>();
    for (const r of this.repos) if (!r.parent) count.set(r.name.toLowerCase(), (count.get(r.name.toLowerCase()) ?? 0) + 1);
    const out: Record<string, string> = {};
    for (const r of this.repos) {
      if (r.parent || (count.get(r.name.toLowerCase()) ?? 0) < 2) continue;
      const parts = r.path.split(/[\\/]/);
      out[r.path] = parts[parts.length - 2] ?? "";
    }
    return out;
  });

  /** Main repos whose worktree list is open in the rail. */
  openTrees = $state<string[]>(loadJson("openTrees", []));

  toggleTrees(path: string) {
    this.openTrees = this.openTrees.includes(path) ? this.openTrees.filter((p) => p !== path) : [...this.openTrees, path];
    save("openTrees", JSON.stringify(this.openTrees));
  }

  /** A folded worktree still shows while it needs attention. */
  showsFolded(r: RepoEntry) {
    return r.path === this.current || this.isWorking(r.path) || this.isUnseen(r.path);
  }

  /** Short text for a repo whose status could not be read. */
  errorText(e: string) {
    if (/not a git repository/i.test(e)) return "Broken worktree: its git folder is missing.";
    return firstLine(e).replace(/^fatal:\s*/i, "");
  }

  /** The tabs a repo shows, from its space. */
  tabsFor(path: string | null): TabId[] {
    const repo = path ? this.repos.find((r) => r.path === path) : undefined;
    const id = repo ? this.spaceOf(repo) : null;
    const tabs = this.spaces.find((x) => x.id === id)?.tabs ?? [];
    const known = tabs.filter((t) => ALL_TABS.includes(t));
    return known.length ? known : ALL_TABS;
  }

  visibleTabs = $derived(this.tabsFor(this.current));

  isWorking(path: string) {
    const a = this.sums[path]?.activity;
    return !!a && this.now - a < WORKING_SECONDS;
  }

  isUnseen(path: string) {
    const a = this.sums[path]?.activity;
    return !!a && a > (this.lastSeen[path] ?? this.startedAt) && path !== this.current;
  }

  repo = $derived(this.repos.find((r) => r.path === this.current));
  summary = $derived(this.current ? this.sums[this.current] : undefined);

  /** Files shown in the middle list for the Changes tab: review files or working-tree changes. */
  changeList = $derived(this.review ? this.review.files : (this.summary?.changes ?? []));

  private toastTimer: ReturnType<typeof setTimeout> | undefined;
  private pollTimer: ReturnType<typeof setInterval> | undefined;

  async init() {
    this.catchErrors();
    try {
      this.config = await api.getConfig();
      await this.reloadRepos(true);
      const last = saved("current");
      if (last && this.repos.some((r) => r.path === last)) this.select(last);
    } catch (e) {
      this.notify(`Could not load projects: ${firstLine(String(e))}`, "err");
    } finally {
      this.loadingRepos = false;
    }
    this.startPolling();
    window.addEventListener("focus", () => {
      this.stamp++;
      this.refresh();
    });
    setInterval(() => {
      this.now = nowSeconds();
      this.checkQuiet();
    }, 5000);
    await this.connectTasks();
    // Ticket counts load in the background once the repos (and their remotes) are known.
    setTimeout(() => this.loadTickets(), 4000);
    appVersion().then((v) => (this.version = v)).catch(() => {});
    // Look for a new version a little after start, then every 6 hours. Development builds only check by hand.
    if (!import.meta.env.DEV) {
      setTimeout(() => this.checkForUpdate(false), 20_000);
      setInterval(() => this.checkForUpdate(false), 6 * 3600_000);
    }
    onOpenFiles(() => this.openWaiting());
    await this.openWaiting();
  }

  // Files opened from Windows ("Open with Tern")

  /** Markdown files show as a page, not as source. */
  mdPreview = $state(saved("mdPreview") !== "0");

  toggleMdPreview() {
    this.mdPreview = !this.mdPreview;
    save("mdPreview", this.mdPreview ? "1" : "0");
  }

  private async openWaiting() {
    try {
      const files = await api.takeOpened();
      if (files.length) await this.openPath(files[files.length - 1]);
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    }
  }

  /** Opens a file by its full path: in its project when Tern knows the project, else on its own. */
  async openPath(full: string) {
    const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
    const want = norm(full);
    const repo = this.repos
      .filter((r) => want.startsWith(norm(r.path) + "/"))
      .sort((a, b) => b.path.length - a.path.length)[0];
    if (repo) {
      if (!this.visible.some((r) => r.path === repo.path)) await this.switchSpace(null);
      this.select(repo.path);
      if (this.tab !== "files" && this.visibleTabs.includes("files")) this.setTab("files");
      this.openFile(full.slice(repo.path.length + 1).replace(/\\/g, "/"));
      return;
    }
    const cut = Math.max(full.lastIndexOf("/"), full.lastIndexOf("\\"));
    this.remember();
    this.editing = false;
    this.lines = null;
    this.view = { kind: "file", repo: full.slice(0, cut), rel: full.slice(cut + 1), loose: true };
  }

  // Tasks

  /** Listens for task output, and picks up tasks that were already running (after a window reload). */
  private async connectTasks() {
    await onTaskEvents(
      (id, data) => {
        this.term(id).write(data);
        feedRun(id, data);
        this.bump();
      },
      (info) => this.onRunExit(info),
    );
    try {
      const existing = await api.listRuns();
      for (const r of existing) {
        const out = await api.runOutput(r.id);
        this.term(r.id).write(out);
        feedRun(r.id, out);
      }
      if (existing.length) {
        this.runs = existing;
        this.activeRun = existing[existing.length - 1].id;
        this.activeTab = "run:" + this.activeRun;
        this.bump();
      }
    } catch {
      /* no runs to restore */
    }
  }

  term(id: number): Term {
    let t = this.terms.get(id);
    if (!t) {
      t = new Term();
      this.terms.set(id, t);
    }
    return t;
  }

  private bump() {
    if (this.tickQueued) return;
    this.tickQueued = true;
    requestAnimationFrame(() => {
      this.tickQueued = false;
      this.termTick++;
    });
  }

  async loadTasks(repo: string, force = false) {
    if (this.tasks[repo] && !force) return this.tasks[repo]!;
    try {
      const list = await api.listTasks(repo);
      this.tasks = { ...this.tasks, [repo]: list };
      return list;
    } catch (e) {
      this.notify(`Could not read tasks: ${firstLine(String(e))}`, "err");
      return [];
    }
  }

  runningIn(repo: string) {
    return this.runs.filter((r) => r.repo === repo && r.running).length;
  }

  lastRun(repo: string, taskId: string) {
    for (let i = this.runs.length - 1; i >= 0; i--) {
      const r = this.runs[i];
      if (r.repo === repo && r.taskId === taskId) return r;
    }
    return undefined;
  }

  async runTask(task: Task, repo = this.current) {
    if (!repo) return this.notify("Open a project first.", "info");
    const live = this.runs.find((r) => r.repo === repo && r.taskId === task.id && r.running);
    if (live) {
      this.showRun(live.id);
      this.notify(`${task.name} is already running. Use Restart to run it again.`, "info");
      return;
    }
    if (RISKY_TASK.test(task.name) || RISKY_TASK.test(task.command)) {
      const ok = await this.ask({
        title: `Run ${task.name} in ${baseName(repo)}?`,
        body: "This task looks like it changes something outside your machine, for example a deploy or a database. Tern runs this command:",
        code: task.cwd ? `${task.command}    (in ${task.cwd})` : task.command,
        confirm: `Run ${task.name}`,
      });
      if (!ok) return;
    }
    try {
      const info = await api.startTask(repo, $state.snapshot(task) as Task);
      this.runs = [...this.runs, info];
      this.showRun(info.id);
    } catch (e) {
      this.notify(`Could not start ${task.name}: ${firstLine(String(e))}`, "err");
    }
  }

  /** Clones a repo into `parent\name`. Progress shows in the panel. When it works, the new repo opens. */
  async cloneRepo(url: string, parent: string, name: string) {
    try {
      const info = await api.cloneRepo(url, parent, name);
      this.runs = [...this.runs, info];
      this.showRun(info.id);
      return true;
    } catch (e) {
      this.notify(`Could not clone: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  runCommand(command: string, repo = this.current) {
    const c = command.trim();
    if (!c) return;
    return this.runTask({ id: "adhoc:" + c, name: c, source: "custom", command: c, cwd: "", detail: c }, repo);
  }

  showRun(id: number) {
    this.activeRun = id;
    this.activeTab = "run:" + id;
    this.setPanel(true);
  }

  // Terminals

  async openTerminal(repo = this.current, opts: { agent?: boolean } = {}) {
    if (!repo) return this.notify("Open a project first.", "info");
    try {
      const { id } = await openShell(repo, (sid) => {
        this.shells = this.shells.map((s) => (s.id === sid ? { ...s, alive: false } : s));
      });
      this.shells = [...this.shells, { id, repo, title: opts.agent ? "Claude Code" : "Terminal", alive: true, created: Date.now() }];
      this.activeTab = "shell:" + id;
      this.setPanel(true);
      // The shell reads this as soon as it is ready.
      if (opts.agent) writeToShell(id, "claude\r").catch(() => {});
    } catch (e) {
      this.notify(`Could not open a terminal: ${firstLine(String(e))}`, "err");
    }
  }

  /** Ctrl+`: show this project's terminal, or make one. Pressed inside the terminal, it hides the panel. */
  toggleTerminal() {
    const repo = this.current;
    if (!repo) return this.notify("Open a project first.", "info");
    const shown = this.shells.find((s) => "shell:" + s.id === this.activeTab);
    if (this.panelOpen && shown?.repo === repo && isInsideTerminal(document.activeElement)) {
      this.setPanel(false);
      return;
    }
    const existing = [...this.shells].reverse().find((s) => s.repo === repo && s.alive);
    if (!existing) return this.openTerminal(repo);
    this.activeTab = "shell:" + existing.id;
    this.setPanel(true);
    setTimeout(() => focusShell(existing.id), 30);
  }

  showShell(id: number) {
    this.activeTab = "shell:" + id;
    this.setPanel(true);
  }

  closeShellTab(id: number) {
    closeShell(id);
    this.shells = this.shells.filter((s) => s.id !== id);
    if (this.activeTab === "shell:" + id) this.pickLastTab();
  }

  private pickLastTab() {
    const tabs = [
      ...this.runs.map((r) => ({ key: "run:" + r.id, at: r.started })),
      ...this.shells.map((s) => ({ key: "shell:" + s.id, at: s.created })),
    ].sort((a, b) => a.at - b.at);
    this.activeTab = tabs.length ? tabs[tabs.length - 1].key : null;
    if (this.activeTab?.startsWith("run:")) this.activeRun = Number(this.activeTab.slice(4));
  }

  async stopRun(id: number) {
    try {
      await api.stopRun(id);
    } catch (e) {
      this.notify(`Could not stop: ${firstLine(String(e))}`, "err");
    }
  }

  stopAll() {
    for (const r of this.runs) if (r.running) this.stopRun(r.id);
  }

  async restartRun(id: number) {
    const r = this.runs.find((x) => x.id === id);
    if (!r) return;
    if (r.running) {
      await this.stopRun(id);
      const until = Date.now() + 10000;
      while (this.runs.find((x) => x.id === id)?.running && Date.now() < until) await new Promise((res) => setTimeout(res, 100));
    }
    await this.closeRun(id);
    await this.runTask({ id: r.taskId, name: r.name, source: "custom", command: r.command, cwd: r.cwd, detail: "" }, r.repo);
  }

  /** Removes a finished run from the panel. A running one is stopped first. */
  async closeRun(id: number) {
    const r = this.runs.find((x) => x.id === id);
    if (!r) return;
    if (r.running) return this.stopRun(id);
    this.runs = this.runs.filter((x) => x.id !== id);
    this.terms.delete(id);
    disposeRun(id);
    api.forgetRun(id).catch(() => {});
    if (this.activeRun === id) this.activeRun = this.runs.length ? this.runs[this.runs.length - 1].id : null;
    if (this.activeTab === "run:" + id) this.pickLastTab();
  }

  private onRunExit(info: RunInfo) {
    this.runs = this.runs.map((r) => (r.id === info.id ? info : r));
    const took = duration((info.ended ?? Date.now()) - info.started);
    const t = this.term(info.id);
    if (info.stopped) t.write(`\r\n\x1b[2m■ Stopped after ${took}\x1b[0m\r\n`);
    else if (info.code === 0) t.write(`\r\n\x1b[32m✓ Finished in ${took}\x1b[0m\r\n`);
    else t.write(`\r\n\x1b[31m✗ Exited with code ${info.code ?? "?"} after ${took}\x1b[0m\r\n`);
    this.bump();
    if (info.stopped) return;
    const where = baseName(info.repo);
    const long = (info.ended ?? 0) - info.started > 8000;
    if (info.code !== 0) this.notify(`${info.name} failed in ${where} (exit code ${info.code ?? "?"})`, "err");
    else if (long) this.notify(`${info.name} finished in ${where} (${took})`, "ok");
    if (info.repo === this.current) this.refresh([info.repo]);
    if (info.taskId.startsWith("clone:") && info.code === 0) {
      const sep = info.repo.includes("\\") ? "\\" : "/";
      const path = info.repo.replace(/[\\/]+$/, "") + sep + info.taskId.slice(6);
      this.reloadRepos(true).then(() => {
        if (this.repos.some((r) => r.path.toLowerCase() === path.toLowerCase())) {
          this.select(this.repos.find((r) => r.path.toLowerCase() === path.toLowerCase())!.path);
          this.notify(`Cloned ${info.taskId.slice(6)}`, "ok");
        } else {
          this.notify(`Cloned into ${path}. Add its folder in Settings to see it here.`, "info");
        }
      });
    }
  }

  async copyRunOutput(id: number, lines = 200) {
    const r = this.runs.find((x) => x.id === id);
    if (!r) return;
    const status = r.running ? "still running" : r.stopped ? "stopped" : `exit code ${r.code}`;
    // The terminal's own screen is exact; the text renderer is the fallback before the task was shown.
    const body = runText(id, lines) || this.term(id).toText(lines);
    const text = `Output of \`${r.command}\` in ${r.repo} (${status}), last ${lines} lines:\n\n${body}`;
    await copyText(text);
    this.notify(`Copied the last ${Math.min(lines, this.term(id).lineCount)} lines of ${r.name}`, "ok");
  }

  /** True once the panel was opened in this session. An empty panel is not brought back from last time. */
  panelTouched = $state(false);

  setPanel(open: boolean) {
    if (open) this.panelTouched = true;
    this.panelOpen = open;
    save("panelOpen", open ? "1" : "0");
  }

  setPanelHeight(h: number) {
    this.panelHeight = h;
    save("panelHeight", String(h));
  }

  async saveCustomTask(repo: string, name: string, command: string, cwd = "") {
    if (!this.config || !command.trim()) return;
    const list = [...(this.config.tasks[repo] ?? []), { name: name.trim() || command.trim(), command: command.trim(), cwd }];
    this.config = { ...this.config, tasks: { ...this.config.tasks, [repo]: list } };
    await api.setConfig($state.snapshot(this.config));
    await this.loadTasks(repo, true);
    this.notify(`Saved "${name.trim() || command.trim()}" as a task`, "ok");
  }

  isTaskPinned(repo: string, id: string) {
    return !!this.config?.pinnedTasks?.[repo]?.includes(id);
  }

  /** Pins a task to the top of the Tasks tab, or unpins it. */
  async toggleTaskPin(repo: string, t: Task) {
    if (!this.config) return;
    const cur = this.config.pinnedTasks?.[repo] ?? [];
    const on = !cur.includes(t.id);
    const list = on ? [...cur, t.id] : cur.filter((x) => x !== t.id);
    const pinnedTasks = { ...(this.config.pinnedTasks ?? {}), [repo]: list };
    if (!list.length) delete pinnedTasks[repo];
    await this.saveConfig({ ...this.config, pinnedTasks });
    this.notify(`${on ? "Pinned" : "Unpinned"} ${t.name}`, "ok");
  }

  async removeCustomTask(repo: string, index: number) {
    if (!this.config) return;
    const list = (this.config.tasks[repo] ?? []).filter((_, i) => i !== index);
    const tasks = { ...this.config.tasks, [repo]: list };
    if (!list.length) delete tasks[repo];
    this.config = { ...this.config, tasks };
    await api.setConfig($state.snapshot(this.config));
    await this.loadTasks(repo, true);
  }

  /** Refreshes often while the window has focus, and 4 times less often in the background. */
  private startPolling() {
    clearInterval(this.pollTimer);
    const seconds = Math.max(3, this.config?.pollSeconds ?? 8);
    let ticks = 0;
    this.pollTimer = setInterval(() => {
      ticks++;
      if (document.hasFocus() || ticks % 4 === 0) this.refresh();
    }, seconds * 1000);
  }

  /** `all` refreshes every repo. This is needed once, so the remote-based space rules can match. */
  async reloadRepos(all = false) {
    this.repos = await api.listRepos();
    await this.refresh(all ? this.repos.map((r) => r.path) : undefined);
  }

  async refresh(paths?: string[]) {
    const list = paths ?? this.visible.filter((r) => !this.isHidden(r)).map((r) => r.path);
    if (!list.length) return;
    const results = await api.summaries(list);
    const next = { ...this.sums };
    for (const s of results) next[s.path] = s;
    this.sums = next;
    this.now = nowSeconds();
    if (this.current && document.hasFocus()) this.markSeen(this.current);
    this.checkQuiet();
    this.checkLogs(results.map((s) => s.path));
    this.checkCi();
    // A stash changes git's branch files, so the stamp moves when stashes come and go.
    const cur = this.current;
    if (cur && this.tab === "changes" && this.stashStamp[cur] !== this.sums[cur]?.refsStamp) this.loadStashes(cur);
    this.checkBranchReview();
  }

  /** Keeps an open branch review current while an agent keeps working. */
  private checkBranchReview() {
    const repo = this.current;
    if (this.review?.kind !== "branch" || !repo) return;
    const s = this.sums[repo];
    const sig = `${s?.refsStamp}|${s?.changes.map((c) => `${c.path}:${c.mtime}`).join(",")}`;
    if (sig === this.branchSig) return;
    this.branchSig = sig;
    this.openBranchReview(this.review.baseRef, true);
  }

  /** Main-like branch names. A branch with another name, or any worktree, can be reviewed against its base. */
  canReviewBranch(path: string | null) {
    if (!path) return false;
    const s = this.sums[path];
    const repo = this.repos.find((r) => r.path === path);
    if (!s?.branch) return !!repo?.parent;
    return !!repo?.parent || !["main", "master", "develop", "dev", "trunk"].includes(s.branch.toLowerCase());
  }

  /** Shows everything this branch or worktree changed since it split from `base` (commits and uncommitted work). */
  async openBranchReview(base?: string, quiet = false) {
    const repo = this.current;
    if (!repo) return;
    try {
      const r = await api.branchReview(repo, base ?? this.reviewBase[repo] ?? null);
      if (repo !== this.current) return;
      const n = r.commits.length;
      const review: Review = {
        kind: "branch",
        title: `${r.branch ?? "HEAD"} since ${r.baseRef}`,
        subtitle: `${n} ${n === 1 ? "commit" : "commits"}, split at ${r.mergeBase.slice(0, 7)}`,
        base: r.mergeBase,
        head: "WORKTREE",
        files: r.files,
        commits: r.commits,
        baseRef: r.baseRef,
        candidates: r.candidates,
      };
      if (base) {
        this.reviewBase = { ...this.reviewBase, [repo]: base };
        save("reviewBase", JSON.stringify(this.reviewBase));
      }
      if (quiet) {
        // Keep the open file. Only the list and the header change.
        if (this.review?.kind === "branch") this.review = review;
        return;
      }
      if (this.tab !== "changes") this.setTab("changes");
      const s = this.sums[repo];
      this.branchSig = `${s?.refsStamp}|${s?.changes.map((c) => `${c.path}:${c.mtime}`).join(",")}`;
      this.enterReview(review);
    } catch (e) {
      if (!quiet) this.notify(`Could not compare with the base: ${firstLine(String(e))}`, "err");
    }
  }

  /** A summary of a branch review, with @ mentions, for pasting to an agent. */
  async copyBranchSummary() {
    const r = this.review;
    const repo = this.current;
    if (r?.kind !== "branch" || !repo) return;
    const commits = (r.commits ?? []).map((c) => `- ${c.short} ${c.subject}`).join("\n");
    const files = r.files.map((c) => `@${c.path} (${c.kind})`).join("\n");
    const text = [
      `Review the work in ${repo}: ${r.title} (split at ${r.base.slice(0, 7)}).`,
      "",
      `Commits (${r.commits?.length ?? 0}):`,
      commits || "- none yet, only uncommitted work",
      "",
      `Files changed (${r.files.length}), including uncommitted and new files:`,
      files,
      "",
      `Full diff: git diff ${r.base.slice(0, 7)}`,
    ].join("\n");
    await this.copyValue(text, "the branch review for an agent");
  }

  private logKey(repo: string) {
    const s = this.sums[repo];
    return `${s?.refsStamp ?? ""}|${s?.last?.sha ?? ""}|${s?.behind ?? 0}|${s?.ahead ?? 0}|${this.logAll}`;
  }

  /** New commits or moved branches: reload the history on screen, and forget the others so they reload when opened. */
  private checkLogs(paths: string[]) {
    let dropped = false;
    const logs = { ...this.logs };
    for (const p of paths) {
      if (!logs[p] || this.logStamp[p] === this.logKey(p)) continue;
      if (p === this.current && this.tab === "history") this.loadLog(p, true);
      else {
        delete logs[p];
        dropped = true;
      }
    }
    if (dropped) this.logs = logs;
  }

  markSeen(path: string) {
    this.lastSeen = { ...this.lastSeen, [path]: nowSeconds() };
    save("lastSeen", JSON.stringify(this.lastSeen));
  }

  /** Says when a repo that was being changed has gone quiet. This usually means an agent finished. */
  private checkQuiet() {
    for (const r of this.visible) {
      const working = this.isWorking(r.path);
      if (working) this.wasWorking.add(r.path);
      else if (this.wasWorking.delete(r.path)) {
        const n = this.sums[r.path]?.changes.length ?? 0;
        const body = `${n} changed ${n === 1 ? "file" : "files"}. No new changes for 2 minutes.`;
        this.notify(`${r.name} went quiet. ${body}`, "info");
      }
    }
  }

  /** F5: reload everything that can go stale. */
  reloadAll() {
    this.stamp++;
    this.refresh();
    const repo = this.current;
    if (!repo) return;
    if (this.tab === "files") this.loadFiles(repo, true);
    if (this.tab === "prs") this.loadPrs(repo, true);
    if (this.tab === "history") this.loadLog(repo, true);
    if (this.tab === "tasks") this.loadTasks(repo, true);
    if (this.tab === "issues") this.loadIssues(repo, true);
    if (this.tab === "ci") this.loadCi(repo, true);
  }

  // CI

  async loadCi(repo: string, force = false) {
    const cur = this.ci[repo];
    if (cur && !force && Date.now() - cur.at < 60_000) return;
    if (cur?.loading) return;
    this.ci = { ...this.ci, [repo]: { loading: true, error: null, items: cur?.items ?? [], at: cur?.at ?? 0 } };
    try {
      const items = await api.ciRuns(repo);
      // Say when a run that was going has finished.
      for (const old of cur?.items ?? []) {
        if (old.status === "completed") continue;
        const now = items.find((x) => x.id === old.id);
        if (now?.status === "completed") {
          const word = { succeeded: "passed", partial: "finished with warnings", failed: "failed", canceled: "was canceled", skipped: "was skipped", "": "finished" }[now.result];
          this.notify(`${now.name} #${now.number} ${word} (${baseName(repo)}, ${now.branch})`, now.result === "failed" ? "err" : now.result === "succeeded" ? "ok" : "info");
        }
      }
      this.ci = { ...this.ci, [repo]: { loading: false, error: null, items, at: Date.now() } };
    } catch (e) {
      this.ci = { ...this.ci, [repo]: { loading: false, error: String(e), items: [], at: Date.now() } };
    }
  }

  // Database tool

  /** The database screen replaces the project's list and viewer while it is open. */
  dataOpen = $state(false);
  dbConns = $state<DbConn[]>([]);
  dbSel = $state<string | null>(saved("dbSel") || null);
  dbSchemas = $state<Record<string, { loading: boolean; error: string | null; data: DbSchema | null }>>({});

  openData(select?: string) {
    if (!this.dataOpen) this.remember();
    this.dataOpen = true;
    this.ticketsOpen = false;
    this.overlay = null;
    this.loadDbConns().then(() => {
      if (select) this.selectDb(select);
      else if (!this.dbConns.some((c) => c.id === this.dbSel) && this.dbConns.length) this.selectDb(this.dbConns[0].id);
      else if (this.dbSel && !this.dbSchemas[this.dbSel]) this.loadDbSchema(this.dbSel);
    });
  }

  closeData() {
    this.dataOpen = false;
  }

  async loadDbConns() {
    try {
      this.dbConns = await api.dbList();
    } catch (e) {
      this.notify(`Could not read the saved connections: ${firstLine(String(e))}`, "err");
    }
  }

  selectDb(id: string) {
    this.dbSel = id;
    save("dbSel", id);
    if (!this.dbSchemas[id]?.data) this.loadDbSchema(id);
  }

  async loadDbSchema(id: string) {
    const conn = this.dbConns.find((c) => c.id === id);
    if (!conn) return;
    this.dbSchemas = { ...this.dbSchemas, [id]: { loading: true, error: null, data: this.dbSchemas[id]?.data ?? null } };
    try {
      const data = await api.dbSchema(conn);
      this.dbSchemas = { ...this.dbSchemas, [id]: { loading: false, error: null, data } };
    } catch (e) {
      this.dbSchemas = { ...this.dbSchemas, [id]: { loading: false, error: String(e), data: null } };
    }
  }

  async saveDbConn(conn: DbConn, password: string | null) {
    const saved = await api.dbSave(conn, password);
    await this.loadDbConns();
    // Settings changed: the old schema may be from another server.
    const { [saved.id]: _, ...rest } = this.dbSchemas;
    this.dbSchemas = rest;
    this.selectDb(saved.id);
    return saved;
  }

  async removeDbConn(id: string) {
    await api.dbRemove(id);
    const { [id]: _, ...rest } = this.dbSchemas;
    this.dbSchemas = rest;
    await this.loadDbConns();
    if (this.dbSel === id) {
      this.dbSel = this.dbConns[0]?.id ?? null;
      save("dbSel", this.dbSel ?? "");
    }
  }


  // Tickets

  /** The tickets view replaces the project's list and viewer while it is open. */
  ticketsOpen = $state(false);
  tickets = $state<{ loading: boolean; items: Ticket[]; errors: { source: string; message: string }[]; at: number }>({
    loading: false,
    items: [],
    errors: [],
    at: 0,
  });
  ticketSel = $state<string | null>(saved("ticketSel") || null);
  ticketDetails = $state<Record<string, { loading: boolean; error: string | null; data: TicketDetail | null; at: number }>>({});

  /** Azure DevOps organizations, from the remotes of the repos Tern knows. */
  private ticketOrgs() {
    const orgs = new Set<string>();
    for (const s of Object.values(this.sums)) {
      const url = s?.remote ?? "";
      const m = url.match(/dev\.azure\.com[/:](?:v3\/)?([^/]+)\//i) ?? url.match(/\/\/(?:[^@/]+@)?([^./]+)\.visualstudio\.com/i);
      if (m) orgs.add(decodeURIComponent(m[1]));
    }
    return [...orgs];
  }

  openTickets() {
    if (!this.ticketsOpen) this.remember();
    this.ticketsOpen = true;
    this.dataOpen = false;
    this.overlay = null;
    if (Date.now() - this.tickets.at > 120_000) this.loadTickets();
  }

  closeTickets() {
    this.ticketsOpen = false;
  }

  async loadTickets(force = false) {
    if (this.tickets.loading) return;
    if (!force && this.tickets.at && Date.now() - this.tickets.at < 30_000) return;
    this.tickets = { ...this.tickets, loading: true };
    const orgs = this.ticketOrgs();
    const hasGithub = Object.values(this.sums).some((s) => s?.provider === "github");
    const jobs: { source: string; run: () => Promise<Ticket[]> }[] = orgs.map((org) => ({ source: `Azure DevOps (${org})`, run: () => api.ticketsAzdo(org) }));
    if (hasGithub || !orgs.length) jobs.push({ source: "GitHub", run: () => api.ticketsGithub() });
    const results = await Promise.allSettled(jobs.map((j) => j.run()));
    const items: Ticket[] = [];
    const errors: { source: string; message: string }[] = [];
    results.forEach((r, i) => {
      if (r.status === "fulfilled") items.push(...r.value);
      else errors.push({ source: jobs[i].source, message: firstLine(String(r.reason)) });
    });
    items.sort((a, b) => b.changed.localeCompare(a.changed));
    this.tickets = { loading: false, items, errors, at: Date.now() };
    // The ticket chosen last time: read its details too, so the right side is not empty.
    const chosen = items.find((t) => t.key === this.ticketSel);
    if (chosen && !this.ticketDetails[chosen.key]) this.loadTicketDetail(chosen);
    if (!this.ticketSel || !items.some((t) => t.key === this.ticketSel)) this.selectTicket(items[0]?.key ?? null);
  }

  selectTicket(key: string | null) {
    this.ticketSel = key;
    save("ticketSel", key ?? "");
    const t = key ? this.tickets.items.find((x) => x.key === key) : undefined;
    if (t) this.loadTicketDetail(t);
  }

  async loadTicketDetail(t: Ticket, force = false) {
    const cur = this.ticketDetails[t.key];
    if (cur && (cur.loading || (!force && Date.now() - cur.at < 120_000))) return;
    this.ticketDetails = { ...this.ticketDetails, [t.key]: { loading: true, error: null, data: cur?.data ?? null, at: cur?.at ?? 0 } };
    try {
      const data = t.provider === "azdo" ? await api.ticketAzdoDetail(t.source, t.project, t.number) : await api.ticketGithubDetail(t.source, t.number);
      this.ticketDetails = { ...this.ticketDetails, [t.key]: { loading: false, error: null, data, at: Date.now() } };
    } catch (e) {
      this.ticketDetails = { ...this.ticketDetails, [t.key]: { loading: false, error: firstLine(String(e)), data: null, at: Date.now() } };
    }
  }

  /** A branch name for a ticket, like `bug/4790-grid-loses-its-sort`. */
  ticketBranch(t: Ticket) {
    const kind = /bug/i.test(t.kind) ? "bug" : /story|feature|epic/i.test(t.kind) ? "feature" : "task";
    const words = t.title.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim().split(" ").filter(Boolean).slice(0, 6).join("-");
    return `${kind}/${t.number}-${words}`;
  }

  /** Copies a ticket as plain text for an agent: the title, fields, and description. */
  async copyTicketForAgent(t: Ticket) {
    const d = this.ticketDetails[t.key]?.data;
    const plain = (html: string, markdown: boolean) => {
      if (markdown) return html.trim();
      const doc = new DOMParser().parseFromString(html, "text/html");
      doc.querySelectorAll("br").forEach((b) => b.replaceWith("\n"));
      doc.querySelectorAll("li").forEach((li) => li.prepend("- "));
      doc.querySelectorAll("p, div, li, h1, h2, h3, tr").forEach((el) => el.append("\n"));
      return (doc.body.textContent ?? "").replace(/\n{3,}/g, "\n\n").trim();
    };
    const lines = [
      `${t.kind} #${t.number}: ${t.title}`,
      `State: ${t.state}${t.priority != null ? `, priority ${t.priority}` : ""}${t.iteration ? `, sprint ${t.iteration}` : ""}`,
      `Project: ${t.project}${t.area && t.area !== t.project ? ` / ${t.area}` : ""}`,
      t.url,
      ...(d?.sections ?? []).flatMap((s) => ["", `## ${s.title}`, plain(s.body, s.markdown)]),
    ];
    await copyText(lines.join("\n"));
    this.notify(`Copied ${t.kind.toLowerCase()} #${t.number} for an agent`, "ok");
  }

  /** Keeps CI lists with running pipelines fresh (every 20 s), so a finished run is noticed. */
  private checkCi() {
    for (const [repo, c] of Object.entries(this.ci)) {
      const live = c.items.some((r) => r.status !== "completed");
      const onScreen = repo === this.current && this.tab === "ci";
      if ((live && Date.now() - c.at > 20_000) || (onScreen && Date.now() - c.at > 60_000)) this.loadCi(repo, true);
    }
  }

  openRun(run: CiRun) {
    if (!this.current) return;
    this.remember();
    this.editing = false;
    this.view = { kind: "run", repo: this.current, run, rel: "" };
  }

  /** Ctrl+1 to Ctrl+6 pick the Nth tab this project shows. */
  tabByNumber(n: number) {
    const tab = this.visibleTabs[n - 1];
    if (tab) this.setTab(tab);
  }

  setLogAll(on: boolean) {
    this.logAll = on;
    save("logAll", on ? "1" : "0");
    if (this.current) this.loadLog(this.current, true);
  }

  async loadIssues(repo: string, force = false) {
    if (this.issues[repo] && !force && this.fresh("issue:" + repo)) return;
    this.listAt["issue:" + repo] = Date.now();
    this.issues = { ...this.issues, [repo]: { loading: true, error: null, items: this.issues[repo]?.items ?? [] } };
    try {
      const items = await api.listIssues(repo, this.issueState);
      this.issues = { ...this.issues, [repo]: { loading: false, error: null, items } };
    } catch (e) {
      this.issues = { ...this.issues, [repo]: { loading: false, error: String(e), items: [] } };
    }
  }

  setIssueState(state: "open" | "closed" | "all") {
    this.issueState = state;
    save("issueState", state);
    if (this.current) this.loadIssues(this.current, true);
  }

  openIssue(number: number) {
    if (!this.current) return;
    this.remember();
    this.editing = false;
    this.lines = null;
    this.view = { kind: "issue", repo: this.current, number, rel: "" };
  }

  // Hover cards and menus

  showHover(node: HTMLElement, data: HoverData) {
    if (this.menu || this.overlay) return;
    const r = node.getBoundingClientRect();
    this.hoverOwner = node;
    this.hoverCard = { rect: { left: r.left, top: r.top, right: r.right, bottom: r.bottom }, data };
  }

  hideHover(node?: HTMLElement) {
    if (node && node !== this.hoverOwner) return;
    this.hoverOwner = null;
    this.hoverCard = null;
  }

  showMenu(e: MouseEvent | { x: number; y: number }, title: string, items: MenuItem[]) {
    if ("preventDefault" in e) {
      e.preventDefault();
      e.stopPropagation();
    }
    this.hideHover();
    this.menu = { x: e.x, y: e.y, title, items };
  }

  async copyValue(text: string, what?: string) {
    try {
      await copyText(text);
      const shown = what ?? (text.length > 60 ? text.slice(0, 57) + "…" : text);
      this.notify(`Copied ${shown.split("\n")[0]}`, "ok");
    } catch (e) {
      this.notify(`Copy failed: ${firstLine(String(e))}`, "err");
    }
  }

  // Navigation

  select(path: string | null) {
    this.ticketsOpen = false;
    this.dataOpen = false;
    if (path === this.current) return;
    this.remember();
    if (this.current) this.markSeen(this.current);
    this.current = path;
    if (path) this.markSeen(path);
    this.review = null;
    this.view = null;
    this.selected = [];
    this.lines = null;
    this.editing = false;
    save("current", path ?? "");
    save("current." + (this.config?.activeSpace ?? "all"), path ?? "");
    if (path && !this.tabsFor(path).includes(this.tab)) this.tab = this.tabsFor(path)[0];
    if (path) this.loadTab(path);
  }

  step(delta: number) {
    const list = this.ordered;
    if (!list.length) return;
    const i = list.findIndex((r) => r.path === this.current);
    const next = i < 0 ? (delta > 0 ? 0 : list.length - 1) : (i + delta + list.length) % list.length;
    this.select(list[next].path);
  }

  /** The open item per project and tab, so switching tabs brings back what each one showed. */
  private tabViews: Record<string, View | null> = {};

  setTab(tab: Tab) {
    if (this.current && !this.visibleTabs.includes(tab)) {
      const label = { changes: "Changes", files: "Files", prs: "Pull requests", issues: "Issues", history: "History", ci: "CI", tasks: "Tasks" }[tab];
      this.notify(`${label} is turned off for this space. Turn it on in Settings.`, "info");
      return;
    }
    if (tab !== this.tab || this.review) this.remember();
    // Each tab keeps its own open item, so the viewer always matches the list on screen.
    if (tab !== this.tab && this.current) {
      this.tabViews[`${this.current}|${this.tab}`] = this.view;
      const back = this.tabViews[`${this.current}|${tab}`] ?? null;
      this.view = back && back.repo === this.current ? back : null;
    }
    this.tab = tab;
    this.review = null;
    this.selected = [];
    save("tab", tab);
    if (this.current) this.loadTab(this.current);
  }

  private loadTab(repo: string) {
    if (this.tab === "changes") this.loadStashes(repo);
    if (this.tab === "tasks") this.loadTasks(repo);
    if (this.tab === "issues") this.loadIssues(repo);
    if (this.tab === "ci") this.loadCi(repo);
    if (this.tab === "files") this.loadFiles(repo);
    if (this.tab === "prs") this.loadPrs(repo);
    if (this.tab === "history") this.loadLog(repo);
  }

  async loadFiles(repo: string, force = false) {
    if (this.files[repo] && !force) return this.files[repo];
    try {
      const list = await api.listFiles(repo);
      this.files = { ...this.files, [repo]: list };
      return list;
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
      return [];
    }
  }

  private fresh(key: string, ms = 120_000) {
    return Date.now() - (this.listAt[key] ?? 0) < ms;
  }

  async loadPrs(repo: string, force = false) {
    if (this.prs[repo] && !force && this.fresh("pr:" + repo)) return;
    this.listAt["pr:" + repo] = Date.now();
    this.prs = { ...this.prs, [repo]: { loading: true, error: null, items: this.prs[repo]?.items ?? [] } };
    try {
      const items = await api.listPrs(repo);
      this.prs = { ...this.prs, [repo]: { loading: false, error: null, items } };
    } catch (e) {
      this.prs = { ...this.prs, [repo]: { loading: false, error: String(e), items: [] } };
    }
  }

  async loadLog(repo: string, force = false) {
    if (this.logs[repo] && !force && this.logStamp[repo] === this.logKey(repo)) return;
    try {
      const key = this.logKey(repo);
      const { commits, ...sync } = await api.log(repo, 300, this.logAll);
      this.logStamp[repo] = key;
      this.logSync = { ...this.logSync, [repo]: sync };
      this.logs = { ...this.logs, [repo]: commits };
    } catch (e) {
      this.logs = { ...this.logs, [repo]: [] };
      this.notify(firstLine(String(e)), "err");
    }
  }

  // Opening things

  /** `record` is off while you step through a list with the arrow keys. */
  openChange(c: Change, record = true) {
    if (!this.current) return;
    if (record) this.remember();
    this.lines = null;
    this.editing = false;
    this.view = {
      kind: "diff",
      repo: this.current,
      rel: c.path,
      oldRel: c.oldPath,
      changeKind: c.kind,
      rev: c.revBase && c.revHead ? { base: c.revBase, head: c.revHead } : this.review ? { base: this.review.base, head: this.review.head } : null,
    };
  }

  openFile(rel: string, line?: number, record = true, find?: { text: string; regex: boolean; matchCase: boolean }) {
    if (!this.current) return;
    if (record) this.remember();
    this.lines = line ? { from: line, to: line } : null;
    this.editing = false;
    this.view = { kind: "file", repo: this.current, rel, line, find, jump: line ? Date.now() : undefined };
  }

  /** Reviews everything a pull would bring, as one diff. */
  async reviewIncoming(repo: string) {
    const sync = this.logSync[repo];
    if (!sync?.upstream || !sync.behind) return;
    this.reviewLoading = "incoming";
    try {
      const r = await api.rangeFiles(repo, sync.base, sync.upstreamSha);
      const n = sync.behind;
      this.enterReview({
        kind: "commit",
        title: `${n} ${n === 1 ? "commit" : "commits"} from ${sync.upstream}`,
        subtitle: `What a pull brings into ${sync.branch ?? "this branch"}`,
        ...r,
      });
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    } finally {
      this.reviewLoading = null;
    }
  }

  async openCommit(c: Commit) {
    const repo = this.current;
    if (!repo) return;
    this.reviewLoading = c.sha;
    try {
      const r = await api.commitFiles(repo, c.sha);
      this.enterReview({ kind: "commit", title: c.subject, subtitle: `${c.short} by ${c.author}`, ...r });
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    } finally {
      this.reviewLoading = null;
    }
  }

  async openPr(pr: PullRequest) {
    const repo = this.current;
    if (!repo) return;
    this.reviewLoading = String(pr.id);
    try {
      const r = await api.preparePr(repo, pr);
      this.enterReview({ kind: "pr", title: pr.title, subtitle: `#${pr.id} ${pr.source} → ${pr.target}`, pr, ...r });
    } catch (e) {
      this.notify(`Could not load PR #${pr.id}: ${firstLine(String(e))}`, "err");
    } finally {
      this.reviewLoading = null;
    }
  }

  private enterReview(r: Review) {
    this.remember();
    this.review = r;
    this.selected = [];
    this.view = null;
    if (r.files.length) this.openChange(r.files[0], false);
  }

  closeReview() {
    this.remember();
    this.review = null;
    this.selected = [];
    this.view = null;
  }

  toggleSelect(rel: string) {
    this.selected = this.selected.includes(rel) ? this.selected.filter((r) => r !== rel) : [...this.selected, rel];
  }

  setDiffMode(mode: "split" | "unified") {
    this.diffMode = mode;
    save("diffMode", mode);
  }

  // Copying for agents

  async copy(format: CopyFormat) {
    const repo = this.view?.kind === "file" && this.view.loose ? this.view.repo : this.current;
    this.overlay = null;
    if (!repo) return this.notify("Open a project first.", "info");
    const rels = this.selected.length ? this.selected : this.view?.rel ? [this.view.rel] : [];
    try {
      let text: string;
      if (format === "diff") {
        if (this.review?.head === "WORKTREE") {
          const scope = rels.length ? this.review.files.filter((c) => rels.includes(c.path)) : this.review.files;
          const untracked = scope.filter((c) => c.kind === "untracked").map((c) => c.path);
          text = await api.basePatch(repo, this.review.base, rels, untracked);
        } else if (this.review) {
          text = await api.revPatch(repo, this.review.base, this.review.head, rels);
        } else {
          const all = this.summary?.changes ?? [];
          const scope = rels.length ? all.filter((c) => rels.includes(c.path)) : all;
          const untracked = scope.filter((c) => c.kind === "untracked").map((c) => c.path);
          text = await api.worktreePatch(repo, rels, untracked);
        }
        if (!text.trim()) return this.notify("No changes to copy.", "info");
      } else if (!rels.length) {
        text = format === "mention" ? "@" + repo : repo;
      } else {
        const lines = this.view && rels.length === 1 && rels[0] === this.view.rel ? this.lines : null;
        text = formatPaths(format, repo, rels, lines);
      }
      await copyText(text);
      this.lastCopy = format;
      save("lastCopy", format);
      const shown = format === "diff" ? `diff (${text.split("\n").length} lines)` : text.split("\n")[0];
      const more = format !== "diff" && rels.length > 1 ? ` and ${rels.length - 1} more` : "";
      this.notify(`Copied ${shown}${more}`, "ok");
    } catch (e) {
      this.notify(`Copy failed: ${firstLine(String(e))}`, "err");
    }
  }

  // Git actions

  /** Shows the confirm dialog and waits for the answer. */
  ask(req: Omit<ConfirmRequest, "resolve">): Promise<boolean> {
    this.confirmReq?.resolve(false);
    return new Promise((resolve) => {
      this.confirmReq = { ...req, resolve };
      this.overlay = "confirm";
    });
  }

  answer(ok: boolean) {
    const req = this.confirmReq;
    this.confirmReq = null;
    if (this.overlay === "confirm") this.overlay = null;
    req?.resolve(ok);
  }

  private mainWarning(branch: string | null | undefined) {
    return branch && MAIN_BRANCHES.includes(branch.toLowerCase())
      ? [`You are pushing straight to ${branch}, not to a feature branch.`]
      : [];
  }

  /** Builds the pull or push question from the repo's state and the commits involved. */
  private async confirmSync(repo: string, action: "pull" | "push"): Promise<boolean> {
    await this.refresh([repo]);
    const s = this.sums[repo];
    if (!s) return false;
    const name = baseName(repo);
    const branch = s.branch ?? "detached HEAD";
    const n = s.changes.length;
    let commits: Commit[] = [];
    try {
      commits = await api.pending(repo, action === "pull");
    } catch {
      /* the dialog still works without the list */
    }
    const count = (k: number) => `${k} ${k === 1 ? "commit" : "commits"}`;

    if (action === "pull") {
      if (!s.upstream) {
        this.notify(`${name}: ${branch} has no upstream branch, so there is nothing to pull from.`, "info");
        return false;
      }
      const warnings: string[] = [];
      if (n) warnings.push(`${n} local ${n === 1 ? "change" : "changes"}. Git stops if the pull would change the same files. Nothing is lost.`);
      if (s.ahead) warnings.push(`This branch also has ${count(s.ahead)} that the remote does not have. A fast-forward pull will fail. You need to merge or rebase.`);
      return this.ask({
        title: `Pull into ${name} / ${branch}?`,
        body: `From ${s.upstream}, fast-forward only. Git fetches first, so newer commits may also come in. This list is from the last fetch, ${ago(s.fetchedAt)}.`,
        commits,
        commitsLabel: commits.length ? `${count(commits.length)} to pull` : "No new commits as of the last fetch",
        warnings,
        confirm: commits.length ? `Pull ${count(commits.length)}` : "Pull",
      });
    }

    const warnings = [...this.mainWarning(s.branch)];
    if (s.behind) warnings.push(`The remote has ${count(s.behind)} you do not have. The push will be rejected. Pull first.`);
    if (n) warnings.push(`${n} uncommitted ${n === 1 ? "change stays" : "changes stay"} on your machine. Only commits are pushed.`);
    const target = s.upstream ?? `a new branch origin/${branch}`;
    return this.ask({
      title: `Push ${name} / ${branch}?`,
      body: s.upstream
        ? `To ${s.upstream}. Other people can see these commits after the push.`
        : `This branch has no upstream yet. Tern runs git push -u origin HEAD, which creates ${target}.`,
      commits,
      commitsLabel: commits.length ? `${count(commits.length)} to push` : "No commits to push",
      warnings,
      confirm: commits.length ? `Push ${count(commits.length)}` : "Push",
      allowed: commits.length > 0,
    });
  }

  async action(repo: string, action: GitAction, quiet = false, confirmed = false) {
    if (this.busy[repo]) return;
    if ((action === "pull" || action === "push") && !confirmed && !(await this.confirmSync(repo, action))) return false;
    this.busy = { ...this.busy, [repo]: action };
    try {
      const out = await api.gitAction(repo, action);
      if (!quiet) {
        const tail = out.trim().split("\n").filter(Boolean).pop();
        const past = { fetch: "Fetched", pull: "Pulled", push: "Pushed" }[action];
        this.notify(`${past} ${baseName(repo)}${tail ? ": " + tail : ""}`, "ok");
      }
      return true;
    } catch (e) {
      this.notify(`${action} failed in ${baseName(repo)}: ${firstLine(String(e))}`, "err");
      return false;
    } finally {
      const { [repo]: _, ...rest } = this.busy;
      this.busy = rest;
      this.refresh([repo]);
    }
  }

  // Search

  openSearch(preset: { query?: string; scope?: string; word?: boolean; kind?: string; defsFor?: string } = {}) {
    this.searchPreset = preset;
    this.overlay = null;
    queueMicrotask(() => (this.overlay = "search"));
  }

  /** Ctrl+F: search the open file, or the open repo when no file is open. */
  searchHere() {
    const word = this.wordAtCursor();
    const selected = word && word.length < 80 ? word : undefined;
    this.openSearch({ scope: this.view?.rel ? "file" : this.current ? "repo" : "space", query: selected });
  }

  /** F12 or Ctrl+click: jump straight to the definition when there is one clear match. */
  async goToDefinition(word = this.wordAtCursor()) {
    const repo = this.current;
    const { cleanWord, definitionPattern, isCodeFile } = await import("./defs");
    const w = cleanWord(word);
    if (!repo || !w) return this.notify("Put the cursor on a name first.", "info");
    try {
      const r = await api.searchTextWith(repo, definitionPattern(w), { perl: true, caseSensitive: true }, 40);
      const code = r.hits.filter((h) => isCodeFile(h.path));
      // One clear place: one hit, all hits in one file (a class and its constructor), or exactly one type.
      const types = code.filter((h) => new RegExp(`\\b(class|interface|struct|enum|record|trait|type)\\s+${w.replace(/\$/g, "\\$")}\\b`).test(h.text));
      const oneFile = code.length > 0 && code.every((h) => h.path === code[0].path);
      const pick = code.length === 1 ? code[0] : oneFile ? [...code].sort((a, b) => a.line - b.line)[0] : types.length === 1 ? types[0] : null;
      if (pick) {
        const h = pick;
        this.openFile(h.path, h.line, true, { text: w, regex: false, matchCase: true });
        this.notify(`Definition of ${w}: ${h.path}:${h.line}`, "ok");
        return;
      }
      if (!r.hits.length) this.notify(`No definition of ${w} found. Showing where it is used.`, "info");
      this.openSearch({ query: w, word: true, scope: "repo", kind: "code", defsFor: w });
    } catch (e) {
      this.notify(`Could not look up ${w}: ${firstLine(String(e))}`, "err");
    }
  }

  /** Shift+F12: every place the word is used in this repo. */
  findUsages(word = this.wordAtCursor()) {
    const w = word.match(/[A-Za-z_$][\w$]*/)?.[0];
    if (!w) return this.notify("Put the cursor on a name first.", "info");
    this.openSearch({ query: w, word: true, scope: "repo", kind: "code", defsFor: w });
  }

  // Branches and worktrees

  /** The main repo of a worktree, or the repo itself. */
  mainOf(path: string) {
    return this.repos.find((r) => r.path === path)?.parent ?? path;
  }

  openBranches(tab: "branches" | "worktrees" = "branches") {
    if (!this.current) return this.notify("Open a project first.", "info");
    this.branchTab = tab;
    this.overlay = "branches";
  }

  private async afterBranchChange(repo: string, message: string) {
    this.notify(message, "ok");
    const { [repo]: _, ...logs } = this.logs;
    this.logs = logs;
    this.review = null;
    this.view = null;
    await this.reloadRepos();
    this.stamp++;
  }

  async switchBranch(name: string, remote: boolean) {
    const repo = this.current;
    const s = repo ? this.sums[repo] : undefined;
    if (!repo || !s) return false;
    if (s.changes.length) {
      const ok = await this.ask({
        title: `Switch ${baseName(repo)} to ${name}?`,
        body: "Git takes your uncommitted changes along to the other branch when it can. It stops, and changes nothing, when they clash with that branch.",
        warnings: [`${s.changes.length} uncommitted ${s.changes.length === 1 ? "change" : "changes"}. To keep them apart, open ${name} in a new worktree instead.`],
        confirm: "Switch anyway",
      });
      if (!ok) return false;
    }
    try {
      await api.switchBranch(repo, name, remote);
      await this.afterBranchChange(repo, `Switched to ${remote ? name.replace(/^[^/]+\//, "") : name}`);
      return true;
    } catch (e) {
      this.notify(`Could not switch: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  async createBranch(name: string, from: string | null) {
    const repo = this.current;
    if (!repo) return false;
    try {
      await api.createBranch(repo, name, from);
      await this.afterBranchChange(repo, `Made branch ${name} and switched to it`);
      return true;
    } catch (e) {
      this.notify(`Could not make the branch: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  async deleteBranch(name: string, merged: boolean) {
    const repo = this.current;
    if (!repo) return false;
    const ok = await this.ask({
      title: `Delete branch ${name}?`,
      body: merged
        ? "Its commits are already in the base branch, so nothing is lost. The remote branch is not touched."
        : "Git deletes it only if its commits are merged somewhere. If they are not, git refuses and nothing changes. The remote branch is not touched.",
      confirm: "Delete branch",
      danger: true,
    });
    if (!ok) return false;
    try {
      await api.deleteBranch(repo, name);
      this.notify(`Deleted branch ${name}`, "ok");
      return true;
    } catch (e) {
      this.notify(`Git kept ${name}: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  /** Makes a worktree and opens it. `agent` also starts Claude Code in it. */
  async newWorktree(branch: string, isNew: boolean, base: string | null, path: string, agent: boolean) {
    const repo = this.current ? this.mainOf(this.current) : null;
    if (!repo) return false;
    try {
      await api.worktreeAdd(repo, path, branch, isNew, base);
      await this.reloadRepos();
      const made = this.repos.find((r) => r.path.toLowerCase() === path.toLowerCase().replace(/[\\/]+$/, ""));
      if (made) {
        this.select(made.path);
        if (agent) this.openTerminal(made.path, { agent: true });
      }
      this.notify(`Made worktree ${baseName(path)} on ${branch}`, "ok");
      return true;
    } catch (e) {
      this.notify(`Could not make the worktree: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  /** Removes a worktree after one question that says what would be lost. */
  async removeWorktree(path: string) {
    const main = this.mainOf(path);
    if (main === path) return this.notify("This is the main folder of the repo, not a worktree.", "info");
    await this.refresh([path]);
    const s = this.sums[path];
    const broken = !!s?.error;
    const changes = s?.changes.length ?? 0;
    const warnings: string[] = [];
    if (changes) warnings.push(`${changes} uncommitted ${changes === 1 ? "change is" : "changes are"} lost.`);
    if (s?.ahead) warnings.push(`${s.ahead} commits are not pushed. They stay on the branch ${s.branch}, which is not deleted.`);
    if (!s?.upstream && s?.branch && !broken) warnings.push(`The branch ${s.branch} stays, so its commits are kept.`);
    if (broken) warnings.push("Git cannot read this worktree. Tern moves its folder to the Recycle Bin and tells git to forget it.");
    const ok = await this.ask({
      title: `Remove worktree ${baseName(path)}?`,
      body: `The folder ${path} goes away. The branch and its commits stay in ${baseName(main)}.`,
      warnings,
      confirm: changes || broken ? "Remove, and lose uncommitted work" : "Remove worktree",
      danger: true,
    });
    if (!ok) return false;
    const running = this.runs.filter((r) => r.repo === path && r.running);
    for (const r of running) await this.stopRun(r.id);
    for (const sh of this.shells.filter((x) => x.repo === path)) this.closeShellTab(sh.id);
    try {
      const out = await api.worktreeRemove(main, path, changes > 0 || broken);
      if (this.current === path) this.select(main);
      await this.reloadRepos();
      this.notify(firstLine(out) || `Removed worktree ${baseName(path)}`, "ok");
      return true;
    } catch (e) {
      this.notify(`Could not remove the worktree: ${firstLine(String(e))}`, "err");
      return false;
    }
  }

  async pruneWorktrees(path: string) {
    const main = this.mainOf(path);
    try {
      const out = await api.worktreePrune(main);
      await this.reloadRepos();
      this.notify(firstLine(out), "ok");
    } catch (e) {
      this.notify(`Clean up failed: ${firstLine(String(e))}`, "err");
    }
  }

  // Themes

  applyTheme() {
    const light = typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: light)").matches;
    const id = this.theme === "system" ? (light ? "daylight" : "slate") : this.theme;
    if (id === "slate") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = id;
    retheme();
  }

  setTheme(id: string) {
    this.theme = id;
    save("theme", id);
    // Crossfade the whole window instead of every color jumping at once.
    const doc = document as Document & { startViewTransition?: (cb: () => void) => unknown };
    if (doc.startViewTransition) doc.startViewTransition(() => this.applyTheme());
    else this.applyTheme();
  }

  // Back and forward

  private navNow(): Nav {
    return { current: this.current, tab: this.tab, view: this.view, review: this.review, tickets: this.ticketsOpen, data: this.dataOpen };
  }

  private sameNav(a: Nav, b: Nav) {
    const v = (x: View | null) => (x ? `${x.kind}|${x.repo}|${x.rel}|${x.kind === "issue" ? x.number : x.kind === "diff" ? (x.rev?.head ?? "") : ""}` : "");
    return a.tickets === b.tickets && a.data === b.data && a.current === b.current && a.tab === b.tab && v(a.view) === v(b.view) && (a.review?.head ?? "") === (b.review?.head ?? "");
  }

  /** Saves where you are before a move, so Back can return here. */
  remember() {
    if (this.restoring) return;
    const now = this.navNow();
    const last = this.navBack[this.navBack.length - 1];
    if (!last || !this.sameNav(last, now)) {
      this.navBack.push(now);
      if (this.navBack.length > 100) this.navBack.shift();
    }
    this.navFwd = [];
    this.canBack = this.navBack.length > 0;
    this.canForward = false;
  }

  private applyNav(n: Nav) {
    this.restoring = true;
    try {
      if (n.current !== this.current) {
        this.current = n.current;
        save("current", n.current ?? "");
        if (n.current) this.markSeen(n.current);
      }
      this.ticketsOpen = n.tickets;
      this.dataOpen = n.data;
      this.tab = n.tab;
      this.review = n.review;
      this.view = n.view;
      this.selected = [];
      this.editing = false;
      this.lines = n.view?.kind === "file" && n.view.line ? { from: n.view.line, to: n.view.line } : null;
      if (n.current) this.loadTab(n.current);
    } finally {
      this.restoring = false;
    }
  }

  back() {
    // Skip places whose project is gone (hidden, or removed from the folders).
    while (this.navBack.length) {
      const n = this.navBack.pop()!;
      if (n.current && !this.repos.some((r) => r.path === n.current)) continue;
      if (this.sameNav(n, this.navNow())) continue;
      this.navFwd.push(this.navNow());
      this.applyNav(n);
      break;
    }
    this.canBack = this.navBack.length > 0;
    this.canForward = this.navFwd.length > 0;
  }

  forward() {
    const n = this.navFwd.pop();
    if (n) {
      this.navBack.push(this.navNow());
      this.applyNav(n);
    }
    this.canBack = this.navBack.length > 0;
    this.canForward = this.navFwd.length > 0;
  }

  // Many projects at once

  /** Rows the project list shows (worktrees under a folded main repo are left out). */
  private railRows() {
    return this.ordered.filter((r) => !r.parent || this.openTrees.includes(r.parent) || this.showsFolded(r));
  }

  /** Click in the project list: plain opens it, Ctrl adds or removes it, Shift picks a range. */
  clickRepo(e: MouseEvent, path: string) {
    if (e.ctrlKey || e.metaKey) {
      let sel = this.repoSel;
      // The open project joins the first Ctrl+click, like in a file manager.
      if (!sel.length && this.current && this.current !== path) sel = [this.current];
      this.repoSel = sel.includes(path) ? sel.filter((p) => p !== path) : [...sel, path];
      this.repoAnchor = path;
      return;
    }
    if (e.shiftKey) {
      const rows = this.railRows().map((r) => r.path);
      const from = rows.indexOf(this.repoAnchor ?? this.current ?? path);
      const to = rows.indexOf(path);
      if (from >= 0 && to >= 0) {
        this.repoSel = rows.slice(Math.min(from, to), Math.max(from, to) + 1);
        return;
      }
    }
    this.repoSel = [];
    this.repoAnchor = path;
    this.select(path);
  }

  clearRepoSel() {
    this.repoSel = [];
  }

  private async pool(paths: string[], fn: (p: string) => Promise<boolean | undefined>) {
    const failed: string[] = [];
    let next = 0;
    const worker = async () => {
      while (next < paths.length) {
        const p = paths[next++];
        if (!(await fn(p))) failed.push(p);
      }
    };
    await Promise.all(Array.from({ length: Math.min(6, paths.length) }, worker));
    return failed;
  }

  private report(verb: string, total: number, failed: string[]) {
    if (!failed.length) return this.notify(`${verb} ${total} ${total === 1 ? "project" : "projects"}`, "ok");
    const names = failed.map(baseName).join(", ");
    this.notify(`${verb} ${total - failed.length} of ${total}. Failed: ${names}. See each project for the reason.`, "err");
  }

  async bulkFetch(paths: string[]) {
    const withRemote = paths.filter((p) => this.sums[p]?.remote);
    if (!withRemote.length) return this.notify("None of these projects has a remote.", "info");
    this.notify(`Fetching ${withRemote.length} projects…`, "info");
    const failed = await this.pool(withRemote, (p) => this.action(p, "fetch", true));
    this.report("Fetched", withRemote.length, failed);
  }

  /** Pull or push many projects. One question lists every project, and what will happen to each. */
  async bulkSync(paths: string[], action: "pull" | "push") {
    await this.refresh(paths);
    const list: NonNullable<ConfirmRequest["list"]> = [];
    const run: string[] = [];
    for (const p of paths) {
      const s = this.sums[p];
      const name = this.repos.find((r) => r.path === p)?.name ?? baseName(p);
      const branch = s?.branch ?? "detached HEAD";
      if (!s || s.error) {
        list.push({ name, detail: "cannot be read, skipped", tone: "skip" });
      } else if (action === "pull") {
        if (!s.upstream) list.push({ name, detail: `${branch} has no upstream, skipped`, tone: "skip" });
        else {
          run.push(p);
          const notes = [s.behind ? `${s.behind} to pull` : "nothing new as of the last fetch"];
          if (s.ahead) notes.push(`also ${s.ahead} unpushed, a fast-forward pull will fail`);
          if (s.changes.length) notes.push(`${s.changes.length} local changes`);
          list.push({ name, detail: `${branch}: ${notes.join(", ")}`, tone: s.ahead || s.changes.length ? "warn" : undefined });
        }
      } else {
        if (s.upstream && !s.ahead) list.push({ name, detail: `${branch}: nothing to push, skipped`, tone: "skip" });
        else {
          run.push(p);
          const notes = [s.upstream ? `${s.ahead} to push to ${s.upstream}` : `creates origin/${branch}`];
          if (s.behind) notes.push(`remote has ${s.behind} new, the push will be rejected`);
          if (MAIN_BRANCHES.includes(branch.toLowerCase())) notes.push(`straight to ${branch}`);
          list.push({ name, detail: `${branch}: ${notes.join(", ")}`, tone: s.behind || MAIN_BRANCHES.includes(branch.toLowerCase()) ? "warn" : undefined });
        }
      }
    }
    const verb = action === "pull" ? "Pull" : "Push";
    const ok = await this.ask({
      title: `${verb} ${run.length} of ${paths.length} ${paths.length === 1 ? "project" : "projects"}?`,
      body: action === "pull"
        ? "Each pull is fast-forward only. Git fetches first, so newer commits may also come in. Projects that cannot pull are skipped."
        : "Other people can see these commits after the push. Projects with nothing to push are skipped.",
      list,
      listLabel: "Projects",
      confirm: `${verb} ${run.length} ${run.length === 1 ? "project" : "projects"}`,
      allowed: run.length > 0,
    });
    if (!ok) return;
    const failed = await this.pool(run, (p) => this.action(p, action, true, true));
    this.report(action === "pull" ? "Pulled" : "Pushed", run.length, failed);
  }

  async bulkHide(paths: string[]) {
    if (!this.config) return;
    const hidden = [...this.config.hidden, ...paths.filter((p) => !this.config!.hidden.includes(p))];
    this.config = { ...this.config, hidden };
    await api.setConfig($state.snapshot(this.config));
    if (this.current && paths.includes(this.current) && !this.showHidden) this.select(null);
    this.repoSel = [];
    this.notify(`Hid ${paths.length} projects. Use "hidden" at the bottom of the list to see them again.`, "info");
  }

  async bulkUnhide(paths: string[]) {
    if (!this.config) return;
    const keys = new Set(paths.map(slash));
    this.config = { ...this.config, hidden: this.config.hidden.filter((p) => !keys.has(slash(p))) };
    await api.setConfig($state.snapshot(this.config));
    this.repoSel = [];
    this.notify(`${paths.length} projects are back in the list.`, "ok");
    this.refresh(paths);
  }

  async bulkPin(paths: string[]) {
    if (!this.config) return;
    const mains = paths.filter((p) => !this.repos.find((r) => r.path === p)?.parent);
    const allPinned = mains.every((p) => this.config!.pinned.includes(p));
    const pinned = allPinned
      ? this.config.pinned.filter((p) => !mains.includes(p))
      : [...this.config.pinned, ...mains.filter((p) => !this.config!.pinned.includes(p))];
    await this.saveConfig({ ...this.config, pinned });
    this.notify(`${allPinned ? "Unpinned" : "Pinned"} ${mains.length} projects`, "ok");
  }

  async bulkMove(paths: string[], id: string | null) {
    if (!this.config) return;
    const assign = { ...this.config.assign };
    for (const p of paths) {
      if (id) assign[p] = id;
      else delete assign[p];
    }
    this.config = { ...this.config, assign };
    await api.setConfig($state.snapshot(this.config));
    const name = id ? this.spaces.find((x) => x.id === id)?.name : "the spaces their rules pick";
    this.notify(`Moved ${paths.length} projects to ${name}`, "ok");
    this.repoSel = [];
    if (this.current && !this.visible.some((r) => r.path === this.current)) this.select(null);
  }

  async fetchAll() {
    const mains = this.visible.filter((r) => !r.parent && this.sums[r.path]?.remote).map((r) => r.path);
    const where = this.activeSpace ? ` in ${this.activeSpace.name}` : "";
    this.notify(`Fetching ${mains.length} projects${where}…`, "info");
    let failed = 0;
    let next = 0;
    const worker = async () => {
      while (next < mains.length) {
        const repo = mains[next++];
        if (!(await this.action(repo, "fetch", true))) failed++;
      }
    };
    await Promise.all(Array.from({ length: Math.min(6, mains.length) }, worker));
    await this.refresh();
    if (!failed) this.notify(`Fetched ${mains.length} projects`, "ok");
  }

  async open(target: OpenTarget, repoArg?: string) {
    const loose = !repoArg && this.view?.kind === "file" && this.view.loose ? this.view.repo : null;
    const repo = repoArg ?? loose ?? this.current;
    if (!repo) return this.notify("Open a project first.", "info");
    const own = repo === this.current || repo === loose;
    const rel = own && (target === "editor" || target === "explorer") ? this.view?.rel || undefined : undefined;
    try {
      await api.openIn(repo, target, rel, own ? this.lines?.from : undefined);
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    }
  }

  openPrInBrowser() {
    const pr = this.review?.pr;
    if (pr) openUrl(pr.url);
  }

  // Spaces

  async switchSpace(id: string | null) {
    if (!this.config || (this.config.activeSpace ?? null) === id) return;
    this.config = { ...this.config, activeSpace: id };
    api.setConfig($state.snapshot(this.config)).catch((e) => this.notify(`Could not save: ${e}`, "err"));
    const back = saved("current." + (id ?? "all"));
    if (this.current && !this.visible.some((r) => r.path === this.current)) this.select(null);
    if (back && this.visible.some((r) => r.path === back)) this.select(back);
    this.wasWorking.clear();
    this.notify(id ? `Space: ${this.activeSpace?.name}` : "Showing all spaces", "info");
    await this.refresh();
  }

  /** Puts a repo in a space by hand. `null` goes back to the rules. */
  async moveToSpace(path: string, id: string | null) {
    if (!this.config) return;
    const assign = { ...this.config.assign };
    if (id) assign[path] = id;
    else delete assign[path];
    this.config = { ...this.config, assign };
    await api.setConfig($state.snapshot(this.config));
    const name = id ? this.spaces.find((x) => x.id === id)?.name : "the space its rules pick";
    this.notify(`Moved ${baseName(path)} to ${name}`, "ok");
    if (this.current === path && !this.visible.some((r) => r.path === path)) this.select(null);
  }

  // Stage and commit

  private pathsFor(changes: Change[]) {
    return changes.flatMap((c) => (c.oldPath ? [c.path, c.oldPath] : [c.path]));
  }

  async stage(changes: Change[] | "all") {
    const repo = this.current;
    if (!repo) return;
    try {
      await api.stage(repo, changes === "all" ? [] : this.pathsFor(changes));
    } catch (e) {
      this.notify(`Stage failed: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([repo]);
  }

  // Hunks

  toggleHunks(on = !this.hunksOpen) {
    this.hunksOpen = on;
    save("hunks", on ? "1" : "0");
    if (!on) this.hunks = null;
  }

  /** Reads the hunks of the file in the diff view. Only working-tree diffs have hunks to stage. */
  async loadHunks(repo: string, rel: string, untracked: boolean) {
    const same = this.hunks?.repo === repo && this.hunks.rel === rel;
    this.hunks = { repo, rel, loading: !same || !this.hunks?.data, error: null, untracked, data: same ? (this.hunks?.data ?? null) : null };
    if (untracked) return void (this.hunks = { ...this.hunks, loading: false });
    try {
      const data = await api.fileHunks(repo, rel);
      if (this.hunks?.repo === repo && this.hunks.rel === rel) this.hunks = { ...this.hunks, loading: false, data };
    } catch (e) {
      if (this.hunks?.repo === repo && this.hunks.rel === rel) this.hunks = { ...this.hunks, loading: false, error: firstLine(String(e)) };
    }
  }

  async applyHunk(h: Hunk, mode: "stage" | "unstage" | "discard") {
    const cur = this.hunks;
    if (!cur || this.hunkBusy) return;
    if (mode === "discard") {
      const ok = await this.ask({
        title: `Discard this change in ${baseName(cur.rel)}?`,
        body: `${h.added} added and ${h.removed} removed ${h.added + h.removed === 1 ? "line goes" : "lines go"} back to how the last commit has them. The rest of the file stays as it is. You can undo this from the status bar for a few minutes.`,
        confirm: "Discard change",
        danger: true,
      });
      if (!ok) return;
    }
    this.hunkBusy = h.patch;
    try {
      const backup = await api.applyHunk(cur.repo, h.patch, mode);
      if (mode === "discard") {
        this.lastDiscard = { repo: cur.repo, backup, tracked: [cur.rel], untracked: 0, count: 1, at: Date.now() };
        this.notify(`Discarded a change in ${baseName(cur.rel)}`, "ok");
      }
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    } finally {
      this.hunkBusy = null;
    }
    await this.refresh([cur.repo]);
    this.stamp++;
    await this.loadHunks(cur.repo, cur.rel, cur.untracked);
  }

  jumpToLine(line: number) {
    this.diffJump = { line, at: Date.now() };
  }

  // Amend

  /** True when the last commit can be amended: there is one, and it is not pushed. */
  canAmend(repo: string) {
    const s = this.sums[repo];
    return !!s?.last && (!s.upstream || s.ahead > 0);
  }

  setAmend(repo: string, on: boolean) {
    this.amend = { ...this.amend, [repo]: on };
    const last = this.sums[repo]?.last;
    const msg = (this.commitMsg[repo] ?? "").trim();
    if (on && last && !msg) this.commitMsg = { ...this.commitMsg, [repo]: last.body ? `${last.subject}\n\n${last.body}` : last.subject };
    if (!on && last && msg === (last.body ? `${last.subject}\n\n${last.body}` : last.subject).trim()) this.commitMsg = { ...this.commitMsg, [repo]: "" };
  }

  // New pull request

  async openNewPr() {
    const repo = this.current;
    if (!repo) return this.notify("Open a project first.", "info");
    this.prDraft = { loading: true, error: null, data: null };
    this.overlay = "newpr";
    try {
      this.prDraft = { loading: false, error: null, data: await api.prDraft(repo) };
    } catch (e) {
      this.prDraft = { loading: false, error: firstLine(String(e)), data: null };
    }
  }

  async createPr(title: string, body: string, base: string, draft: boolean) {
    const repo = this.current;
    if (!repo || this.prCreating) return;
    this.prCreating = true;
    try {
      const url = await api.prCreate(repo, title, body, base, draft);
      this.overlay = null;
      this.notify(`Opened the pull request: ${url}`, "ok");
      if (url) openUrl(url);
      this.loadPrs(repo, true);
    } catch (e) {
      this.notify(`Could not create the pull request: ${firstLine(String(e))}`, "err");
    } finally {
      this.prCreating = false;
      await this.refresh([repo]);
    }
  }

  // Updates

  /** Asks GitHub Releases for a newer version. Quiet unless `manual`. */
  async checkForUpdate(manual = true) {
    if (this.updateCheck.state === "checking" || this.update?.state === "downloading") return;
    this.updateCheck = { state: "checking", at: Date.now() };
    try {
      const info = await findUpdate();
      if (info) {
        this.update = { info, state: "ready", progress: 0 };
        this.updateCheck = { state: "idle", at: Date.now() };
        if (manual) this.notify(`Tern ${info.version} is available.`, "ok");
      } else {
        this.updateCheck = { state: "current", at: Date.now() };
        if (manual) this.notify(`Tern ${this.version} is the newest version.`, "ok");
      }
    } catch (e) {
      this.updateCheck = { state: "failed", at: Date.now(), error: firstLine(String(e)) };
      if (manual) this.notify(`Could not check for updates: ${firstLine(String(e))}`, "err");
    }
  }

  /** Downloads and installs the update, then restarts Tern. Running tasks and terminals stop. */
  async installUpdate() {
    const u = this.update;
    if (!u || u.state === "downloading") return;
    const running = this.runs.filter((r) => r.running).length + this.shells.filter((s) => s.alive).length;
    if (running) {
      const ok = await this.ask({
        title: `Update to Tern ${u.info.version} now?`,
        body: `Tern restarts to finish the update. ${running} running ${running === 1 ? "task or terminal stops" : "tasks and terminals stop"}.`,
        confirm: "Update and restart",
      });
      if (!ok) return;
    }
    this.update = { ...u, state: "downloading", progress: 0 };
    try {
      await u.info.install((done, total) => {
        if (this.update) this.update = { ...this.update, progress: total ? Math.min(1, done / total) : 0 };
      });
    } catch (e) {
      this.update = { ...u, state: "failed", progress: 0 };
      this.notify(`The update failed: ${firstLine(String(e))}`, "err");
    }
  }

  // Stashes and discarding

  async loadStashes(repo: string) {
    this.stashStamp[repo] = this.sums[repo]?.refsStamp;
    try {
      this.stashes = { ...this.stashes, [repo]: await api.stashList(repo) };
    } catch {
      this.stashes = { ...this.stashes, [repo]: [] };
    }
  }

  /** The changes an action is for: the right-clicked one, or all selected ones when it is part of the selection. */
  changesFor(change: Change): Change[] {
    const all = this.summary?.changes ?? [];
    if (this.selected.length > 1 && this.selected.includes(change.path)) return all.filter((c) => this.selected.includes(c.path));
    return [change];
  }

  /** Stashes changes (new files included): all of them, or the given files. */
  async stashChanges(changes: Change[] | "all") {
    const repo = this.current;
    const s = this.summary;
    if (!repo || !s?.changes.length) return this.notify("There are no changes to stash.", "info");
    const list = changes === "all" ? s.changes : changes;
    const first = baseName(list[0].path);
    const message = list.length === 1 ? first : `${first} and ${list.length - 1} more`;
    try {
      await api.stashPush(repo, message, changes === "all" ? [] : this.pathsFor(list));
      this.selected = [];
      if (this.view?.kind === "diff" || this.view?.kind === "file") this.view = null;
      this.notify(`Stashed ${list.length} ${list.length === 1 ? "file" : "files"}. Find them under Stashes.`, "ok");
    } catch (e) {
      this.notify(`Stash failed: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([repo]);
    await this.loadStashes(repo);
  }

  /** Throws away local changes, after asking. Changed files can be brought back with undoDiscard. */
  async discardChanges(changes: Change[] | "all") {
    const repo = this.current;
    const s = this.summary;
    if (!repo || !s?.changes.length) return this.notify("There are no changes to discard.", "info");
    const list = changes === "all" ? s.changes : changes;
    const fresh = list.filter((c) => c.kind === "untracked");
    const tracked = list.filter((c) => c.kind !== "untracked");
    const n = list.length;
    const ok = await this.ask({
      title: `Discard ${n === 1 ? baseName(list[0].path) : `${n} files`} in ${baseName(repo)}?`,
      body: [
        tracked.length ? `${tracked.length} changed ${tracked.length === 1 ? "file goes" : "files go"} back to the last commit, staged and unstaged edits both.` : "",
        fresh.length ? `${fresh.length} new ${fresh.length === 1 ? "file goes" : "files go"} to the Recycle Bin.` : "",
        "You can undo this from the status bar for a few minutes.",
      ].filter(Boolean).join(" "),
      list: list.slice(0, 40).map((c) => ({ name: c.path, detail: c.kind === "untracked" ? "new file, to the Recycle Bin" : c.kind })),
      listLabel: n > 40 ? `First 40 of ${n} files` : "Files",
      confirm: `Discard ${n === 1 ? "it" : `${n} files`}`,
      danger: true,
    });
    if (!ok) return;
    try {
      const trackedPaths = this.pathsFor(tracked);
      const backup = await api.discard(repo, trackedPaths, fresh.map((c) => c.path));
      this.lastDiscard = { repo, backup, tracked: trackedPaths, untracked: fresh.length, count: n, at: Date.now() };
      this.selected = [];
      if (this.view?.kind === "diff" && list.some((c) => c.path === this.view!.rel)) this.view = null;
      this.notify(`Discarded ${n} ${n === 1 ? "file" : "files"}`, "ok");
    } catch (e) {
      this.notify(`Discard failed: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([repo]);
  }

  async undoDiscard() {
    const d = this.lastDiscard;
    if (!d) return this.notify("There is no discard to undo.", "info");
    this.lastDiscard = null;
    try {
      await api.undoDiscard(d.repo, d.backup, d.tracked);
      const bin = d.untracked ? ` ${d.untracked} new ${d.untracked === 1 ? "file is" : "files are"} in the Recycle Bin; restore ${d.untracked === 1 ? "it" : "them"} from there.` : "";
      this.notify(`Brought back the changed files.${bin}`, "ok");
    } catch (e) {
      this.notify(`Undo failed: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([d.repo]);
  }

  /** Shows what a stash holds, as a review (new files included). */
  async openStash(st: Stash) {
    const repo = this.current;
    if (!repo) return;
    this.reviewLoading = st.sha;
    try {
      const f = await api.stashFiles(repo, st.sha);
      const fresh: Change[] = f.untrackedHead
        ? f.untracked.map((path) => ({ path, oldPath: null, kind: "added", staged: false, unstaged: false, mtime: null, revBase: f.emptyTree, revHead: f.untrackedHead! }))
        : [];
      this.enterReview({ kind: "stash", stash: st, title: st.message, subtitle: `${st.name} on ${st.branch || "a detached HEAD"}`, base: f.base, head: f.head, files: [...f.files, ...fresh] });
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    } finally {
      this.reviewLoading = null;
    }
  }

  async applyStash(st: Stash, pop: boolean) {
    const repo = this.current;
    if (!repo) return;
    try {
      await api.stashApply(repo, st.sha, pop);
      if (this.review?.head === st.sha) this.closeReview();
      this.notify(pop ? `Popped "${st.message}"` : `Applied "${st.message}". The stash is still saved.`, "ok");
    } catch (e) {
      // A conflict leaves the stash in place; git says which files.
      this.notify(`${pop ? "Pop" : "Apply"} stopped: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([repo]);
    await this.loadStashes(repo);
  }

  async dropStash(st: Stash) {
    const repo = this.current;
    if (!repo) return;
    const n = st.files + st.untracked.length;
    const ok = await this.ask({
      title: `Drop the stash "${st.message}"?`,
      body: `It holds ${n} ${n === 1 ? "file" : "files"} from ${st.branch || "a detached HEAD"}. Tern cannot bring a dropped stash back.`,
      confirm: "Drop stash",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.stashDrop(repo, st.sha);
      if (this.review?.head === st.sha) this.closeReview();
      this.notify(`Dropped "${st.message}"`, "ok");
    } catch (e) {
      this.notify(`Drop failed: ${firstLine(String(e))}`, "err");
    }
    await this.loadStashes(repo);
  }

  /** Undoes the last commit when it is not pushed. Its changes come back staged. */
  async undoLastCommit() {
    const repo = this.current;
    const s = this.summary;
    if (!repo || !s?.last) return;
    if (s.upstream && !s.ahead) return this.notify("The last commit is already pushed, so Tern will not undo it.", "info");
    const ok = await this.ask({
      title: `Undo the last commit in ${baseName(repo)}?`,
      body: `"${s.last.subject}" is removed from ${s.branch ?? "this branch"}. Its changes come back as staged changes, so nothing is lost. It is not pushed yet.`,
      code: "git reset --soft HEAD~1",
      confirm: "Undo commit",
    });
    if (!ok) return;
    try {
      await api.undoCommit(repo);
      this.commitMsg = { ...this.commitMsg, [repo]: s.last.body ? `${s.last.subject}\n\n${s.last.body}` : s.last.subject };
      this.notify("Undid the commit. Its changes are staged, and its message is back in the commit box.", "ok");
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
    }
    await this.refresh([repo]);
  }

  async unstage(changes: Change[] | "all") {
    const repo = this.current;
    if (!repo) return;
    try {
      await api.unstage(repo, changes === "all" ? [] : this.pathsFor(changes));
    } catch (e) {
      this.notify(`Unstage failed: ${firstLine(String(e))}`, "err");
    }
    await this.refresh([repo]);
  }

  async commit(push = this.pushAfterCommit) {
    const repo = this.current;
    const s = this.summary;
    if (!repo || !s || this.committing) return false;
    const message = (this.commitMsg[repo] ?? "").trim();
    if (!message) {
      this.notify("Write a commit message first.", "info");
      return false;
    }
    const amend = !!this.amend[repo] && this.canAmend(repo);
    if (!s.changes.length && !amend) {
      this.notify("There is nothing to commit.", "info");
      return false;
    }
    // Amending with nothing staged changes only the message; it does not sweep in every change.
    const stageAll = !amend && !s.changes.some((c) => c.staged);
    if (push) {
      const files = stageAll ? s.changes.length : s.changes.filter((c) => c.staged).length;
      const warnings = [...this.mainWarning(s.branch)];
      if (s.behind) warnings.push(`The remote has ${s.behind} commits you do not have. The commit will work, but the push will be rejected.`);
      const ok = await this.ask({
        title: `Commit and push ${baseName(repo)} / ${s.branch ?? "detached HEAD"}?`,
        body: `Tern commits ${files} ${files === 1 ? "file" : "files"} as "${firstLine(message)}", then pushes to ${s.upstream ?? `a new branch origin/${s.branch}`}.${s.ahead ? ` ${s.ahead} earlier unpushed commits go too.` : ""}`,
        warnings,
        confirm: "Commit and push",
      });
      if (!ok) return false;
    }
    this.committing = true;
    try {
      const out = await api.commit(repo, message, stageAll, push, amend);
      this.commitMsg = { ...this.commitMsg, [repo]: "" };
      this.amend = { ...this.amend, [repo]: false };
      const head = out.split("\n").find((l) => l.startsWith("[")) ?? firstLine(out);
      this.notify(`${amend ? "Amended" : push ? "Committed and pushed" : "Committed"}: ${head}`, "ok");
      const { [repo]: _, ...logs } = this.logs;
      this.logs = logs;
      this.view = null;
      return true;
    } catch (e) {
      this.notify(firstLine(String(e)), "err");
      return false;
    } finally {
      this.committing = false;
      await this.refresh([repo]);
    }
  }

  setPushAfterCommit(on: boolean) {
    this.pushAfterCommit = on;
    save("pushAfterCommit", on ? "1" : "0");
  }

  setSortRecent(on: boolean) {
    this.sortRecent = on;
    save("sortRecent", on ? "1" : "0");
  }

  // Settings

  async saveConfig(next: Config) {
    await api.setConfig(next);
    this.config = next;
    this.startPolling();
    await this.reloadRepos();
  }

  /**
   * Drag and drop in the sidebar. Puts `paths` (main repos) before `before`, or at the end of the group when it is
   * null. Dropped before a project, they take its pin; at the end they keep theirs. `space` (when given) assigns them.
   */
  async placeRepos(paths: string[], before: string | null, space?: string | null) {
    if (!this.config) return;
    const moving = paths.filter((p) => this.repos.some((r) => r.path === p && !r.parent));
    if (!moving.length || (before && moving.includes(before))) return;
    const all = this.sortRepos(this.repos).filter((r) => !r.parent);
    const target = before ? all.find((r) => r.path === before) : null;
    const pin = target ? target.pinned : null;
    const rest = all.filter((r) => !moving.includes(r.path)).map((r) => r.path);
    let i = before ? rest.indexOf(before) : -1;
    if (i < 0) {
      // The end of the group: after the last project of that space (or of everything in one-space view).
      const sp = space !== undefined ? space : this.activeSpace?.id;
      const inGroup = rest.filter((p) => sp === undefined || this.spaceOf(this.repos.find((r) => r.path === p)!) === sp);
      const last = inGroup[inGroup.length - 1];
      i = last ? rest.indexOf(last) + 1 : rest.length;
    }
    rest.splice(i, 0, ...moving);
    const pinned = pin === null ? this.config.pinned : pin ? [...new Set([...this.config.pinned, ...moving])] : this.config.pinned.filter((p) => !moving.includes(p));
    const assign = { ...this.config.assign };
    let moved = "";
    if (space !== undefined) {
      for (const p of moving) {
        const r = this.repos.find((x) => x.path === p)!;
        if (this.spaceOf(r) === space) continue;
        if (space) assign[p] = space;
        else delete assign[p];
        moved = space ? (this.spaces.find((x) => x.id === space)?.name ?? "") : "the space its rules pick";
      }
    }
    await this.saveConfig({ ...this.config, order: rest, pinned, assign });
    const what = moving.length === 1 ? baseName(moving[0]) : `${moving.length} projects`;
    if (moved) this.notify(`Moved ${what} to ${moved}`, "ok");
  }

  /** Alt Shift Up / Down in the sidebar: moves a project one place within its group. */
  async nudgeRepo(path: string, dir: -1 | 1) {
    const g = this.groups.find((x) => x.repos.some((r) => r.path === path));
    if (!g) return;
    const mains = g.repos.filter((r) => !r.parent).map((r) => r.path);
    const i = mains.indexOf(path);
    if (dir < 0 && i > 0) await this.placeRepos([path], mains[i - 1], g.space?.id ?? null);
    else if (dir > 0 && i >= 0 && i < mains.length - 1) await this.placeRepos([path], mains[i + 2] ?? null, g.space?.id ?? null);
  }

  async togglePin(path: string) {
    if (!this.config) return;
    const pinned = this.config.pinned.includes(path)
      ? this.config.pinned.filter((p) => p !== path)
      : [...this.config.pinned, path];
    await this.saveConfig({ ...this.config, pinned });
  }

  async hide(path: string) {
    if (!this.config || this.config.hidden.includes(path)) return;
    this.config = { ...this.config, hidden: [...this.config.hidden, path] };
    await api.setConfig($state.snapshot(this.config));
    if (this.current === path && !this.showHidden) this.select(null);
    this.notify(`Hid ${baseName(path)}. Use "hidden" at the bottom of the list to see it again.`, "info");
  }

  async unhide(path: string) {
    if (!this.config) return;
    const hidden = this.config.hidden.filter((p) => slash(p) !== slash(path));
    this.config = { ...this.config, hidden };
    await api.setConfig($state.snapshot(this.config));
    this.notify(`${baseName(path)} is back in the list.`, "ok");
    this.refresh([path]);
  }

  setShowHidden(on: boolean) {
    this.showHidden = on;
    save("showHidden", on ? "1" : "0");
    if (on) this.refresh(this.repos.filter((r) => this.isHidden(r)).map((r) => r.path));
  }

  async copyRepoPath(path: string) {
    await copyText(path);
    this.notify(`Copied ${path}`, "ok");
  }

  notify(text: string, kind: "ok" | "err" | "info" = "info") {
    clearTimeout(this.toastTimer);
    const id = Date.now() + Math.random();
    this.toast = { text, kind, id };
    this.toastTimer = setTimeout(() => (this.toast = null), kind === "err" ? 7000 : 2600);
    this.notices = [{ id, text, kind, at: Date.now(), repo: this.current }, ...this.notices].slice(0, 200);
    if (kind === "err") this.log("error", text);
  }

  /** Writes a line to Tern's log file (for bug reports). Never throws. */
  log(level: "error" | "warn" | "info", message: string) {
    api.logAppend(level, message).catch(() => {});
  }

  /** Script errors and failed promises go to the log, so "Copy diagnostics" can show them. */
  private catchErrors() {
    window.addEventListener("error", (e) => this.log("error", `script: ${e.message} at ${e.filename}:${e.lineno}`));
    window.addEventListener("unhandledrejection", (e) => this.log("error", `promise: ${String(e.reason?.stack ?? e.reason)}`.slice(0, 2000)));
  }

  /** A report for a bug: versions, this computer, tools, counts, recent errors, and the end of the log. */
  async copyDiagnostics() {
    try {
      const [facts, tail] = await Promise.all([api.diagFacts(), api.logTail(80)]);
      const errors = this.notices.filter((n) => n.kind === "err").slice(0, 15);
      const lines = [
        `Tern ${this.version ?? "?"} diagnostics`,
        "",
        `OS: ${facts.os} (${facts.arch})`,
        `WebView: ${facts.webview}`,
        ...facts.tools.map(([name, v]) => `${name}: ${v.replace(/\s+/g, " ").trim() || "not found"}`),
        `Theme: ${this.theme} · window ${window.innerWidth}x${window.innerHeight} · zoom ${Math.round(devicePixelRatio * 100)}%`,
        `Projects: ${this.repos.length} found, ${this.visible.length} in view, ${this.spaces.length} spaces · refresh every ${this.config?.pollSeconds ?? "?"}s`,
        `Running: ${this.runs.filter((r) => r.running).length} tasks, ${this.shells.length} terminals`,
        "",
        `Recent errors (${errors.length}):`,
        ...(errors.length ? errors.map((n) => `- ${new Date(n.at).toISOString()} ${n.text}`) : ["- none"]),
        "",
        `Log (last ${tail.length} lines of ${facts.logDir || "the log"}):`,
        "```",
        ...tail,
        "```",
      ];
      await copyText(lines.join("\n"));
      this.notify("Copied diagnostics. Paste them into a bug report.", "ok");
    } catch (e) {
      this.notify(`Could not collect diagnostics: ${firstLine(String(e))}`, "err");
    }
  }

  async openLogFolder() {
    try {
      await api.openLogDir();
    } catch (e) {
      this.notify(`Could not open the log folder: ${firstLine(String(e))}`, "err");
    }
  }

  get unreadNotices() {
    return this.notices.filter((n) => n.at > this.noticesSeen && n.kind !== "info").length;
  }

  toggleNotices(open = !this.noticesOpen) {
    this.noticesOpen = open;
    if (open) this.noticesSeen = Date.now();
  }

  clearNotices() {
    this.notices = [];
    this.noticesSeen = Date.now();
  }

  toggleSidebar(open = this.sidebarCollapsed) {
    this.sidebarCollapsed = !open;
    save("sidebar", open ? "1" : "0");
  }

  /** Opens Settings, on one section when given (appearance, spaces, folders, projects, updates, advanced). */
  openSettings(section?: string) {
    try {
      if (section) localStorage.setItem("tern.settingsSection", section);
    } catch {}
    this.overlay = "settings";
  }
}

export const app = new Store();
