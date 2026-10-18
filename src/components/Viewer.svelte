<script lang="ts">
  import { untrack } from "svelte";
  import { EditorView, keymap } from "@codemirror/view";
  import { EditorState, Compartment, type Extension, type Text } from "@codemirror/state";
  import { MergeView, unifiedMergeView, getChunks, getOriginalDoc, type Chunk } from "@codemirror/merge";
  import { app, type View } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { baseExtensions, diffExtensions, editableState, jumpTo, languageFor } from "../lib/cm";
  import { baseName, bytes, dirName, kindLetter } from "../lib/format";
  import type { FileText } from "../lib/types";
  import IssueView from "./IssueView.svelte";
  import RunView from "./RunView.svelte";
  import MarkdownView from "./MarkdownView.svelte";
  import HunkPanel from "./HunkPanel.svelte";
  import ImageView from "./ImageView.svelte";
  import ImageDiff from "./ImageDiff.svelte";
  import PdfView from "./PdfView.svelte";
  import DataTree from "./DataTree.svelte";
  import CsvTable from "./CsvTable.svelte";
  import LogView from "./LogView.svelte";
  import DataView from "./DataView.svelte";
  import type { DbConn } from "../lib/types";
  import { hasPage, isBinaryKind, richKind } from "../lib/filekind";
  import { codeMenu } from "../lib/menus";
  import { commitUrl } from "../lib/format";
  import { openUrl } from "../lib/api";
  import { hover } from "../lib/hover";
  import Icon from "./Icon.svelte";

  let host: HTMLDivElement | undefined = $state();
  let status = $state<"idle" | "loading" | "ready" | "error" | "binary" | "large">("idle");
  let message = $state("");
  let counts = $state<{ add: number; del: number } | null>(null);
  let dirty = $state(false);
  let saving = $state(false);
  /** The text in the editor, for the Markdown page. */
  let docText = $state("");

  let editor: EditorView | null = null;
  let merge: MergeView | null = null;
  const editable = new Compartment();
  const language = new Compartment();
  /** Long lines wrap or scroll sideways. One switch for the file view and both diff layouts. */
  const wrapping = new Compartment();
  const WRAP_KEY = "tern.wrap";
  let wrap = $state(readWrap());
  function readWrap() {
    try {
      return localStorage.getItem(WRAP_KEY) === "1";
    } catch {
      return false;
    }
  }
  function toggleWrap() {
    wrap = !wrap;
    try {
      localStorage.setItem(WRAP_KEY, wrap ? "1" : "0");
    } catch {
      /* storage blocked: the choice lasts until the window closes */
    }
  }
  const wrapExt = () => wrapping.of(wrap ? EditorView.lineWrapping : []);

  /** The blocks of change in the diff on screen, as positions in the new text. */
  let chunkList: readonly Chunk[] = [];
  let chunkTotal = $state(0);
  /** The change the cursor is in, or -1. */
  let chunkAt = $state(-1);
  /** Asking about unsaved edits for this target; loads for it wait for the answer. */
  let asking: string | null = null;
  let lastKey = "";
  let lastView: View | null = null;
  let lastMode: "split" | "unified" = "unified";
  /** The last jump done, so a reload does not jump again. */
  let lastJump: number | undefined;
  /** The text on screen. A reload with the same text keeps the view as it is. */
  let lastText = "";
  let seq = 0;

  const keyOf = (v: View) => `${v.kind}|${v.repo}|${v.rel}|${v.kind === "diff" ? (v.rev?.head ?? "wt") : ""}`;

  function viewsNow() {
    return merge ? [merge.a, merge.b] : editor ? [editor] : [];
  }

  /** The word under a point, or under the cursor when no point is given. */
  function wordAt(view: EditorView | undefined, x?: number, y?: number) {
    if (!view) return "";
    const pos = x !== undefined && y !== undefined ? view.posAtCoords({ x, y }) : view.state.selection.main.head;
    if (pos == null) return "";
    const sel = view.state.selection.main;
    if (x === undefined && !sel.empty) return view.state.sliceDoc(sel.from, sel.to);
    const range = view.state.wordAt(pos);
    return range ? view.state.sliceDoc(range.from, range.to) : "";
  }

  /** Right-click on code: definition and usages of the word, then copy actions. */
  function onContext(e: MouseEvent) {
    if (status !== "ready") return;
    const views = viewsNow();
    const target = views.find((view) => view.dom.contains(e.target as Node)) ?? views[views.length - 1];
    const sel = target ? target.state.sliceDoc(target.state.selection.main.from, target.state.selection.main.to) : "";
    codeMenu(e, sel, wordAt(target, e.clientX, e.clientY));
  }

  // Ctrl+F, F12, and Shift+F12 use the word at the cursor in the focused (or last) code view.
  app.wordAtCursor = () => {
    const views = viewsNow();
    const focused = views.find((view) => view.hasFocus) ?? views[views.length - 1];
    return wordAt(focused);
  };

  /** Ctrl+click on a name goes to its definition. */
  const ctrlClick = EditorView.domEventHandlers({
    mousedown(e, view) {
      if (!(e.ctrlKey || e.metaKey) || e.button !== 0) return false;
      const w = wordAt(view, e.clientX, e.clientY);
      if (!w) return false;
      e.preventDefault();
      app.goToDefinition(w);
      return true;
    },
  });

  const revSha = $derived(app.view?.kind === "diff" ? (app.view.rev?.head ?? null) : null);

  function destroy() {
    editor?.destroy();
    merge?.destroy();
    editor = merge = null;
    setChunks([]);
  }

  function setChunks(list: readonly Chunk[]) {
    chunkList = list;
    chunkTotal = list.length;
    chunkAt = -1;
  }

  /** The change that holds a position in the new text, or -1. */
  function chunkIndexAt(pos: number) {
    return chunkList.findIndex((c) => pos >= c.fromB && pos <= Math.max(c.toB, c.fromB));
  }

  /** The view that holds the new text: the right side of a split diff, or the unified view. */
  const newSide = () => (merge ? merge.b : editor);

  /** Moves to the next (1) or previous (-1) change, wrapping around at the ends. */
  function stepChange(dir: 1 | -1) {
    const view = newSide();
    const n = chunkList.length;
    if (!view || !n) return;
    const head = view.state.selection.main.head;
    const at = chunkIndexAt(head);
    let next: number;
    if (at >= 0) next = (at + dir + n) % n;
    else if (dir === 1) {
      next = chunkList.findIndex((c) => c.fromB > head);
      if (next < 0) next = 0;
    } else {
      next = -1;
      for (let i = n - 1; i >= 0; i--) if (chunkList[i].fromB < head) { next = i; break; }
      if (next < 0) next = n - 1;
    }
    const c = chunkList[next];
    jumpTo(view, view.state.doc.lineAt(Math.min(c.fromB, view.state.doc.length)).number);
    chunkAt = next;
  }

  /** Keys inside the viewer: F7 and Shift+F7 step through changes, Alt+Z wraps long lines. */
  function onKeydown(e: KeyboardEvent) {
    if (e.key === "F7" && !e.ctrlKey && !e.altKey && app.view?.kind === "diff" && chunkTotal) {
      e.preventDefault();
      stepChange(e.shiftKey ? -1 : 1);
    } else if (e.altKey && !e.ctrlKey && !e.shiftKey && e.code === "KeyZ" && canWrap) {
      e.preventDefault();
      toggleWrap();
    }
  }

  function scroller(): HTMLElement | null {
    return host?.querySelector(".cm-mergeView, .cm-scroller") as HTMLElement | null;
  }

  function linesIn(doc: Text, from: number, to: number): number {
    const slice = doc.sliceString(from, Math.min(to, doc.length));
    if (!slice) return 0;
    return slice.split("\n").length - (slice.endsWith("\n") ? 1 : 0);
  }

  function countChunks(chunks: readonly Chunk[], a: Text, b: Text) {
    let add = 0;
    let del = 0;
    for (const c of chunks) {
      del += linesIn(a, c.fromA, c.toA);
      add += linesIn(b, c.fromB, c.toB);
    }
    counts = { add, del };
  }

  const trackSelection = EditorView.updateListener.of((u) => {
    if (u.docChanged && app.view?.kind === "file") {
      dirty = true;
      docText = u.state.doc.toString();
    }
    if (!u.selectionSet && !u.docChanged) return;
    const doc = u.state.doc;
    const sel = u.state.selection.main;
    const from = doc.lineAt(sel.from).number;
    let to = doc.lineAt(sel.to).number;
    if (to > from && sel.to === doc.line(to).from) to--;
    app.lines = { from, to };
    if (chunkList.length && u.view === newSide()) chunkAt = chunkIndexAt(sel.head);
  });

  const saveKey = keymap.of([{ key: "Mod-s", run: () => (save(), true) }]);

  async function save() {
    const v = app.view;
    if (!editor || v?.kind !== "file" || !dirty || saving) return;
    saving = true;
    try {
      await api.writeFile(v.repo, v.rel, editor.state.doc.toString());
      dirty = false;
      app.notify(`Saved ${v.rel}`, "ok");
      if (!v.loose) app.refresh([v.repo]);
    } catch (e) {
      app.notify(`Save failed: ${e}`, "err");
    } finally {
      saving = false;
    }
  }

  function unusable(f: FileText): "binary" | "large" | null {
    if (f.binary) return "binary";
    if (f.tooLarge) return "large";
    return null;
  }

  async function load(v: View | null, mode: "split" | "unified") {
    // A reload of the same target while the unsaved-edits question is open waits for the answer.
    if (asking && v && keyOf(v) === asking) return;
    const my = ++seq;
    if (!v || v.kind === "issue" || v.kind === "run") {
      destroy();
      status = "idle";
      lastKey = "";
      lastView = null;
      dirty = false;
      return;
    }
    const k = keyOf(v);
    const same = k === lastKey;
    if (dirty && !same) {
      const rel = lastView?.rel ?? "this file";
      asking = k;
      let ok = false;
      try {
        ok = await app.ask({
          title: "Discard unsaved edits?",
          body: `Your edits to ${rel} are not saved. Opening another file throws them away.`,
          confirm: "Discard edits",
          danger: true,
        });
      } finally {
        asking = null;
      }
      if (!ok) {
        if (my === seq) app.view = lastView;
        return;
      }
      dirty = false;
      if (my !== seq) return;
    }
    if (dirty && same) return;
    const wantJump = v.kind === "file" && !!v.line && v.jump !== lastJump;
    const keepScroll = same && mode === lastMode && !wantJump ? (scroller()?.scrollTop ?? 0) : 0;
    if (!same) {
      status = "loading";
      counts = null;
    }
    try {
      // Pictures and PDFs have their own views, which read the file themselves.
      if (isBinaryKind(richKind(v.rel))) {
        destroy();
        lastText = "";
        docText = "";
        status = "ready";
        lastKey = k;
        lastView = v;
        lastMode = mode;
        return;
      }
      const lang = languageFor(baseName(v.rel));
      if (v.kind === "file") {
        const f = await api.readFile(v.repo, v.rel);
        const langExt = await lang;
        if (my !== seq || !host) return;
        const text = `file|${f.binary}|${f.tooLarge}|${f.text}`;
        if (same && mode === lastMode && text === lastText && status === "ready") {
          // Same file, same text: keep the view, but do a new jump if one was asked for.
          if (wantJump && editor) {
            lastJump = v.jump;
            jumpTo(editor, v.line!, v.find);
          }
          lastView = v;
          return;
        }
        lastText = text;
        docText = f.text ?? "";
        destroy();
        const bad = unusable(f);
        if (bad) {
          status = bad;
          message = bytes(f.size);
        } else {
          editor = new EditorView({
            parent: host,
            state: EditorState.create({
              doc: f.text ?? "",
              extensions: [baseExtensions({ editable, language, editing: app.editing }), wrapExt(), trackSelection, saveKey, ctrlClick],
            }),
          });
          editor.dispatch({ effects: language.reconfigure(langExt) });
          // Development builds only: lets tests read the open code viewer.
          if (import.meta.env.DEV) (window as unknown as { __cm: EditorView }).__cm = editor;
          if (wantJump) {
            lastJump = v.jump;
            const ed = editor;
            // Wait one frame so the editor has its size; then the line can sit in the middle.
            requestAnimationFrame(() => jumpTo(ed, v.line!, v.find));
          }
          status = "ready";
        }
      } else {
        // "WORKTREE" means the files on disk now (a branch review includes uncommitted work).
        const d = v.rev?.head === "WORKTREE"
          ? await api.baseDiff(v.repo, v.rev.base, v.rel, v.oldRel)
          : v.rev
            ? await api.revDiff(v.repo, v.rev.base, v.rev.head, v.rel, v.oldRel)
            : await api.worktreeDiff(v.repo, v.rel, v.oldRel);
        const langExt = await lang;
        if (my !== seq || !host) return;
        const text = `diff|${d.old.text}|${d.new.text}|${d.old.binary}|${d.new.binary}`;
        if (same && mode === lastMode && text === lastText && status === "ready") return;
        lastText = text;
        destroy();
        const bad = unusable(d.old) ?? unusable(d.new);
        if (bad) {
          status = bad;
          message = bytes(Math.max(d.old.size, d.new.size));
        } else {
          build(d.old.text ?? "", d.new.text ?? "", langExt, mode);
          status = "ready";
        }
      }
      lastKey = k;
      lastView = v;
      lastMode = mode;
      if (keepScroll) requestAnimationFrame(() => scroller()?.scrollTo({ top: keepScroll }));
    } catch (e) {
      if (my !== seq) return;
      destroy();
      status = "error";
      message = String(e);
      lastKey = k;
      lastView = v;
    }
  }

  function build(oldText: string, newText: string, lang: Extension, mode: "split" | "unified") {
    const collapse = { margin: 3, minSize: 6 };
    if (mode === "split") {
      merge = new MergeView({
        parent: host!,
        a: { doc: oldText, extensions: [diffExtensions(lang), wrapExt()] },
        b: { doc: newText, extensions: [diffExtensions(lang), wrapExt(), trackSelection, ctrlClick] },
        collapseUnchanged: collapse,
        highlightChanges: true,
        gutter: true,
      });
      countChunks(merge.chunks, merge.a.state.doc, merge.b.state.doc);
      setChunks(merge.chunks);
      if (import.meta.env.DEV) (window as unknown as { __merge: MergeView }).__merge = merge;
    } else {
      editor = new EditorView({
        parent: host!,
        state: EditorState.create({
          doc: newText,
          extensions: [
            diffExtensions(lang),
            wrapExt(),
            unifiedMergeView({ original: oldText, mergeControls: false, collapseUnchanged: collapse, highlightChanges: true, gutter: true }),
            trackSelection,
            ctrlClick,
          ],
        }),
      });
      const c = getChunks(editor.state);
      if (c) {
        countChunks(c.chunks, getOriginalDoc(editor.state), editor.state.doc);
        setChunks(c.chunks);
      }
    }
  }

  // Reload when the target, the diff mode, or the disk stamp changes.
  $effect(() => {
    const v = app.view;
    const mode = app.diffMode;
    app.stamp;
    untrack(() => load(v, mode));
  });

  // Turn quick edit on or off without reloading.
  $effect(() => {
    const on = app.editing;
    untrack(() => {
      if (!editor || app.view?.kind !== "file") return;
      editor.dispatch({ effects: editable.reconfigure(editableState(on)) });
      if (on) editor.focus();
    });
  });

  // Turn line wrapping on or off in every code view on screen.
  $effect(() => {
    const on = wrap;
    untrack(() => {
      for (const view of viewsNow()) view.dispatch({ effects: wrapping.reconfigure(on ? EditorView.lineWrapping : []) });
    });
  });

  $effect(() => () => destroy());

  const v = $derived(app.view);
  /** The folder part of the path in the toolbar ("" for a file at the project root). */
  const dirText = $derived(
    v && (v.kind === "file" || v.kind === "diff")
      ? (v.kind === "file" && v.loose ? v.repo.replace(/\\/g, "/") + "/" : "") + (dirName(v.rel) ? dirName(v.rel) + "/" : "")
      : "",
  );
  /** Hunks belong to diffs of the working tree (not commits or pull requests). */
  const hunkable = $derived(v?.kind === "diff" && !v.rev && v.changeKind !== "deleted");
  const showHunks = $derived(hunkable && app.hunksOpen);
  $effect(() => {
    const cur = app.view;
    app.stamp;
    if (!showHunks || cur?.kind !== "diff") return;
    untrack(() => app.loadHunks(cur.repo, cur.rel, cur.changeKind === "untracked"));
  });
  // The hunk panel's line buttons scroll the diff.
  $effect(() => {
    const j = app.diffJump;
    if (!j) return;
    untrack(() => {
      const views = viewsNow();
      const target = views[views.length - 1];
      if (target) jumpTo(target, Math.min(j.line, target.state.doc.lines));
    });
  });
  /** A connection for an SQLite file in the file list. Not saved: it lives while the file is open. */
  function sqliteConn(repo: string, rel: string): DbConn {
    const sep = repo.includes("\\") ? "\\" : "/";
    const file = repo.replace(/[\\/]+$/, "") + sep + rel.split("/").join(sep);
    return { id: "file:" + file, name: baseName(rel), engine: "sqlite", file, host: "", port: 0, database: "", user: "", tls: "off", hasPassword: false, allowWrites: false, color: "", entra: false };
  }
  const rk = $derived(v && (v.kind === "file" || v.kind === "diff") ? richKind(v.rel) : null);
  const binary = $derived(isBinaryKind(rk));
  /** A file with a Page view (Markdown page, data tree, table, picture, or live log) next to its source. */
  const pageable = $derived(v?.kind === "file" && hasPage(rk));
  /** Tables, trees, and logs read big files themselves, so they also show when the code view says "too large". */
  const bigOk = $derived(rk === "csv" || rk === "tsv" || rk === "log" || rk === "json" || rk === "yaml" || rk === "toml");
  const page = $derived(pageable && app.mdPreview && !app.editing && (status === "ready" || (status === "large" && bigOk)));
  const pageLabel = $derived(
    rk === "markdown" ? "Page" : rk === "csv" || rk === "tsv" ? "Table" : rk === "svg" ? "Image" : rk === "log" ? "Log" : "Tree",
  );

  /** A link in a Markdown page to another file: open it, relative to this file. */
  function openLink(href: string) {
    if (v?.kind !== "file") return;
    const [path, hash] = href.split("#");
    if (!path) return;
    const parts = (dirName(v.rel) ? dirName(v.rel) + "/" : "").concat(decodeURIComponent(path)).split("/");
    const out: string[] = [];
    for (const p of parts) {
      if (p === "..") out.pop();
      else if (p && p !== ".") out.push(p);
    }
    const rel = out.join("/");
    void hash;
    if (v.loose) app.openPath(v.repo + "/" + rel);
    else app.openFile(rel);
  }
  const changed = $derived(
    v?.kind === "file" && !app.review ? app.summary?.changes.find((c) => c.path === v.rel) : undefined,
  );
  /** Wrap applies to code on screen: the file view and both diff layouts. */
  const canWrap = $derived((v?.kind === "file" || v?.kind === "diff") && !binary && status === "ready" && !page);
  const showNav = $derived(v?.kind === "diff" && !binary && status === "ready" && chunkTotal > 0);

  /** File or Diff: the other view of the same file, when there is one. */
  const otherView = $derived.by((): { label: string; title: string; run: () => void } | null => {
    if (v?.kind === "diff") {
      if ((!v.rev || v.rev.head === "WORKTREE") && v.changeKind !== "deleted") {
        const rel = v.rel;
        return { label: "File", title: "Show the whole file", run: () => app.openFile(rel, app.lines?.from) };
      }
      return null;
    }
    const c = changed;
    return c ? { label: "Diff", title: "Show what changed", run: () => app.openChange(c) } : null;
  });
  const canEdit = $derived(v?.kind === "file" && !app.editing && !binary && status !== "large");

  /** The narrow-window menu: the actions that leave the toolbar when it has no room. */
  function moreMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const o = otherView;
    app.showMenu({ x: r.left, y: r.bottom + 4 }, "More", [
      ...(o ? [{ label: o.title, run: o.run }] : []),
      ...(canEdit ? [{ label: "Quick edit", hint: "Ctrl E", run: () => (app.editing = true) }] : []),
      { label: "Open in VS Code", hint: "Ctrl Shift E", run: () => app.open("editor") },
      { label: "Copy for agent", hint: "Ctrl Shift C", run: () => (app.overlay = "copy") },
    ]);
  }

  /** The empty screen's words and tips follow the tab on the left. */
  type Tip = { keys: string[]; text: string };
  const idle = $derived.by((): { lead: string; tips: Tip[] } => {
    const n = app.summary?.changes.length ?? 0;
    const copy: Tip = { keys: ["Ctrl Shift C"], text: "Copy paths or a diff for an agent" };
    const help: Tip = { keys: ["F1"], text: "All shortcuts" };
    switch (app.tab) {
      case "changes":
        return {
          lead: n ? `${n} changed ${n === 1 ? "file" : "files"}. Use ↑ ↓ to step through them.` : "No changes. Edit a file and it shows up here.",
          tips: [
            { keys: ["Space"], text: "Select more than one file" },
            { keys: ["S"], text: "Stage or unstage" },
            { keys: ["Ctrl Enter"], text: "Commit the staged files" },
            { keys: ["F7"], text: "Next change in a diff" },
            copy,
            help,
          ],
        };
      case "prs":
        return { lead: "Pick a pull request to review its changes.", tips: [{ keys: ["↑", "↓"], text: "Step through the list" }, { keys: ["F7"], text: "Next change in a diff" }, copy, help] };
      case "issues":
        return { lead: "Pick an issue to read it here.", tips: [{ keys: ["↑", "↓"], text: "Step through the list" }, help] };
      case "history":
        return { lead: "Pick a commit to see the files it changed.", tips: [{ keys: ["↑", "↓"], text: "Step through the list" }, { keys: ["F7"], text: "Next change in a diff" }, copy, help] };
      case "ci":
        return { lead: "Pick a run to see its stages and jobs.", tips: [{ keys: ["↑", "↓"], text: "Step through the list" }, help] };
      case "tasks":
        return { lead: "Run a task from the list, or open a terminal.", tips: [{ keys: ["Ctrl `"], text: "Open a terminal in this project" }, { keys: ["Ctrl J"], text: "Show or hide the bottom panel" }, help] };
      default:
        return {
          lead: "Pick a file, or press Ctrl P to find one.",
          tips: [{ keys: ["Ctrl P"], text: "Find a file by name" }, { keys: ["Ctrl E"], text: "Quick edit the open file" }, copy, help],
        };
    }
  });

  function retry() {
    lastKey = "";
    lastText = "";
    load(app.view, app.diffMode);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="viewer" aria-label="Viewer" onkeydown={onKeydown}>
  {#if v && v.kind !== "issue" && v.kind !== "run"}
    <header class="toolbar">
      <button class="path" class:code={v.kind === "diff"} onclick={() => app.copy(v.kind === "file" && v.loose ? "absolute" : "relative")}
        use:hover={() => ({ title: v.kind === "file" && v.loose ? "Copy full path" : "Copy relative path", hint: "Ctrl Shift C for more ways to copy" })}>
        {#if v.kind === "diff"}<span class="kind mono k-{v.changeKind}">{kindLetter[v.changeKind]}</span>{/if}
        <span class="name">{#if dirText}<span class="dir">{dirText}</span>{/if}<span class="file">{baseName(v.rel)}</span></span>
      </button>
      {#if v.kind === "diff" && v.oldRel}<span class="meta mono">from {v.oldRel}</span>{/if}
      {#if counts}<span class="counts mono"><span class="add">+{counts.add}</span> <span class="del">−{counts.del}</span></span>{/if}
      {#if revSha && app.review?.kind === "commit"}
        <button class="chip mono sha" onclick={() => app.copyValue(revSha, `commit ID ${revSha.slice(0, 7)}`)}
          use:hover={() => ({ title: "Copy commit ID", rows: [{ label: "Commit", value: revSha, mono: true }], hint: "Right-click for more" })}
          oncontextmenu={(e) => { const url = commitUrl(app.summary?.remote, revSha); app.showMenu(e, revSha.slice(0, 7), [
            { label: "Copy commit ID", run: () => app.copyValue(revSha.slice(0, 7)) },
            { label: "Copy the full commit ID", run: () => app.copyValue(revSha) },
            ...(url ? [{ label: "Open in browser", run: () => openUrl(url) }] : []),
          ]); }}>{revSha.slice(0, 7)}</button>
      {/if}
      {#if app.lines && status === "ready" && !page}
        <button class="chip mono lines" onclick={() => app.copy("line")} use:hover={() => ({ title: "Copy path with line numbers" })}>
          L{app.lines.from}{app.lines.to !== app.lines.from ? `–${app.lines.to}` : ""}
        </button>
      {/if}
      <span class="grow"></span>

      {#if showNav}
        <div class="nav" role="group" aria-label="Changes">
          <button class="icon-btn sm" onclick={() => stepChange(-1)} aria-label="Previous change"
            use:hover={() => ({ title: "Previous change", hint: "Shift F7" })}>
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 10l4-4 4 4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
          <span class="meta nav-label">{chunkAt >= 0 ? `Change ${chunkAt + 1} of ${chunkTotal}` : `${chunkTotal} ${chunkTotal === 1 ? "change" : "changes"}`}</span>
          <button class="icon-btn sm" onclick={() => stepChange(1)} aria-label="Next change"
            use:hover={() => ({ title: "Next change", hint: "F7" })}>
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
        </div>
      {/if}

      {#if v.kind === "diff"}
        {#if !binary}
          <div class="seg" role="group" aria-label="Diff layout">
            <button class:on={app.diffMode === "unified"} onclick={() => app.setDiffMode("unified")}>Unified</button>
            <button class:on={app.diffMode === "split"} onclick={() => app.setDiffMode("split")}>Split</button>
          </div>
        {/if}
        {#if hunkable && !binary}
          <button class="btn sm" class:ghost={!app.hunksOpen} class:on={app.hunksOpen} aria-pressed={app.hunksOpen} onclick={() => app.toggleHunks()}
            use:hover={() => ({ title: "Hunks", text: "Stage, unstage, or discard one block of changes.", hint: "Ctrl Shift H" })}>Hunks</button>
        {/if}
      {:else if pageable}
        <div class="seg" role="group" aria-label="Layout">
          <button class:on={app.mdPreview && !app.editing} onclick={() => { if (!app.mdPreview) app.toggleMdPreview(); app.editing = false; }}
            use:hover={() => ({ title: `Show as a ${pageLabel.toLowerCase()}`, hint: "Ctrl Shift V" })}>{pageLabel}</button>
          <button class:on={!app.mdPreview || app.editing} onclick={() => app.mdPreview && app.toggleMdPreview()}
            use:hover={() => ({ title: "Show the source", hint: "Ctrl Shift V" })}>Source</button>
        </div>
      {/if}
      {#if canWrap}
        <button class="btn sm" class:ghost={!wrap} class:on={wrap} aria-pressed={wrap} onclick={toggleWrap}
          use:hover={() => ({ title: wrap ? "Long lines wrap" : "Long lines scroll sideways", hint: "Alt Z" })}>Wrap</button>
      {/if}
      {#if showNav || (v.kind === "diff" && !binary) || pageable || canWrap}<span class="sep"></span>{/if}

      {#if v.kind === "diff"}
        {#if (!v.rev || v.rev.head === "WORKTREE") && v.changeKind !== "deleted"}
          <button class="btn sm ghost lo" onclick={() => app.openFile(v.rel, app.lines?.from)} use:hover={() => ({ title: "Show the whole file" })}>File</button>
        {/if}
      {:else}
        {#if changed}
          <button class="btn sm ghost lo" onclick={() => app.openChange(changed)} use:hover={() => ({ title: "Show what changed" })}>Diff</button>
        {/if}
        {#if app.editing}
          <button class="btn sm primary" disabled={!dirty || saving} onclick={save} use:hover={() => ({ title: "Save", hint: "Ctrl S" })}>{saving ? "Saving…" : dirty ? "Save" : "Saved"}</button>
          <button class="btn sm ghost" onclick={() => (app.editing = false)} use:hover={() => ({ title: "Stop editing", hint: "Esc" })}>Done</button>
        {:else if !binary && status !== "large"}
          <button class="btn sm ghost lo" onclick={() => (app.editing = true)} use:hover={() => ({ title: "Quick edit", hint: "Ctrl E" })}>Edit</button>
        {/if}
      {/if}
      <button class="btn sm ghost lo" onclick={() => app.open("editor")} use:hover={() => ({ title: "Open in VS Code", hint: "Ctrl Shift E" })}>VS Code</button>
      <button class="btn sm ghost lo" onclick={() => (app.overlay = "copy")} use:hover={() => ({ title: "Copy paths or a diff for an agent", hint: "Ctrl Shift C" })}>Copy for agent</button>
      <button class="icon-btn sm more" onclick={moreMenu} aria-label="More actions" aria-haspopup="menu"
        use:hover={() => ({ title: "More actions", text: [otherView?.label, canEdit ? "Edit" : "", "VS Code", "Copy for agent"].filter(Boolean).join(", ") })}>
        <Icon name="more" />
      </button>
    </header>
  {/if}

  <div class="body">
    {#if v?.kind === "issue"}
      {#key v.number}<IssueView repo={v.repo} number={v.number} />{/key}
    {:else if v?.kind === "run"}
      {#key v.run.id}<RunView repo={v.repo} run={v.run} />{/key}
    {/if}
    {#if v && binary && status === "ready"}
      {#if v.kind === "file"}
        {#key v.rel}
          {#if rk === "pdf"}<PdfView repo={v.repo} rel={v.rel} />
          {:else if rk === "sqlite"}<DataView fixed={sqliteConn(v.repo, v.rel)} />
          {:else}<ImageView repo={v.repo} rel={v.rel} />{/if}
        {/key}
      {:else if v.kind === "diff"}
        {@const head = v.rev ? (v.rev.head === "WORKTREE" ? null : v.rev.head) : null}
        {@const base = v.rev ? v.rev.base : "HEAD"}
        {#if rk === "pdf"}
          {#key `${v.rel}|${base}|${head}`}
            {#if v.changeKind === "deleted"}<PdfView repo={v.repo} rel={v.oldRel ?? v.rel} rev={base} />{:else}<PdfView repo={v.repo} rel={v.rel} rev={head} />{/if}
          {/key}
        {:else if rk === "sqlite"}
          <div class="cover">
            <div class="state">
              <span class="state-title">No text diff</span>
              <p>This is an SQLite database. Open the file to look inside it.</p>
              {#if v.changeKind !== "deleted"}
                <div class="actions"><button class="btn" onclick={() => app.openFile(v.rel)}>Open the file</button></div>
              {/if}
            </div>
          </div>
        {:else}
          <ImageDiff repo={v.repo} rel={v.rel} oldRel={v.oldRel ?? null} {base} {head} />
        {/if}
      {/if}
    {/if}
    {#if page && v}
      {#if rk === "markdown"}<MarkdownView text={docText} onLink={openLink} />
      {:else if rk === "svg"}{#key v.rel}<ImageView repo={v.repo} rel={v.rel} />{/key}
      {:else if rk === "log"}{#key `${v.repo}|${v.rel}`}<LogView repo={v.repo} rel={v.rel} />{/key}
      {:else if rk === "csv" || rk === "tsv"}<CsvTable text={status === "large" ? null : docText} tsv={rk === "tsv"} repo={v.repo} rel={v.rel} />
      {:else if rk === "json" || rk === "yaml" || rk === "toml"}<DataTree text={status === "large" ? null : docText} kind={rk} repo={v.repo} rel={v.rel} />
      {/if}
    {/if}
    {#if showHunks && status === "ready" && !binary}<HunkPanel />{/if}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="cm" bind:this={host} class:hidden={status !== "ready" || v?.kind === "issue" || v?.kind === "run" || page || binary}
      class:with-hunks={showHunks} oncontextmenu={onContext}></div>
    {#if page}
      <!-- The page view covers the states below. -->
    {:else if status === "loading"}
      <div class="cover delay"><div class="state"><p>Loading…</p></div></div>
    {:else if status === "binary"}
      <div class="cover">
        <div class="state">
          <span class="state-title">Binary file</span>
          <p>This file ({message}) has no text to show.</p>
          <div class="actions"><button class="btn" onclick={() => app.open("explorer")}>Show in folder</button></div>
        </div>
      </div>
    {:else if status === "large"}
      <div class="cover">
        <div class="state">
          <span class="state-title">Too large to open here</span>
          <p>This file is {message}. Files over 5 MB do not open in Tern.</p>
          <div class="actions"><button class="btn" onclick={() => app.open("editor")}>Open in VS Code</button></div>
        </div>
      </div>
    {:else if status === "error"}
      <div class="cover">
        <div class="state">
          <span class="state-title">Could not open this file</span>
          <pre>{message}</pre>
          <div class="actions"><button class="btn" onclick={retry}>Try again</button></div>
        </div>
      </div>
    {:else if status === "idle" && v?.kind !== "issue" && v?.kind !== "run"}
      <div class="cover">
        <div class="state idle">
          <span class="state-title">{idle.lead}</span>
          <dl>
            {#each idle.tips as tip (tip.text)}
              <dt>{#each tip.keys as k, i}{#if i}{" "}{/if}<kbd>{k}</kbd>{/each}</dt><dd>{tip.text}</dd>
            {/each}
          </dl>
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .viewer {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg);
  }
  .toolbar {
    white-space: nowrap;
    /* Narrow windows: the actions below move into the ⋯ menu (container queries at the end). */
    container: viewerbar / inline-size;
  }
  /* Everything keeps its size except the path and the "from" note, which shrink. */
  .toolbar > :global(*) {
    flex-shrink: 0;
  }
  .toolbar > .path,
  .toolbar > .meta {
    flex-shrink: 1;
  }
  .path {
    display: flex;
    align-items: center;
    gap: var(--s2);
    min-width: 0;
    overflow: hidden;
    font-size: var(--fs-md);
  }
  /* Diff paths are code-like; a file's name reads as a title. */
  .path.code {
    font-family: var(--mono);
    font-size: var(--fs-sm);
  }
  .path:hover .file {
    text-decoration: underline;
    text-decoration-color: var(--line-strong);
  }
  /* The folder gives way first; the file name shrinks only when the folder is gone. */
  .dir {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    flex-shrink: 100000;
  }
  .dir:empty {
    display: none;
  }
  .file {
    color: var(--text);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    flex-shrink: 1;
  }
  /* Folder and file name sit flush, as one path. */
  .name {
    display: flex;
    min-width: 0;
    overflow: hidden;
  }
  .kind {
    font-weight: 700;
  }
  .counts {
    font-size: var(--fs-xs);
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .chip.sha,
  .chip.lines {
    cursor: pointer;
  }
  .chip.sha:hover,
  .chip.lines:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .nav-label {
    min-width: 92px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .more {
    display: none;
  }
  @container viewerbar (max-width: 760px) {
    .lo {
      display: none;
    }
    .more {
      display: inline-flex;
    }
  }
  @container viewerbar (max-width: 600px) {
    .nav-label {
      display: none;
    }
  }
  .body {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .cm {
    position: absolute;
    inset: 0;
    overflow: hidden;
    user-select: text;
  }
  .cm.with-hunks {
    right: min(400px, 45%);
  }
  .cm.hidden {
    visibility: hidden;
  }
  /* Split diff: the two sides scroll up and down together in one box; each side scrolls sideways
     only when its lines are too long. Both sides fill the pane, so short files leave no gap. */
  .cm :global(.cm-mergeView) {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .cm :global(.cm-mergeViewEditors) {
    min-height: 100%;
  }
  .cm :global(.cm-mergeViewEditor) {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .cm :global(.cm-mergeView .cm-editor) {
    height: auto;
    flex: 1 0 auto;
  }
  /* The scroller fills its side, so a sideways scrollbar sits at the bottom edge, not under the last line. */
  .cm :global(.cm-mergeView .cm-scroller) {
    flex: 1 1 auto;
    overflow-x: auto;
    overflow-y: hidden !important;
  }
  /* The single-view room to scroll the last line up is not needed here: the box scrolls, not the editor. */
  .cm :global(.cm-mergeView .cm-content) {
    padding-bottom: var(--s6);
  }
  .cm :global(.cm-merge-a) {
    border-right: 1px solid var(--line);
  }
  .cm > :global(.cm-editor) {
    height: 100%;
  }
  /* States sit over the (hidden) code area. */
  .cover {
    position: absolute;
    inset: 0;
    display: flex;
    overflow: auto;
  }
  /* Loading that ends within 300 ms shows nothing. */
  .cover.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
  .state.idle {
    max-width: 520px;
    gap: var(--s5);
  }
  .state.idle .state-title {
    font-size: var(--fs-lg);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--s3) var(--s5);
    margin: 0;
    text-align: left;
  }
  dt {
    text-align: right;
  }
  dd {
    margin: 0;
  }
</style>
