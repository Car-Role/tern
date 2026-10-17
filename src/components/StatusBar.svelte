<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { copyText } from "../lib/api";
  import { hover } from "../lib/hover";

  async function copyRepo() {
    if (!app.current) return;
    await copyText(app.current);
    app.notify(`Copied ${app.current}`, "ok");
  }
  const s = $derived(app.current ? app.sums[app.current] : undefined);
</script>

<footer class="bar">
  {#if app.current}
    <button class="repo mono" onclick={copyRepo} use:hover={() => ({ title: "Copy the project path", text: app.current ?? "" })}>{app.current}</button>
    {#if s?.behind || s?.ahead}
      <span class="sync mono">
        {#if s?.behind}<span class="behind">↓{s.behind}</span>{/if}
        {#if s?.ahead}<span class="ahead">↑{s.ahead}</span>{/if}
      </span>
    {/if}
  {/if}
  <span class="msg {app.toast?.kind ?? ''}" role="status" aria-live="polite">
    {#key app.toast?.id}
      {#if app.toast}<button class="toast fx-fade" onclick={() => app.toggleNotices(true)}>{app.toast.text}</button>{/if}
    {/key}
  </span>
  {#if app.update}
    {@const u = app.update}
    <button class="upd" disabled={u.state === "downloading"} onclick={() => app.installUpdate()}
      use:hover={() => ({ title: u.state === "failed" ? "Try the update again" : `Download Tern ${u.info.version} and restart` })}>
      {#if u.state === "downloading"}Updating… {Math.round(u.progress * 100)}%{:else if u.state === "failed"}Update failed, try again{:else}Tern {u.info.version} is ready · Restart to update{/if}
    </button>
  {/if}
  {#if app.lastDiscard && app.now * 1000 - app.lastDiscard.at < 180_000}
    <button class="undo" onclick={() => app.undoDiscard()} use:hover={() => ({ title: "Bring back the files you discarded", text: "Available for 3 minutes after a discard." })}>
      Undo discard of {app.lastDiscard.count} {app.lastDiscard.count === 1 ? "file" : "files"}
    </button>
  {/if}
  <button class="keys" onclick={() => (app.overlay = "help")} use:hover={() => ({ title: "Keyboard shortcuts", text: "Every action is also in the command palette.", hint: "Ctrl Shift P" })}>
    <kbd>F1</kbd> Shortcuts
  </button>
</footer>

<style>
  .bar {
    grid-column: 1 / -1;
    grid-row: 4;
    display: flex;
    align-items: center;
    gap: var(--s4);
    height: var(--h-status);
    padding: 0 var(--s3) 0 var(--s4);
    border-top: 1px solid var(--line);
    background: var(--panel);
    font-size: var(--fs-xs);
    white-space: nowrap;
    min-width: 0;
  }
  .repo {
    color: var(--faint);
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 38%;
  }
  .repo:hover {
    color: var(--text);
  }
  .sync {
    display: flex;
    gap: var(--s2);
    font-size: var(--fs-xs);
  }
  .behind {
    color: var(--blue);
  }
  .ahead {
    color: var(--add);
  }
  .msg {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--muted);
  }
  .toast {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: bottom;
    font-size: var(--fs-xs);
    color: inherit;
    animation: tern-rise-in 180ms var(--ease-out);
  }
  .msg.ok {
    color: var(--add);
  }
  .msg.err {
    color: var(--del);
  }
  .upd {
    color: var(--on-accent);
    background: var(--accent);
    font-size: var(--fs-xs);
    font-weight: 600;
    height: 18px;
    padding: 0 var(--s3);
    border-radius: var(--r-sm);
  }
  .upd:disabled {
    opacity: 0.8;
  }
  .undo {
    color: var(--accent);
    font-size: var(--fs-xs);
    height: 18px;
    padding: 0 var(--s3);
    border-radius: var(--r-sm);
    background: var(--accent-soft);
  }
  .undo:hover {
    background: var(--amber-sel);
  }
  .keys {
    display: flex;
    align-items: center;
    gap: var(--s1);
    color: var(--faint);
    font-size: var(--fs-xs);
  }
  .keys:hover {
    color: var(--text);
  }
</style>
