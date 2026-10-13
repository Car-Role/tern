<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { copyText } from "../lib/api";

  async function copyRepo() {
    if (!app.current) return;
    await copyText(app.current);
    app.notify(`Copied ${app.current}`, "ok");
  }
</script>

<footer class="bar">
  {#if app.current}
    <button class="repo mono" onclick={copyRepo} title="Copy project path">{app.current}</button>
  {:else}
    <span class="faint">No project open</span>
  {/if}
  <span class="msg {app.toast?.kind ?? ''}" role="status" aria-live="polite">
    {#key app.toast?.id}<span class="toast fx-fade">{app.toast?.text ?? ""}</span>{/key}
  </span>
  {#if app.runs.length}
    {@const n = app.runs.filter((r) => r.running).length}
    <button class="tasks" class:live={n > 0} onclick={() => app.setPanel(!app.panelOpen)} title="Show or hide the task panel (Ctrl+J)">
      {n ? `${n} running` : `${app.runs.length} ${app.runs.length === 1 ? "task" : "tasks"}`}
    </button>
  {/if}
  <span class="hints">
    <span><kbd>Ctrl Shift C</kbd> copy</span>
    <span><kbd>F1</kbd> keys</span>
  </span>
</footer>

<style>
  .bar {
    grid-column: 1 / -1;
    grid-row: 4;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    border-top: 1px solid var(--line);
    background: var(--panel);
    font-size: 12px;
    white-space: nowrap;
    min-width: 0;
  }
  .repo {
    color: var(--faint);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 38%;
  }
  .repo:hover {
    color: var(--text);
  }
  .msg {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted);
  }
  .toast {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: bottom;
    animation: tern-rise-in 180ms var(--ease-out);
  }
  .msg.ok {
    color: var(--add);
  }
  .msg.err {
    color: var(--del);
  }
  .tasks {
    color: var(--muted);
    font-size: 11.5px;
  }
  .tasks:hover {
    color: var(--text);
  }
  .tasks.live {
    color: var(--add);
  }
  .hints {
    display: flex;
    gap: 12px;
    color: var(--faint);
  }
  .hints kbd {
    margin-right: 3px;
  }
</style>
