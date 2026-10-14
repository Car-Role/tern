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
  let filter = $state("");
  let expanded = $state<Record<string, string[]>>({});
  let commitBox: HTMLTextAreaElement | undefined = $state();
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

  const rows = $derived.by((): Row[] => {
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
    return hist.map((c): Row =>
      c.fold
        ? { type: "fold", id: FOLD, count: c.fold }
        : { type: "commit", id: c.sha, commit: c, flow: incoming.has(c.sha) ? "in" : outgoing.has(c.sha) ? "out" : null },
    );
  });

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
  const graph = $derived(app.tab === "history" && !reviewing ? layoutGraph(hist) : null);
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
      hint: "Enter reads it here. Right-click to copy it for an agent.",
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

  // Keep the cursor on the file that is open, so arrow keys continue from there.
  $effect(() => {
    const v = app.view;
    if (!v) return;
    const at = untrack(() => cursor);
    if (rows[at] && rowPath(rows[at]) === v.rel) return;
    const i = rows.findIndex((r) => (r.type === "change" || r.type === "file") && rowPath(r) === v.rel);
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
    changes: "Changes", files: "Files", prs: "PRs", issues: "Issues", history: "History", ci: "CI", tasks: "Tasks",
  };
  // Each space picks its own tabs. Ctrl+1, Ctrl+2, and so on follow this order.
  const tabs = $derived(app.visibleTabs.map((id, i) => ({ id, label: tabLabel[id], key: String(i + 1) })));

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
</script>

<section class="pane" aria-label="Project">
  <!-- Right-click here is a mouse shortcut. Shift+F10 opens the same menu from the keyboard. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="head" oncontextmenu={(e) => repoMenu(e, repo)}>
    <div class="title">
      <h1 title={repo}>{app.repo?.name ?? baseName(repo)}{#if app.nameHint[repo]}<span class="hint">{app.nameHint[repo]}</span>{/if}</h1>
      <button class="branch mono" onclick={() => app.openBranches("branches")}
        use:hover={() => ({ title: s?.branch ?? "Detached HEAD", rows: s?.upstream ? [{ label: "Tracks", value: s.upstream, mono: true }] : [{ label: "Upstream", value: "none, local only" }], hint: "Click or Ctrl+B for branches and worktrees" })}>
        {s?.branch ?? "detached"}{#if s && !s.upstream}<span class="local">local only</span>{/if}<span class="caret">▾</span>
      </button>
    </div>
    <div class="actions">
      <button class="act" onclick={() => (app.overlay = "search")} aria-label="Search this repo"
        use:hover={() => ({ title: "Search this repo", text: "Tab widens it to the space or to all repos.", hint: "Ctrl+Shift+F" })}><svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M10.4 10.4 14 14" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg></button>
      <button class="act" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "fetch")} title="Fetch">
        {app.busy[repo] === "fetch" ? "Fetching…" : "Fetch"}
      </button>
      <button class="act" class:hot={!!s?.behind} disabled={!!app.busy[repo]} onclick={() => app.action(repo, "pull")} use:hover={() => syncCard("pull")}>
        Pull{#if s?.behind}<span class="n">↓{s.behind}</span>{/if}
      </button>
      <button class="act" class:hot={!!s?.ahead} disabled={!!app.busy[repo]} onclick={() => app.action(repo, "push")} use:hover={() => syncCard("push")}>
        Push{#if s?.ahead}<span class="n">↑{s.ahead}</span>{/if}
      </button>
    </div>
  </header>

  <div class="tabs" role="tablist">
    {#each tabs as t}
      <button role="tab" class="tab" class:on={app.tab === t.id} aria-selected={app.tab === t.id} onclick={() => app.setTab(t.id)} title="Ctrl+{t.key}">
        {t.label}
        {#if t.id === "changes" && s?.changes.length}<span class="badge">{s.changes.length}</span>{/if}
        {#if t.id === "prs" && app.prs[repo]?.items.length}<span class="badge">{app.prs[repo].items.length}</span>{/if}
        {#if t.id === "issues" && app.issues[repo]?.items.length}<span class="badge">{app.issues[repo].items.length}</span>{/if}
        {#if t.id === "ci"}
          {@const mine = app.ci[repo]?.items.find((x) => x.branch === s?.branch)}
          {#if mine}<span class="cistate r-{mine.result} s-{mine.status}" use:popOnDone={mine.status}>{runIcon(mine)}</span>{/if}
        {/if}
        {#if t.id === "tasks" && app.runningIn(repo)}<span class="badge live">{app.runningIn(repo)}</span>{/if}
      </button>
    {/each}
  </div>

  {#if app.tab === "changes" && app.review?.kind !== "stash" && (app.canReviewBranch(repo) || app.review?.kind === "branch")}
    <div class="toolbar">
      <div class="seg" role="group" aria-label="What to show">
        <button class:on={!reviewing} onclick={() => reviewing && app.closeReview()}>Uncommitted</button>
        <button class:on={reviewing} onclick={() => !reviewing && app.openBranchReview()}>Since {app.review?.baseRef ?? "main"}</button>
      </div>
      {#if reviewing && app.review?.kind === "branch"}
        <button class="hbtn" onclick={() => app.copyBranchSummary()}>Copy for an agent</button>
      {/if}
    </div>
  {/if}

  {#if reviewing && app.review}
    <div class="review">
      <button class="back" onclick={() => app.closeReview()} title="Back (Backspace)" aria-label="Back">←</button>
      <div class="rtext">
        <div class="rtitle">{app.review.title}</div>
        <div class="rsub mono">{app.review.subtitle}, {app.review.files.length} files</div>
      </div>
      {#if app.review.pr}
        <button class="act small" onclick={() => app.openPrInBrowser()}>Open</button>
      {/if}
      {#if app.review.stash}
        {@const st = app.review.stash}
        <button class="act small" onclick={() => app.applyStash(st, true)} title="Apply it and remove it from the list">Pop</button>
        <button class="act small" onclick={(e) => stashMenu(e, st)} aria-label="More stash actions">⋯</button>
      {/if}
      {#if app.review.kind === "branch" && (app.review.candidates?.length ?? 0) > 1}
        <select class="basepick" value={app.review.baseRef} onchange={(e) => app.openBranchReview(e.currentTarget.value)} aria-label="Compare with">
          {#each app.review.candidates ?? [] as c}<option value={c}>{c}</option>{/each}
        </select>
      {/if}
    </div>
    {#if app.review.kind === "branch" && app.review.commits?.length}
      <button class="commits-toggle" onclick={() => (showCommits = !showCommits)} aria-expanded={showCommits}>
        {showCommits ? "▾" : "▸"} {app.review.commits.length} {app.review.commits.length === 1 ? "commit" : "commits"} on this branch
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

  {#if app.tab === "files"}
    <div class="filter">
      <input
        placeholder="Filter files"
        bind:value={filter}
        onkeydown={onKey}
        spellcheck="false"
        aria-label="Filter files"
      />
    </div>
  {/if}

  {#if app.tab === "tasks"}
    <form class="runline" onsubmit={(e) => { e.preventDefault(); app.runCommand(command); }}>
      <input bind:value={command} placeholder="Run a command here, for example npm install" spellcheck="false" aria-label="Command to run" />
      <button type="submit" disabled={!command.trim()}>Run</button>
      <button type="button" disabled={!command.trim()} onclick={() => { app.saveCustomTask(repo, command, command); command = ""; }}
        title="Save this command in the Tasks list for this project">Save</button>
    </form>
  {/if}

  {#if app.tab === "history" && !reviewing}
    <div class="toolbar">
      <label class="switch">
        <input type="checkbox" checked={app.logAll} onchange={(e) => app.setLogAll(e.currentTarget.checked)} />
        All branches
      </label>
      <span class="faint">{app.logs[repo]?.length ?? 0} commits</span>
    </div>
    {#if sync}
      {@const here = sync.branch ?? "HEAD"}
      <div class="syncbar" class:behind={sync.behind > 0}>
        {#if !sync.upstream}
          <span class="faint">{here} has no remote branch. Push it to make one.</span>
        {:else}
          <div class="syncline">
            {#if sync.behind}
              <span class="in">↓ {sync.behind}</span>
              <span>{sync.behind === 1 ? "commit" : "commits"} on <b class="mono">{sync.upstream}</b> not pulled into <b class="mono">{here}</b></span>
            {:else}
              <span class="ok">✓</span><span><b class="mono">{here}</b> has everything on <b class="mono">{sync.upstream}</b></span>
            {/if}
            {#if sync.ahead}<span class="out">↑ {sync.ahead} to push</span>{/if}
          </div>
          <div class="syncline">
            <span class="faint">as of the last fetch, {ago(s?.fetchedAt)}</span>
            <span class="grow"></span>
            <button class="hbtn" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "fetch")}>{app.busy[repo] === "fetch" ? "Fetching…" : "Fetch"}</button>
            {#if sync.behind}
              <button class="hbtn" disabled={app.reviewLoading === "incoming"} onclick={() => app.reviewIncoming(repo)}
                title="One diff of everything the pull brings">{app.reviewLoading === "incoming" ? "Loading…" : "Review"}</button>
              <button class="hbtn primary" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "pull")}>{app.busy[repo] === "pull" ? "Pulling…" : "Pull"}</button>
            {/if}
            {#if sync.ahead}
              <button class="hbtn" disabled={!!app.busy[repo]} onclick={() => app.action(repo, "push")}>{app.busy[repo] === "push" ? "Pushing…" : "Push"}</button>
            {/if}
          </div>
          {#if unfolded[repo] && incoming.size > 7}
            <button class="link small" onclick={() => (unfolded = { ...unfolded, [repo]: false })}>Fold the incoming commits again</button>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}

  {#if app.tab === "issues"}
    <div class="toolbar">
      <div class="seg" role="group" aria-label="Which issues">
        {#each [["open", "Open"], ["closed", "Closed"], ["all", "All"]] as [id, label]}
          <button class:on={app.issueState === id} onclick={() => app.setIssueState(id as "open" | "closed" | "all")}>{label}</button>
        {/each}
      </div>
      <button class="hbtn" onclick={() => app.loadIssues(repo, true)}>Refresh</button>
    </div>
  {/if}

  {#if app.selected.length}
    <div class="selbar">
      <span>{app.selected.length} selected</span>
      <button onclick={() => app.copy("mention")}>Copy @</button>
      <button onclick={() => app.copy("relative")}>Copy paths</button>
      {#if app.tab === "changes" || reviewing}<button onclick={() => app.copy("diff")}>Copy diff</button>{/if}
      <button class="faint" onclick={() => (app.selected = [])}>Clear</button>
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="list" bind:this={listEl} tabindex="0" onkeydown={onKey} role="listbox" aria-label="Items">
    {#if app.tab === "changes" && !reviewing && s && !s.changes.length}
      <div class="empty">
        <p>No changes. The working tree matches the last commit.</p>
        {#if s.behind}<p><button class="link" onclick={() => app.action(repo, "pull")}>Pull {s.behind} new commits</button></p>{/if}
      </div>
    {:else if app.tab === "files" && !app.files[repo]}
      <p class="empty">Reading files…</p>
    {:else if app.tab === "prs" && !reviewing}
      {@const pr = app.prs[repo]}
      {#if pr?.loading && !pr.items.length}
        <p class="empty">Loading pull requests…</p>
      {:else if pr?.error}
        <div class="empty">
          <p class="err mono">{pr.error}</p>
          <p>{prHint(pr.error)}</p>
          <button class="link" onclick={() => app.loadPrs(repo, true)}>Try again</button>
        </div>
      {:else if pr && !pr.items.length}
        <div class="empty">
          <p>No open pull requests.</p>
          <button class="link" onclick={() => app.openNewPr()}>Create one for {s?.branch ?? "this branch"}</button>
        </div>
      {/if}
    {:else if app.tab === "history" && !reviewing && !app.logs[repo]}
      <p class="empty">Reading history…</p>
    {:else if app.tab === "ci"}
      {@const c = app.ci[repo]}
      {#if c?.loading && !c.items.length}
        <p class="empty">Loading CI runs…</p>
      {:else if c?.error}
        <div class="empty">
          <p class="err mono">{c.error}</p>
          {#if s?.provider === "azdo"}<p>Check that az is signed in: run az login in a terminal.</p>{:else if s?.provider === "github"}<p>Check that gh is signed in: run gh auth login in a terminal.</p>{/if}
          <button class="link" onclick={() => app.loadCi(repo, true)}>Try again</button>
        </div>
      {:else if c && !c.items.length}
        <p class="empty">No pipeline runs found for this repo.</p>
      {/if}
    {:else if app.tab === "issues"}
      {@const iss = app.issues[repo]}
      {#if iss?.loading && !iss.items.length}
        <p class="empty">Loading issues…</p>
      {:else if iss?.error}
        <div class="empty">
          <p class="err mono">{iss.error}</p>
          {#if s?.provider === "github"}<p>Check that gh is signed in: run gh auth login in a terminal.</p>{/if}
          <button class="link" onclick={() => app.loadIssues(repo, true)}>Try again</button>
        </div>
      {:else if iss && !iss.items.length}
        <p class="empty">No {app.issueState === "all" ? "" : app.issueState} issues.</p>
      {/if}
    {:else if app.tab === "tasks" && !app.tasks[repo]}
      <p class="empty">Looking for tasks…</p>
    {:else if app.tab === "tasks" && !app.tasks[repo]?.length}
      <p class="empty">
        Tern found no npm scripts, VS Code tasks, .NET, Gradle, Cargo, Make, or Docker Compose files here.
        Type a command above and press Save to add one.
      </p>
    {/if}

    {#each rows as r, i (r.id)}
      {@const path = rowPath(r)}
      <div
        class="row {r.type}"
        class:incoming={r.type === "commit" && r.flow === "in"}
        class:reveal={r.type === "commit" && revealed.has(r.commit.sha)}
        class:fx-fade={r.type === "commit" && revealed.has(r.commit.sha)}
        class:cursor={i === cursor}
        class:open={path !== null && app.view?.rel === path}
        class:sel={path !== null && app.selected.includes(path)}
        data-i={i}
        role="option"
        aria-selected={path !== null && app.selected.includes(path)}
        tabindex="-1"
        onclick={(e) => click(e, i)}
        onkeydown={() => {}}
        oncontextmenu={(e) => {
          if (r.type === "change") fileMenu(e, r.change.path, r.change);
          else if (r.type === "file") fileMenu(e, r.path);
          else if (r.type === "commit") commitMenu(e, r.commit);
          else if (r.type === "pr") prMenu(e, r.pr);
          else if (r.type === "issue") issueMenu(e, r.issue);
          else if (r.type === "run") ciMenu(e, r.run);
          else if (r.type === "stash") stashMenu(e, r.stash);
          else if (r.type === "task") taskMenu(e, r.task, r.custom);
          else if (r.type === "dir") fileMenu(e, r.path);
          else return;
          cursor = anchor = i;
        }}
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
            <button class="hbtn" onclick={() => app.unstage("all")} title="Unstage all">Unstage all</button>
          {:else if r.section === "unstaged"}
            <button class="hbtn" onclick={() => app.stage("all")} title="Stage all">Stage all</button>
          {/if}
          {#if r.section === "staged" || r.section === "unstaged"}
            {@const sec = r.section}
            <button class="hbtn more" aria-label="Stash or discard"
              onclick={(e) => { e.stopPropagation(); const b = e.currentTarget.getBoundingClientRect(); sectionMenu({ x: b.left, y: b.bottom + 4 }, sec); }}
              use:hover={() => ({ title: "Stash or discard", text: "Stash these changes, or throw them away." })}>⋯</button>
          {/if}
        {:else if r.type === "stash"}
          <span class="stashicon" aria-hidden="true">
            <svg viewBox="0 0 16 16" width="13" height="13"><path d="M2.5 5.5h11v7a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1zM2 3h12v2.5H2zM6.5 8h3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" stroke-linecap="round"/></svg>
          </span>
          <div class="two">
            <div class="line1"><span class="ptitle">{r.stash.message}</span></div>
            <div class="line2 mono">
              {#if app.reviewLoading === r.stash.sha}<span class="amber">loading…</span>{/if}
              <span>{r.stash.branch || "detached"}</span>
              <span class="faint">{r.stash.files + r.stash.untracked.length} {r.stash.files + r.stash.untracked.length === 1 ? "file" : "files"}</span>
              <span class="faint">{ago(r.stash.time)}</span>
            </div>
          </div>
          <button class="tbtn hover" onclick={(e) => { e.stopPropagation(); app.applyStash(r.stash, true); }} title="Apply it and remove it from the list">Pop</button>
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
              <div class="line2 mono">
                {#if where}<span class="tfrom">{where}</span>{/if}
                {#if r.task.detail && r.task.detail !== r.task.name}<span class="faint">{r.task.detail}</span>{/if}
              </div>
            {/if}
          </div>
          <button class="tbtn pin" class:on={r.pinned} onclick={(e) => { e.stopPropagation(); app.toggleTaskPin(repo, r.task); }}
            title={r.pinned ? "Unpin (P)" : "Pin to the top (P)"} aria-label={r.pinned ? "Unpin" : "Pin to the top"}>
            <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true"><path d="M10 1.5l4.5 4.5-2 .5-2.6 2.6.6 3.4-1.5 1.5-2.8-2.8L2.5 15 1 13.5l3.7-3.7L1.9 7l1.5-1.5 3.4.6L9.4 3.5z" fill={r.pinned ? "currentColor" : "none"} stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>
          </button>
          {#if last?.running}
            <button class="tbtn" onclick={(e) => { e.stopPropagation(); app.showRun(last.id); }} title="Show output">Output</button>
            <button class="tbtn danger" onclick={(e) => { e.stopPropagation(); app.stopRun(last.id); }} title="Stop (X)">Stop</button>
          {:else if r.custom !== null}
            <button class="tbtn hover" onclick={(e) => { e.stopPropagation(); app.removeCustomTask(repo, r.custom!); }} title="Remove this saved command">Remove</button>
          {/if}
        {:else if r.type === "change"}
          <span class="kind mono k-{r.change.kind}">{kindLetter[r.change.kind]}</span>
          <span class="fname">{baseName(r.change.path)}</span>
          {#if r.change.oldPath}
            <span class="from">
              from {dirName(r.change.oldPath) === dirName(r.change.path) ? baseName(r.change.oldPath) : r.change.oldPath}
            </span>
          {/if}
          <span class="dir">&lrm;{dirName(r.change.path)}&lrm;</span>
          {#if isHot(r.change)}<span class="fresh"></span>{/if}
          {#if r.section}
            <button
              class="stagebtn"
              title={r.section === "staged" ? "Unstage (S)" : "Stage (S)"}
              aria-label={r.section === "staged" ? "Unstage" : "Stage"}
              onclick={(e) => { e.stopPropagation(); r.section === "staged" ? app.unstage([r.change]) : app.stage([r.change]); }}
            >{r.section === "staged" ? "−" : "+"}</button>
          {/if}
        {:else if r.type === "dir"}
          <span class="indent" style:width="{r.depth * 14}px"></span>
          <span class="chev">{r.open ? "▾" : "▸"}</span>
          <span class="fname dirname">{r.name}</span>
        {:else if r.type === "file"}
          <span class="indent" style:width="{r.depth * 14}px"></span>
          <span class="chev"></span>
          <span class="fname">{r.depth === 0 && filter ? baseName(r.name) : r.name}</span>
          {#if filter}<span class="dir">&lrm;{dirName(r.path)}&lrm;</span>{/if}
        {:else if r.type === "pr"}
          <div class="two">
            <div class="line1">
              <span class="prid mono">#{r.pr.id}</span>
              <span class="ptitle">{r.pr.title}</span>
            </div>
            <div class="line2 mono">
              {#if app.reviewLoading === String(r.pr.id)}<span class="amber">loading…</span>{/if}
              {#if r.pr.draft}<span class="tag">draft</span>{/if}
              <span class="rv rv-{r.pr.review}">{{ approved: "approved", changes: "changes asked", waiting: "needs review", none: "" }[r.pr.review]}</span>
              <span>{r.pr.author}</span>
              <span class="faint">{r.pr.source} → {r.pr.target}</span>
              <span class="faint">{agoIso(r.pr.updated)}</span>
            </div>
          </div>
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
          {#if graph?.rows[i]}
            <GraphCell row={graph.rows[i]} lanes={graph.lanes} height={COMMIT_ROW} head={r.commit.refs.startsWith("HEAD")} />
          {/if}
          <div class="two">
            <div class="line1">
              {#each parseRefs(r.commit.refs) as ref}<span class="ref ref-{ref.kind}">{ref.name}</span>{/each}
              <span class="ptitle">{r.commit.subject}</span>
            </div>
            <div class="line2 mono">
              {#if r.flow === "in"}<span class="flow in" title="Not pulled yet: only on {sync?.upstream}">↓</span>
              {:else if r.flow === "out"}<span class="flow out" title="Not pushed yet: only on this computer">↑</span>{/if}
              {#if app.reviewLoading === r.commit.sha}<span class="amber">loading…</span>{/if}
              <button class="sha" onclick={(e) => { e.stopPropagation(); app.copyValue(r.commit.short, `commit ID ${r.commit.short}`); }}
                aria-label="Copy commit ID {r.commit.short}">{r.commit.short}</button>
              <span>{r.commit.author}</span>
              <span class="faint">{ago(r.commit.time)}</span>
            </div>
          </div>
        {:else if r.type === "run"}
          <span class="cistate r-{r.run.result} s-{r.run.status}" use:popOnDone={r.run.status}>{runIcon(r.run)}</span>
          <div class="two">
            <div class="line1">
              <span class="ptitle">{r.run.title || r.run.name}</span>
            </div>
            <div class="line2 mono">
              <span class="faint">#{r.run.number}</span>
              <span>{r.run.branch}</span>
              <span class="faint">{r.run.by}</span>
              <span class="faint">{agoIso(r.run.started)}{#if runTook(r.run)}, {runTook(r.run)}{/if}</span>
            </div>
          </div>
        {:else if r.type === "issue"}
          <span class="istate" class:closed={r.issue.state !== "open"} aria-label={r.issue.state}></span>
          <div class="two">
            <div class="line1">
              <span class="prid mono">#{r.issue.number}</span>
              <span class="ptitle">{r.issue.title}</span>
            </div>
            <div class="line2">
              {#each r.issue.labels.slice(0, 3) as l}<span class="label" style={labelStyle(l.color)}>{l.name}</span>{/each}
              <span class="mono">{r.issue.author}</span>
              {#if r.issue.comments}<span class="faint mono">{r.issue.comments} {r.issue.comments === 1 ? "comment" : "comments"}</span>{/if}
              <span class="faint mono">{agoIso(r.issue.updated)}</span>
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if app.tab === "changes" && !reviewing && s && (s.changes.length || app.canAmend(repo))}
    {@const stagedN = s.changes.filter((c) => c.staged).length}
    <div class="commit">
      <textarea
        bind:this={commitBox}
        rows="2"
        placeholder="Commit message"
        spellcheck="true"
        bind:value={() => app.commitMsg[repo] ?? "", (v) => (app.commitMsg = { ...app.commitMsg, [repo]: v })}
        onkeydown={commitKey}
        aria-label="Commit message"
      ></textarea>
      <div class="crow">
        <label class="push">
          <input type="checkbox" checked={app.pushAfterCommit} onchange={(e) => app.setPushAfterCommit(e.currentTarget.checked)} />
          Push after
        </label>
        {#if app.canAmend(repo)}
          <label class="push" use:hover={() => ({ title: "Amend the last commit", text: "Add the staged changes to the last commit and replace its message, instead of making a new commit. Only for a commit that is not pushed." })}>
            <input type="checkbox" checked={!!app.amend[repo]} onchange={(e) => app.setAmend(repo, e.currentTarget.checked)} />
            Amend
          </label>
        {/if}
        {#if !app.canAmend(repo)}<span class="faint keys"><kbd>Ctrl Enter</kbd></span>{/if}
        <button class="cbtn" disabled={app.committing || !(app.commitMsg[repo] ?? "").trim() || (!s.changes.length && !app.amend[repo])} onclick={() => app.commit()}>
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
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 12px 12px 8px 14px;
  }
  .title {
    min-width: 0;
  }
  h1 {
    font-size: 15px;
    font-weight: 650;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .branch {
    color: var(--muted);
    font-size: 11.5px;
  }
  h1 .hint {
    margin-left: 8px;
    font-size: 12px;
    font-weight: 400;
    color: var(--faint);
  }
  button.branch {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px;
    margin-left: -4px;
    border-radius: 4px;
  }
  button.branch:hover {
    background: var(--hover);
    color: var(--text);
  }
  .caret {
    font-size: 9px;
    color: var(--faint);
    margin-left: 3px;
  }
  .local {
    margin-left: 6px;
    color: var(--faint);
    font-family: var(--ui);
  }
  .actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
  .act {
    font-size: 12px;
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
    display: inline-flex;
    gap: 4px;
    align-items: center;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .act:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .act:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .act.hot {
    color: var(--text);
    border-color: var(--amber);
  }
  .act .n {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--amber);
  }
  .act.small {
    padding: 2px 7px;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 10px;
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tab {
    padding: 6px 6px 7px;
    color: var(--muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }
  .tab:hover {
    color: var(--text);
  }
  .tab.on {
    color: var(--text);
    border-bottom-color: var(--amber);
  }
  .badge {
    font-size: 10.5px;
    font-family: var(--mono);
    color: var(--muted);
    background: var(--raised);
    border-radius: 8px;
    padding: 0 5px;
  }
  .review {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .back {
    width: 24px;
    height: 24px;
    border-radius: var(--radius);
    color: var(--muted);
    flex-shrink: 0;
  }
  .back:hover {
    background: var(--hover);
    color: var(--text);
  }
  .rtext {
    min-width: 0;
    flex: 1;
  }
  .rtitle {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rsub {
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .filter {
    padding: 8px 10px 4px;
  }
  .filter input {
    width: 100%;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px 8px;
    outline: none;
  }
  .filter input:focus {
    border-color: var(--amber);
  }
  .selbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
    font-size: 12px;
    background: var(--amber-soft);
    color: var(--text);
  }
  .selbar span {
    flex: 1;
  }
  .selbar button {
    color: var(--amber);
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 6px 12px;
    outline: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 8px;
    min-height: 26px;
    border-radius: 5px;
    cursor: default;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.open {
    background: var(--raised);
  }
  .row.sel {
    background: var(--amber-soft);
  }
  .list:focus-within .row.cursor,
  .list:focus .row.cursor {
    box-shadow: inset 0 0 0 1px var(--amber);
  }
  .kind {
    width: 12px;
    text-align: center;
    font-weight: 700;
    font-size: 11px;
    flex-shrink: 0;
  }
  .fname {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
    max-width: 70%;
  }
  .dir {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
    direction: rtl;
    text-align: left;
    flex: 1;
    min-width: 0;
  }
  .from {
    color: var(--blue);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }
  .runline {
    display: flex;
    gap: 6px;
    padding: 8px 10px 4px;
  }
  .runline input {
    flex: 1;
    min-width: 0;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px 8px;
    outline: none;
    font-family: var(--mono);
    font-size: 12px;
  }
  .runline input:focus {
    border-color: var(--amber);
  }
  .runline button {
    font-size: 12px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .runline button:hover:not(:disabled) {
    color: var(--text);
  }
  .runline button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .tbtn.pin {
    display: flex;
    align-items: center;
    padding: 3px 5px;
    color: var(--faint);
    opacity: 0;
  }
  .row:hover .tbtn.pin,
  .row.cursor .tbtn.pin {
    opacity: 1;
  }
  .tbtn.pin:hover {
    color: var(--text);
  }
  .tbtn.pin.on {
    color: var(--amber);
    opacity: 1;
  }
  .tfrom {
    color: var(--muted);
  }
  .row.task {
    padding: 4px 8px;
  }
  .tstate {
    width: 14px;
    text-align: center;
    font-size: 11px;
    color: var(--faint);
    flex-shrink: 0;
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
  .tbtn {
    font-size: 11.5px;
    color: var(--muted);
    padding: 1px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .tbtn:hover {
    color: var(--text);
    background: var(--line);
  }
  .tbtn.danger:hover {
    color: var(--del);
  }
  .tbtn.hover {
    visibility: hidden;
  }
  .row:hover .tbtn.hover,
  .row.cursor .tbtn.hover {
    visibility: visible;
  }
  .badge.live {
    color: var(--add);
  }
  .row.header {
    min-height: 24px;
    margin-top: 6px;
    padding-right: 4px;
  }
  .row.header:hover {
    background: none;
  }
  .row.header:first-child {
    margin-top: 0;
  }
  .hlabel {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .grow {
    flex: 1;
  }
  .hbtn {
    font-size: 11.5px;
    color: var(--faint);
    padding: 0 4px;
    border-radius: 4px;
  }
  .hbtn:hover {
    color: var(--amber);
  }
  .fresh {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--add);
    flex-shrink: 0;
  }
  .stagebtn {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    color: var(--muted);
    font-family: var(--mono);
    visibility: hidden;
    flex-shrink: 0;
    margin-left: auto;
  }
  .row:hover .stagebtn,
  .row.cursor .stagebtn {
    visibility: visible;
  }
  .stagebtn:hover {
    background: var(--line);
    color: var(--text);
  }
  .commit {
    border-top: 1px solid var(--line);
    padding: 8px 10px 10px;
    background: var(--panel);
  }
  .commit textarea {
    width: 100%;
    resize: vertical;
    min-height: 44px;
    max-height: 200px;
    background: var(--bg);
    color: var(--text);
    font: 13px/1.4 var(--ui);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 6px 8px;
    outline: none;
    user-select: text;
  }
  .commit textarea:focus {
    border-color: var(--amber);
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
  }
  .push {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--muted);
  }
  .push input {
    accent-color: var(--amber);
    margin: 0;
  }
  .keys {
    margin-left: auto;
  }
  .cbtn {
    font-size: 12px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: var(--radius);
    background: var(--amber);
    color: var(--on-accent);
  }
  .cbtn:disabled {
    background: var(--raised);
    color: var(--faint);
    cursor: default;
  }
  .indent {
    flex-shrink: 0;
  }
  .chev {
    width: 10px;
    color: var(--faint);
    font-size: 10px;
    flex-shrink: 0;
  }
  .dirname {
    color: var(--muted);
  }
  .row.pr,
  .row.commit {
    padding: 6px 8px;
  }
  .two {
    min-width: 0;
    flex: 1;
  }
  .line1 {
    display: flex;
    gap: 6px;
    min-width: 0;
  }
  .ptitle {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prid {
    color: var(--muted);
  }
  .line2 {
    display: flex;
    gap: 8px;
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    margin-top: 1px;
  }
  .sha {
    color: var(--amber);
    font: inherit;
    padding: 0 3px;
    margin: 0 -3px;
    border-radius: 3px;
  }
  .sha:hover {
    background: var(--amber-soft);
  }
  .row.commit {
    height: 42px;
    min-height: 42px;
    padding: 0 8px 0 0;
    gap: 4px;
    border-radius: 0;
  }
  .row.commit .two {
    padding-left: 2px;
  }
  .ref {
    flex-shrink: 0;
    font: 11px/16px var(--mono);
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid var(--line-strong);
    color: var(--muted);
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ref-head {
    color: var(--amber);
    border-color: var(--amber);
  }
  .ref-remote {
    color: var(--blue);
    border-color: transparent;
    background: var(--hover);
  }
  .ref-tag {
    color: var(--add);
    border-color: transparent;
    background: var(--add-bg);
  }
  .basepick {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-size: 12px;
    padding: 1px 4px;
    flex-shrink: 0;
  }
  .commits-toggle {
    text-align: left;
    padding: 5px 12px;
    font-size: 12px;
    color: var(--muted);
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .commits-toggle:hover {
    color: var(--text);
  }
  .bcommits {
    list-style: none;
    margin: 0;
    padding: 4px 8px;
    max-height: 160px;
    overflow-y: auto;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .bcommits li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 2px 4px;
    font-size: 12px;
    white-space: nowrap;
  }
  .bcommits .sha {
    font-family: var(--mono);
    font-size: 11px;
  }
  .csub {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .syncbar {
    padding: 7px 12px 8px;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .syncbar.behind {
    background: color-mix(in srgb, var(--blue) 7%, transparent);
  }
  .syncline {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 6px;
    min-width: 0;
  }
  .syncline b {
    font-weight: 600;
    font-size: 11.5px;
  }
  .syncline .grow {
    flex: 1;
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
  .hbtn.primary {
    color: var(--on-accent);
    background: var(--amber);
    border-color: var(--amber);
  }
  .link.small {
    font-size: 11.5px;
    align-self: flex-start;
  }
  /* Fade only: these are commits to read, so they do not move. */
  .row.reveal {
    animation: tern-fade-in 180ms var(--ease-out);
  }
  .row.fold {
    height: 30px;
    min-height: 30px;
    padding: 0 8px 0 0;
    gap: 4px;
    border-radius: 0;
  }
  .foldtext {
    font-size: 12px;
    padding-left: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .flow {
    font-weight: 700;
    flex-shrink: 0;
  }
  .row.incoming:not(.cursor):not(:hover) {
    background: color-mix(in srgb, var(--blue) 6%, transparent);
  }
  .row.fold:not(.cursor):not(:hover) {
    background: color-mix(in srgb, var(--blue) 6%, transparent);
  }
  .flow.in {
    color: var(--blue);
  }
  .flow.out {
    color: var(--add);
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 12px;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
  }
  .switch input {
    accent-color: var(--amber);
    margin: 0;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 1px 9px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .row.stash {
    padding: 6px 8px;
  }
  .stashicon {
    color: var(--muted);
    flex-shrink: 0;
    align-self: flex-start;
    margin-top: 2px;
  }
  .hbtn.more {
    font-size: 13px;
    line-height: 1;
    padding: 1px 6px;
  }
  .row.run {
    padding: 6px 8px;
  }
  .cistate {
    width: 14px;
    text-align: center;
    flex-shrink: 0;
    align-self: flex-start;
    margin-top: 1px;
    color: var(--faint);
  }
  .cistate.r-succeeded {
    color: var(--add);
  }
  .cistate.r-partial {
    color: var(--amber);
  }
  .cistate.r-failed {
    color: var(--del);
  }
  .cistate.s-running {
    color: var(--blue);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .row.issue {
    padding: 6px 8px;
  }
  .istate {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 2px solid var(--add);
    flex-shrink: 0;
    align-self: flex-start;
    margin-top: 5px;
  }
  .istate.closed {
    border-color: var(--syn-keyword);
    background: var(--syn-keyword);
  }
  .label {
    font-size: 11px;
    line-height: 15px;
    padding: 0 6px;
    border-radius: 8px;
    color: var(--text);
    background: color-mix(in srgb, var(--lc) 28%, transparent);
    border: 1px solid color-mix(in srgb, var(--lc) 55%, transparent);
    white-space: nowrap;
  }
  .amber {
    color: var(--amber);
  }
  .tag {
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    line-height: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rv-approved {
    color: var(--add);
  }
  .rv-changes {
    color: var(--del);
  }
  .rv-waiting {
    color: var(--amber);
  }
  .rv-none {
    display: none;
  }
  .empty {
    color: var(--muted);
    padding: 16px 10px;
    margin: 0;
    white-space: normal;
  }
  .empty p {
    margin: 0 0 8px;
  }
  .err {
    color: var(--del);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
  .link {
    color: var(--amber);
  }
</style>
