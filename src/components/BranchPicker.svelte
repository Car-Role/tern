<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { rank } from "../lib/fuzzy";
  import { ago, baseName } from "../lib/format";
  import type { Branch } from "../lib/types";

  const repo = app.current!;
  const main = app.mainOf(repo);
  let tab = $state(app.branchTab);
  let branches = $state<Branch[]>([]);
  let base = $state<string | null>(null);
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  let showRemote = $state(false);
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  // New worktree form
  let formOpen = $state(false);
  let wtBranch = $state("");
  let wtBase = $state("");
  let wtPath = $state("");
  let wtAgent = $state(false);
  let bases = $state<string[]>([]);
  let busy = $state(false);

  async function load() {
    loading = true;
    error = "";
    try {
      const l = await api.listBranches(repo);
      branches = l.branches;
      base = l.base;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const exists = $derived(branches.some((b) => b.name === query.trim()));
  const shown = $derived.by(() => {
    const pool = branches.filter((b) => showRemote || !b.remote || query.trim());
    const q = query.trim();
    return q ? rank(pool, q, (b) => b.name, 200).map((x) => x.item) : pool;
  });
  /** Rows: an optional "create" row, then branches. */
  const canCreate = $derived(!!query.trim() && !exists);
  const rowCount = $derived(shown.length + (canCreate ? 1 : 0));
  const pick = $derived(canCreate ? (active === 0 ? null : shown[active - 1]) : shown[active]);

  $effect(() => {
    query;
    showRemote;
    active = 0;
  });

  // Worktrees of this repo: the main folder plus each linked one.
  const worktrees = $derived(app.repos.filter((r) => r.path === main || r.parent === main));

  function localName(b: Branch) {
    return b.remote ? b.name.replace(/^[^/]+\//, "") : b.name;
  }

  async function doSwitch(b: Branch) {
    if (b.worktree && b.worktree.toLowerCase() !== repo.toLowerCase()) {
      // It is already open in another worktree. Git allows a branch in one worktree at a time.
      const wt = app.repos.find((r) => r.path.toLowerCase() === b.worktree!.toLowerCase());
      app.overlay = null;
      if (wt) app.select(wt.path);
      else app.notify(`${b.name} is checked out in ${b.worktree}`, "info");
      return;
    }
    app.overlay = null;
    await app.switchBranch(b.name, b.remote);
  }

  async function openForm(branch: string) {
    formOpen = true;
    wtBranch = branch;
    bases = await api.baseCandidates(repo).catch(() => []);
    wtBase = bases.find((x) => x.startsWith("origin/")) ?? bases[0] ?? "";
    await suggest();
  }

  async function suggest() {
    const name = wtBranch.trim();
    wtPath = name ? await api.suggestWorktreePath(main, name.replace(/^origin\//, "")).catch(() => "") : "";
  }

  const wtIsNew = $derived(!!wtBranch.trim() && !branches.some((b) => b.name === wtBranch.trim()));
  const wtTaken = $derived(branches.find((b) => b.name === wtBranch.trim() && b.worktree));

  async function createWorktree() {
    if (!wtBranch.trim() || !wtPath.trim() || busy) return;
    busy = true;
    const ok = await app.newWorktree(wtBranch.trim(), wtIsNew, wtIsNew ? wtBase || null : null, wtPath.trim(), wtAgent);
    busy = false;
    if (ok) app.overlay = null;
  }

  async function createBranch() {
    app.overlay = null;
    await app.createBranch(query.trim(), null);
  }

  function onKey(e: KeyboardEvent) {
    if (tab !== "branches" || formOpen) return;
    if (e.key === "ArrowDown") active = Math.min(rowCount - 1, active + 1);
    else if (e.key === "ArrowUp") active = Math.max(0, active - 1);
    else if (e.key === "Enter" && e.altKey) {
      if (pick) openForm(pick.name);
      else if (canCreate) openForm(query.trim());
    } else if (e.key === "Enter") {
      if (pick) doSwitch(pick);
      else if (canCreate) createBranch();
    } else return;
    e.preventDefault();
    tick().then(() => listEl?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
  }

  onMount(() => {
    load();
    input?.focus();
  });
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="dialog" role="dialog" aria-label="Branches and worktrees">
    <header>
      <div class="seg" role="tablist">
        <button role="tab" class:on={tab === "branches"} aria-selected={tab === "branches"} onclick={() => { tab = "branches"; formOpen = false; }}>Branches</button>
        <button role="tab" class:on={tab === "worktrees"} aria-selected={tab === "worktrees"} onclick={() => (tab = "worktrees")}>Worktrees <span class="n">{worktrees.length - 1}</span></button>
      </div>
      <span class="where">{app.repos.find((r) => r.path === main)?.name ?? baseName(main)}</span>
    </header>

    {#if formOpen}
      <div class="form">
        <h3>New worktree</h3>
        <p class="hint">A worktree is a second folder for the same repo, on its own branch. Work there does not touch your main folder.</p>
        <label>Branch
          <input bind:value={wtBranch} list="tern-branch-list" spellcheck="false" oninput={suggest} placeholder="new-branch-name or an existing branch" />
          <datalist id="tern-branch-list">
            {#each branches.filter((b) => !b.worktree) as b}<option value={b.name}></option>{/each}
          </datalist>
        </label>
        {#if wtTaken}
          <p class="warn">{wtBranch} is already checked out in {wtTaken.worktree}. Git allows a branch in one worktree at a time.</p>
        {:else if wtIsNew}
          <label>Start from
            <select bind:value={wtBase}>
              {#each bases as b}<option value={b}>{b}</option>{/each}
              <option value="">the current commit</option>
            </select>
          </label>
          <p class="hint small">A new branch named <strong>{wtBranch}</strong> is made.</p>
        {/if}
        <label>Folder <input class="mono" bind:value={wtPath} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={wtAgent} /> Start Claude Code in it</label>
        <div class="buttons">
          <button class="btn" onclick={() => (formOpen = false)}>Back</button>
          <button class="btn primary" disabled={!wtBranch.trim() || !wtPath.trim() || !!wtTaken || busy} onclick={createWorktree}>
            {busy ? "Making…" : "Make worktree"}
          </button>
        </div>
      </div>
    {:else if tab === "branches"}
      <div class="top">
        <input bind:this={input} bind:value={query} onkeydown={onKey} placeholder="Find a branch, or type a new name" spellcheck="false" aria-label="Branch" />
        <label class="check small"><input type="checkbox" bind:checked={showRemote} /> Remote branches</label>
      </div>
      <div class="list" bind:this={listEl} role="listbox">
        {#if loading}<p class="note">Reading branches…</p>{/if}
        {#if error}<p class="note err">{error}</p>{/if}
        {#if canCreate}
          <button class="row create" class:on={active === 0} role="option" aria-selected={active === 0} onmousemove={() => (active = 0)} onclick={createBranch}>
            <span class="plus">+</span>
            <span class="bname">Make branch <strong>{query.trim()}</strong> from here</span>
            <span class="grow"></span>
            <span class="keys faint"><kbd>Enter</kbd> switch to it, <kbd>Alt Enter</kbd> in a new worktree</span>
          </button>
        {/if}
        {#each shown as b, i (b.name)}
          {@const idx = canCreate ? i + 1 : i}
          <div class="row" class:on={idx === active} class:current={b.current} role="option" aria-selected={idx === active} tabindex="-1"
            onmousemove={() => (active = idx)} onclick={() => doSwitch(b)} onkeydown={() => {}}>
            <span class="mark">{b.current ? "●" : ""}</span>
            <div class="two">
              <div class="l1">
                <span class="bname mono">{b.name}</span>
                {#if b.ahead}<span class="ahead mono">↑{b.ahead}</span>{/if}
                {#if b.behind}<span class="behind mono">↓{b.behind}</span>{/if}
                {#if b.worktree && b.worktree.toLowerCase() !== repo.toLowerCase()}<span class="tag wt">in {baseName(b.worktree)}</span>{/if}
                {#if b.merged}<span class="tag merged">merged</span>{/if}
                {#if b.gone}<span class="tag gone">remote gone</span>{/if}
              </div>
              <div class="l2"><span class="sha mono">{b.sha}</span> {b.subject} <span class="faint">{b.author}, {ago(b.time)}</span></div>
            </div>
            {#if idx === active}
              <div class="acts">
                {#if !b.current}<button onclick={(e) => { e.stopPropagation(); doSwitch(b); }}>{b.worktree ? "Open" : "Switch"}</button>{/if}
                {#if !b.worktree}<button onclick={(e) => { e.stopPropagation(); openForm(b.name); }}>New worktree</button>{/if}
                {#if !b.remote && !b.current && !b.worktree}
                  <button class="danger" onclick={async (e) => { e.stopPropagation(); if (await app.deleteBranch(b.name, b.merged)) load(); }}>Delete</button>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
      <p class="foot faint">
        <kbd>Enter</kbd> switch. <kbd>Alt Enter</kbd> open in a new worktree. {#if base}"merged" means its commits are in {base}.{/if}
      </p>
    {:else}
      <div class="list">
        {#each worktrees as w (w.path)}
          {@const s = app.sums[w.path]}
          {@const isMain = w.path === main}
          <div class="row wtrow" class:current={w.path === repo}>
            <span class="mark">{w.path === repo ? "●" : ""}</span>
            <div class="two">
              <div class="l1">
                <span class="bname">{isMain ? "Main folder" : w.name}</span>
                <span class="mono faint">{s?.branch ?? (s?.error ? "" : "detached")}</span>
                {#if s?.changes.length}<span class="tag chg">{s.changes.length} changed</span>{/if}
                {#if app.isWorking(w.path)}<span class="tag working">working</span>{/if}
                {#if s?.error}<span class="tag gone">broken</span>{/if}
              </div>
              <div class="l2 mono faint">{w.path}</div>
            </div>
            <div class="acts always">
              {#if w.path !== repo}<button onclick={() => { app.overlay = null; app.select(w.path); }}>Open</button>{/if}
              {#if !isMain && !s?.error}
                <button onclick={async () => { app.overlay = null; app.select(w.path); await app.openBranchReview(); }}>Review</button>
              {/if}
              {#if !isMain}<button class="danger" onclick={async () => { app.overlay = null; await app.removeWorktree(w.path); }}>Remove</button>{/if}
            </div>
          </div>
        {/each}
      </div>
      <div class="buttons">
        <button class="btn" onclick={() => app.pruneWorktrees(main)} title="Forget worktrees whose folder was deleted">Clean up missing</button>
        <span class="grow"></span>
        <button class="btn primary" onclick={() => openForm("")}>New worktree…</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 10vh 16px 16px;
    z-index: 10;
  }
  .dialog {
    width: min(720px, 100%);
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    padding: 12px 14px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .where {
    color: var(--muted);
    font-size: 12px;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 3px 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .n {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--faint);
  }
  .top {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 8px;
  }
  input,
  select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 6px 9px;
    outline: none;
    font: inherit;
  }
  input:focus,
  select:focus {
    border-color: var(--amber);
  }
  .top input {
    flex: 1;
    font-size: 14px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .check input {
    accent-color: var(--amber);
    margin: 0;
  }
  .list {
    flex: 1;
    min-height: 120px;
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 5px 8px;
    border-radius: 5px;
    cursor: default;
  }
  .row.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--amber);
  }
  .mark {
    width: 10px;
    color: var(--amber);
    font-size: 9px;
    flex-shrink: 0;
  }
  .plus {
    width: 10px;
    color: var(--add);
    font-weight: 700;
  }
  .two {
    flex: 1;
    min-width: 0;
  }
  .l1 {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .bname {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .current .bname {
    font-weight: 600;
  }
  .l2 {
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
  }
  .sha {
    color: var(--amber);
    font-size: 11px;
  }
  .ahead {
    color: var(--add);
    font-size: 11px;
  }
  .behind {
    color: var(--blue);
    font-size: 11px;
  }
  .tag {
    font-size: 10.5px;
    padding: 0 6px;
    border-radius: 8px;
    border: 1px solid var(--line-strong);
    color: var(--muted);
    white-space: nowrap;
  }
  .tag.wt {
    color: var(--blue);
    border-color: transparent;
    background: var(--hover);
  }
  .tag.merged {
    color: var(--add);
    border-color: transparent;
    background: var(--add-bg);
  }
  .tag.gone {
    color: var(--del);
    border-color: transparent;
    background: var(--del-bg);
  }
  .tag.chg {
    color: var(--amber);
    border-color: transparent;
    background: var(--amber-soft);
  }
  .tag.working {
    color: var(--add);
    border-color: var(--add);
  }
  .acts {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  .acts button {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 4px;
    color: var(--muted);
  }
  .acts button:hover {
    background: var(--line);
    color: var(--text);
  }
  .acts button.danger:hover {
    color: var(--del);
  }
  .grow {
    flex: 1;
  }
  .keys {
    font-size: 11px;
  }
  .note {
    margin: 8px;
    color: var(--muted);
  }
  .note.err {
    color: var(--del);
    font-family: var(--mono);
    font-size: 12px;
    white-space: pre-wrap;
  }
  .foot {
    margin: 8px 2px 0;
    font-size: 11.5px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .form h3 {
    margin: 0;
    font-size: 14px;
  }
  .form label:not(.check) {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .hint.small {
    margin-top: -4px;
  }
  .warn {
    margin: 0;
    color: var(--amber);
    font-size: 12px;
  }
  .buttons {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    align-items: center;
    margin-top: 10px;
  }
  .btn {
    padding: 5px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
  }
  .btn.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--on-accent);
    font-weight: 600;
  }
  .btn.primary:disabled {
    background: var(--raised);
    border-color: var(--line);
    color: var(--faint);
    cursor: default;
  }
</style>
