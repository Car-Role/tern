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
  import { hasPage, isBinaryKind, richKind } from "../lib/filekind";
  import { codeMenu } from "../lib/menus";
  import { commitUrl } from "../lib/format";
  import { openUrl } from "../lib/api";

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
      if (!confirm(`Discard your unsaved edits to ${lastView?.rel}?`)) {
        app.view = lastView;
        return;
      }
      dirty = false;
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
              extensions: [baseExtensions({ editable, language, editing: app.editing }), trackSelection, saveKey, ctrlClick],
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
        a: { doc: oldText, extensions: diffExtensions(lang) },
        b: { doc: newText, extensions: [diffExtensions(lang), trackSelection, ctrlClick] },
        collapseUnchanged: collapse,
        highlightChanges: true,
        gutter: true,
      });
      countChunks(merge.chunks, merge.a.state.doc, merge.b.state.doc);
      if (import.meta.env.DEV) (window as unknown as { __merge: MergeView }).__merge = merge;
    } else {
      editor = new EditorView({
        parent: host!,
        state: EditorState.create({
          doc: newText,
          extensions: [
            diffExtensions(lang),
            unifiedMergeView({ original: oldText, mergeControls: false, collapseUnchanged: collapse, highlightChanges: true, gutter: true }),
            trackSelection,
            ctrlClick,
          ],
        }),
      });
      const c = getChunks(editor.state);
      if (c) countChunks(c.chunks, getOriginalDoc(editor.state), editor.state.doc);
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

  $effect(() => () => destroy());

  const v = $derived(app.view);
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
  const rk = $derived(v && (v.kind === "file" || v.kind === "diff") ? richKind(v.rel) : null);
  const binary = $derived(isBinaryKind(rk));
  /** A file with a Page view (Markdown page, data tree, table, picture, or live log) next to its source. */
  const pageable = $derived(v?.kind === "file" && hasPage(rk));
  /** Tables, trees, and logs read big files themselves, so they also show when the code view says "too large". */
  const bigOk = $derived(rk === "csv" || rk === "tsv" || rk === "log" || rk === "json" || rk === "yaml" || rk === "toml");
  const page = $derived(pageable && app.mdPreview && !app.editing && (status === "ready" || (status === "large" && bigOk)));
  const pageLabel = $derived(
    rk === "markdown" ? "Page" : rk === "csv" || rk === "tsv" ? "Table" : rk === "svg" ? "Image" : rk === "log" ? "Live" : "Tree",
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
</script>

<section class="viewer" aria-label="Viewer">
  {#if v && v.kind !== "issue" && v.kind !== "run"}
    <header>
      <button class="path" onclick={() => app.copy(v.kind === "file" && v.loose ? "absolute" : "relative")} title={v.kind === "file" && v.loose ? "Copy full path" : "Copy relative path"}>
        {#if v.kind === "file" && v.loose}<span class="dir">{v.repo.replace(/\\/g, "/")}/</span>{/if}
        {#if v.kind === "diff"}<span class="kind mono k-{v.changeKind}">{kindLetter[v.changeKind]}</span>{/if}
        <span class="dir">{dirName(v.rel) ? dirName(v.rel) + "/" : ""}</span><span class="file">{baseName(v.rel)}</span>
      </button>
      {#if v.kind === "diff" && v.oldRel}<span class="faint mono renamed">from {v.oldRel}</span>{/if}
      {#if counts}<span class="counts mono"><span class="add">+{counts.add}</span> <span class="del">−{counts.del}</span></span>{/if}
      {#if revSha && app.review?.kind === "commit"}
        <button class="shachip mono" onclick={() => app.copyValue(revSha, `commit ID ${revSha.slice(0, 7)}`)}
          oncontextmenu={(e) => { const url = commitUrl(app.summary?.remote, revSha); app.showMenu(e, revSha.slice(0, 7), [
            { label: "Copy commit ID", run: () => app.copyValue(revSha.slice(0, 7)) },
            { label: "Copy the full commit ID", run: () => app.copyValue(revSha) },
            ...(url ? [{ label: "Open on the web", run: () => openUrl(url) }] : []),
          ]); }}>{revSha.slice(0, 7)}</button>
      {/if}
      {#if app.lines && status === "ready" && !page}
        <button class="lines mono" onclick={() => app.copy("line")} title="Copy path with line numbers">
          L{app.lines.from}{app.lines.to !== app.lines.from ? `–${app.lines.to}` : ""}
        </button>
      {/if}
      <span class="grow"></span>

      {#if v.kind === "diff"}
        {#if !binary}
          <div class="seg" role="group" aria-label="Diff layout">
            <button class:on={app.diffMode === "unified"} onclick={() => app.setDiffMode("unified")}>Unified</button>
            <button class:on={app.diffMode === "split"} onclick={() => app.setDiffMode("split")}>Split</button>
          </div>
        {/if}
        {#if hunkable && !binary}
          <button class="btn" class:on={app.hunksOpen} onclick={() => app.toggleHunks()} title="Stage, unstage, or discard one block of changes (Ctrl+Shift+H)">Hunks</button>
        {/if}
        {#if (!v.rev || v.rev.head === "WORKTREE") && v.changeKind !== "deleted"}
          <button class="btn" onclick={() => app.openFile(v.rel, app.lines?.from)} title="Show the whole file">File</button>
        {/if}
      {:else}
        {#if pageable}
          <div class="seg" role="group" aria-label="Layout">
            <button class:on={app.mdPreview && !app.editing} onclick={() => { if (!app.mdPreview) app.toggleMdPreview(); app.editing = false; }} title="Show as a {pageLabel.toLowerCase()} (Ctrl+Shift+V)">{pageLabel}</button>
            <button class:on={!app.mdPreview || app.editing} onclick={() => app.mdPreview && app.toggleMdPreview()} title="Show the source (Ctrl+Shift+V)">Source</button>
          </div>
        {/if}
        {#if changed}
          <button class="btn" onclick={() => app.openChange(changed)} title="Show what changed">Diff</button>
        {/if}
        {#if app.editing}
          <button class="btn primary" disabled={!dirty || saving} onclick={save}>{saving ? "Saving…" : dirty ? "Save" : "Saved"}</button>
          <button class="btn" onclick={() => (app.editing = false)}>Done</button>
        {:else if !binary && status !== "large"}
          <button class="btn" onclick={() => (app.editing = true)} title="Quick edit (Ctrl+E)">Edit</button>
        {/if}
      {/if}
      <button class="btn" onclick={() => app.open("editor")} title="Open in VS Code (Ctrl+Shift+E)">VS Code</button>
      <button class="btn" onclick={() => (app.overlay = "copy")} title="Copy for an agent (Ctrl+Shift+C)">Copy</button>
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
          {#if rk === "pdf"}<PdfView repo={v.repo} rel={v.rel} />{:else}<ImageView repo={v.repo} rel={v.rel} />{/if}
        {/key}
      {:else if v.kind === "diff"}
        {@const head = v.rev ? (v.rev.head === "WORKTREE" ? null : v.rev.head) : null}
        {@const base = v.rev ? v.rev.base : "HEAD"}
        {#if rk === "pdf"}
          {#key `${v.rel}|${base}|${head}`}
            {#if v.changeKind === "deleted"}<PdfView repo={v.repo} rel={v.oldRel ?? v.rel} rev={base} />{:else}<PdfView repo={v.repo} rel={v.rel} rev={head} />{/if}
          {/key}
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
      <!-- The page view covers the note below. -->
    {:else if status === "loading"}
      <p class="note">Loading…</p>
    {:else if status === "binary"}
      <p class="note">This is a binary file ({message}). There is no text to show.</p>
    {:else if status === "large"}
      <p class="note">This file is {message}. Files over 5 MB do not open here. Open it in VS Code instead.</p>
    {:else if status === "error"}
      <div class="note">
        <p>Could not open this file.</p>
        <pre class="err">{message}</pre>
      </div>
    {:else if status === "idle" && v?.kind !== "issue" && v?.kind !== "run"}
      <div class="idle">
        {#if app.summary?.changes.length && app.tab === "changes"}
          <p>{app.summary.changes.length} changed files. Use <kbd>↑</kbd> <kbd>↓</kbd> in the list to step through them.</p>
        {:else}
          <p>Pick a file on the left, or press <kbd>Ctrl P</kbd> to find one.</p>
        {/if}
        <dl>
          <dt><kbd>Ctrl Shift C</kbd></dt><dd>Copy paths or a diff for an agent</dd>
          <dt><kbd>Space</kbd></dt><dd>Select more than one file</dd>
          <dt><kbd>S</kbd></dt><dd>Stage or unstage, then <kbd>Ctrl Enter</kbd> to commit</dd>
          <dt><kbd>Ctrl `</kbd></dt><dd>Open a terminal in this project</dd>
          <dt><kbd>F1</kbd></dt><dd>All shortcuts</dd>
        </dl>
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
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 14px;
    height: 41px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
    white-space: nowrap;
  }
  .path {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .path:hover .file {
    text-decoration: underline;
    text-decoration-color: var(--amber);
  }
  .dir {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file {
    color: var(--text);
    font-weight: 600;
  }
  .kind {
    font-weight: 700;
  }
  .renamed {
    font-size: 11px;
  }
  .counts {
    font-size: 11.5px;
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .lines {
    font-size: 11px;
    color: var(--amber);
    background: var(--amber-soft);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .grow {
    flex: 1;
  }
  .shachip {
    font-size: 11px;
    color: var(--amber);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 6px;
  }
  .shachip:hover {
    border-color: var(--amber);
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 2px 8px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .btn {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: var(--radius);
    color: var(--muted);
    border: 1px solid transparent;
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line);
  }
  .btn.primary {
    color: var(--on-accent);
    background: var(--amber);
  }
  .btn.primary:disabled {
    background: var(--raised);
    color: var(--muted);
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
  .btn.on {
    color: var(--text);
    border-color: var(--line-strong);
    background: var(--raised);
  }
  .cm.hidden {
    visibility: hidden;
  }
  .cm :global(.cm-mergeView) {
    height: 100%;
    overflow-y: auto;
  }
  .cm :global(.cm-mergeViewEditors) {
    min-height: 100%;
  }
  .cm :global(.cm-mergeView .cm-editor) {
    height: auto;
  }
  .cm :global(.cm-merge-a) {
    border-right: 1px solid var(--line);
  }
  .cm > :global(.cm-editor) {
    height: 100%;
  }
  .note {
    position: absolute;
    top: 24px;
    left: 24px;
    right: 24px;
    margin: 0;
    color: var(--muted);
  }
  .err {
    color: var(--del);
    font-family: var(--mono);
    font-size: 12px;
    white-space: pre-wrap;
    user-select: text;
  }
  .idle {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    /* "safe": when the tips do not fit, they start at the top instead of spilling out. */
    justify-content: safe center;
    overflow: auto;
    padding: 16px 12%;
    color: var(--muted);
  }
  .idle p {
    font-size: 14px;
    color: var(--text);
    margin: 0 0 20px;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 8px 16px;
    margin: 0;
  }
  dt {
    text-align: right;
  }
  dd {
    margin: 0;
  }
</style>
