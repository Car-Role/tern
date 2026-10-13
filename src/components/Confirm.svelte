<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { ago } from "../lib/format";

  const req = app.confirmReq;
  let okBtn: HTMLButtonElement | undefined = $state();
  let cancelBtn: HTMLButtonElement | undefined = $state();
  const allowed = req?.allowed !== false;

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && allowed && document.activeElement !== cancelBtn) {
      e.preventDefault();
      app.answer(true);
    }
  }

  onMount(() => (allowed ? okBtn : cancelBtn)?.focus({ preventScroll: true }));
  // Closing any other way (Esc, a click outside) means "no".
  onDestroy(() => {
    if (app.confirmReq === req) app.answer(false);
  });
</script>

{#if req}
  <div class="backdrop fx-fade" role="presentation" onmousedown={(e) => e.target === e.currentTarget && app.answer(false)}>
    <div class="dialog fx-fade" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1" onkeydown={onKey}>
      <h2 id="confirm-title">{req.title}</h2>
      <div class="scroll">
      <p class="body">{req.body}</p>
      {#if req.code}<pre class="code mono">{req.code}</pre>{/if}

      {#if req.warnings?.length}
        <ul class="warnings">
          {#each req.warnings as w}<li>{w}</li>{/each}
        </ul>
      {/if}

      {#if req.list?.length}
        <p class="label">{req.listLabel ?? ""}</p>
        <ul class="projects">
          {#each req.list as it}
            <li class={it.tone ?? ""}><span class="pname">{it.name}</span><span class="pdetail">{it.detail}</span></li>
          {/each}
        </ul>
      {/if}

      {#if req.commits}
        <p class="label">{req.commitsLabel}</p>
        {#if req.commits.length}
          <ol class="commits">
            {#each req.commits.slice(0, 12) as c (c.sha)}
              <li>
                <span class="sha mono">{c.short}</span>
                <span class="subject">{c.subject}</span>
                <span class="meta faint">{c.author}, {ago(c.time)}</span>
              </li>
            {/each}
          </ol>
          {#if req.commits.length > 12}<p class="more faint">and {req.commits.length - 12} more</p>{/if}
        {/if}
      {/if}

      </div>
      <div class="buttons">
        <button class="btn" bind:this={cancelBtn} onclick={() => app.answer(false)}>Cancel</button>
        <button class="btn primary" bind:this={okBtn} disabled={!allowed} onclick={() => app.answer(true)}>{req.confirm}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Opened with the mouse, now and then: a short entrance. Closing stays instant. */
  .backdrop {
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .dialog {
    animation: tern-dialog-in 150ms var(--ease-out);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.5);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: min(14vh, 80px) 16px 16px;
    z-index: 20;
  }
  .dialog {
    width: min(560px, 100%);
    max-height: calc(100vh - min(14vh, 80px) - 16px);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
    padding: 18px 20px 16px;
    outline: none;
    user-select: text;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 15px;
    font-weight: 650;
  }
  .body {
    margin: 0 0 12px;
    color: var(--muted);
  }
  .code {
    margin: -4px 0 12px;
    padding: 8px 10px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    white-space: pre-wrap;
    word-break: break-all;
  }
  .warnings {
    margin: 0 0 12px;
    padding: 8px 12px 8px 26px;
    background: var(--amber-soft);
    border-radius: var(--radius);
    color: var(--text);
  }
  .warnings li + li {
    margin-top: 4px;
  }
  .label {
    margin: 0 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .commits {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .commits li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 5px 10px;
    white-space: nowrap;
  }
  .commits li + li {
    border-top: 1px solid var(--line);
  }
  .sha {
    color: var(--amber);
    flex-shrink: 0;
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .meta {
    font-size: 11.5px;
    flex-shrink: 0;
  }
  .more {
    margin: 4px 0 0;
    font-size: 12px;
  }
  .projects {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    max-height: 40vh;
    overflow-y: auto;
  }
  .projects li {
    display: flex;
    gap: 12px;
    padding: 5px 10px;
    align-items: baseline;
  }
  .projects li + li {
    border-top: 1px solid var(--line);
  }
  .pname {
    font-weight: 600;
    flex-shrink: 0;
    min-width: 140px;
  }
  .pdetail {
    color: var(--muted);
    font-family: var(--mono);
    font-size: 11.5px;
  }
  .projects li.warn .pdetail {
    color: var(--amber);
  }
  .projects li.skip {
    opacity: 0.55;
  }
  .scroll {
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
    flex-shrink: 0;
  }
  .btn {
    padding: 5px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover {
    color: var(--text);
    border-color: var(--line-strong);
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
