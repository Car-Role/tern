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
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && app.answer(false)}>
    <div class="dialog sm" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1" onkeydown={onKey}>
      <div class="dialog-head"><h2 id="confirm-title">{req.title}</h2></div>
      <div class="dialog-body">
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
      <div class="dialog-foot">
        <button class="btn" bind:this={cancelBtn} onclick={() => app.answer(false)}>Cancel</button>
        <button class="btn primary" class:danger={req.danger} bind:this={okBtn} disabled={!allowed} onclick={() => app.answer(true)}>{req.confirm}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog {
    user-select: text;
  }
  .body {
    margin: 0 0 var(--s4);
    color: var(--muted);
  }
  .code {
    margin: 0 0 var(--s4);
    padding: var(--s3) var(--s4);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    white-space: pre-wrap;
    word-break: break-all;
  }
  .warnings {
    margin: 0 0 var(--s4);
    padding: var(--s3) var(--s4) var(--s3) var(--s6);
    background: var(--accent-soft);
    border-radius: var(--radius);
    color: var(--text);
  }
  .warnings li + li {
    margin-top: var(--s1);
  }
  .label {
    margin: 0 0 var(--s1);
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--muted);
  }
  .commits,
  .projects {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .commits li,
  .projects li {
    display: flex;
    gap: var(--s3);
    align-items: baseline;
    padding: var(--s1) var(--s3);
  }
  .commits li {
    white-space: nowrap;
  }
  .commits li + li,
  .projects li + li {
    border-top: 1px solid var(--line);
  }
  .sha {
    color: var(--muted);
    flex-shrink: 0;
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .meta {
    font-size: var(--fs-xs);
    flex-shrink: 0;
  }
  .more {
    margin: var(--s1) 0 0;
    font-size: var(--fs-sm);
  }
  .projects {
    max-height: 40vh;
    overflow-y: auto;
  }
  .pname {
    font-weight: 600;
    flex-shrink: 0;
    min-width: 140px;
  }
  .pdetail {
    color: var(--muted);
    font-family: var(--mono);
    font-size: var(--fs-xs);
  }
  .projects li.warn .pdetail {
    color: var(--accent);
  }
  .projects li.skip {
    opacity: 0.55;
  }
</style>
