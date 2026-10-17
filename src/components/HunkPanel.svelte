<script lang="ts">
  import { app } from "../lib/store.svelte";
  import type { Hunk } from "../lib/types";
  import { hover } from "../lib/hover";

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
      <button class="jump mono" onclick={() => app.jumpToLine(hk.newStart)} use:hover={() => ({ title: "Show this change in the diff" })}>L{hk.newStart}</button>
      <span class="hlabel mono">{label(hk)}</span>
      <span class="counts mono"><span class="a">+{hk.added}</span> <span class="d">−{hk.removed}</span></span>
    </div>
    <pre class="lines">{#each hk.lines.slice(0, MAX_LINES) as l}<span class={lineClass(l)}>{l}
</span>{/each}{#if hk.lines.length > MAX_LINES}<span class="more">… {hk.lines.length - MAX_LINES} more lines</span>{/if}</pre>
    <div class="hact">
      {#if staged}
        <button class="btn sm" disabled={busy} onclick={() => app.applyHunk(hk, "unstage")}>Unstage</button>
      {:else}
        <button class="btn sm danger" disabled={busy} onclick={() => app.applyHunk(hk, "discard")}>Discard</button>
        <button class="btn sm" disabled={busy} onclick={() => app.applyHunk(hk, "stage")}>Stage</button>
      {/if}
    </div>
  </div>
{/snippet}

<aside class="hunks" aria-label="Hunks">
  <header class="toolbar sub">
    <span class="title">Hunks</span>
    <span class="meta">Stage or discard one block of changes</span>
    <span class="grow"></span>
    <button class="icon-btn sm" onclick={() => app.toggleHunks(false)} aria-label="Close hunks"
      use:hover={() => ({ title: "Close hunks", hint: "Ctrl Shift H" })}>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
    </button>
  </header>
  <div class="body">
    {#if !h || h.loading}
      <div class="state"><p>Reading the changes…</p></div>
    {:else if h.error}
      <div class="state">
        <span class="state-title">Could not read the changes</span>
        <pre>{h.error}</pre>
      </div>
    {:else if h.untracked}
      <div class="state">
        <span class="state-title">New file</span>
        <p>It has no blocks yet. Stage the whole file from the list (<kbd>S</kbd>).</p>
      </div>
    {:else if h.data?.binary}
      <div class="state">
        <span class="state-title">Binary file</span>
        <p>Stage or discard the whole file from the list.</p>
      </div>
    {:else if h.data && !h.data.unstaged.length && !h.data.staged.length}
      <div class="state"><p>No changes left in this file.</p></div>
    {:else if h.data}
      {#if h.data.unstaged.length}
        <div class="section-label">Not staged <span class="badge">{h.data.unstaged.length}</span></div>
        {#each h.data.unstaged as hk (hk.patch)}{@render hunk(hk, false)}{/each}
      {/if}
      {#if h.data.staged.length}
        <div class="section-label">Staged <span class="badge">{h.data.staged.length}</span></div>
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
  .toolbar .meta {
    flex-shrink: 1;
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: var(--s1) var(--s3) var(--s5);
  }
  .body > .state {
    margin: 0 auto;
  }
  .section-label {
    padding: var(--s4) var(--s1) var(--s2);
    flex-shrink: 0;
  }
  .hunk {
    flex-shrink: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    margin-bottom: var(--s3);
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
    gap: var(--s3);
    padding: var(--s1) var(--s3);
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
    min-width: 0;
  }
  .jump {
    color: var(--muted);
    font-size: var(--fs-xs);
    flex-shrink: 0;
  }
  .jump:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .hlabel {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .counts {
    flex-shrink: 0;
    font-size: var(--fs-xs);
  }
  .a {
    color: var(--add);
  }
  .d {
    color: var(--del);
  }
  .lines {
    margin: 0;
    padding: var(--s2) 0;
    font: var(--fs-xs) / 1.5 var(--mono);
    overflow-x: auto;
    white-space: pre;
  }
  .lines span {
    display: block;
    padding: 0 var(--s3);
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
    gap: var(--s2);
    justify-content: flex-end;
    padding: var(--s2) var(--s3);
    border-top: 1px solid var(--line);
  }
</style>
