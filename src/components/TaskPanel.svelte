<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { api, copyText, openUrl } from "../lib/api";
  import { clearRun, clearShell, shellText } from "../lib/shells";
  import ShellView from "./ShellView.svelte";
  import RunTermView from "./RunTermView.svelte";
  import { baseName, duration } from "../lib/format";
  import type { RunInfo } from "../lib/types";


  const shell = $derived(app.activeTab?.startsWith("shell:") ? app.shells.find((s) => "shell:" + s.id === app.activeTab) : undefined);
  const run = $derived(
    shell ? undefined : (app.runs.find((r) => "run:" + r.id === app.activeTab) ?? app.runs.find((r) => r.id === app.activeRun) ?? app.runs[app.runs.length - 1]),
  );

  // Task runs and terminals share one tab strip, oldest first.
  const tabs = $derived(
    [
      ...app.runs.map((r) => ({ key: "run:" + r.id, at: r.started, run: r, shell: undefined })),
      ...app.shells.map((s) => ({ key: "shell:" + s.id, at: s.created, run: undefined, shell: s })),
    ].sort((a, b) => a.at - b.at),
  );
  const activeKey = $derived(shell ? "shell:" + shell.id : run ? "run:" + run.id : null);

  async function copyShell(id: number, name: string) {
    await copyText(`Terminal output from ${name}, last 200 lines:\n\n${shellText(id)}`);
    app.notify("Copied the terminal output", "ok");
  }
  const urls = $derived.by(() => {
    app.termTick;
    return run ? [...app.term(run.id).urls] : [];
  });

  function runState(r: RunInfo) {
    if (r.running) return "running";
    if (r.stopped) return "stopped";
    return r.code === 0 ? "ok" : "failed";
  }

  function elapsed(r: RunInfo) {
    app.now;
    return duration((r.ended ?? Date.now()) - r.started);
  }

  function drag(e: PointerEvent) {
    const startY = e.clientY;
    const start = app.panelHeight;
    const move = (ev: PointerEvent) => {
      const h = Math.min(window.innerHeight - 160, Math.max(120, start - (ev.clientY - startY)));
      app.panelHeight = h;
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      app.setPanelHeight(app.panelHeight);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

<section class="panel" style:height="{app.panelHeight}px" aria-label="Tasks output">
  <div class="grip" role="separator" aria-orientation="horizontal" onpointerdown={drag}></div>
  <header>
    <div class="tabs" role="tablist">
      {#each tabs as t (t.key)}
        {#if t.run}
          {@const r = t.run}
          {@const st = runState(r)}
          <div class="tab" class:on={activeKey === t.key} role="tab" aria-selected={activeKey === t.key} tabindex="0"
            onclick={() => app.showRun(r.id)} onkeydown={(e) => e.key === "Enter" && app.showRun(r.id)}
            title="{r.command}{r.cwd ? `  (in ${r.cwd})` : ''}  in {r.repo}">
            <span class="dot {st}" aria-label={st}></span>
            <span class="tname">{r.name}</span>
            <span class="trepo">{baseName(r.repo)}</span>
            <button class="x" aria-label={r.running ? "Stop" : "Close"} title={r.running ? "Stop" : "Close"}
              onclick={(e) => { e.stopPropagation(); app.closeRun(r.id); }}>{r.running ? "■" : "×"}</button>
          </div>
        {:else if t.shell}
          {@const s = t.shell}
          <div class="tab" class:on={activeKey === t.key} role="tab" aria-selected={activeKey === t.key} tabindex="0"
            onclick={() => app.showShell(s.id)} onkeydown={(e) => e.key === "Enter" && app.showShell(s.id)}
            title="{s.title} in {s.repo}">
            <span class="prompt" class:dead={!s.alive} aria-hidden="true">&gt;_</span>
            <span class="tname">{s.title}</span>
            <span class="trepo">{baseName(s.repo)}</span>
            <button class="x" aria-label="Close terminal" title="Close terminal"
              onclick={(e) => { e.stopPropagation(); app.closeShellTab(s.id); }}>×</button>
          </div>
        {/if}
      {/each}
      <button class="add" onclick={() => app.openTerminal()} title="New terminal in this project (Ctrl+Shift+`)" aria-label="New terminal">+</button>
    </div>
    <div class="tools">
      {#each urls.slice(0, 3) as u}
        <button class="url mono" onclick={() => openUrl(u)} title="Open {u} in the browser">Open {u.replace(/^https?:\/\//, "").replace(/\/$/, "")}</button>
      {/each}
      {#if shell}
        {#if shell.repo !== app.current}
          <button class="btn" onclick={() => app.select(shell.repo)} title="Open this project">{baseName(shell.repo)}</button>
        {/if}
        <button class="btn" onclick={() => clearShell(shell.id)}>Clear</button>
        <button class="btn" onclick={() => copyShell(shell.id, shell.title)} title="Copy the last 200 lines as plain text">Copy output</button>
      {/if}
      {#if run}
        <span class="time faint mono">{elapsed(run)}</span>
        {#if run.repo !== app.current}
          <button class="btn" onclick={() => app.select(run.repo)} title="Open this project">{baseName(run.repo)}</button>
        {/if}
        <button class="btn" onclick={() => clearRun(run.id)}>Clear</button>
        <button class="btn" onclick={() => app.restartRun(run.id)}>Restart</button>
        {#if run.running}<button class="btn danger" onclick={() => app.stopRun(run.id)}>Stop</button>{/if}
        <button class="btn" onclick={() => app.copyRunOutput(run.id)} title="Copy the last 200 lines as plain text">Copy output</button>
      {/if}
      <button class="btn" onclick={() => app.setPanel(false)} title="Hide the panel (Ctrl+J)" aria-label="Hide the panel">▾</button>
    </div>
  </header>

  {#if shell}
    {#key shell.id}<ShellView id={shell.id} />{/key}
  {:else if run}
    <!-- A real terminal: type into it as you would in Windows Terminal. -->
    {#key run.id}<RunTermView id={run.id} />{/key}
  {:else}
    <p class="empty">
      Nothing here yet. Run a task from the Tasks tab (<kbd>Ctrl 5</kbd> or <kbd>Ctrl Shift B</kbd>),
      or open a terminal with <kbd>Ctrl `</kbd>.
    </p>
  {/if}
</section>

<style>
  .panel {
    grid-column: 3 / -1;
    grid-row: 3;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    border-top: 1px solid var(--line);
    background: var(--panel);
    position: relative;
  }
  .grip {
    position: absolute;
    top: -3px;
    left: 0;
    right: 0;
    height: 6px;
    cursor: row-resize;
    z-index: 1;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 4px;
    height: 34px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .tabs {
    display: flex;
    gap: 2px;
    overflow-x: auto;
    flex: 1;
    min-width: 0;
    scrollbar-width: none;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 4px 3px 9px;
    border-radius: var(--radius);
    white-space: nowrap;
    color: var(--muted);
    cursor: default;
    flex-shrink: 0;
  }
  .tab:hover {
    background: var(--hover);
  }
  .tab.on {
    background: var(--raised);
    color: var(--text);
  }
  .tname {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .trepo {
    color: var(--faint);
    font-size: 11.5px;
  }
  .x {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    color: var(--faint);
    font-size: 11px;
  }
  .x:hover {
    background: var(--line);
    color: var(--text);
  }
  .prompt {
    font: 700 10px var(--mono);
    color: var(--add);
  }
  .prompt.dead {
    color: var(--faint);
  }
  .add {
    width: 24px;
    height: 24px;
    border-radius: var(--radius);
    color: var(--muted);
    flex-shrink: 0;
    font-size: 15px;
  }
  .add:hover {
    background: var(--hover);
    color: var(--text);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--faint);
  }
  .dot.running {
    background: var(--add);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .dot.ok {
    background: var(--add);
  }
  .dot.failed {
    background: var(--del);
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .time {
    font-size: 11px;
    margin-right: 4px;
  }
  .url {
    font-size: 11.5px;
    color: var(--blue);
    padding: 2px 8px;
    border: 1px solid var(--line);
    border-radius: 12px;
  }
  .url:hover {
    border-color: var(--blue);
  }
  .btn {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: var(--radius);
    color: var(--muted);
    border: 1px solid transparent;
  }
  .btn:hover {
    color: var(--text);
    border-color: var(--line);
  }
  .btn.danger:hover {
    color: var(--del);
  }
  .empty {
    margin: 0;
    padding: 16px;
    color: var(--muted);
  }
</style>
