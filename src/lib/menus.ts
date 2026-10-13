// Right-click menus. Each function opens the menu for one kind of thing.

import { app, type MenuItem } from "./store.svelte";
import { openUrl } from "./api";
import { absolutePath, baseName, commitUrl, fileUrl, withLines } from "./format";
import type { Change, CiRun, Commit, Issue, PullRequest, Stash, Task } from "./types";

type At = MouseEvent | { x: number; y: number };

const item = (label: string, run: () => void, extra: Partial<Exclude<MenuItem, "sep">> = {}): MenuItem => ({ label, run, ...extra });

export function repoMenu(e: At, path: string) {
  const repo = app.repos.find((r) => r.path === path);
  const s = app.sums[path];
  const hidden = repo ? app.isHidden(repo) : false;
  const selfHidden = repo ? app.isHidden({ ...repo, parent: null }) : false;
  const items: MenuItem[] = [
    item("Open", () => app.select(path)),
    item("Copy path", () => app.copyValue(path)),
    item("Copy as @ mention", () => app.copyValue("@" + path)),
    ...(s?.remote ? [item("Copy remote address", () => app.copyValue(s.remote!))] : []),
    ...(s?.branch ? [item("Copy branch name", () => app.copyValue(s.branch!))] : []),
    "sep",
    item("Fetch", () => app.action(path, "fetch")),
    item(s?.behind ? `Pull ${s.behind}…` : "Pull…", () => app.action(path, "pull")),
    item(s?.ahead ? `Push ${s.ahead}…` : "Push…", () => app.action(path, "push")),
    "sep",
    item("Open a terminal here", () => app.openTerminal(path)),
    item("Start Claude Code here", () => app.openTerminal(path, { agent: true })),
    item("Open an external terminal", () => app.open("terminal", path)),
    item("Open in VS Code", () => app.open("editor", path)),
    item("Show in File Explorer", () => app.open("explorer", path)),
    "sep",
    item("Branches…", () => {
      app.select(path);
      app.openBranches("branches");
    }),
    item("Worktrees…", () => {
      app.select(path);
      app.openBranches("worktrees");
    }),
    ...(repo?.parent ? [item("Remove this worktree…", () => app.removeWorktree(path), { danger: true })] : []),
    ...(app.canReviewBranch(path)
      ? [
          item("Review everything since main", async () => {
            app.select(path);
            await app.openBranchReview();
          }),
        ]
      : []),
    ...(repo && !repo.parent ? [item(repo.pinned ? "Unpin" : "Pin to top", () => app.togglePin(path))] : []),
    item("Move to a space…", () => {
      app.moveTargets = [path];
      app.overlay = "move";
    }),
    hidden && !selfHidden
      ? item("Unhide its main repo", () => repo?.parent && app.unhide(repo.parent))
      : hidden
        ? item("Unhide", () => app.unhide(path))
        : item("Hide", () => app.hide(path), { danger: true }),
  ];
  app.showMenu(e, repo?.name ?? baseName(path), items);
}

/** The menu for several projects picked with Ctrl+click or Shift+click. */
export function bulkMenu(e: At, paths: string[]) {
  const n = paths.length;
  const anyHidden = paths.some((p) => {
    const r = app.repos.find((x) => x.path === p);
    return r ? app.isHidden({ ...r, parent: null }) : false;
  });
  const items: MenuItem[] = [
    item(`Fetch ${n}`, () => app.bulkFetch(paths)),
    item(`Pull ${n}…`, () => app.bulkSync(paths, "pull")),
    item(`Push ${n}…`, () => app.bulkSync(paths, "push")),
    "sep",
    item("Copy their paths", () => app.copyValue(paths.join("\n"), `${n} paths`)),
    item("Copy as @ mentions", () => app.copyValue(paths.map((p) => "@" + p).join(" "), `${n} @ mentions`)),
    item("Open a terminal in each", () => paths.slice(0, 8).forEach((p) => app.openTerminal(p))),
    "sep",
    item("Pin or unpin", () => app.bulkPin(paths)),
    item("Move to a space…", () => {
      app.moveTargets = [...paths];
      app.overlay = "move";
    }),
    anyHidden ? item("Unhide", () => app.bulkUnhide(paths)) : item(`Hide ${n}`, () => app.bulkHide(paths), { danger: true }),
    "sep",
    item("Clear the selection", () => app.clearRepoSel()),
  ];
  app.showMenu(e, `${n} projects`, items);
}

/** A file in the Changes, Files, or review list. `change` is set for changed files. */
export function fileMenu(e: At, rel: string, change?: Change) {
  const repo = app.current;
  if (!repo) return;
  const s = app.sums[repo];
  const inReview = !!app.review;
  const lines = app.view?.rel === rel ? app.lines : null;
  const web = s?.remote && s.branch && !inReview ? fileUrl(s.remote, s.branch, rel) : null;
  const items: MenuItem[] = [
    item("Copy relative path", () => app.copyValue(rel)),
    item("Copy absolute path", () => app.copyValue(absolutePath(repo, rel))),
    item("Copy as @ mention", () => app.copyValue("@" + rel)),
    ...(lines ? [item(`Copy ${withLines(rel, lines)}`, () => app.copyValue(withLines(rel, lines)))] : []),
    ...(change
      ? [
          item("Copy diff", async () => {
            const prev = app.selected;
            app.selected = [rel];
            await app.copy("diff");
            app.selected = prev;
          }),
        ]
      : []),
    "sep",
    ...(change && change.kind !== "deleted" ? [item("Show the whole file", () => app.openFile(rel))] : []),
    ...(change ? [item("Show the diff", () => app.openChange(change))] : []),
    ...(change && !inReview
      ? (() => {
          const group = app.changesFor(change);
          const n = group.length > 1 ? ` ${group.length} files` : "";
          return [
            change.staged && !change.unstaged
              ? item(`Unstage${n}`, () => app.unstage(group))
              : item(`Stage${n}`, () => app.stage(group)),
            item(`Stash${n || " this file"}`, () => app.stashChanges(group)),
            item(`Discard${n ? n : " changes"}…`, () => app.discardChanges(group), { danger: true }),
          ];
        })()
      : []),
    item("Open in VS Code", () => {
      app.openFile(rel);
      app.open("editor");
    }),
    item("Show in File Explorer", () => {
      app.openFile(rel);
      app.open("explorer");
    }),
    ...(web ? [item("Open on the web", () => openUrl(web))] : []),
  ];
  app.showMenu(e, baseName(rel), items);
}

export function commitMenu(e: At, c: Commit) {
  const repo = app.current;
  const s = repo ? app.sums[repo] : undefined;
  const web = commitUrl(s?.remote, c.sha);
  const items: MenuItem[] = [
    item("Show the files it changed", () => app.openCommit(c)),
    ...(s?.last?.sha === c.sha && (!s.upstream || s.ahead > 0)
      ? [item("Undo this commit (keep its changes)…", () => app.undoLastCommit())]
      : []),
    "sep",
    item(`Copy commit ID ${c.short}`, () => app.copyValue(c.short)),
    item("Copy the full commit ID", () => app.copyValue(c.sha)),
    item("Copy the message", () => app.copyValue(c.body ? `${c.subject}\n\n${c.body}` : c.subject, "the commit message")),
    item("Copy for an agent", () =>
      app.copyValue(`Commit ${c.sha} in ${repo}\nAuthor: ${c.author} <${c.email}>\n\n${c.subject}${c.body ? "\n\n" + c.body : ""}\n\nSee it with: git show ${c.short}`, `commit ${c.short} for an agent`),
    ),
    ...(web ? [item("Open on the web", () => openUrl(web))] : []),
  ];
  app.showMenu(e, `${c.short} ${c.subject}`, items);
}

export function prMenu(e: At, pr: PullRequest) {
  const items: MenuItem[] = [
    item("Review the changes", () => app.openPr(pr)),
    item("Open in the browser", () => openUrl(pr.url)),
    "sep",
    item("Copy link", () => app.copyValue(pr.url)),
    item(`Copy #${pr.id}`, () => app.copyValue(`#${pr.id}`)),
    item(`Copy branch ${pr.source}`, () => app.copyValue(pr.source)),
    item("Copy for an agent", () =>
      app.copyValue(`Pull request #${pr.id}: ${pr.title}\n${pr.url}\nBranch ${pr.source} into ${pr.target}, by ${pr.author}.`, `#${pr.id} for an agent`),
    ),
  ];
  app.showMenu(e, `#${pr.id} ${pr.title}`, items);
}

export function ciMenu(e: At, run: CiRun) {
  const items: MenuItem[] = [
    item("Show stages and jobs", () => app.openRun(run)),
    item("Open in the browser", () => openUrl(run.url)),
    "sep",
    item("Copy link", () => app.copyValue(run.url)),
    item(`Copy run number ${run.number}`, () => app.copyValue(run.number)),
    ...(run.sha ? [item(`Copy commit ID ${run.sha.slice(0, 7)}`, () => app.copyValue(run.sha.slice(0, 7)))] : []),
  ];
  app.showMenu(e, `${run.name} #${run.number}`, items);
}

export function stashMenu(e: At, st: Stash) {
  const items: MenuItem[] = [
    item("Show what it holds", () => app.openStash(st)),
    "sep",
    item("Pop (apply and remove)", () => app.applyStash(st, true)),
    item("Apply (keep the stash)", () => app.applyStash(st, false)),
    "sep",
    item(`Copy its name ${st.name}`, () => app.copyValue(st.name)),
    item("Copy the message", () => app.copyValue(st.message)),
    "sep",
    item("Drop…", () => app.dropStash(st), { danger: true }),
  ];
  app.showMenu(e, st.message, items);
}

/** The ⋯ menu on the Staged and Changes headers. */
export function sectionMenu(e: At, section: "staged" | "unstaged") {
  const all = app.summary?.changes ?? [];
  const group = section === "staged" ? all.filter((c) => c.staged) : all.filter((c) => c.unstaged);
  const label = section === "staged" ? "staged" : "unstaged";
  const items: MenuItem[] = [
    item(`Stash all changes (${all.length})`, () => app.stashChanges("all")),
    item(`Stash the ${label} files (${group.length})`, () => app.stashChanges(group)),
    "sep",
    item(`Discard the ${label} files (${group.length})…`, () => app.discardChanges(group), { danger: true }),
    item(`Discard all changes (${all.length})…`, () => app.discardChanges("all"), { danger: true }),
  ];
  app.showMenu(e, section === "staged" ? "Staged" : "Changes", items);
}

export function issueMenu(e: At, issue: Issue) {
  const items: MenuItem[] = [
    item("Read it here", () => app.openIssue(issue.number)),
    item("Open in the browser", () => openUrl(issue.url)),
    "sep",
    item("Copy link", () => app.copyValue(issue.url)),
    item(`Copy #${issue.number}`, () => app.copyValue(`#${issue.number}`)),
    item("Copy the title", () => app.copyValue(issue.title)),
    item("Copy for an agent", () => copyIssueForAgent(issue.number)),
  ];
  app.showMenu(e, `#${issue.number} ${issue.title}`, items);
}

export async function copyIssueForAgent(number: number) {
  const repo = app.current;
  if (!repo) return;
  try {
    const { api } = await import("./api");
    const d = await api.issueDetail(repo, number);
    const labels = d.issue.labels.map((l) => l.name).join(", ");
    const comments = d.comments.map((c) => `--- ${c.author} (${c.created.slice(0, 10)}):\n${c.body}`).join("\n\n");
    const text = [
      `GitHub issue #${d.issue.number}: ${d.issue.title}`,
      d.issue.url,
      `State: ${d.issue.state}. Opened by ${d.issue.author}.${labels ? ` Labels: ${labels}.` : ""}`,
      "",
      d.body || "(no description)",
      ...(comments ? ["", "Comments:", comments] : []),
    ].join("\n");
    await app.copyValue(text, `issue #${number} for an agent`);
  } catch (err) {
    app.notify(`Could not read the issue: ${err}`, "err");
  }
}

export function taskMenu(e: At, t: Task, custom: number | null) {
  const repo = app.current;
  if (!repo) return;
  const last = app.lastRun(repo, t.id);
  const items: MenuItem[] = [
    item(last?.running ? "Show the output" : "Run", () => (last?.running ? app.showRun(last.id) : app.runTask(t))),
    ...(last ? [item("Restart", () => app.restartRun(last.id))] : []),
    ...(last?.running ? [item("Stop", () => app.stopRun(last.id), { danger: true })] : []),
    item(app.isTaskPinned(repo, t.id) ? "Unpin" : "Pin to the top", () => app.toggleTaskPin(repo, t), { hint: "P" }),
    "sep",
    item("Copy the command", () => app.copyValue(t.command)),
    ...(last ? [item("Copy the last output for an agent", () => app.copyRunOutput(last.id))] : []),
    ...(t.source !== "custom" ? [item("Save a copy as my own command", () => app.saveCustomTask(repo, t.name, t.command, t.cwd))] : []),
    ...(custom !== null ? [item("Remove this saved command", () => app.removeCustomTask(repo, custom), { danger: true })] : []),
  ];
  app.showMenu(e, t.name, items);
}

/** Right-click on code: copy the selection or the place in the file. */
export function codeMenu(e: At, selection: string, word = "") {
  const repo = app.current;
  const v = app.view;
  if (!repo || !v || v.kind === "issue") return;
  const at = withLines(v.rel, app.lines ?? { from: 1, to: 1 });
  const name = word.match(/[A-Za-z_$][\w$]*/)?.[0] ?? "";
  const items: MenuItem[] = [
    ...(name
      ? [
          item(`Go to definition of ${name}`, () => app.goToDefinition(name), { hint: "F12" }),
          item(`Find where ${name} is used`, () => app.findUsages(name), { hint: "Shift F12" }),
          item(`Search for ${name}`, () => app.openSearch({ query: name, scope: "repo" })),
          "sep" as const,
        ]
      : []),
    item("Copy", () => app.copyValue(selection, "the selection"), { disabled: !selection }),
    item(`Copy ${at}`, () => app.copyValue(at)),
    item(`Copy @${v.rel}`, () => app.copyValue("@" + v.rel)),
    item("Copy the selection with its place", () => app.copyValue(`${at}\n\`\`\`\n${selection}\n\`\`\``, `${at} with the code`), { disabled: !selection }),
    "sep",
    item("Open in VS Code at this line", () => app.open("editor")),
  ];
  app.showMenu(e, at, items);
}
