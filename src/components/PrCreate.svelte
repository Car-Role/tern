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

<div class="backdrop fx-fade" role="presentation" onmousedown={(e) => e.target === e.currentTarget && !busy && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="dialog fx-fade" role="dialog" aria-label="New pull request" tabindex="-1" onkeydown={onKey}>
    <h2>New pull request</h2>
    {#if !d || d.loading}
      <p class="muted">Reading the branch…</p>
    {:else if d.error}
      <p class="err mono">{d.error}</p>
      <div class="actions"><button class="btn" onclick={() => (app.overlay = null)}>Close</button></div>
    {:else if d.data}
      {@const x = d.data}
      <p class="muted">
        From <b class="mono">{x.branch}</b> into
        <select bind:value={base} aria-label="Into branch">
          {#each x.bases as b}<option value={b}>{b}</option>{/each}
        </select>
        on {where}, in {baseName(app.current ?? "")}.
      </p>
      {#if !x.pushed}
        <p class="note">Tern pushes <span class="mono">{x.branch}</span> first, because the remote does not have all of its commits yet.</p>
      {/if}
      <label class="field">
        <span>Title</span>
        <input bind:this={titleEl} bind:value={title} spellcheck="true" />
      </label>
      <label class="field">
        <span>Description</span>
        <textarea bind:value={body} rows="7" spellcheck="true" placeholder="What changed, and why"></textarea>
      </label>
      {#if x.commits.length}
        <details class="commits">
          <summary>{x.commits.length} {x.commits.length === 1 ? "commit" : "commits"} since {base}</summary>
          <ul>{#each x.commits as c}<li>{c}</li>{/each}</ul>
        </details>
      {/if}
      <div class="actions">
        <label class="draft"><input type="checkbox" bind:checked={draft} /> Open as a draft</label>
        <span class="grow"></span>
        <button class="btn" onclick={() => (app.overlay = null)} disabled={busy}>Cancel</button>
        <button class="btn primary" onclick={submit} disabled={busy || !title.trim()}>
          {busy ? (x.pushed ? "Creating…" : "Pushing and creating…") : x.pushed ? "Create pull request" : "Push and create"}
        </button>
      </div>
      <p class="keys faint"><kbd>Ctrl Enter</kbd> creates it</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 10vh;
    background: rgba(8, 10, 14, 0.45);
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .dialog {
    width: min(620px, calc(100vw - 32px));
    max-height: 80vh;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    padding: 18px 20px 14px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
    animation: tern-dialog-in 200ms var(--ease-out);
  }
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .muted {
    color: var(--muted);
    font-size: 12.5px;
    margin: 0 0 12px;
    line-height: 1.8;
  }
  .muted b {
    color: var(--text);
    font-weight: 600;
  }
  select {
    font: inherit;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 2px 6px;
  }
  .note {
    font-size: 12px;
    color: var(--amber);
    background: var(--amber-soft);
    padding: 6px 10px;
    border-radius: 6px;
    margin: 0 0 12px;
  }
  .err {
    color: var(--del);
    font-size: 12px;
    white-space: pre-wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .field input,
  .field textarea {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 7px 10px;
    outline: none;
    resize: vertical;
  }
  .field input:focus,
  .field textarea:focus {
    border-color: var(--amber);
  }
  .commits {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 10px;
  }
  .commits summary {
    cursor: pointer;
  }
  .commits ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .grow {
    flex: 1;
  }
  .draft {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .draft input {
    accent-color: var(--amber);
  }
  .btn {
    font-size: 12.5px;
    padding: 5px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .btn.primary {
    color: var(--on-accent);
    background: var(--amber);
    border-color: var(--amber);
  }
  .btn:disabled {
    opacity: 0.55;
  }
  .keys {
    margin: 8px 0 0;
    font-size: 11px;
    text-align: right;
  }
</style>
