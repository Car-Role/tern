<script lang="ts">
  import { tick, untrack } from "svelte";
  import { app, WORKING_SECONDS, type Tab } from "../lib/store.svelte";
  import { ago, agoIso, baseName, dirName, duration, kindLetter } from "../lib/format";
  import { rank } from "../lib/fuzzy";
  import { hover, type HoverData } from "../lib/hover";
  import { ciMenu, commitMenu, fileMenu, issueMenu, prMenu, repoMenu, sectionMenu, stashMenu, taskMenu } from "../lib/menus";
  import { FOLD, foldIncoming, layoutGraph, parseRefs, type HistItem } from "../lib/graph";
  import GraphCell from "./GraphCell.svelte";
  import { popOnDone } from "../lib/motion";
  import type { Change, CiRun, Commit, Issue, PullRequest, Stash, Task, TaskSource } from "../lib/types";

  type Row =
    | { type: "header"; id: string; label: string; count: number; section?: "staged" | "unstaged" | "stashes" }
    | { type: "stash"; id: string; stash: Stash }
    | { type: "task"; id: string; task: Task; custom: number | null; pinned: boolean }
    | { type: "change"; id: string; change: Change; section: "staged" | "unstaged" | null }
    | { type: "dir"; id: string; path: string; name: string; depth: number; open: boolean }
    | { type: "file"; id: string; path: string; name: string; depth: number }
    | { type: "pr"; id: string; pr: PullRequest }
    | { type: "commit"; id: string; commit: Commit; flow: "in" | "out" | null }
    | { type: "fold"; id: string; count: number }
    | { type: "issue"; id: string; issue: Issue }
    | { type: "run"; id: string; run: CiRun };

  type Node = { dirs: Map<string, Node>; files: string[] };

  let listEl: HTMLDivElement | undefined = $state();
  let cursor = $state(0);
  let anchor = $state(0);
  /** One filter per tab (and one for the files of a review), kept while the pane lives. */
  let filters = $state<Record<string, string>>({});
  let filterEl: HTMLInputElement | undefined = $state();
  let expanded = $state<Record<string, string[]>>({});
  let commitBox: HTMLTextAreaElement | undefined = $state();
  /** True while the user moves through the list with keys. The cursor ring shows only then (and while the list has focus). */
  let kbd = $state(false);
  let command = $state("");

  const sourceLabel: Record<TaskSource, string> = {
    custom: "Saved commands",
    vscode: "VS Code tasks",
    npm: "npm scripts",
    dotnet: ".NET",
    gradle: "Gradle",
    cargo: "Cargo",
    make: "Make",
    compose: "Docker Compose",
  };

  const repo = $derived(app.current!);
  const s = $derived(app.summary);
  const reviewing = $derived(
    !!app.review && (app.tab === "prs" || app.tab === "history" || ((app.review.kind === "branch" || app.review.kind === "stash") && app.tab === "changes")),
  );
  let showCommits = $state(false);

  const filterKey = $derived(reviewing ? "review" : app.tab);
  const filter = $derived(filters[filterKey] ?? "");
  const setFilter = (v: string) => (filters = { ...filters, [filterKey]: v });
  /** The filter as lowercase words. Every word must appear in a row's text. */
  const words = $derived(filter.trim().toLowerCase().split(/\s+/).filter(Boolean));
  const filterHint: Record<Tab, string> = {
    changes: "Filter changes", files: "Filter files", prs: "Filter pull requests", issues: "Filter issues",
    history: "Filter commits", ci: "Filter runs", tasks: "Filter tasks",
  };

  // The file tree is built once per file list; only open folders are turned into rows.
  const tree = $derived.by(() => {
    const files = app.files[repo];
    const root: Node = { dirs: new Map(), files: [] };
    if (!files) return root;
    for (const f of files) {
      const parts = f.split("/");
      let node = root;
      for (let i = 0; i < parts.length - 1; i++) {
        let next = node.dirs.get(parts[i]);
        if (!next) node.dirs.set(parts[i], (next = { dirs: new Map(), files: [] }));
        node = next;
      }
      node.files.push(parts[parts.length - 1]);
    }
    return root;
  });

  // Every row before the filter. Files filter here (fuzzy); the other tabs filter in `rows` below.
  const baseRows = $derived.by((): Row[] => {
    if (reviewing) {
      return app.review!.files.map((c) => ({ type: "change", id: c.path, change: c, section: null }));
    }
    if (app.tab === "changes") {
      let list = s?.changes ?? [];
      if (app.sortRecent) list = [...list].sort((a, b) => (b.mtime ?? 0) - (a.mtime ?? 0));
      const staged = list.filter((c) => c.staged);
      const unstaged = list.filter((c) => c.unstaged);
      const out: Row[] = [];
      if (staged.length) {
        out.push({ type: "header", id: "h:staged", label: "Staged", count: staged.length, section: "staged" });
        for (const c of staged) out.push({ type: "change", id: "s:" + c.path, change: c, section: "staged" });
      }
      if (unstaged.length) {
        out.push({ type: "header", id: "h:unstaged", label: "Changes", count: unstaged.length, section: "unstaged" });
        for (const c of unstaged) out.push({ type: "change", id: "u:" + c.path, change: c, section: "unstaged" });
      }
      const stashes = app.stashes[repo] ?? [];
      if (stashes.length) {
        out.push({ type: "header", id: "h:stashes", label: "Stashes", count: stashes.length, section: "stashes" });
        for (const st of stashes) out.push({ type: "stash", id: "st:" + st.sha, stash: st });
      }
      return out;
    }
    if (app.tab === "files") {
      const files = app.files[repo] ?? [];
      if (filter.trim()) {
        return rank(files, filter.trim(), (f) => f, 200).map(({ item }) => ({
          type: "file", id: item, path: item, name: item, depth: 0,
        }));
      }
      const open = new Set(expanded[repo] ?? []);
      const out: Row[] = [];
      const walk = (node: Node, prefix: string, depth: number) => {
        const dirs = [...node.dirs.keys()].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
        for (const d of dirs) {
          const path = prefix + d;
          const isOpen = open.has(path);
          out.push({ type: "dir", id: path + "/", path, name: d, depth, open: isOpen });
          if (isOpen) walk(node.dirs.get(d)!, path + "/", depth + 1);
        }
        const files = [...node.files].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
        for (const f of files) out.push({ type: "file", id: prefix + f, path: prefix + f, name: f, depth });
      };
      walk(tree, "", 0);
      return out;
    }
    if (app.tab === "tasks") {
      const list = app.tasks[repo] ?? [];
      const out: Row[] = [];
      // Saved commands are removed by their place in the saved list.
      const customAt = new Map(list.filter((t) => t.source === "custom").map((t, i) => [t.id, i]));
      const row = (t: Task, pinned: boolean): Row => ({ type: "task", id: t.id, task: t, custom: customAt.get(t.id) ?? null, pinned });
      // Pinned tasks come first, in the order they were pinned.
      const pins = (app.config?.pinnedTasks?.[repo] ?? []).map((id) => list.find((t) => t.id === id)).filter((t): t is Task => !!t);
      if (pins.length) {
        out.push({ type: "header", id: "h:pinned", label: "Pinned", count: pins.length });
        for (const t of pins) out.push(row(t, true));
      }
      const groups = new Map<string, Task[]>();
      for (const t of list) {
        if (pins.includes(t)) continue;
        const key = taskGroup(t);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key)!.push(t);
      }
      for (const [label, tasks] of groups) {
        out.push({ type: "header", id: "h:" + label, label, count: tasks.length });
        for (const t of tasks) out.push(row(t, false));
      }
      return out;
    }
    if (app.tab === "prs") return (app.prs[repo]?.items ?? []).map((pr) => ({ type: "pr", id: "#" + pr.id, pr }));
    if (app.tab === "ci") return (app.ci[repo]?.items ?? []).map((run) => ({ type: "run", id: "r" + run.id, run }));
    if (app.tab === "issues") return (app.issues[repo]?.items ?? []).map((issue) => ({ type: "issue", id: "i" + issue.number, issue }));
    // While filtering, every loaded commit can match, so the incoming ones are not folded away.
    return (words.length ? (app.logs[repo] ?? []) : hist).map((c: HistItem): Row =>
      c.fold
        ? { type: "fold", id: FOLD, count: c.fold }
        : { type: "commit", id: c.sha, commit: c, flow: incoming.has(c.sha) ? "in" : outgoing.has(c.sha) ? "out" : null },
    );
  });

  /** The text a row shows, which the filter matches. */
  function rowText(r: Row): string {
    switch (r.type) {
      case "change": return `${r.change.path} ${r.change.oldPath ?? ""}`;
      case "file":
      case "dir": return r.path;
      case "stash": return `${r.stash.message} ${r.stash.branch} ${r.stash.name}`;
      case "task": return `${r.task.name} ${r.task.command} ${r.task.detail}`;
      case "pr": return `#${r.pr.id} ${r.pr.title} ${r.pr.author} ${r.pr.source} ${r.pr.target}`;
      case "issue": return `#${r.issue.number} ${r.issue.title} ${r.issue.labels.map((l) => l.name).join(" ")}`;
      case "run": return `${r.run.name} ${r.run.title} ${r.run.branch} #${r.run.number}`;
      case "commit": return `${r.commit.subject} ${r.commit.author} ${r.commit.sha}`;
      default: return "";
    }
  }

  const rows = $derived.by((): Row[] => {
    if ((app.tab === "files" && !reviewing) || !words.length) return baseRows;
    const out: Row[] = [];
    // Section headers stay only when something under them matches, and count only what matches.
    let head = -1;
    const dropEmptyHead = () => {
      if (head >= 0 && head === out.length - 1) out.pop();
    };
    for (const r of baseRows) {
      if (r.type === "header") {
        dropEmptyHead();
        head = out.length;
        out.push({ ...r, count: 0 });
      } else if (r.type !== "fold") {
        const text = rowText(r).toLowerCase();
        if (!words.every((w) => text.includes(w))) continue;
        out.push(r);
        const h = out[head];
        if (h?.type === "header") h.count++;
      }
    }
    dropEmptyHead();
    return out;
  });
  /** The filter hides every row, though the tab has some. */
  const noMatch = $derived(
    !!filter.trim() && !rows.length && (app.tab === "files" && !reviewing ? (app.files[repo]?.length ?? 0) > 0 : baseRows.length > 0),
  );

  // Commits a pull would bring, and commits a push would send.
  const sync = $derived(app.logSync[repo]);
  const incoming = $derived(new Set(sync?.incoming ?? []));
  const outgoing = $derived(new Set(sync?.outgoing ?? []));
  /** Repos where the user opened the fold of incoming commits. */
  let unfolded = $state<Record<string, boolean>>({});
  /** Commits just shown by opening the fold. They fade in once. */
  let revealed = $state(new Set<string>());
  function unfold() {
    const shown = new Set(hist.map((c) => c.sha));
    revealed = new Set((app.logs[repo] ?? []).filter((c) => !shown.has(c.sha)).map((c) => c.sha));
    unfolded = { ...unfolded, [repo]: true };
    setTimeout(() => (revealed = new Set()), 400);
  }
  const hist: HistItem[] = $derived(
    unfolded[repo] ? (app.logs[repo] ?? []) : foldIncoming(app.logs[repo] ?? [], incoming, sync?.base ?? ""),
  );
  /** Incoming commits that are not in the loaded list (more than 300). */
  const unloaded = $derived(Math.max(0, (sync?.behind ?? 0) - (app.logs[repo] ?? []).filter((c) => incoming.has(c.sha)).length));

  const taskGroup = (t: Task) => (t.source === "npm" && t.cwd ? `npm scripts in ${t.cwd}/` : sourceLabel[t.source]);

  // The graph lines up 1:1 with the history rows.
  // No graph while filtering: the matching commits are not a connected history.
  const graph = $derived(app.tab === "history" && !reviewing && !words.length ? layoutGraph(hist) : null);
  // Keep these in sync with --row2 and --row-fold in the style below.
  const COMMIT_ROW = 42;
  const FOLD_ROW = 30;

  const kindText: Record<string, string> = {
    added: "Added", modified: "Changed", deleted: "Deleted", renamed: "Renamed", untracked: "New file, not tracked yet",
    conflict: "Merge conflict", copied: "Copied", typechange: "Type changed",
  };

  function changeCard(c: Change): HoverData {
    const rows: NonNullable<HoverData["rows"]> = [{ label: "Change", value: kindText[c.kind] ?? c.kind, tone: c.kind === "deleted" || c.kind === "conflict" ? "del" : c.kind === "modified" ? "amber" : "add" }];
    if (c.oldPath) rows.push({ label: "Was", value: c.oldPath, mono: true });
    if (!app.review) rows.push({ label: "State", value: c.staged && c.unstaged ? "Partly staged" : c.staged ? "Staged" : "Not staged" });
    if (c.mtime) rows.push({ label: "Saved", value: `${ago(c.mtime)} (${new Date(c.mtime * 1000).toLocaleString()})`, tone: isHot(c) ? "add" : undefined });
    return { title: c.path, rows, hint: app.review ? "Right-click to copy the path or the diff" : "S stages it. Right-click to copy the path, @ mention, or diff." };
  }

  function commitCard(c: Commit): HoverData {
    const refs = parseRefs(c.refs).map((r) => r.name).join(", ");
    const rows: NonNullable<HoverData["rows"]> = [
      { label: "Commit", value: c.sha, mono: true },
      { label: "Author", value: c.email ? `${c.author} <${c.email}>` : c.author },
      { label: "Date", value: `${new Date(c.time * 1000).toLocaleString()} (${ago(c.time)})` },
    ];
    if (c.parents.length > 1) rows.push({ label: "Merge of", value: c.parents.map((x) => x.slice(0, 7)).join(" and "), mono: true });
    if (refs) rows.push({ label: "Points here", value: refs, mono: true });
    return { title: c.subject, rows, text: c.body || undefined, hint: "Click the ID to copy it. Right-click for more." };
  }

  function prCard(pr: PullRequest): HoverData {
    const review = { approved: "Approved", changes: "Changes asked", waiting: "Needs review", none: "No reviews yet" }[pr.review];
    return {
      title: `#${pr.id} ${pr.title}`,
      rows: [
        { label: "Branch", value: `${pr.source} into ${pr.target}`, mono: true },
        { label: "Author", value: pr.author },
        { label: "Review", value: review, tone: pr.review === "approved" ? "add" : pr.review === "changes" ? "del" : undefined },
        ...(pr.draft ? [{ label: "State", value: "Draft" }] : []),
        { label: "Updated", value: agoIso(pr.updated) },
      ],
      hint: "Enter reviews the changes here. Right-click to copy the link.",
    };
  }

  function issueCard(i: Issue): HoverData {
    return {
      title: `#${i.number} ${i.title}`,
      rows: [
        { label: "State", value: i.state, tone: i.state === "open" ? "add" : undefined },
        { label: "Opened by", value: `${i.author}, ${agoIso(i.created)}` },
        ...(i.assignees.length ? [{ label: "Assigned", value: i.assignees.join(", ") }] : []),
        ...(i.labels.length ? [{ label: "Labels", value: i.labels.map((l) => l.name).join(", ") }] : []),
        { label: "Comments", value: String(i.comments) },
        { label: "Updated", value: agoIso(i.updated) },
      ],
      hint: "Enter reads it here. Right-click for Copy for agent and more.",
    };
  }

  const runWord = (r: CiRun) =>
    r.status === "running" ? "Running" : r.status === "queued" ? "Waiting" : { succeeded: "Succeeded", partial: "Finished with warnings", failed: "Failed", canceled: "Canceled", skipped: "Skipped", "": "Finished" }[r.result];
  const runIcon = (r: CiRun) =>
    r.status === "running" ? "●" : r.status === "queued" ? "○" : { succeeded: "✓", partial: "◐", failed: "✗", canceled: "■", skipped: "–", "": "·" }[r.result];
  const runTook = (r: CiRun) => (r.started ? duration((r.finished ? Date.parse(r.finished) : Date.now()) - Date.parse(r.started)) : "");

  function runCard(r: CiRun): HoverData {
    return {
      title: `${r.name} #${r.number}`,
      rows: [
        { label: "Result", value: runWord(r), tone: r.result === "failed" ? "del" : r.result === "succeeded" ? "add" : r.status === "running" ? "blue" : "amber" },
        { label: "Branch", value: r.branch, mono: true },
        ...(r.sha ? [{ label: "Commit", value: r.sha, mono: true }] : []),
        { label: r.provider === "azdo" ? "Started by" : "Trigger", value: r.by },
        { label: "Started", value: agoIso(r.started) },
        ...(runTook(r) ? [{ label: r.finished ? "Took" : "Running for", value: runTook(r) }] : []),
      ],
      text: r.title || undefined,
      hint: "Enter shows stages, jobs, and errors. Right-click to copy the link.",
    };
  }

  function stashCard(st: Stash): HoverData {
    return {
      title: st.message,
      rows: [
        { label: "Name", value: st.name, mono: true },
        { label: "Made on", value: st.branch || "a detached HEAD", mono: true },
        { label: "When", value: ago(st.time) },
        { label: "Changed files", value: String(st.files) },
        ...(st.untracked.length ? [{ label: "New files", value: st.untracked.slice(0, 4).join(", ") + (st.untracked.length > 4 ? ` and ${st.untracked.length - 4} more` : "") }] : []),
      ],
      hint: "Enter shows what it holds. Pop puts it back. Right-click for apply and drop.",
    };
  }

  function taskCard(t: Task): HoverData {
    const last = app.lastRun(repo, t.id);
    const rows: NonNullable<HoverData["rows"]> = [{ label: "Runs", value: t.command, mono: true }];
    if (t.cwd) rows.push({ label: "In folder", value: t.cwd + "/", mono: true });
    if (last) {
      const took = duration((last.ended ?? Date.now()) - last.started);
      rows.push({
        label: "Last run",
        value: last.running ? `running for ${took}` : last.stopped ? `stopped after ${took}` : `exit code ${last.code} after ${took}`,
        tone: last.running || last.code === 0 ? "add" : last.stopped ? undefined : "del",
      });
    }
    return { title: t.name, rows, hint: "Enter runs it. R restarts, X stops. Right-click for more." };
  }

  function labelStyle(color: string) {
    const c = /^[0-9a-f]{6}$/i.test(color) ? `#${color}` : "var(--faint)";
    return `--lc:${c}`;
  }

  // Reset the cursor when the list itself changes (new project, tab, or review).
  let listKey = $derived(`${repo}|${app.tab}|${app.review?.head ?? ""}|${filter}`);
  $effect(() => {
    listKey;
    cursor = anchor = untrack(() => Math.max(0, rows.findIndex((r) => r.type !== "header")));
  });

  /** The path of the file or diff open in the viewer, when it belongs to this project. Only that row is "open". */
  const openRel = $derived.by(() => {
    const v = app.view;
    if (!v || v.repo !== repo) return null;
    if (v.kind === "diff") return v.rel;
    if (v.kind === "file" && !v.loose) return v.rel;
    return null;
  });

  // Keep the cursor on the file that is open, so arrow keys continue from there.
  $effect(() => {
    const rel = openRel;
    if (!rel) return;
    const at = untrack(() => cursor);
    if (rows[at] && rowPath(rows[at]) === rel) return;
    const i = rows.findIndex((r) => (r.type === "change" || r.type === "file") && rowPath(r) === rel);
    if (i >= 0) cursor = i;
  });

  $effect(() => {
    const at = untrack(() => cursor);
    if (rows[at]?.type !== "header") return;
    const next = rows.findIndex((r, i) => i > at && r.type !== "header");
    cursor = anchor = next >= 0 ? next : 0;
  });

  function rowPath(r: Row): string | null {
    if (r.type === "change") return r.change.path;
    if (r.type === "file") return r.path;
    return null;
  }

  function toggleDir(path: string, force?: boolean) {
    const cur = new Set(expanded[repo] ?? []);
    const open = force ?? !cur.has(path);
    if (open) cur.add(path);
    else cur.delete(path);
    expanded = { ...expanded, [repo]: [...cur] };
  }

  function activate(r: Row, preview = false) {
    // Stepping with the arrow keys (preview) does not add Back steps.
    if (r.type === "change") app.openChange(r.change, !preview);
    else if (r.type === "file") app.openFile(r.path, undefined, !preview);
    else if (r.type === "dir" && !preview) toggleDir(r.path);
    else if (r.type === "pr" && !preview) app.openPr(r.pr);
    else if (r.type === "commit" && !preview) app.openCommit(r.commit);
    else if (r.type === "fold" && !preview) unfold();
    else if (r.type === "task" && !preview) app.runTask(r.task);
    else if (r.type === "issue" && !preview) app.openIssue(r.issue.number);
    else if (r.type === "run") app.openRun(r.run);
    else if (r.type === "stash" && !preview) app.openStash(r.stash);
  }

  async function moveTo(i: number, preview = true) {
    if (!rows.length) return;
    const dir = i < cursor ? -1 : 1;
    i = Math.max(0, Math.min(rows.length - 1, i));
    // Section headers cannot hold the cursor. Step past them.
    while (rows[i]?.type === "header") i += dir;
    if (i < 0 || i >= rows.length) i = cursor;
    cursor = i;
    anchor = cursor;
    if (preview) activate(rows[cursor], true);
    await tick();
    listEl?.querySelector(`[data-i="${cursor}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function click(e: MouseEvent, i: number) {
    const r = rows[i];
    if (r.type === "header") return;
    const path = rowPath(r);
    if (path && (e.ctrlKey || e.metaKey)) {
      app.toggleSelect(path);
      cursor = anchor = i;
      return;
    }
    if (path && e.shiftKey) {
      const [a, b] = [Math.min(anchor, i), Math.max(anchor, i)];
      app.selected = rows.slice(a, b + 1).map(rowPath).filter((p): p is string => !!p);
      cursor = i;
      return;
    }
    cursor = anchor = i;
    activate(r);
  }

  /** `S`: stage the files under the cursor or selected, or unstage them in the Staged section. */
  function toggleStage() {
    const r = rows[cursor];
    if (r?.type !== "change" || !r.section) return;
    const all = s?.changes ?? [];
    const targets = app.selected.length ? all.filter((c) => app.selected.includes(c.path)) : [r.change];
    if (r.section === "staged") app.unstage(targets);
    else app.stage(targets);
  }

  function isHot(c: Change) {
    return !!c.mtime && app.now - c.mtime < WORKING_SECONDS;
  }

  function commitKey(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      e.stopPropagation();
      app.commit(e.shiftKey || app.pushAfterCommit).then((ok) => ok && listEl?.focus());
    } else if (e.key === "Escape") {
      e.stopPropagation();
      listEl?.focus();
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === "Enter" && app.tab === "changes" && !reviewing) {
      e.preventDefault();
      commitBox?.focus();
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const r = rows[cursor];
    const page = Math.max(1, Math.floor((listEl?.clientHeight ?? 400) / 34) - 1);
    const typing = e.target instanceof HTMLInputElement;
    if (["ArrowDown", "ArrowUp", "PageDown", "PageUp"].includes(e.key) || (!typing && ["j", "k", "Home", "End"].includes(e.key))) kbd = true;
    switch (e.key) {
      case "ArrowDown":
        moveTo(cursor + 1);
        break;
      case "ArrowUp":
        moveTo(cursor - 1);
        break;
      case "j":
        if (typing) return;
        moveTo(cursor + 1);
        break;
      case "k":
        if (typing) return;
        moveTo(cursor - 1);
        break;
      case "PageDown":
        moveTo(cursor + page);
        break;
      case "PageUp":
        moveTo(cursor - page);
        break;
      case "Home":
        if (typing) return;
        moveTo(0);
        break;
      case "End":
        if (typing) return;
        moveTo(rows.length - 1);
        break;
      case "Enter":
        if (r) activate(r);
        break;
      case " ": {
        if (typing) return;
        const p = r && rowPath(r);
        if (p) app.toggleSelect(p);
        break;
      }
      case "ArrowRight":
        if (typing) return;
        if (r?.type === "dir") toggleDir(r.path, true);
        break;
      case "ArrowLeft":
        if (typing) return;
        if (r?.type === "dir" && r.open) toggleDir(r.path, false);
        else if (r && (r.type === "dir" || r.type === "file") && r.depth > 0) {
          const parent = dirName(r.type === "dir" ? r.path : r.path);
          const pi = rows.findIndex((x) => x.type === "dir" && x.path === parent);
          if (pi >= 0) moveTo(pi, false);
        }
        break;
      case "c":
        if (typing) return;
        app.overlay = "copy";
        break;
      case "s":
        if (typing) return;
        toggleStage();
        break;
      case "p":
        if (typing || r?.type !== "task") return;
        app.toggleTaskPin(repo, r.task);
        break;
      case "r":
      case "x": {
        if (typing || r?.type !== "task") return;
        const last = app.lastRun(repo, r.task.id);
        if (e.key === "x") {
          if (last?.running) app.stopRun(last.id);
        } else if (last) app.restartRun(last.id);
        else app.runTask(r.task);
        break;
      }
      case "/":
        if (typing) return;
        filterEl?.focus();
        filterEl?.select();
        break;
      case "Backspace":
        if (typing) return;
        if (app.review) app.closeReview();
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  // Move keyboard focus to the list when a project opens, so arrow keys work right away.
  $effect(() => {
    repo;
    app.tab;
    tick().then(() => {
      if (!(document.activeElement instanceof HTMLInputElement)) listEl?.focus();
    });
  });

  const tabLabel: Record<Tab, string> = {
    changes: "Changes", files: "Files", prs: "Pull requests", issues: "Issues", history: "History", ci: "CI", tasks: "Tasks",
  };
  /** Shorter labels for a narrow pane. */
  const tabShort: Partial<Record<Tab, string>> = { prs: "PRs" };
  // Each space picks its own tabs. Ctrl 1, Ctrl 2, and so on follow this order.
  const tabs = $derived(app.visibleTabs.map((id, i) => ({ id, label: tabLabel[id], short: tabShort[id], key: String(i + 1) })));

  /** Keys in the filter: Esc clears it and goes back to the list, ArrowDown moves into the list, Enter opens. */
  function filterKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setFilter("");
      listEl?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      kbd = true;
      listEl?.focus();
      moveTo(cursor);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (rows[cursor]) activate(rows[cursor]);
    }
  }

  /** Opens a row's right-click menu at a point, or at the mouse. */
  function rowMenu(at: MouseEvent | { x: number; y: number }, r: Row, i: number) {
    if (r.type === "change") fileMenu(at, r.change.path, r.change);
    else if (r.type === "file") fileMenu(at, r.path);
    else if (r.type === "commit") commitMenu(at, r.commit);
    else if (r.type === "pr") prMenu(at, r.pr);
    else if (r.type === "issue") issueMenu(at, r.issue);
    else if (r.type === "run") ciMenu(at, r.run);
    else if (r.type === "stash") stashMenu(at, r.stash);
    else if (r.type === "task") taskMenu(at, r.task, r.custom);
    else if (r.type === "dir") fileMenu(at, r.path);
    else return;
    cursor = anchor = i;
  }

  /** The ⋯ button: the same menu as a right-click, under the button. */
  function below(e: MouseEvent) {
    e.stopPropagation();
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: b.left, y: b.bottom + 4 };
  }

  function syncCard(kind: "pull" | "push"): HoverData {
    const n = kind === "pull" ? (s?.behind ?? 0) : (s?.ahead ?? 0);
    const up = s?.upstream;
    if (!up) return { title: kind === "pull" ? "Pull" : "Push", text: kind === "pull" ? "This branch has no upstream, so there is nothing to pull from." : "This branch is not on a remote yet. Push creates it." };
    return {
      title: kind === "pull" ? `Pull from ${up}` : `Push to ${up}`,
      rows: [
        { label: kind === "pull" ? "To pull" : "To push", value: `${n} ${n === 1 ? "commit" : "commits"}`, tone: n ? (kind === "pull" ? "blue" : "add") : undefined },
        { label: "As of", value: `the last fetch, ${ago(s?.fetchedAt)}` },
      ],
      hint: "Tern shows the commits and asks before it runs.",
    };
  }

  function prHint(err: string) {
    const provider = s?.provider;
    if (provider === "github") return "Check that gh is signed in: run gh auth login in a terminal.";
    if (provider === "azdo") return "Check that az is signed in: run az login in a terminal.";
    return "";
  }

  // Tabs rest on words without icons, so the strip looks the same in every project. When words do not fit they give way:
  // no counts (tighter), then only the chosen tab labeled, then icons only.
  let tablistEl: HTMLDivElement | undefined = $state();
  let tabMode = $state<"text" | "tight" | "bare" | "active" | "icons">("text");
  let fitting = false;
  async function fitTabs() {
    const el = tablistEl;
    if (!el || fitting) return;
    fitting = true;
    try {
      for (const mode of ["text", "tight", "bare", "active", "icons"] as const) {
        tabMode = mode;
        await tick();
        // Measure the tabs themselves (scrollWidth is never less than the box), with a little slack at the edge.
        const first = el.firstElementChild as HTMLElement | null;
        const last = el.lastElementChild as HTMLElement | null;
        const used = first && last ? last.offsetLeft + last.offsetWidth - first.offsetLeft : 0;
        if (used <= el.clientWidth - 4) break;
      }
      el.querySelector<HTMLElement>("[role=tab].on")?.scrollIntoView({ block: "nearest", inline: "nearest" });
    } finally {
      fitting = false;
    }
  }
  $effect(() => {
    const el = tablistEl;
    if (!el) return;
    const ro = new ResizeObserver(() => fitTabs());
    ro.observe(el);
    return () => ro.disconnect();
  });
  // Tabs, badges, or the chosen tab changed: fit again.
  $effect(() => {
    void tabs.length;
    void app.tab;
    void s?.changes.length;
    void app.prs[repo]?.items.length;
    void app.issues[repo]?.items.length;
    void app.runningIn(repo);
    void app.ci[repo]?.items.length;
    untrack(() => fitTabs());
  });
</script>

{#snippet tabIcon(id: Tab)}
  <svg class="ticon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#if id === "changes"}
      <path d="M8 2.5v6M5 5.5h6M5 12.5h6" />
    {:else if id === "files"}
      <path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.5h4.9A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" />
    {:else if id === "prs"}
      <circle cx="4" cy="3.5" r="1.5" /><circle cx="4" cy="12.5" r="1.5" /><circle cx="12" cy="12.5" r="1.5" />
      <path d="M4 5v6M12 11V6.5a2 2 0 0 0-2-2H7.5M9 3 7.5 4.5 9 6" />
    {:else if id === "issues"}
      <circle cx="8" cy="8" r="5.5" /><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" />
    {:else if id === "history"}
      <circle cx="8" cy="8" r="5.5" /><path d="M8 5v3l2 1.5" />
    {:else if id === "ci"}
      <path d="M2 8h2.5L6 4l3 8 1.5-4H14" />
    {:else}
      <path d="M3 4.5 6.5 8 3 11.5M8.5 12H13" />
    {/if}
  </svg>
{/snippet}

{#snippet refreshIcon()}
  <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M13 8a5 5 0 1 1-1.46-3.54M13 2.5v3h-3" />
  </svg>
{/snippet}

{#snippet moreIcon()}
  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
    <circle cx="3.5" cy="8" r="1.25" fill="currentColor" /><circle cx="8" cy="8" r="1.25" fill="currentColor" /><circle cx="12.5" cy="8" r="1.25" fill="currentColor" />
  </svg>
{/snippet}

{#snippet more(r: Row, i: number)}
  <button class="icon-btn sm hov" aria-label="More actions" onclick={(e) => rowMenu(below(e), r, i)}
    use:hover={() => ({ title: "More actions", hint: "Right-click the row for the same menu" })}>{@render moreIcon()}</button>
{/snippet}

{#snippet skeletons(n: number, two: boolean)}
  <div class="skel" aria-hidden="true">
    {#each { length: n } as _, k}
      <div class="skelrow" class:two>
        <div class="skeleton" style:width="{[72, 58, 80, 64, 50][k % 5]}%"></div>
        {#if two}<div class="skeleton thin" style:width="{[40, 52, 34, 46, 38][k % 5]}%"></div>{/if}
      </div>
    {/each}
  </div>
{/snippet}

<section class="pane" aria-label="Project">
  <!-- Right-click here is a mouse shortcut. Shift F10 opens the same menu from the keyboard. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="toolbar head" oncontextmenu={(e) => repoMenu(e, repo)}>
    <h1 class="title" use:hover={() => ({ title: app.repo?.name ?? baseName(repo), rows: [{ label: "Folder", value: repo, mono: true }], hint: "Right-click for project actions" })}>
      {app.repo?.name ?? baseName(repo)}{#if app.nameHint[repo]}<span class="hint">{app.nameHint[repo]}</span>{/if}
    </h1>
    <button class="btn ghost sm branch" onclick={() => app.openBranches("branches")}
      use:hover={() => ({ title: s?.branch ?? "Detached HEAD", rows: s?.upstream ? [{ label: "Tracks", value: s.upstream, mono: true }] : [{ label: "Upstream", value: "none, local only" }], hint: "Click or Ctrl B for branches and worktrees" })}>
      <span class="bname">{s?.branch ?? "detached"}</span>{#if s && !s.upstream}<span class="local">local only</span>{/if}
      <svg class="caret" viewBox="0 0 16 16" width="10" height="10" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
    <span class="grow"></span>
    <div class="sync" role="group" aria-label="Sync">
      <button class="btn ghost sm" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "fetch")}
        use:hover={() => ({ title: "Fetch", text: "Get the latest commits from the remote. Your files do not change.", rows: [{ label: "Last fetch", value: ago(s?.fetchedAt) }] })}>
        {app.busy[repo] === "fetch" ? "Fetching…" : "Fetch"}
      </button>
      <button class="btn ghost sm" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "pull")} use:hover={() => syncCard("pull")}>
        Pull{#if s?.behind}<span class="n in">↓{s.behind}</span>{/if}
      </button>
      <button class="btn ghost sm" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "push")} use:hover={() => syncCard("push")}>
        Push{#if s?.ahead}<span class="n out">↑{s.ahead}</span>{/if}
      </button>
    </div>
  </header>

  <div class="tabbar">
    <div class="tabs tablist mode-{tabMode}" role="tablist" bind:this={tablistEl}>
      {#each tabs as t}
        <button role="tab" class:on={app.tab === t.id} aria-selected={app.tab === t.id} aria-label={t.label} onclick={() => app.setTab(t.id)}
          use:hover={() => ({ title: t.label, hint: `Ctrl ${t.key}` })}>
          {@render tabIcon(t.id)}
          <span class="tlabel" class:long={!!t.short}>{t.label}</span>
          {#if t.short}<span class="tlabel short">{t.short}</span>{/if}
          {#if t.id === "changes" && s?.changes.length}<span class="badge">{s.changes.length}</span>{/if}
          {#if t.id === "prs" && app.prs[repo]?.items.length}<span class="badge">{app.prs[repo].items.length}</span>{/if}
          {#if t.id === "issues" && app.issues[repo]?.items.length}<span class="badge">{app.issues[repo].items.length}</span>{/if}
          {#if t.id === "ci"}
            {@const mine = app.ci[repo]?.items.find((x) => x.branch === s?.branch)}
            {#if mine}<span class="cistate r-{mine.result} s-{mine.status}" use:popOnDone={mine.status}>{runIcon(mine)}</span>{/if}
          {/if}
          {#if t.id === "tasks" && app.runningIn(repo)}<span class="badge add">{app.runningIn(repo)}</span>{/if}
        </button>
      {/each}
    </div>
  </div>

  {#if reviewing && app.review}
    <div class="toolbar sub review">
      <button class="icon-btn" onclick={() => app.closeReview()} aria-label="Back" use:hover={() => ({ title: "Back", hint: "Backspace" })}>
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M13 8H3M7 4 3 8l4 4" /></svg>
      </button>
      <div class="rhead">
        <span class="title" title={app.review.title}>{app.review.title}</span>
        <span class="meta rmeta">{app.review.subtitle}, {app.review.files.length} {app.review.files.length === 1 ? "file" : "files"}</span>
      </div>
      {#if app.review.pr}
        <button class="btn ghost sm" onclick={() => app.openPrInBrowser()}>Open in browser</button>
      {/if}
      {#if app.review.stash}
        {@const st = app.review.stash}
        <button class="btn primary sm" onclick={() => app.applyStash(st, true)} use:hover={() => ({ title: "Pop", text: "Apply it and remove it from the list." })}>Pop</button>
        <button class="icon-btn sm" onclick={(e) => stashMenu(below(e), st)} aria-label="More stash actions"
          use:hover={() => ({ title: "More stash actions", text: "Apply, drop, and copy." })}>{@render moreIcon()}</button>
      {/if}
      {#if app.review.kind === "branch"}
        {#if (app.review.candidates?.length ?? 0) > 1}
          <select class="field basepick" value={app.review.baseRef} onchange={(e) => app.openBranchReview(e.currentTarget.value)} aria-label="Compare with"
            use:hover={() => ({ title: "Compare with", text: "The branch these changes are measured from." })}>
            {#each app.review.candidates ?? [] as c}<option value={c}>{c}</option>{/each}
          </select>
        {/if}
        <button class="btn ghost sm" onclick={() => app.copyBranchSummary()}>Copy for agent</button>
      {/if}
    </div>
    {#if app.review.kind === "branch" && app.review.commits?.length}
      <button class="commits-toggle" onclick={() => (showCommits = !showCommits)} aria-expanded={showCommits}>
        <svg class="chevron" class:open={showCommits} viewBox="0 0 16 16" width="10" height="10" aria-hidden="true"><path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
        {app.review.commits.length} {app.review.commits.length === 1 ? "commit" : "commits"} on this branch
      </button>
      {#if showCommits}
        <ul class="bcommits">
          {#each app.review.commits as c (c.sha)}
            <li use:hover={() => commitCard(c)} oncontextmenu={(e) => commitMenu(e, c)}>
              <button class="sha" onclick={() => app.copyValue(c.short, `commit ID ${c.short}`)}>{c.short}</button>
              <span class="csub">{c.subject}</span>
              <span class="faint">{ago(c.time)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  {/if}

  <div class="toolbar sub filterbar">
    <input
      class="field grow"
      bind:this={filterEl}
      bind:value={() => filter, setFilter}
      placeholder={reviewing ? "Filter files" : filterHint[app.tab]}
      aria-label={reviewing ? "Filter files" : filterHint[app.tab]}
      onkeydown={filterKeydown}
      spellcheck="false"
    />
    {#if app.tab === "changes" && app.review?.kind !== "stash" && (app.canReviewBranch(repo) || app.review?.kind === "branch")}
      <div class="seg" role="group" aria-label="What to show">
        <button class:on={!reviewing} onclick={() => reviewing && app.closeReview()}>Uncommitted</button>
        <button class:on={reviewing} onclick={() => !reviewing && app.openBranchReview()}>Since {app.review?.baseRef ?? "main"}</button>
      </div>
    {/if}
    {#if !reviewing}
      {#if app.tab === "history"}
        <label class="check">
          <input type="checkbox" checked={app.logAll} onchange={(e) => app.setLogAll(e.currentTarget.checked)} />
          All branches
        </label>
        <span class="count faint">{app.logs[repo]?.length ?? 0} commits</span>
      {:else if app.tab === "issues"}
        <div class="seg" role="group" aria-label="Which issues">
          {#each [["open", "Open"], ["closed", "Closed"], ["all", "All"]] as [id, label]}
            <button class:on={app.issueState === id} onclick={() => app.setIssueState(id as "open" | "closed" | "all")}>{label}</button>
          {/each}
        </div>
        <button class="icon-btn" aria-label="Refresh issues" disabled={app.issues[repo]?.loading} onclick={() => app.loadIssues(repo, true)}
          use:hover={() => ({ title: "Refresh issues" })}>{@render refreshIcon()}</button>
      {:else if app.tab === "prs"}
        <button class="icon-btn" aria-label="Refresh pull requests" disabled={app.prs[repo]?.loading} onclick={() => app.loadPrs(repo, true)}
          use:hover={() => ({ title: "Refresh pull requests" })}>{@render refreshIcon()}</button>
        <button class="btn ghost sm" onclick={() => app.openNewPr()}>New pull request…</button>
      {:else if app.tab === "ci"}
        <button class="icon-btn" aria-label="Refresh runs" disabled={app.ci[repo]?.loading} onclick={() => app.loadCi(repo, true)}
          use:hover={() => ({ title: "Refresh runs" })}>{@render refreshIcon()}</button>
      {/if}
    {/if}
  </div>

  {#if app.tab === "tasks" && !reviewing}
    <form class="toolbar sub runline" onsubmit={(e) => { e.preventDefault(); app.runCommand(command); }}>
      <input class="field mono grow" bind:value={command} placeholder="Run a command…" spellcheck="false" aria-label="Command to run" />
      <button class="btn sm" class:primary={!!command.trim()} type="submit" disabled={!command.trim()}>Run</button>
      <button class="btn ghost sm" type="button" disabled={!command.trim()} onclick={() => { app.saveCustomTask(repo, command, command); command = ""; }}
        use:hover={() => ({ title: "Save", text: "Save this command in the Tasks list for this project." })}>Save</button>
    </form>
  {/if}

  {#if app.tab === "history" && !reviewing && sync}
    {@const here = sync.branch ?? "HEAD"}
    <div class="syncbar" class:behind={sync.behind > 0}>
      {#if !sync.upstream}
        <span class="faint">{here} has no remote branch. Push it to make one.</span>
      {:else}
        <div class="syncline">
          {#if sync.behind}
            <span class="in">↓ {sync.behind}</span>
            <span>{sync.behind === 1 ? "commit" : "commits"} on <b>{sync.upstream}</b> not pulled into <b>{here}</b></span>
          {:else}
            <span class="ok">✓</span><span><b>{here}</b> has everything on <b>{sync.upstream}</b></span>
          {/if}
          {#if sync.ahead}<span class="out">↑ {sync.ahead} to push</span>{/if}
        </div>
        <div class="syncline">
          <span class="faint">as of the last fetch, {ago(s?.fetchedAt)}</span>
          <span class="grow"></span>
          {#if unfolded[repo] && incoming.size > 7}
            <button class="btn ghost sm" onclick={() => (unfolded = { ...unfolded, [repo]: false })}>Fold incoming</button>
          {/if}
          {#if sync.behind}
            <button class="btn ghost sm" disabled={app.reviewLoading === "incoming"} onclick={() => app.reviewIncoming(repo)}
              use:hover={() => ({ title: "Review incoming", text: "One diff of everything the pull brings." })}>{app.reviewLoading === "incoming" ? "Loading…" : "Review incoming"}</button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if app.selected.length}
    <div class="toolbar sub selbar">
      <span class="selcount">{app.selected.length} selected</span>
      <span class="grow"></span>
      <button class="btn ghost sm" onclick={() => app.copy("mention")}>Copy @</button>
      <button class="btn ghost sm" onclick={() => app.copy("relative")}>Copy paths</button>
      {#if app.tab === "changes" || reviewing}<button class="btn ghost sm" onclick={() => app.copy("diff")}>Copy diff</button>{/if}
      <button class="icon-btn sm" aria-label="Clear selection" onclick={() => (app.selected = [])} use:hover={() => ({ title: "Clear selection", hint: "Esc" })}>
        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="list" class:kbd bind:this={listEl} tabindex="0" onkeydown={onKey} onmousedown={() => (kbd = false)} role="listbox" aria-label="Items">
    {#if noMatch}
      <div class="state">
        <div class="state-title">Nothing matches</div>
        <p>No rows contain “{filter.trim()}”.</p>
        <div class="actions"><button class="btn sm" onclick={() => { setFilter(""); listEl?.focus(); }}>Clear filter</button></div>
      </div>
    {:else if app.tab === "changes" && !reviewing && s && !s.changes.length}
      <div class="state" class:inline={rows.length > 0}>
        <div class="state-title">No changes</div>
        <p>The working tree matches the last commit.</p>
        {#if s.behind}<div class="actions"><button class="btn sm" onclick={() => app.action(repo, "pull")}>Pull {s.behind} new {s.behind === 1 ? "commit" : "commits"}</button></div>{/if}
      </div>
    {:else if app.tab === "files" && !reviewing && !app.files[repo]}
      <div class="state"><p>Reading files…</p></div>
    {:else if app.tab === "prs" && !reviewing}
      {@const pr = app.prs[repo]}
      {#if pr?.loading && !pr.items.length}
        {@render skeletons(5, true)}
      {:else if pr?.error}
        <div class="state">
          <div class="state-title">Pull requests did not load</div>
          <pre>{pr.error}</pre>
          {#if prHint(pr.error)}<p>{prHint(pr.error)}</p>{/if}
          <div class="actions"><button class="btn sm" onclick={() => app.loadPrs(repo, true)}>Try again</button></div>
        </div>
      {:else if pr && !pr.items.length}
        <div class="state">
          <div class="state-title">No open pull requests</div>
          <div class="actions"><button class="btn sm" onclick={() => app.openNewPr()}>Create one for {s?.branch ?? "this branch"}…</button></div>
        </div>
      {/if}
    {:else if app.tab === "history" && !reviewing && !app.logs[repo]}
      {@render skeletons(5, true)}
    {:else if app.tab === "ci" && !reviewing}
      {@const c = app.ci[repo]}
      {#if c?.loading && !c.items.length}
        {@render skeletons(5, true)}
      {:else if c?.error}
        <div class="state">
          <div class="state-title">CI runs did not load</div>
          <pre>{c.error}</pre>
          {#if s?.provider === "azdo"}<p>Check that az is signed in: run az login in a terminal.</p>{:else if s?.provider === "github"}<p>Check that gh is signed in: run gh auth login in a terminal.</p>{/if}
          <div class="actions"><button class="btn sm" onclick={() => app.loadCi(repo, true)}>Try again</button></div>
        </div>
      {:else if c && !c.items.length}
        <div class="state">
          <div class="state-title">No pipeline runs</div>
          <p>No CI runs were found for this project.</p>
        </div>
      {/if}
    {:else if app.tab === "issues" && !reviewing}
      {@const iss = app.issues[repo]}
      {#if iss?.loading && !iss.items.length}
        {@render skeletons(5, true)}
      {:else if iss?.error}
        <div class="state">
          <div class="state-title">Issues did not load</div>
          <pre>{iss.error}</pre>
          {#if s?.provider === "github"}<p>Check that gh is signed in: run gh auth login in a terminal.</p>{/if}
          <div class="actions"><button class="btn sm" onclick={() => app.loadIssues(repo, true)}>Try again</button></div>
        </div>
      {:else if iss && !iss.items.length}
        <div class="state">
          <div class="state-title">No {app.issueState === "all" ? "" : app.issueState} issues</div>
        </div>
      {/if}
    {:else if app.tab === "tasks" && !reviewing && !app.tasks[repo]}
      <div class="state"><p>Looking for tasks…</p></div>
    {:else if app.tab === "tasks" && !reviewing && !app.tasks[repo]?.length}
      <div class="state">
        <div class="state-title">No tasks found</div>
        <p>Tern found no npm scripts, VS Code tasks, .NET, Gradle, Cargo, Make, or Docker Compose files here.</p>
        <p>Type a command above and press Save to add one.</p>
      </div>
    {/if}

    {#each rows as r, i (r.id)}
      {@const path = rowPath(r)}
      <div
        class="row {r.type}"
        class:nograph={(r.type === "commit" || r.type === "fold") && !graph}
        class:incoming={r.type === "commit" && r.flow === "in"}
        class:reveal={r.type === "commit" && revealed.has(r.commit.sha)}
        class:fx-fade={r.type === "commit" && revealed.has(r.commit.sha)}
        class:cursor={i === cursor}
        class:open={path !== null && openRel === path}
        class:sel={path !== null && app.selected.includes(path)}
        data-i={i}
        role="option"
        aria-selected={path !== null && app.selected.includes(path)}
        tabindex="-1"
        onclick={(e) => click(e, i)}
        onkeydown={() => {}}
        oncontextmenu={(e) => rowMenu(e, r, i)}
        use:hover={() =>
          r.type === "change" ? changeCard(r.change)
          : r.type === "commit" ? commitCard(r.commit)
          : r.type === "pr" ? prCard(r.pr)
          : r.type === "issue" ? issueCard(r.issue)
          : r.type === "run" ? runCard(r.run)
          : r.type === "stash" ? stashCard(r.stash)
          : r.type === "task" ? taskCard(r.task)
          : r.type === "file" && (filter || r.name.length > 34) ? { title: r.path, hint: "Right-click to copy the path or @ mention" }
          : null}
      >
        {#if r.type === "header"}
          <span class="hlabel">{r.label} <span class="faint">{r.count}</span></span>
          <span class="grow"></span>
          {#if r.section === "staged"}
            <button class="btn ghost sm" onclick={(e) => { e.stopPropagation(); app.unstage("all"); }}>Unstage all</button>
          {:else if r.section === "unstaged"}
            <button class="btn ghost sm" onclick={(e) => { e.stopPropagation(); app.stage("all"); }}>Stage all</button>
          {/if}
          {#if r.section === "staged" || r.section === "unstaged"}
            {@const sec = r.section}
            <button class="icon-btn sm" aria-label="Stash or discard"
              onclick={(e) => sectionMenu(below(e), sec)}
              use:hover={() => ({ title: "Stash or discard", text: "Stash these changes, or throw them away." })}>{@render moreIcon()}</button>
          {/if}
        {:else if r.type === "stash"}
          <span class="stashicon" aria-hidden="true">
            <svg viewBox="0 0 16 16" width="13" height="13"><path d="M2.5 5.5h11v7a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1zM2 3h12v2.5H2zM6.5 8h3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" stroke-linecap="round"/></svg>
          </span>
          <div class="two">
            <div class="line1"><span class="ptitle">{r.stash.message}</span></div>
            <div class="line2">
              {#if app.reviewLoading === r.stash.sha}<span class="loading">loading…</span>{/if}
              <span>{r.stash.branch || "detached"}</span>
              <span>{r.stash.files + r.stash.untracked.length} {r.stash.files + r.stash.untracked.length === 1 ? "file" : "files"}</span>
              <span class="age">{ago(r.stash.time)}</span>
            </div>
          </div>
          <button class="btn ghost sm hov" onclick={(e) => { e.stopPropagation(); app.applyStash(r.stash, true); }}
            use:hover={() => ({ title: "Pop", text: "Apply it and remove it from the list." })}>Pop</button>
          {@render more(r, i)}
        {:else if r.type === "task"}
          {@const last = app.lastRun(repo, r.task.id)}
          {@const where = r.pinned ? taskGroup(r.task) : ""}
          {@const st = !last ? "" : last.running ? "running" : last.stopped ? "stopped" : last.code === 0 ? "ok" : "failed"}
          <span class="tstate {st}">
            {st === "running" ? "" : st === "ok" ? "✓" : st === "failed" ? "✗" : st === "stopped" ? "■" : "▷"}
          </span>
          <div class="two">
            <div class="line1"><span class="ptitle">{r.task.name}</span></div>
            {#if (r.task.detail && r.task.detail !== r.task.name) || where}
              <div class="line2">
                {#if where}<span class="tfrom">{where}</span>{/if}
                {#if r.task.detail && r.task.detail !== r.task.name}<span>{r.task.detail}</span>{/if}
              </div>
            {/if}
          </div>
          <button class="icon-btn sm pin" class:hov={!r.pinned} onclick={(e) => { e.stopPropagation(); app.toggleTaskPin(repo, r.task); }}
            aria-label={r.pinned ? "Unpin" : "Pin"} use:hover={() => ({ title: r.pinned ? "Unpin" : "Pin", hint: "P" })}>
            <svg class:pinned={r.pinned} width="12" height="12" viewBox="0 0 16 16" aria-hidden="true"><path d="M10 1.5l4.5 4.5-2 .5-2.6 2.6.6 3.4-1.5 1.5-2.8-2.8L2.5 15 1 13.5l3.7-3.7L1.9 7l1.5-1.5 3.4.6L9.4 3.5z" fill={r.pinned ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>
          </button>
          {#if last?.running}
            <button class="icon-btn sm hov" onclick={(e) => { e.stopPropagation(); app.showRun(last.id); }} aria-label="Show the output"
              use:hover={() => ({ title: "Show the output", hint: "Enter" })}>
              <svg width="13" height="13" viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 3.5h11v9h-11zM5 7l2 1.5L5 10M8.5 10.5h2.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </button>
            <button class="icon-btn sm hov" onclick={(e) => { e.stopPropagation(); app.stopRun(last.id); }} aria-label="Stop" use:hover={() => ({ title: "Stop", hint: "X" })}>
              <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3.5" width="9" height="9" rx="1.5" fill="currentColor" /></svg>
            </button>
          {:else if r.custom !== null}
            <button class="btn ghost sm hov" onclick={(e) => { e.stopPropagation(); app.removeCustomTask(repo, r.custom!); }}
              use:hover={() => ({ title: "Remove", text: "Remove this saved command." })}>Remove</button>
          {/if}
          {@render more(r, i)}
        {:else if r.type === "change"}
          <span class="kind mono k-{r.change.kind}">{kindLetter[r.change.kind]}</span>
          <span class="fname">{baseName(r.change.path)}</span>
          {#if r.change.oldPath}
            <span class="from">
              from {dirName(r.change.oldPath) === dirName(r.change.path) ? baseName(r.change.oldPath) : r.change.oldPath}
            </span>
          {/if}
          <span class="fpath">&lrm;{dirName(r.change.path)}&lrm;</span>
          {#if isHot(r.change)}<span class="fresh"></span>{/if}
          {#if r.section}
            <button
              class="icon-btn sm hov"
              aria-label={r.section === "staged" ? "Unstage" : "Stage"}
              use:hover={() => ({ title: r.section === "staged" ? "Unstage" : "Stage", hint: "S" })}
              onclick={(e) => { e.stopPropagation(); r.section === "staged" ? app.unstage([r.change]) : app.stage([r.change]); }}
            >
              <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
                {#if r.section === "staged"}<path d="M3.5 8h9" />{:else}<path d="M8 3.5v9M3.5 8h9" />{/if}
              </svg>
            </button>
          {/if}
          {@render more(r, i)}
        {:else if r.type === "dir"}
          <span class="indent" style:width="{r.depth * 14}px"></span>
          <span class="chev">{r.open ? "▾" : "▸"}</span>
          <span class="fname dirname">{r.name}</span>
          <span class="grow"></span>
          {@render more(r, i)}
        {:else if r.type === "file"}
          <span class="indent" style:width="{r.depth * 14}px"></span>
          <span class="chev"></span>
          <span class="fname">{r.depth === 0 && filter ? baseName(r.name) : r.name}</span>
          {#if filter}<span class="fpath">&lrm;{dirName(r.path)}&lrm;</span>{:else}<span class="grow"></span>{/if}
          {@render more(r, i)}
        {:else if r.type === "pr"}
          <div class="two">
            <div class="line1">
              <span class="prid mono">#{r.pr.id}</span>
              <span class="ptitle">{r.pr.title}</span>
            </div>
            <div class="line2">
              {#if app.reviewLoading === String(r.pr.id)}<span class="loading">loading…</span>{/if}
              {#if r.pr.draft}<span class="chip">draft</span>{/if}
              <span class="rv rv-{r.pr.review}">{{ approved: "approved", changes: "changes asked", waiting: "needs review", none: "" }[r.pr.review]}</span>
              <span>{r.pr.author}</span>
              <span class="branches">{r.pr.source} → {r.pr.target}</span>
              <span class="age">{agoIso(r.pr.updated)}</span>
            </div>
          </div>
          {@render more(r, i)}
        {:else if r.type === "fold"}
          {#if graph?.rows[i]}
            <GraphCell row={graph.rows[i]} lanes={graph.lanes} height={FOLD_ROW} />
          {/if}
          <span class="foldtext">
            <span class="in">⋯ {r.count + unloaded} more {r.count + unloaded === 1 ? "commit" : "commits"}</span>
            on {sync?.upstream}{#if unloaded}<span class="faint"> ({r.count} shown when opened)</span>{/if}
            <span class="faint">· Enter or click to show</span>
          </span>
        {:else if r.type === "commit"}
          {@const refs = parseRefs(r.commit.refs)}
          {#if graph?.rows[i]}
            <GraphCell row={graph.rows[i]} lanes={graph.lanes} height={COMMIT_ROW} head={r.commit.refs.startsWith("HEAD")} />
          {/if}
          <div class="two">
            <div class="line1">
              {#each refs.slice(0, 2) as ref}<span class="chip ref ref-{ref.kind}">{ref.name}</span>{/each}
              {#if refs.length > 2}
                <span class="chip ref" use:hover={() => ({ title: "Points here", text: refs.map((x) => x.name).join(", ") })}>+{refs.length - 2}</span>
              {/if}
              <span class="ptitle subject">{r.commit.subject}</span>
            </div>
            <div class="line2">
              {#if r.flow === "in"}<span class="flow in" title="Not pulled yet: only on {sync?.upstream}">↓</span>
              {:else if r.flow === "out"}<span class="flow out" title="Not pushed yet: only on this computer">↑</span>{/if}
              {#if app.reviewLoading === r.commit.sha}<span class="loading">loading…</span>{/if}
              <button class="sha" onclick={(e) => { e.stopPropagation(); app.copyValue(r.commit.short, `commit ID ${r.commit.short}`); }}
                aria-label="Copy commit ID {r.commit.short}">{r.commit.short}</button>
              <span>{r.commit.author}</span>
              <span class="age">{ago(r.commit.time)}</span>
            </div>
          </div>
          {@render more(r, i)}
        {:else if r.type === "run"}
          <span class="cistate r-{r.run.result} s-{r.run.status}" use:popOnDone={r.run.status}>{runIcon(r.run)}</span>
          <div class="two">
            <div class="line1">
              <span class="ptitle">{r.run.title || r.run.name}</span>
            </div>
            <div class="line2">
              <span class="age">#{r.run.number}</span>
              <span>{r.run.branch}</span>
              <span>{r.run.by}</span>
              <span class="age">{agoIso(r.run.started)}{#if runTook(r.run)}, {runTook(r.run)}{/if}</span>
            </div>
          </div>
          {@render more(r, i)}
        {:else if r.type === "issue"}
          <span class="istate" class:closed={r.issue.state !== "open"} aria-label={r.issue.state}></span>
          <div class="two">
            <div class="line1">
              <span class="prid mono">#{r.issue.number}</span>
              <span class="ptitle">{r.issue.title}</span>
            </div>
            <div class="line2">
              {#each r.issue.labels.slice(0, 3) as l}<span class="chip label" style={labelStyle(l.color)}>{l.name}</span>{/each}
              <span>{r.issue.author}</span>
              {#if r.issue.comments}<span>{r.issue.comments} {r.issue.comments === 1 ? "comment" : "comments"}</span>{/if}
              <span class="age">{agoIso(r.issue.updated)}</span>
            </div>
          </div>
          {@render more(r, i)}
        {/if}
      </div>
    {/each}
  </div>

  {#if app.tab === "changes" && !reviewing && s && (s.changes.length || app.canAmend(repo))}
    {@const stagedN = s.changes.filter((c) => c.staged).length}
    <div class="commitbox">
      <textarea
        class="field"
        bind:this={commitBox}
        rows="2"
        placeholder="Commit message"
        spellcheck="true"
        bind:value={() => app.commitMsg[repo] ?? "", (v) => (app.commitMsg = { ...app.commitMsg, [repo]: v })}
        onkeydown={commitKey}
        aria-label="Commit message"
      ></textarea>
      <div class="crow">
        <label class="check">
          <input type="checkbox" checked={app.pushAfterCommit} onchange={(e) => app.setPushAfterCommit(e.currentTarget.checked)} />
          Push after
        </label>
        {#if app.canAmend(repo)}
          <label class="check" use:hover={() => ({ title: "Amend the last commit", text: "Add the staged changes to the last commit and replace its message, instead of making a new commit. Only for a commit that is not pushed." })}>
            <input type="checkbox" checked={!!app.amend[repo]} onchange={(e) => app.setAmend(repo, e.currentTarget.checked)} />
            Amend
          </label>
        {/if}
        <span class="grow"></span>
        {#if !app.canAmend(repo)}<kbd>Ctrl Enter</kbd>{/if}
        <button class="btn" class:primary={!!(app.commitMsg[repo] ?? "").trim()} disabled={app.committing || !(app.commitMsg[repo] ?? "").trim() || (!s.changes.length && !app.amend[repo])} onclick={() => app.commit()}
          use:hover={() => ({ title: "Commit", hint: "Ctrl Enter, or Ctrl Shift Enter to commit and push" })}>
          {#if app.committing}Committing…{:else if app.amend[repo]}
            {stagedN ? `Amend with ${stagedN} staged` : "Amend the message"}{app.pushAfterCommit ? " and push" : ""}
          {:else}
            {stagedN ? `Commit ${stagedN} staged` : `Commit all ${s.changes.length}`}{app.pushAfterCommit ? " and push" : ""}
          {/if}
        </button>
      </div>
    </div>
  {/if}
</section>

<style>
  .pane {
    /* Local sizes. --row2 and --row-fold must match COMMIT_ROW and FOLD_ROW in the script (the graph is drawn to them). */
    --row2: 42px;
    --row-fold: 30px;
    --h-tabs: 36px;
    --h-review: 48px;
    --line1: 18px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
    container: listpane / inline-size;
  }

  /* Header */
  .head h1 {
    margin: 0;
    flex-shrink: 1;
  }
  .head .hint {
    margin-left: var(--s3);
    font-size: var(--fs-sm);
    font-weight: 400;
    color: var(--faint);
  }
  .branch {
    min-width: 0;
    flex-shrink: 1;
    max-width: 45%;
    gap: var(--s1);
  }
  .bname {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .local {
    color: var(--faint);
  }
  .caret {
    color: var(--faint);
  }
  .sync {
    display: flex;
    align-items: center;
    gap: var(--s1);
    flex-shrink: 0;
  }
  .sync .n {
    margin-left: var(--s1);
    font-variant-numeric: tabular-nums;
  }
  .sync .n.in {
    color: var(--blue);
  }
  .sync .n.out {
    color: var(--add);
  }

  /* Tabs */
  .tabbar {
    display: flex;
    height: var(--h-tabs);
    min-height: var(--h-tabs);
    padding: 0 var(--s3);
    border-bottom: 1px solid var(--line);
  }
  /* The tabs scroll sideways when they do not fit; the extra pixel keeps the chosen tab's underline on the line. */
  .tablist {
    flex: 1;
    overflow-x: auto;
    scrollbar-width: none;
    margin-bottom: -1px;
    padding-bottom: 1px;
  }
  .ticon {
    flex-shrink: 0;
  }
  /* The tabs rest on short words without icons, the same in every project (see fitTabs()). When words do not fit:
     less space (every count kept) → no counts → only the chosen tab labeled → icons. Words stay longest. */
  .tlabel.short {
    display: none;
  }
  .mode-short .tlabel.long,
  .mode-text .tlabel.long,
  .mode-tight .tlabel.long,
  .mode-bare .tlabel.long {
    display: none;
  }
  .mode-short .tlabel.short,
  .mode-text .tlabel.short,
  .mode-tight .tlabel.short,
  .mode-bare .tlabel.short {
    display: inline;
  }
  .mode-text :global(.ticon),
  .mode-tight :global(.ticon),
  .mode-bare :global(.ticon) {
    display: none;
  }
  /* "tight" keeps every count with less space between tabs, so choosing a tab never changes the others. */
  .mode-tight [role="tab"] {
    padding: 0 3px;
    gap: var(--s1);
  }
  /* Never less than 6px between words: the space comes out of the padding, not the gap. */
  .mode-tight {
    gap: var(--s2);
  }
  .mode-bare [role="tab"] {
    padding: 0 var(--s2);
  }
  .mode-bare [role="tab"]:not(.on) .badge {
    display: none;
  }
  .mode-active .tlabel.tlabel,
  .mode-icons .tlabel.tlabel {
    display: none;
  }
  /* "active": icons only, except the chosen tab keeps its (short) label. */
  .mode-active .on .tlabel:not(.long):not(.short),
  .mode-active .on .tlabel.short.short {
    display: inline;
  }
  @container listpane (max-width: 330px) {
    .count {
      display: none;
    }
  }

  /* Bars under the tabs */
  .filterbar .field {
    min-width: 80px;
  }
  .count {
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
  /* The review header has two lines: the title, then the meta. Each line ends with an ellipsis. */
  .toolbar.sub.review {
    height: var(--h-review);
    min-height: var(--h-review);
  }
  .rhead {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .rmeta {
    font-size: var(--fs-xs);
  }
  .basepick {
    max-width: 140px;
  }
  .selcount {
    font-size: var(--fs-sm);
    color: var(--text);
    font-weight: 600;
  }
  .commits-toggle {
    display: flex;
    align-items: center;
    gap: var(--s2);
    text-align: left;
    padding: var(--s1) var(--s4);
    font-size: var(--fs-sm);
    color: var(--muted);
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .commits-toggle:hover {
    color: var(--text);
  }
  .chevron {
    transition: transform 120ms var(--ease-out);
  }
  .chevron.open {
    transform: rotate(90deg);
  }
  .bcommits {
    list-style: none;
    margin: 0;
    padding: var(--s1) var(--s3);
    max-height: 160px;
    overflow-y: auto;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .bcommits li {
    display: flex;
    gap: var(--s3);
    align-items: baseline;
    padding: 0 var(--s1);
    line-height: var(--line1);
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .bcommits .sha {
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .csub {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* What a pull or a push would move. Information only: the buttons live in the header. */
  .syncbar {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    padding: var(--s2) var(--s4);
    font-size: var(--fs-sm);
    color: var(--muted);
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .syncbar.behind {
    background: color-mix(in srgb, var(--blue) 5%, var(--panel));
  }
  .syncline {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--s1) var(--s2);
    min-width: 0;
    min-height: var(--h-ctl-sm);
  }
  .syncline b {
    font-weight: 600;
    color: var(--text);
  }
  .syncbar .in,
  .foldtext .in {
    color: var(--blue);
    font-weight: 600;
  }
  .syncbar .out {
    color: var(--add);
    font-weight: 600;
  }
  .syncbar .ok {
    color: var(--add);
  }

  /* The list */
  .list {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: var(--s1) var(--s2) var(--s4);
    outline: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: 0 var(--s3);
    min-height: var(--h-row);
    border-radius: var(--radius);
    cursor: default;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.open {
    background: var(--raised);
  }
  .row.sel {
    background: var(--accent-soft);
  }
  /* The accent ring marks the keyboard cursor, only while the list has focus and the user moves with keys. */
  .list.kbd:focus-within .row.cursor {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  /* Row buttons show on hover, on the keyboard cursor, and while one has focus. */
  .row .hov {
    visibility: hidden;
  }
  .row:hover .hov,
  .list.kbd:focus-within .row.cursor .hov,
  .row:focus-within .hov {
    visibility: visible;
  }
  .row.header {
    min-height: var(--h-ctl);
    margin-top: var(--s2);
    padding: 0 var(--s1) 0 var(--s3);
  }
  .row.header:hover {
    background: none;
  }
  .row.header:first-child {
    margin-top: 0;
  }
  .hlabel {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--muted);
  }

  .kind {
    width: var(--s4);
    text-align: center;
    font-weight: 700;
    font-size: var(--fs-xs);
    flex-shrink: 0;
  }
  .fname {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
    max-width: 70%;
  }
  /* The folder part of a file path, cut at the start when long (so it must not be named .dir: folder rows use that). */
  .fpath {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: var(--fs-sm);
    direction: rtl;
    text-align: left;
    flex: 1;
    min-width: 0;
  }
  .from {
    color: var(--blue);
    font-size: var(--fs-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }
  .fresh {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--add);
    flex-shrink: 0;
  }
  .indent {
    flex-shrink: 0;
  }
  .chev {
    width: 10px;
    color: var(--faint);
    font-size: var(--fs-xs);
    flex-shrink: 0;
  }
  .dirname {
    color: var(--muted);
  }

  .two {
    min-width: 0;
    flex: 1;
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-width: 0;
    line-height: var(--line1);
  }
  .ptitle {
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .prid {
    color: var(--muted);
    flex-shrink: 0;
  }
  /* Meta lines use the UI font. Text truncates; chips, hashes, and ages keep their width. */
  .line2 {
    display: flex;
    align-items: center;
    gap: var(--s3);
    font-family: var(--ui);
    font-size: var(--fs-xs);
    color: var(--faint);
    overflow: hidden;
    min-width: 0;
  }
  .line2 > span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line2 > .chip,
  .line2 > .rv,
  .line2 > .age,
  .line2 > .loading,
  .line2 > .flow,
  .line2 > .sha {
    flex-shrink: 0;
    overflow: visible;
  }
  .line2 > .branches {
    flex-shrink: 3;
  }
  .loading {
    color: var(--muted);
  }
  .row.pr,
  .row.stash,
  .row.run,
  .row.issue {
    padding: var(--s2) var(--s3);
  }
  .row.task {
    padding: var(--s1) var(--s3);
  }

  /* Tasks */
  .tfrom {
    color: var(--muted);
  }
  .tstate {
    width: 14px;
    height: var(--line1);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--fs-xs);
    color: var(--faint);
    flex-shrink: 0;
  }
  /* With a second line, the state icon lines up with the first line, like the text. */
  .row.task:has(.line2) .tstate {
    align-self: flex-start;
  }
  .tstate.ok {
    color: var(--add);
  }
  .tstate.failed {
    color: var(--del);
  }
  .tstate.running::before {
    content: "";
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--add);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  .pin svg.pinned {
    color: var(--text);
  }
  .stashicon {
    color: var(--muted);
    flex-shrink: 0;
    align-self: flex-start;
    height: var(--line1);
    display: flex;
    align-items: center;
  }

  /* History: these heights must match COMMIT_ROW and FOLD_ROW, which the graph is drawn to. */
  .row.commit {
    height: var(--row2);
    min-height: var(--row2);
    padding: 0 var(--s3) 0 0;
    gap: var(--s1);
    border-radius: 0;
  }
  .row.commit .two {
    padding-left: var(--s1);
  }
  .row.fold {
    height: var(--row-fold);
    min-height: var(--row-fold);
    padding: 0 var(--s3) 0 0;
    gap: var(--s1);
    border-radius: 0;
  }
  .row.nograph {
    padding-left: var(--s3);
  }
  .foldtext {
    font-size: var(--fs-sm);
    padding-left: var(--s1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.incoming:not(.open):not(.sel):not(:hover),
  .row.fold:not(.open):not(:hover) {
    background: color-mix(in srgb, var(--blue) 6%, transparent);
  }
  /* Fade only: these are commits to read, so they do not move. */
  .row.reveal {
    animation: tern-fade-in 180ms var(--ease-out);
  }
  .flow {
    font-weight: 700;
    flex-shrink: 0;
  }
  .flow.in {
    color: var(--blue);
  }
  .flow.out {
    color: var(--add);
  }
  .sha {
    color: var(--faint);
    font: var(--fs-xs) var(--mono);
    padding: 0 var(--s1);
    margin: 0 calc(-1 * var(--s1));
    border-radius: var(--r-sm);
  }
  .sha:hover {
    color: var(--text);
    background: var(--hover);
  }
  /* Ref chips: at most two show, then a +N chip. They shrink before the subject drops under 80px. */
  .ref {
    flex: 0 1 auto;
    min-width: 3ch;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
    line-height: 16px;
  }
  .ref-head {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .ref-remote {
    color: var(--blue);
  }
  .ref-tag {
    color: var(--add);
  }
  .subject {
    flex: 1 1 0;
    min-width: 80px;
  }

  /* CI */
  .cistate {
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    color: var(--faint);
  }
  .row .cistate {
    align-self: flex-start;
    line-height: var(--line1);
  }
  .cistate.r-succeeded {
    color: var(--add);
  }
  .cistate.r-partial {
    color: var(--accent);
  }
  .cistate.r-failed {
    color: var(--del);
  }
  .cistate.s-running {
    color: var(--blue);
    animation: pulse 1.4s ease-in-out infinite;
  }

  /* Issues and pull requests */
  .istate {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 2px solid var(--add);
    flex-shrink: 0;
    align-self: flex-start;
    margin-top: calc((var(--line1) - 8px) / 2);
  }
  .istate.closed {
    border-color: var(--syn-keyword);
    background: var(--syn-keyword);
  }
  /* Issue labels: a chip tinted with the label's own color. */
  .label {
    color: var(--text);
    background: color-mix(in srgb, var(--lc) 28%, transparent);
    border-color: color-mix(in srgb, var(--lc) 55%, transparent);
  }
  .rv-approved {
    color: var(--add);
  }
  .rv-changes {
    color: var(--del);
  }
  .rv-waiting {
    color: var(--muted);
  }
  .rv-none {
    display: none;
  }

  /* States */
  .list .state {
    white-space: normal;
  }
  .state pre {
    word-break: break-word;
  }
  /* Skeleton rows wait 300 ms before they show, so a fast load shows nothing. */
  .skel {
    display: flex;
    flex-direction: column;
    animation: tern-fade-in 200ms 300ms both;
  }
  .skelrow {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: var(--s2);
    height: var(--h-row);
    padding: 0 var(--s3);
  }
  .skelrow.two {
    height: var(--row2);
  }
  .skelrow .skeleton {
    height: var(--s3);
  }
  .skelrow .skeleton.thin {
    height: var(--s2);
  }

  /* Commit box */
  .commitbox {
    border-top: 1px solid var(--line);
    padding: var(--s3) var(--s3) var(--s3);
    background: var(--panel);
    flex-shrink: 0;
  }
  .commitbox textarea {
    display: block;
    width: 100%;
    min-height: calc(var(--h-ctl) * 2);
    max-height: 200px;
    font-size: var(--fs-md);
    user-select: text;
  }
  .crow {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin-top: var(--s2);
  }
</style>
