<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { rank } from "../lib/fuzzy";
  import { ago, baseName } from "../lib/format";
  import type { Branch } from "../lib/types";
  import { hover } from "../lib/hover";

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
  <div class="dialog lg picker" role="dialog" aria-modal="true" aria-label="Branches and worktrees">
    <div class="dialog-head head">
      <div class="tabs" role="tablist">
        <button role="tab" class:on={tab === "branches"} aria-selected={tab === "branches"} onclick={() => { tab = "branches"; formOpen = false; }}>Branches</button>
        <button role="tab" class:on={tab === "worktrees"} aria-selected={tab === "worktrees"} onclick={() => (tab = "worktrees")}>Worktrees <span class="badge">{worktrees.length - 1}</span></button>
      </div>
      <span class="grow"></span>
      <span class="where">{app.repos.find((r) => r.path === main)?.name ?? baseName(main)}</span>
    </div>

    {#if formOpen}
      <div class="dialog-body form">
        <h3>New worktree</h3>
        <p class="help">A worktree is a second folder for the same project, on its own branch. Work there does not touch your main folder.</p>
        <label class="lbl">Branch
          <input class="field" bind:value={wtBranch} list="tern-branch-list" spellcheck="false" oninput={suggest} placeholder="new-branch-name or an existing branch" />
          <datalist id="tern-branch-list">
            {#each branches.filter((b) => !b.worktree) as b}<option value={b.name}></option>{/each}
          </datalist>
        </label>
        {#if wtTaken}
          <p class="warn">{wtBranch} is already checked out in {wtTaken.worktree}. Git allows a branch in one worktree at a time.</p>
        {:else if wtIsNew}
          <label class="lbl">Start from
            <select class="field" bind:value={wtBase}>
              {#each bases as b}<option value={b}>{b}</option>{/each}
              <option value="">the current commit</option>
            </select>
          </label>
          <p class="help">A new branch named <strong>{wtBranch}</strong> is made.</p>
        {/if}
        <label class="lbl">Folder <input class="field mono" bind:value={wtPath} spellcheck="false" /></label>
        <label class="check"><input type="checkbox" bind:checked={wtAgent} /> Start Claude Code in it</label>
      </div>
      <div class="dialog-foot">
        <span class="grow"></span>
        <button class="btn" onclick={() => (formOpen = false)}>Back</button>
        <button class="btn primary" disabled={!wtBranch.trim() || !wtPath.trim() || !!wtTaken || busy} onclick={createWorktree}>
          {busy ? "Making…" : "Make worktree"}
        </button>
      </div>
    {:else if tab === "branches"}
      <div class="top">
        <input class="field grow" bind:this={input} bind:value={query} onkeydown={onKey} placeholder="Find a branch, or type a new name" spellcheck="false" aria-label="Branch" />
        <label class="check"><input type="checkbox" bind:checked={showRemote} /> Remote branches</label>
      </div>
      <div class="list" bind:this={listEl} role="listbox">
        {#if loading}<p class="note">Reading branches…</p>{/if}
        {#if error}
          <div class="state inline">
            <span class="state-title">Could not read branches</span>
            <pre>{error}</pre>
            <div class="actions"><button class="btn sm" onclick={load}>Try again</button></div>
          </div>
        {/if}
        {#if canCreate}
          <button class="row create" class:on={active === 0} role="option" aria-selected={active === 0} onmousemove={() => (active = 0)} onclick={createBranch}>
            <span class="plus">+</span>
            <span class="bname">Make branch <strong>{query.trim()}</strong> from here</span>
            <span class="grow"></span>
            <span class="keys"><kbd>Enter</kbd> switch to it · <kbd>Alt Enter</kbd> in a new worktree</span>
          </button>
        {/if}
        {#each shown as b, i (b.name)}
          {@const idx = canCreate ? i + 1 : i}
          <div class="row" class:on={idx === active} class:current={b.current} role="option" aria-selected={idx === active} tabindex="-1"
            onmousemove={() => (active = idx)} onclick={() => doSwitch(b)} onkeydown={() => {}}>
            <span class="mark">{b.current ? "●" : ""}</span>
            <div class="two">
              <div class="l1">
                <span class="bname">{b.name}</span>
                {#if b.ahead}<span class="ahead mono">↑{b.ahead}</span>{/if}
                {#if b.behind}<span class="behind mono">↓{b.behind}</span>{/if}
                {#if b.worktree && b.worktree.toLowerCase() !== repo.toLowerCase()}<span class="chip wt">in {baseName(b.worktree)}</span>{/if}
                {#if b.merged}<span class="chip merged">merged</span>{/if}
                {#if b.gone}<span class="chip gone">remote gone</span>{/if}
              </div>
              <div class="l2"><span class="sha mono">{b.sha}</span> {b.subject} <span class="faint">{b.author}, {ago(b.time)}</span></div>
            </div>
            {#if idx === active}
              <div class="acts">
                {#if !b.current}<button class="btn ghost sm" onclick={(e) => { e.stopPropagation(); doSwitch(b); }}>{b.worktree ? "Open" : "Switch"}</button>{/if}
                {#if !b.worktree}<button class="btn ghost sm" onclick={(e) => { e.stopPropagation(); openForm(b.name); }}>New worktree</button>{/if}
                {#if !b.remote && !b.current && !b.worktree}
                  <button class="btn ghost sm danger" onclick={async (e) => { e.stopPropagation(); if (await app.deleteBranch(b.name, b.merged)) load(); }}>Delete</button>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
      <div class="dialog-foot">
        <span class="hint"><kbd>Enter</kbd> switch · <kbd>Alt Enter</kbd> open in a new worktree{#if base}{" · "}"merged" means its commits are in {base}{/if}</span>
      </div>
    {:else}
      <div class="list wtlist">
        {#each worktrees as w (w.path)}
          {@const s = app.sums[w.path]}
          {@const isMain = w.path === main}
          <div class="row wtrow" class:current={w.path === repo}>
            <span class="mark">{w.path === repo ? "●" : ""}</span>
            <div class="two">
              <div class="l1">
                <span class="bname">{isMain ? "Main folder" : w.name}</span>
                <span class="faint">{s?.branch ?? (s?.error ? "" : "detached")}</span>
                {#if s?.changes.length}<span class="chip chg">{s.changes.length} changed</span>{/if}
                {#if app.isWorking(w.path)}<span class="chip working">working</span>{/if}
                {#if s?.error}<span class="chip gone">broken</span>{/if}
              </div>
              <div class="l2 mono faint">{w.path}</div>
            </div>
            <div class="acts">
              {#if w.path !== repo}<button class="btn ghost sm" onclick={() => { app.overlay = null; app.select(w.path); }}>Open</button>{/if}
              {#if !isMain && !s?.error}
                <button class="btn ghost sm" onclick={async () => { app.overlay = null; app.select(w.path); await app.openBranchReview(); }}>Review</button>
              {/if}
              {#if !isMain}<button class="btn ghost sm danger" onclick={async () => { app.overlay = null; await app.removeWorktree(w.path); }}>Remove</button>{/if}
            </div>
          </div>
        {/each}
      </div>
      <div class="dialog-foot">
        <button class="btn" onclick={() => app.pruneWorktrees(main)} use:hover={() => ({ title: "Clean up missing", text: "Forget worktrees whose folder was deleted." })}>Clean up missing</button>
        <span class="grow"></span>
        <button class="btn primary" onclick={() => openForm("")}>New worktree…</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .picker {
    height: min(640px, calc(88vh - var(--s5)));
  }
  /* The head holds tabs, so they run the full height and their underline sits on the divider. */
  .head {
    align-items: stretch;
    padding-top: 0;
    padding-bottom: 0;
    flex-shrink: 0;
  }
  .where {
    align-self: center;
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .top {
    display: flex;
    gap: var(--s4);
    align-items: center;
    padding: var(--s4) var(--s5) var(--s3);
    flex-shrink: 0;
  }
  .check {
    white-space: nowrap;
  }
  /* The list is as tall as its branches, up to half the window, so a small project gives a small dialog. */
  .list {
    flex: 0 1 auto;
    max-height: 50vh;
    min-height: 120px;
    overflow-y: auto;
    margin: 0 var(--s5) var(--s4);
    background: var(--bg);
    border-radius: var(--radius);
    padding: var(--s1);
  }
  .wtlist {
    margin-top: var(--s4);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    min-height: var(--h-row);
    text-align: left;
    padding: var(--s1) var(--s3);
    border-radius: var(--r-sm);
    cursor: default;
  }
  .row.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .mark {
    width: var(--s3);
    color: var(--text);
    font-size: var(--fs-xs);
    flex-shrink: 0;
  }
  .plus {
    width: var(--s3);
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
    gap: var(--s2);
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
    font-size: var(--fs-xs);
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: var(--s1);
  }
  .sha {
    color: var(--faint);
    font-size: var(--fs-xs);
  }
  .ahead {
    color: var(--add);
    font-size: var(--fs-xs);
  }
  .behind {
    color: var(--blue);
    font-size: var(--fs-xs);
  }
  /* Tones for the small labels on a row. */
  .chip.wt {
    color: var(--blue);
    border-color: transparent;
    background: var(--hover);
  }
  .chip.merged,
  .chip.working {
    color: var(--add);
    border-color: transparent;
    background: var(--add-bg);
  }
  .chip.gone {
    color: var(--del);
    border-color: transparent;
    background: var(--del-bg);
  }
  .chip.chg {
    color: var(--accent);
    border-color: transparent;
    background: var(--accent-soft);
  }
  .acts {
    display: flex;
    gap: var(--s1);
    flex-shrink: 0;
  }
  .keys {
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .note {
    margin: var(--s3);
    color: var(--muted);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    flex: 1;
    padding-top: var(--s4);
  }
  .form h3 {
    margin: 0;
    font-size: var(--fs-md);
    font-weight: 600;
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .help {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .warn {
    margin: 0;
    color: var(--del);
    font-size: var(--fs-sm);
  }
</style>
