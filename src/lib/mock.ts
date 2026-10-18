// Fake back end for designing in a plain browser. Never loaded inside the Tauri app.

import type { Change, Commit, Config, PullRequest, RepoEntry, RunInfo, Summary, Task } from "./types";
import { qaBytes, qaFiles, qaReadFile, qaReadTail, qaReadText } from "./mockfiles";
import { mockDb } from "./mockdb";

// Fake task runner: prints dev-server-like output with colors.
type Listeners = { out: (id: number, d: string) => void; exit: (i: RunInfo) => void };
let listeners: Listeners | null = null;
const fakeRuns = new Map<number, { info: RunInfo; timer: ReturnType<typeof setInterval>; out: string }>();
let nextRun = 0;

export function mockTaskEvents(out: Listeners["out"], exit: Listeners["exit"]) {
  listeners = { out, exit };
  return () => (listeners = null);
}

function emit(id: number, d: string) {
  const r = fakeRuns.get(id);
  if (r) r.out += d;
  listeners?.out(id, d);
}

function finish(id: number, code: number, stopped = false) {
  const r = fakeRuns.get(id);
  if (!r || !r.info.running) return;
  clearInterval(r.timer);
  r.info = { ...r.info, running: false, ended: Date.now(), code, stopped };
  listeners?.exit(r.info);
}

function fakeStart(repo: string, task: Task): RunInfo {
  const id = ++nextRun;
  const info: RunInfo = {
    id, repo, taskId: task.id, name: task.name, command: task.command, cwd: task.cwd,
    started: Date.now(), ended: null, code: null, running: true, stopped: false,
  };
  const dev = /dev|start|serve|watch/.test(task.name);
  let step = 0;
  const timer = setInterval(() => {
    step++;
    if (dev) {
      if (step === 1) emit(id, "\x1b[1m\x1b[32m  VITE v8.3.1\x1b[0m  ready in \x1b[1m412\x1b[0m ms\r\n\r\n");
      if (step === 2) emit(id, "  \x1b[32m➜\x1b[0m  \x1b[1mLocal\x1b[0m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[0m\x1b[36m/\x1b[0m\r\n");
      if (step > 3 && step % 4 === 0) emit(id, `\x1b[2m${new Date().toLocaleTimeString()}\x1b[0m \x1b[36m[vite]\x1b[0m \x1b[32mhmr update\x1b[0m /src/App.svelte\r\n`);
    } else {
      if (step < 6) emit(id, `\rBuilding… ${step * 20}%`);
      if (step === 6) emit(id, "\r\x1b[KBuilding… done\r\n\x1b[33mwarning\x1b[0m: unused variable `x`\r\n\x1b[32m✓ built in 1.2s\x1b[0m\r\n");
      if (step === 7) finish(id, task.name.includes("test") ? 1 : 0);
    }
  }, 350);
  fakeRuns.set(id, { info, timer, out: "" });
  emit(id, `\x1b[2m> ${task.command}${task.cwd ? `    (in ${task.cwd})` : ""}\x1b[0m\r\n`);
  return info;
}

const root = "C:\\Users\\you\\Code";
const now = Math.floor(Date.now() / 1000);

const repos: RepoEntry[] = [
  { path: `${root}\\docs-site`, name: "docs-site", parent: null, pinned: true },
  { path: `${root}\\notes-app`, name: "notes-app", parent: null, pinned: false },
  { path: `${root}\\notes-app\\.claude\\worktrees\\fix-sync`, name: "fix-sync", parent: `${root}\\notes-app`, pinned: false },
  { path: `${root}\\Anchor Project`, name: "Anchor Project", parent: null, pinned: false },
  { path: `${root}\\pixel-paint`, name: "pixel-paint", parent: null, pinned: false },
  { path: `${root}\\storefront-web`, name: "storefront-web", parent: null, pinned: false },
  { path: `C:\\Work\\billing-api`, name: "billing-api", parent: null, pinned: false },
];

const files = [
  "README.md",
  "package.json",
  "src/main.ts",
  "src/app/router.ts",
  "src/app/session.ts",
  "src/components/Button.svelte",
  "src/components/Table.svelte",
  "src/lib/fetcher.ts",
  "src/lib/format.ts",
  "src-tauri/src/lib.rs",
  "src-tauri/Cargo.toml",
  "tests/session.test.ts",
  ...qaFiles,
];

let changes: Change[] = [
  { path: "src/app/session.ts", oldPath: null, kind: "modified", staged: false, unstaged: true, mtime: now - 20 },
  { path: "src/lib/fetcher.ts", oldPath: null, kind: "modified", staged: true, unstaged: true, mtime: now - 400 },
  { path: "src/lib/retry.ts", oldPath: null, kind: "untracked", staged: false, unstaged: true, mtime: now - 50 },
  { path: "src/components/Grid.svelte", oldPath: "src/components/Table.svelte", kind: "renamed", staged: true, unstaged: false, mtime: now - 3000 },
  { path: "docs/old-notes.md", oldPath: null, kind: "deleted", staged: false, unstaged: true, mtime: null },
  { path: "samples/screenshot.png", oldPath: null, kind: "modified", staged: false, unstaged: true, mtime: now - 90 },
  { path: "samples/new.png", oldPath: null, kind: "untracked", staged: false, unstaged: true, mtime: now - 95 },
];

const shaOf = (i: number) => (((i + 1) * 2654435761) >>> 0).toString(16).padStart(8, "0").repeat(5).slice(0, 40);

function commit(i: number, subject: string, parents: number[] = [i + 1], refs = "", author = "Ada Park"): Commit {
  const sha = shaOf(i);
  return {
    sha, short: sha.slice(0, 7), subject, author, email: "ada@example.com", time: now - i * 5400, refs,
    parents: parents.filter((p) => p < 12).map(shaOf),
    body: i % 3 === 0 ? "Longer explanation of the change.\n\nRefs #142." : "",
  };
}

// A small history with a feature branch and a merge, for the graph.
const commits = [
  commit(0, "Merge branch 'feat/retry'", [1, 2], "HEAD -> main, origin/main"),
  commit(1, "Fix table sort on empty columns", [3]),
  commit(2, "Retry fetches with backoff", [4], "feat/retry, origin/feat/retry"),
  commit(3, "Add Grid component", [5]),
  commit(4, "Move session refresh into its own module", [5]),
  commit(5, "Bump tauri to 2.4", [6], "tag: v0.4.0"),
  commit(6, "Merge branch 'docs'", [7, 8]),
  commit(7, "Tidy README", [9]),
  commit(8, "Write the setup guide", [9], "docs"),
  commit(9, "Start the project", [10]),
  commit(10, "First commit", []),
];

const stress = (() => {
  try {
    return localStorage.getItem("tern.stressTest") === "1";
  } catch {
    return false;
  }
})();
const bigChanges: Change[] = Array.from({ length: 5000 }, (_, i) => ({
  path: `src/generated/part-${String(Math.floor(i / 100)).padStart(2, "0")}/file-${i}.ts`, oldPath: null,
  kind: i % 7 === 0 ? "untracked" : "modified", staged: i % 5 === 0, unstaged: i % 5 !== 0, mtime: now - i,
}));

function summary(r: RepoEntry, i: number): Summary {
  const dirty = i % 2 === 0;
  const azdo = r.path.startsWith("C:\\Work");
  // Stress test (localStorage "tern.stressTest" = "1"): storefront-web gets 5,000 changed files and 3,000 commits.
  const mine = stress && r.name === "storefront-web" ? bigChanges : dirty ? (i === 0 ? changes : changes.slice(0, Math.max(1, 5 - i))) : [];
  return {
    path: r.path,
    branch: r.parent ? "claude/fix-sync" : i === 3 ? "feature/anchors" : "main",
    upstream: i === 4 ? null : "origin/main",
    ahead: i === 1 ? 2 : 0,
    behind: i === 3 ? 5 : 0,
    changes: mine,
    last: commit(i, commits[i % commits.length].subject),
    remote: azdo
      ? "https://acme@dev.azure.com/acme/Platform/_git/billing-api"
      : i === 4
        ? null
        : "https://github.com/Car-Role/x.git",
    provider: azdo ? "azdo" : i === 4 ? "none" : "github",
    fetchedAt: now - 60 * (i * 7 + 2),
    refsStamp: 1,
    activity: i === 0 ? Math.floor(Date.now() / 1000) - 15 : Math.max(0, ...mine.map((c) => c.mtime ?? 0)) || now - i * 5400,
    error: null,
  };
}

const oldText = `import { fetcher } from "../lib/fetcher";

export type Session = { user: string; expires: number };

export async function loadSession(): Promise<Session | null> {
  const res = await fetcher("/api/session");
  if (!res.ok) return null;
  return res.json();
}

export function isExpired(s: Session): boolean {
  return s.expires < Date.now();
}
`;

const newText = `import { fetcher } from "../lib/fetcher";
import { retry } from "../lib/retry";

export type Session = { user: string; expires: number; scopes: string[] };

export async function loadSession(): Promise<Session | null> {
  const res = await retry(() => fetcher("/api/session"), { attempts: 3 });
  if (!res.ok) return null;
  return res.json();
}

export function isExpired(s: Session, now = Date.now()): boolean {
  return s.expires < now;
}

export function hasScope(s: Session, scope: string): boolean {
  return s.scopes.includes(scope);
}
`;

const prs: PullRequest[] = [
  {
    id: 142, title: "Retry session fetches with backoff", author: "ada", source: "feat/retry", target: "main",
    url: "https://github.com", draft: false, updated: new Date().toISOString(), headSha: "abc", review: "waiting", provider: "github",
  },
  {
    id: 139, title: "Grid component replaces Table", author: "dana", source: "grid", target: "main",
    url: "https://github.com", draft: true, updated: new Date().toISOString(), headSha: "def", review: "changes", provider: "github",
  },
];

let config: Config = {
  roots: [root],
  extra: [],
  hidden: [],
  pinned: [repos[0].path],
  scanDepth: 3,
  pollSeconds: 8,
  spaces: [
    { id: "work", name: "Work", color: "#6fa8ff", remotes: ["acme"], folders: [], tabs: ["changes", "files", "history", "tasks"] },
    { id: "personal", name: "Personal", color: "#f0b54a", remotes: ["github.com/Car-Role"], folders: [], tabs: ["changes", "files", "prs", "issues", "history", "tasks"] },
    { id: "side", name: "Side projects", color: "#5fbf8b", remotes: [], folders: [`${root}\\docs-site`], tabs: [] },
  ],
  assign: {},
  defaultSpace: "personal",
  activeSpace: null,
  tasks: {},
  pinnedTasks: {},
};

const text = (t: string) => ({ text: t, binary: false, tooLarge: false, size: t.length });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function mock(cmd: string, args: Record<string, any>): Promise<unknown> {
  await wait(cmd === "prepare_pr" || cmd === "list_prs" ? 400 : 30);
  if (cmd.startsWith("db_")) return mockDb(cmd, args);
  if (cmd === "diag_facts") return { os: "Microsoft Windows [Version 10.0.26200]", arch: "x86_64", webview: "141.0", tools: [["git", "git version 2.51.0"], ["gh", "gh version 2.80.0"], ["az", "azure-cli 2.77.0"], ["VS Code", "1.105.0"]], logDir: "C:\Users\you\AppData\Local\com.carl.scout\logs" };
  if (cmd === "log_append" || cmd === "open_log_dir") return null;
  if (cmd === "log_tail") return ["2026-10-17 18:04:05 INFO  Tern 0.4.0 started"];
  switch (cmd) {
    case "get_config": return config;
    case "set_config": config = args.config; return null;
    case "config_path": return "C:\\Users\\you\\AppData\\Roaming\\com.carl.scout\\config.json";
    case "list_repos": return repos;
    case "summaries": return (args.paths as string[]).map((p) => summary(repos.find((r) => r.path === p)!, repos.findIndex((r) => r.path === p)));
    case "list_files": return files;
    case "file_bytes": return qaBytes(String(args.rel), args.rev ?? null);
    case "read_text": return qaReadText(String(args.rel));
    case "read_tail": return qaReadTail(String(args.rel), Number(args.offset));
    case "read_file":
      if (String(args.rel).startsWith("samples/") && qaReadFile(String(args.rel))) return qaReadFile(String(args.rel));
      if (String(args.rel).endsWith("fetcher.ts")) {
        // A long file, so jumping to a line can be tested.
        const lines = Array.from({ length: 200 }, (_, i) => (i === 119 ? "export function fetcher(url: string) { return fetch(url); }" : `// filler line ${i + 1}`));
        return text(lines.join("\n"));
      }
      if (String(args.rel).endsWith(".md")) {
        // Markdown with a table whose first column is short, to check that cells never break into letters.
        return text([
          "# Features built so far", "",
          "| ID | Feature | Where |", "| --- | --- | --- |",
          "| F1 | Print preview is temporary: nothing is saved between opens; old saved layouts (`app.print-layout.<siteId>`) are removed on open | print-layout.component.ts, print-layout-store.service.ts |",
          "| F2 | Segmented toggles (Text size, Text position, Fields, Landscape/Portrait) look clickable when not selected | print-layout.component.scss `.pl-segmented` |",
          "| F10 | Delete on the canvas: selection bar trash, right-click \"Delete item\", Delete and Backspace keys | board-canvas.component.* |",
          "", "- [x] A task that is done", "- [ ] A task that is not",
        ].join("\n"));
      }
      return text(args.rel.endsWith("session.ts") ? newText : `// ${args.rel}\n\nexport const value = 1;\n`);
    case "write_file": return null;
    case "worktree_diff":
    case "rev_diff": return { old: text(oldText), new: text(newText) };
    case "commit_files": return { base: "b", head: args.sha, files: changes.slice(0, 3) };
    case "log": {
      if (stress && String(args.repo).endsWith("storefront-web")) {
        const big = Array.from({ length: 3000 }, (_, i) => {
          const sha = shaOf(1000 + i);
          return { sha, short: sha.slice(0, 7), subject: `Change number ${3000 - i}`, author: i % 3 ? "Sam Lee" : "Ada Park", email: "o@example.com",
            time: now - i * 900, refs: i === 0 ? "HEAD -> main, origin/main" : "", parents: i < 2999 ? [shaOf(1001 + i)] : [], body: "" };
        });
        return { commits: big, branch: "main", upstream: "origin/main", upstreamSha: big[0].sha, base: big[0].sha, behind: 0, ahead: 0, incoming: [], outgoing: [] };
      }
      // 15 new commits on origin/main that a pull would bring, above the local main.
      const up = Array.from({ length: 15 }, (_, i) => {
        const sha = shaOf(100 + i);
        return {
          sha, short: sha.slice(0, 7), subject: `Remote work ${15 - i}`, author: i % 2 ? "Sam Lee" : "Ada Park", email: "o@example.com",
          time: now + (15 - i) * 600, refs: i === 0 ? "origin/main, origin/HEAD" : "", parents: [i < 14 ? shaOf(101 + i) : commits[0].sha], body: "",
        };
      });
      const local = commits.map((c) => ({ ...c, refs: c.refs.replace(", origin/main", "") }));
      return {
        commits: [...up, ...local], branch: "main", upstream: "origin/main", upstreamSha: up[0].sha, base: commits[0].sha,
        behind: 15, ahead: 0, incoming: up.map((c) => c.sha), outgoing: [],
      };
    }
    case "range_files": return { base: args.base, head: args.head, files: changes.slice(0, 3) };
    case "worktree_patch":
    case "rev_patch": return "diff --git a/src/app/session.ts b/src/app/session.ts\n+import { retry } from \"../lib/retry\";\n";
    case "git_action": await wait(600); return `${args.action}: ok`;
    case "pending": return args.incoming ? commits.slice(0, 3) : commits.slice(0, 2);
    case "list_issues": return [
      { number: 87, title: "Search results lose their highlight after sorting", author: "jt", state: "open", labels: [{ name: "bug", color: "d73a4a" }], assignees: [], comments: 1, updated: new Date().toISOString(), created: new Date().toISOString(), url: "https://github.com" },
      { number: 73, title: "Draft is lost when clicking back", author: "claudio", state: "open", labels: [{ name: "enhancement", color: "a2eeef" }, { name: "ui", color: "5319e7" }], assignees: ["ada"], comments: 0, updated: new Date(Date.now() - 86400e3 * 20).toISOString(), created: new Date().toISOString(), url: "https://github.com" },
    ];
    case "issue_detail": return {
      issue: { number: args.number, title: "Search results lose their highlight after sorting", author: "jt", state: "open", labels: [{ name: "bug", color: "d73a4a" }], assignees: [], comments: 1, updated: new Date().toISOString(), created: new Date().toISOString(), url: "https://github.com" },
      body: "After I **sort** the results, the matches are no longer highlighted.\n\n1. Search for a word\n2. Sort by date\n\n```\nexpected: highlight\n```\n\n<script>alert('x')</script>",
      comments: [{ author: "cb", body: "The highlight is applied before the sort runs, so it is lost.", created: new Date().toISOString() }],
    };
    case "azdo_projects": return ["Platform", "Billing", "Web"];
    case "azdo_repos": return [
      { name: "invoices-service", group: args.project, url: "https://acme@dev.azure.com/acme/Billing/_git/invoices-service", description: "", size: 4845069, updated: "", private: true, disabled: false },
      { name: "catalog-api", group: args.project, url: "https://acme@dev.azure.com/acme/Billing/_git/catalog-api", description: "", size: 1200000, updated: "", private: true, disabled: false },
    ];
    case "github_repos": return [
      { name: "weather-cli", group: "Car-Role", url: "https://github.com/Car-Role/weather-cli.git", description: "Weather in your terminal", size: 2400000, updated: new Date().toISOString(), private: false, disabled: false },
    ];
    case "clone_repo": return fakeStart(args.parent, { id: "clone:" + args.name, name: "clone " + args.name, source: "custom", command: `git clone --progress ${args.url} ${args.name}`, cwd: "", detail: "" });
    case "search_text": {
      const q = String(args.query).toLowerCase();
      const lines = [
        { path: "src/app/session.ts", line: 6, text: "export async function loadSession(): Promise<Session | null> {" },
        { path: "src/app/session.ts", line: 7, text: "  const res = await retry(() => fetcher(\"/api/session\"), { attempts: 3 });" },
        { path: "src/lib/fetcher.ts", line: 120, text: "export function fetcher(url: string) { return fetch(url); }" },
        { path: "tests/session.test.ts", line: 4, text: "  it(\"loads the session\", async () => { await loadSession(); });" },
      ].filter((h) => h.text.toLowerCase().includes(q));
      return { hits: lines, truncated: false };
    }
    case "search_text_with": {
      const q = String(args.query);
      const all = [
        { path: "src/app/session.ts", line: 6, text: "export async function loadSession(): Promise<Session | null> {" },
        { path: "src/app/router.ts", line: 14, text: "  const s = await loadSession();" },
        { path: "tests/session.test.ts", line: 4, text: "  it(\"loads the session\", async () => { await loadSession(); });" },
        { path: "src/lib/fetcher.ts", line: 120, text: "export function fetcher(url: string) { return fetch(url); }" },
      ];
      const paths: string[] = args.opts?.paths ?? [];
      const word = q.match(/[A-Za-z_$][\w$]*/g)?.find((w) => !["b", "s", "function", "class", "const", "let", "var"].includes(w)) ?? q;
      let hits = all.filter((h) => (args.opts?.perl ? new RegExp(`function\\s+${word}\\b`).test(h.text) : h.text.toLowerCase().includes(q.toLowerCase())));
      if (paths.length) hits = hits.filter((h) => paths.includes(h.path));
      return { hits, truncated: false };
    }
    case "search_commits": return commits.filter((c) => c.subject.toLowerCase().includes(String(args.query).toLowerCase()));
    case "take_opened": return [];
    case "tickets_azdo": {
      const ago = (h: number) => new Date(Date.now() - h * 3600000).toISOString();
      const t = (number: number, title: string, state: string, kind: string, project: string, area: string, priority: number, tags: string[], h: number) => ({
        key: `azdo:${args.org}:${number}`, provider: "azdo", source: args.org, number, title, state, kind, project, area,
        iteration: "2026.40", priority, tags, changed: ago(h), url: "https://dev.azure.com",
      });
      return [
        t(4812, "Sign-in form loses focus after a failed attempt on mobile", "To Do", "Task", "Web", "Accounts", 2, [], 30),
        t(4815, "Point Android builds to the staging server", "To Do", "Task", "Web", "Mobile", 2, [], 31),
        t(4790, "Grid loses its sort when the filter changes", "Active", "Bug", "Billing", "Invoices", 1, ["Regression"], 4),
        t(4655, "Export a report as CSV", "New", "User Story", "Billing", "Reports", 3, ["Reports"], 52),
      ];
    }
    case "tickets_github": return [
      { key: "gh:Car-Role/notes-app#87", provider: "github", source: "Car-Role/notes-app", number: 87, title: "Search results lose their highlight after sorting", state: "Open", kind: "Issue",
        project: "notes-app", area: "", iteration: "", priority: null, tags: ["bug"], changed: new Date(Date.now() - 86400000).toISOString(), url: "https://github.com" },
    ];
    case "ticket_azdo_detail": return {
      markdown: false, createdBy: "Sam Lee", created: new Date(Date.now() - 5 * 86400000).toISOString(), assignedTo: "Ada Park",
      sections: [
        { title: "Description", body: "<div>After a failed sign-in, the password field loses focus on <b>mobile</b>.</div>", markdown: false },
        { title: "Repro steps", body: "<ol><li>Open the sign-in page on a phone</li><li>Enter a wrong password</li><li>See that the field loses focus</li></ol><img src=\"https://dev.azure.com/acme/x/_apis/wit/attachments/1?fileName=sign-in.png\" alt=\"sign-in.png\">", markdown: false },
      ],
      links: [{ kind: "parent", label: "Parent #4800", url: "https://dev.azure.com" }, { kind: "commit", label: "Fixed in Commit 374c66b", url: "" }],
      comments: [{ author: "Sam Lee", created: new Date(Date.now() - 86400000).toISOString(), body: "<div>Seen on Android 14 too.</div>", markdown: false }],
    };
    case "ticket_image": await wait(400); return "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="240"><rect width="480" height="240" rx="12" fill="#2a3240"/><text x="240" y="128" fill="#d6dbe3" font-family="sans-serif" font-size="20" text-anchor="middle">Screenshot</text></svg>');
    case "ticket_github_detail": return { markdown: true, createdBy: "jt", created: new Date().toISOString(), assignedTo: "Car-Role",
      sections: [{ title: "Description", body: "Matches are not **highlighted** after the results are sorted.", markdown: true }], links: [], comments: [] };
    case "os_build": return 0;
    case "run_resize": return null;
    case "ci_runs": {
      const ago = (m: number) => new Date(Date.now() - m * 60000).toISOString();
      return [
        { id: 3, name: "catalog-api", number: "2026.10.2.3", title: "Retry fetches with backoff", status: "running", result: "", branch: "main", sha: "9e3779b", by: "Ada Park", started: ago(2), finished: "", url: "https://dev.azure.com", provider: "azdo" },
        { id: 2, name: "catalog-api", number: "2026.10.2.2", title: "Fix table sort", status: "completed", result: "partial", branch: "main", sha: "3c6ef36", by: "Ada Park", started: ago(90), finished: ago(84), url: "https://dev.azure.com", provider: "azdo" },
        { id: 1, name: "catalog-api", number: "2026.10.1.9", title: "Add Grid component", status: "completed", result: "failed", branch: "feat/grid", sha: "78dde6c", by: "Sam Lee", started: ago(600), finished: ago(593), url: "https://dev.azure.com", provider: "azdo" },
      ];
    }
    case "ci_detail": return [
      { name: "BuildService", kind: "stage", depth: 0, status: "completed", result: "partial", started: "", finished: "", issues: [] },
      { name: "BuildService", kind: "job", depth: 1, status: "completed", result: "partial", started: new Date(Date.now() - 300000).toISOString(), finished: new Date(Date.now() - 60000).toISOString(), issues: [{ kind: "warning", message: "Some tests were skipped." }] },
      { name: "Restore", kind: "step", depth: 2, status: "completed", result: "succeeded", started: "", finished: "", issues: [] },
      { name: "Test", kind: "step", depth: 2, status: "completed", result: "failed", started: "", finished: "", issues: [{ kind: "error", message: "CatalogEntryTests.Rounds_prices failed: expected 1.25, got 1.2" }] },
    ];
    case "list_branches": return {
      base: "origin/main",
      branches: [
        { name: "main", remote: false, upstream: "origin/main", ahead: 0, behind: 2, gone: false, sha: "9e3779b", time: now - 300, author: "Ada Park", subject: "Merge branch 'feat/retry'", current: true, worktree: null, merged: false },
        { name: "claude/fix-sync", remote: false, upstream: null, ahead: 0, behind: 0, gone: false, sha: "3c6ef36", time: now - 3600, author: "Claude", subject: "Fix sync retry timing", current: false, worktree: "C:\\Users\\you\\Code\\notes-app\\.claude\\worktrees\\fix-sync", merged: false },
        { name: "feat/retry", remote: false, upstream: "origin/feat/retry", ahead: 0, behind: 0, gone: false, sha: "daa66d1", time: now - 10800, author: "Ada Park", subject: "Retry fetches with backoff", current: false, worktree: null, merged: true },
        { name: "old/spike", remote: false, upstream: "origin/old/spike", ahead: 0, behind: 0, gone: true, sha: "13c6ef3", time: now - 86400 * 40, author: "Ada Park", subject: "Try a grid layout", current: false, worktree: null, merged: false },
        { name: "origin/main", remote: true, upstream: null, ahead: 0, behind: 0, gone: false, sha: "1a2b3c4", time: now - 200, author: "Dana", subject: "Bump version", current: false, worktree: null, merged: false },
        { name: "origin/feat/export", remote: true, upstream: null, ahead: 0, behind: 0, gone: false, sha: "5d6e7f8", time: now - 7200, author: "Dana", subject: "Export to CSV", current: false, worktree: null, merged: false },
      ],
    };
    case "switch_branch": case "create_branch": case "delete_branch": return `ok: ${args.name}`;
    case "suggest_worktree_path": return `${args.repo}.worktrees\\${String(args.branch).replace(/[^\w.-]+/g, "-")}`;
    case "worktree_add": case "worktree_remove": case "worktree_prune": return "ok";
    case "base_candidates": return ["main", "origin/main"];
    case "branch_review": return {
      baseRef: args.base || "main", mergeBase: commits[3].sha, head: commits[0].sha, branch: "claude/fix-sync",
      commits: commits.slice(0, 3), files: changes.slice(0, 4), candidates: ["main", "origin/main"],
    };
    case "base_diff": return { old: text(oldText), new: text(newText) };
    case "base_patch": return "diff --git a/src/app/session.ts b/src/app/session.ts\n+retry\n";
    case "file_hunks": return {
      binary: false,
      unstaged: [
        { header: "@@ -1,3 +1,4 @@", newStart: 1, added: 1, removed: 0, lines: [' import { fetcher } from "../lib/fetcher";', '+import { retry } from "../lib/retry";', "", " export type Session = { user: string };"], patch: "p1" },
        { header: "@@ -9,4 +10,8 @@ export function isExpired", newStart: 10, added: 4, removed: 1, lines: [" export function isExpired(s: Session): boolean {", "-  return s.expires < Date.now();", "+  return s.expires < now;", "+}", "+", "+export function hasScope(s: Session, scope: string) {", "   return s.scopes.includes(scope);"], patch: "p2" },
      ],
      staged: [{ header: "@@ -5,3 +6,3 @@ export async function loadSession", newStart: 6, added: 1, removed: 1, lines: [" export async function loadSession() {", '-  const res = await fetcher("/api/session");', '+  const res = await retry(() => fetcher("/api/session"));', "   if (!res.ok) return null;"], patch: "p3" }],
    };
    case "apply_hunk": return args.mode === "discard" ? shaOf(51) : "";
    case "pr_draft": return { provider: "github", branch: "feat/retry", bases: ["main", "develop"], title: "Retry session fetches with backoff",
      body: "- Add a retry helper\n- Retry session fetches with backoff", commits: ["Retry session fetches with backoff", "Add a retry helper"], pushed: false };
    case "pr_create": await wait(700); return "https://github.com/Car-Role/x/pull/143";
    case "stash_list": return [
      { name: "stash@{0}", sha: shaOf(40), message: "Half-done retry settings", branch: "main", time: now - 5400, files: 2, untracked: ["src/lib/retry.config.ts"] },
      { name: "stash@{1}", sha: shaOf(41), message: "WIP before switching to feat/retry", branch: "feat/retry", time: now - 86400 * 2, files: 1, untracked: [] },
    ];
    case "stash_files": return { base: commits[1].sha, head: commits[0].sha, files: changes.slice(0, 2).map((c) => ({ ...c, staged: false, unstaged: false })),
      untracked: ["src/lib/retry.config.ts"], untrackedHead: commits[2].sha, emptyTree: "4b825dc642cb6eb9a060e54bf8d69288fbee4904" };
    case "stash_push": case "stash_apply": case "stash_drop": case "undo_discard": return "ok";
    case "discard": return shaOf(50);
    case "undo_commit": return commits[0].sha;
    case "stage":
      changes = changes.map((c) => ((args.paths as string[]).length === 0 || args.paths.includes(c.path) ? { ...c, staged: true, unstaged: false } : c));
      return null;
    case "unstage":
      changes = changes.map((c) => ((args.paths as string[]).length === 0 || args.paths.includes(c.path) ? { ...c, staged: false, unstaged: true } : c));
      return null;
    case "commit": await wait(300); changes = []; return "[main 1a2b3c4] " + String(args.message).split("\n")[0];
    case "list_prs": return prs;
    case "prepare_pr": return { base: "b", head: "h", files: changes.slice(0, 2) };
    case "open_in": return null;
    case "list_tasks": return [
      ...(config.tasks[args.repo] ?? []).map((t, i) => ({ id: `custom:${i}:${t.name}`, name: t.name, source: "custom", command: t.command, cwd: t.cwd, detail: t.command })),
      { id: "vscode::npm: start-develop", name: "npm: start-develop", source: "vscode", command: "npm run start-develop", cwd: "", detail: "npm run start-develop" },
      { id: "npm::dev", name: "dev", source: "npm", command: "npm run dev", cwd: "", detail: "vite" },
      { id: "npm::build", name: "build", source: "npm", command: "npm run build", cwd: "", detail: "vite build" },
      { id: "npm::test", name: "test", source: "npm", command: "npm run test", cwd: "", detail: "vitest run" },
      { id: "npm:cdk:cdk:deploy:dev", name: "cdk:deploy:dev", source: "npm", command: "bun run cdk:deploy:dev", cwd: "cdk", detail: "cdk deploy --all" },
      { id: "dotnet::build App.sln", name: "build App.sln", source: "dotnet", command: "dotnet build App.sln", cwd: "", detail: "" },
      { id: "dotnet::watch Api", name: "watch Api", source: "dotnet", command: "dotnet watch run --project Api/Api.csproj", cwd: "", detail: "Api/Api.csproj" },
    ];
    case "start_task": return fakeStart(args.repo, args.task);
    case "stop_run": emit(args.id, "^C\r\n"); finish(args.id, 1, true); return null;
    case "run_input": emit(args.id, String(args.text)); return null;
    case "run_output": return fakeRuns.get(args.id)?.out ?? "";
    case "list_runs": return [...fakeRuns.values()].map((r) => r.info);
    case "forget_run": fakeRuns.delete(args.id); return null;
  }
  throw new Error(`mock: unknown command ${cmd}`);
}
