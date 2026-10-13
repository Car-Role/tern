// Shapes returned by the Rust commands in src-tauri/src/lib.rs.

export type RepoEntry = { path: string; name: string; parent: string | null; pinned: boolean };

export type ChangeKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "untracked"
  | "conflict"
  | "copied"
  | "typechange";

export type Change = {
  path: string;
  oldPath: string | null;
  kind: ChangeKind;
  staged: boolean;
  unstaged: boolean;
  /** Unix seconds when the file last changed on disk. */
  mtime: number | null;
};

export type Commit = {
  sha: string;
  short: string;
  subject: string;
  author: string;
  time: number;
  refs: string;
  /** Parent commit ids. Two or more means a merge. */
  parents: string[];
  email: string;
  /** Message text after the subject line. */
  body: string;
};

export type Summary = {
  path: string;
  branch: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  changes: Change[];
  last: Commit | null;
  remote: string | null;
  provider: "github" | "azdo" | "other" | "none";
  fetchedAt: number | null;
  /** Unix seconds of the newest change: a changed file saved, or the last commit. */
  activity: number | null;
  /** Changes whenever a branch, tag, or HEAD moves. */
  refsStamp: number | null;
  error: string | null;
};

export type FileText = { text: string | null; binary: boolean; tooLarge: boolean; size: number };
export type FileDiff = { old: FileText; new: FileText };

export type PullRequest = {
  id: number;
  title: string;
  author: string;
  source: string;
  target: string;
  url: string;
  draft: boolean;
  updated: string;
  headSha: string | null;
  review: "approved" | "changes" | "waiting" | "none";
  provider: "github" | "azdo";
};

export type RevFiles = { base: string; head: string; files: Change[] };

export type SearchHit = { path: string; line: number; text: string };
export type TextResult = { hits: SearchHit[]; truncated: boolean };

export type Branch = {
  name: string;
  remote: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
  gone: boolean;
  sha: string;
  time: number;
  author: string;
  subject: string;
  current: boolean;
  worktree: string | null;
  merged: boolean;
};

export type BranchList = { branches: Branch[]; base: string | null };

export type BranchReview = {
  baseRef: string;
  mergeBase: string;
  head: string;
  branch: string | null;
  commits: Commit[];
  files: Change[];
  candidates: string[];
};

export type TaskSource = "npm" | "vscode" | "dotnet" | "gradle" | "cargo" | "make" | "compose" | "custom";

export type Task = {
  id: string;
  name: string;
  source: TaskSource;
  command: string;
  /** Folder relative to the repo, "" for the root. */
  cwd: string;
  detail: string;
};

export type RunInfo = {
  id: number;
  repo: string;
  taskId: string;
  name: string;
  command: string;
  cwd: string;
  /** Unix milliseconds. */
  started: number;
  ended: number | null;
  code: number | null;
  running: boolean;
  stopped: boolean;
};

export type CustomTask = { name: string; command: string; cwd: string };

export type TabId = "changes" | "files" | "prs" | "issues" | "history" | "ci" | "tasks";

export type CiRun = {
  id: number;
  name: string;
  number: string;
  title: string;
  status: "queued" | "running" | "completed";
  result: "succeeded" | "partial" | "failed" | "canceled" | "skipped" | "";
  branch: string;
  sha: string;
  by: string;
  started: string;
  finished: string;
  url: string;
  provider: "azdo" | "github";
};

export type CiStep = {
  name: string;
  kind: "stage" | "job" | "step";
  depth: number;
  status: CiRun["status"];
  result: CiRun["result"];
  started: string;
  finished: string;
  issues: { kind: string; message: string }[];
};

export type Space = {
  id: string;
  name: string;
  color: string;
  remotes: string[];
  folders: string[];
  /** Tabs for repos in this space, in order. Empty means all tabs. */
  tabs: TabId[];
};

export type Label = { name: string; color: string };

export type Issue = {
  number: number;
  title: string;
  author: string;
  state: string;
  labels: Label[];
  assignees: string[];
  comments: number;
  updated: string;
  created: string;
  url: string;
};

export type IssueDetail = { issue: Issue; body: string; comments: { author: string; body: string; created: string }[] };

export type RemoteRepo = {
  name: string;
  /** Azure DevOps project, or GitHub owner. */
  group: string;
  url: string;
  description: string;
  size: number;
  updated: string;
  private: boolean;
  disabled: boolean;
};

export type Config = {
  roots: string[];
  extra: string[];
  hidden: string[];
  pinned: string[];
  scanDepth: number;
  pollSeconds: number;
  spaces: Space[];
  assign: Record<string, string>;
  defaultSpace: string | null;
  activeSpace: string | null;
  tasks: Record<string, CustomTask[]>;
  /** Repo path to the ids of its pinned tasks, in pin order. */
  pinnedTasks: Record<string, string[]>;
};

export type GitAction = "fetch" | "pull" | "push";

/** A ticket assigned to you: an Azure DevOps work item or a GitHub issue. */
export type Ticket = {
  key: string;
  provider: "azdo" | "github";
  /** The Azure DevOps organization, or the GitHub `owner/repo`. */
  source: string;
  number: number;
  title: string;
  state: string;
  /** Task, Bug, User Story, Feature, Issue, ... */
  kind: string;
  project: string;
  area: string;
  iteration: string;
  priority: number | null;
  tags: string[];
  changed: string;
  url: string;
};

export type TicketDetail = {
  /** true: bodies are Markdown (GitHub). false: HTML (Azure DevOps). */
  markdown: boolean;
  sections: { title: string; body: string; markdown: boolean }[];
  links: { kind: "commit" | "pr" | "branch" | "parent" | "child" | "related"; label: string; url: string }[];
  comments: { author: string; created: string; body: string; markdown: boolean }[];
  createdBy: string;
  created: string;
  assignedTo: string;
};

/** The history list, and how the branch stands against its remote branch. */
export type History = {
  commits: Commit[];
  branch: string | null;
  upstream: string | null;
  upstreamSha: string;
  /** Where the branch and its remote branch split. */
  base: string;
  behind: number;
  ahead: number;
  /** Commits a pull brings (up to 5000). */
  incoming: string[];
  /** Commits a push sends (up to 5000). */
  outgoing: string[];
};
export type OpenTarget = "terminal" | "agent" | "editor" | "explorer";
