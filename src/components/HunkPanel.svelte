<script lang="ts">
  import { app } from "../lib/store.svelte";
  import type { Hunk } from "../lib/types";

  const h = $derived(app.hunks);
  const MAX_LINES = 14;

  function lineClass(l: string) {
    return l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : l.startsWith("\\") ? "note" : "ctx";
  }
  /** "@@ -12,6 +12,8 @@ fn load" -> "fn load" (the part after the second @@), or the line numbers. */
  function label(hk: Hunk) {
    const after = hk.header.split("@@").slice(2).join("@@").trim();
    return after || `Line ${hk.newStart}`;
  }
</script>

{#snippet hunk(hk: Hunk, staged: boolean)}
  {@const busy = app.hunkBusy === hk.patch}
  <div class="hunk" class:busy>
    <div class="hhead">
      <button class="jump mono" onclick={() => app.jumpToLine(hk.newStart)} title="Show this change in the diff">L{hk.newStart}</button>
      <span class="hlabel mono">{label(hk)}</span>
      <span class="counts mono"><span class="a">+{hk.added}</span> <span class="d">−{hk.removed}</span></span>
    </div>
    <pre class="lines">{#each hk.lines.slice(0, MAX_LINES) as l}<span class={lineClass(l)}>{l}
</span>{/each}{#if hk.lines.length > MAX_LINES}<span class="more">… {hk.lines.length - MAX_LINES} more lines</span>{/if}</pre>
    <div class="hact">
      {#if staged}
        <button class="hb" disabled={busy} onclick={() => app.applyHunk(hk, "unstage")}>Unstage</button>
      {:else}
        <button class="hb primary" disabled={busy} onclick={() => app.applyHunk(hk, "stage")}>Stage</button>
        <button class="hb danger" disabled={busy} onclick={() => app.applyHunk(hk, "discard")}>Discard</button>
      {/if}
    </div>
  </div>
{/snippet}

<aside class="hunks" aria-label="Hunks">
  <header>
    <span class="title">Hunks</span>
    <span class="faint">Stage or discard one block of changes</span>
    <span class="grow"></span>
    <button class="x" onclick={() => app.toggleHunks(false)} aria-label="Close hunks" title="Close (Ctrl+Shift+H)">×</button>
  </header>
  <div class="body">
    {#if !h || h.loading}
      <p class="note">Reading the changes…</p>
    {:else if h.error}
      <p class="note err mono">{h.error}</p>
    {:else if h.untracked}
      <p class="note">This is a new file, so it has no blocks yet. Stage the whole file from the list (<kbd>S</kbd>).</p>
    {:else if h.data?.binary}
      <p class="note">This is a binary file. Stage or discard the whole file from the list.</p>
    {:else if h.data && !h.data.unstaged.length && !h.data.staged.length}
      <p class="note">No changes left in this file.</p>
    {:else if h.data}
      {#if h.data.unstaged.length}
        <div class="sec">Not staged <span class="faint">{h.data.unstaged.length}</span></div>
        {#each h.data.unstaged as hk (hk.patch)}{@render hunk(hk, false)}{/each}
      {/if}
      {#if h.data.staged.length}
        <div class="sec">Staged <span class="faint">{h.data.staged.length}</span></div>
        {#each h.data.staged as hk (hk.patch)}{@render hunk(hk, true)}{/each}
      {/if}
    {/if}
  </div>
</aside>

<style>
  .hunks {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(400px, 45%);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-left: 1px solid var(--line);
    animation: tern-fade-in 150ms var(--ease-out);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 10px 10px 8px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .title {
    font-weight: 650;
    font-size: 13px;
  }
  .grow {
    flex: 1;
  }
  .x {
    color: var(--muted);
    font-size: 16px;
    line-height: 1;
    padding: 0 4px;
  }
  .x:hover {
    color: var(--text);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 10px 16px;
  }
  .sec {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
    padding: 12px 4px 6px;
  }
  .note {
    color: var(--muted);
    font-size: 12.5px;
    padding: 12px 4px;
    margin: 0;
  }
  .note.err {
    color: var(--del);
    white-space: pre-wrap;
  }
  .hunk {
    border: 1px solid var(--line);
    border-radius: 8px;
    margin-bottom: 8px;
    overflow: hidden;
    background: var(--bg);
    transition: opacity 150ms var(--ease-out);
  }
  .hunk.busy {
    opacity: 0.5;
  }
  .hhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
    min-width: 0;
  }
  .jump {
    color: var(--amber);
    font-size: 11px;
    flex-shrink: 0;
  }
  .jump:hover {
    text-decoration: underline;
  }
  .hlabel {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .counts {
    flex-shrink: 0;
  }
  .a {
    color: var(--add);
  }
  .d {
    color: var(--del);
  }
  .lines {
    margin: 0;
    padding: 6px 0;
    font: 11.5px/1.5 var(--mono);
    overflow-x: auto;
    white-space: pre;
  }
  .lines span {
    display: block;
    padding: 0 8px;
  }
  .lines .add {
    background: var(--add-bg);
    color: var(--text);
  }
  .lines .del {
    background: var(--del-bg);
    color: var(--text);
  }
  .lines .ctx,
  .lines .note,
  .lines .more {
    color: var(--muted);
  }
  .hact {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    padding: 6px 8px;
    border-top: 1px solid var(--line);
  }
  .hb {
    font-size: 12px;
    padding: 2px 10px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .hb:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .hb.primary {
    color: var(--on-accent);
    background: var(--amber);
    border-color: var(--amber);
  }
  .hb.danger:hover:not(:disabled) {
    color: var(--del);
    border-color: var(--del);
  }
</style>
