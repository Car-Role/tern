<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { baseName } from "../lib/format";

  const d = $derived(app.prDraft);
  let title = $state("");
  let body = $state("");
  let base = $state("");
  let draft = $state(false);
  let titleEl: HTMLInputElement | undefined = $state();
  let filled = false;

  // Fill the form once the draft arrives.
  $effect(() => {
    const x = d?.data;
    if (!x || filled) return;
    filled = true;
    title = x.title;
    body = x.body;
    base = x.bases[0] ?? "";
    tick().then(() => titleEl?.select());
  });

  const busy = $derived(app.prCreating);
  const where = $derived(d?.data?.provider === "azdo" ? "Azure DevOps" : "GitHub");

  function submit() {
    if (!title.trim() || busy) return;
    app.createPr(title, body, base, draft);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      app.overlay = null;
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submit();
    }
  }
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && !busy && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="pr-title" tabindex="-1" onkeydown={onKey}>
    <div class="dialog-head">
      <h2 id="pr-title">New pull request</h2>
    </div>
    {#if !d || d.loading}
      <div class="dialog-body">
        <div class="state" role="status"><p>Reading the branch…</p></div>
      </div>
      <div class="dialog-foot">
        <button class="btn" onclick={() => (app.overlay = null)}>Cancel</button>
      </div>
    {:else if d.error}
      <div class="dialog-body">
        <div class="state inline">
          <span class="state-title">Could not read the branch</span>
          <pre>{d.error}</pre>
        </div>
      </div>
      <div class="dialog-foot">
        <button class="btn" onclick={() => (app.overlay = null)}>Close</button>
      </div>
    {:else if d.data}
      {@const x = d.data}
      <div class="dialog-body">
        <p class="from">
          From <b>{x.branch}</b> into
          <select class="field" bind:value={base} aria-label="Into branch">
            {#each x.bases as b}<option value={b}>{b}</option>{/each}
          </select>
          on {where}, in {baseName(app.current ?? "")}.
        </p>
        {#if !x.pushed}
          <p class="note">Tern pushes <b>{x.branch}</b> first, because the remote does not have all of its commits yet.</p>
        {/if}
        <label class="lbl">
          <span>Title</span>
          <input class="field" bind:this={titleEl} bind:value={title} spellcheck="true" />
        </label>
        <label class="lbl">
          <span>Description</span>
          <textarea class="field" bind:value={body} rows="7" spellcheck="true" placeholder="What changed, and why"></textarea>
        </label>
        {#if x.commits.length}
          <details class="commits">
            <summary>{x.commits.length} {x.commits.length === 1 ? "commit" : "commits"} since {base}</summary>
            <ul>{#each x.commits as c}<li>{c}</li>{/each}</ul>
          </details>
        {/if}
        <label class="check"><input type="checkbox" bind:checked={draft} /> Open as a draft</label>
      </div>
      <div class="dialog-foot">
        <span class="hint"><kbd>Ctrl Enter</kbd> creates it</span>
        <button class="btn" onclick={() => (app.overlay = null)} disabled={busy}>Cancel</button>
        <button class="btn primary" onclick={submit} disabled={busy || !title.trim()}>
          {busy ? (x.pushed ? "Creating…" : "Pushing and creating…") : x.pushed ? "Create pull request" : "Push and create"}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .from {
    margin: 0 0 var(--s4);
    color: var(--muted);
    font-size: var(--fs-sm);
    line-height: 2;
  }
  .from b {
    color: var(--text);
    font-weight: 600;
  }
  .from select {
    height: var(--h-ctl-sm);
    padding: 0 var(--s1);
  }
  .note {
    font-size: var(--fs-sm);
    color: var(--text);
    background: var(--raised);
    padding: var(--s2) var(--s3);
    border-radius: var(--radius);
    margin: 0 0 var(--s4);
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    margin-bottom: var(--s4);
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .lbl .field {
    font-size: var(--fs-md);
  }
  .commits {
    font-size: var(--fs-sm);
    color: var(--muted);
    margin-bottom: var(--s4);
  }
  .commits summary {
    cursor: pointer;
  }
  .commits ul {
    margin: var(--s2) 0 0;
    padding-left: var(--s5);
  }
</style>
