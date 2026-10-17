<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { copyText, openUrl } from "../lib/api";
  import { clearRun, clearShell, shellText } from "../lib/shells";
  import { hover } from "../lib/hover";
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
  const stateText: Record<string, string> = { running: "Running", stopped: "Stopped", ok: "Finished", failed: "Failed" };

  function elapsed(r: RunInfo) {
    app.now;
    return duration((r.ended ?? Date.now()) - r.started);
  }

  /** Closes a task's tab. A running task asks first, then stops and closes. */
  async function closeRunTab(r: RunInfo) {
    if (r.running) {
      const ok = await app.ask({
        title: `Stop ${r.name} and close it?`,
        body: "The task stops, and its output is removed from the panel.",
        code: r.command,
        confirm: "Stop and close",
        danger: true,
      });
      if (!ok) return;
      await app.stopRun(r.id);
      const until = Date.now() + 10000;
      while (app.runs.find((x) => x.id === r.id)?.running && Date.now() < until) await new Promise((res) => setTimeout(res, 100));
    }
    await app.closeRun(r.id);
  }

  function runMenu(e: MouseEvent, r: RunInfo) {
    app.showMenu(e, r.name, [
      { label: "Restart", run: () => app.restartRun(r.id) },
      ...(r.running ? [{ label: "Stop", danger: true, run: () => app.stopRun(r.id) }] : []),
      "sep",
      { label: "Copy command", run: () => app.copyValue(r.command) },
      { label: "Copy output for agent", run: () => app.copyRunOutput(r.id) },
      "sep",
      { label: "Show project", disabled: r.repo === app.current, run: () => app.select(r.repo) },
      "sep",
      { label: r.running ? "Stop and close…" : "Close", danger: r.running, run: () => closeRunTab(r) },
    ]);
  }

  function shellMenu(e: MouseEvent, s: { id: number; title: string; repo: string }) {
    app.showMenu(e, s.title, [
      { label: "Copy output for agent", run: () => copyShell(s.id, s.title) },
      { label: "Clear", run: () => clearShell(s.id) },
      "sep",
      { label: "Show project", disabled: s.repo === app.current, run: () => app.select(s.repo) },
      "sep",
      { label: "Close", run: () => app.closeShellTab(s.id) },
    ]);
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

{#snippet xIcon()}
  <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
{/snippet}

<section class="panel" style:height="{app.panelHeight}px" aria-label="Tasks output">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="grip" role="separator" aria-orientation="horizontal" aria-label="Resize the panel. Double-click to reset." onpointerdown={drag}
    ondblclick={() => app.setPanelHeight(260)}></div>
  <header class="toolbar sub phead">
    <div class="ptabs" role="tablist">
      {#each tabs as t (t.key)}
        {#if t.run}
          {@const r = t.run}
          {@const st = runState(r)}
          <div class="ptab" class:on={activeKey === t.key} role="tab" aria-selected={activeKey === t.key} tabindex="0"
            onclick={() => app.showRun(r.id)} onkeydown={(e) => e.key === "Enter" && app.showRun(r.id)} oncontextmenu={(e) => runMenu(e, r)}
            use:hover={() => ({ title: r.name, rows: [{ label: "Command", value: r.command, mono: true }, ...(r.cwd ? [{ label: "Folder", value: r.cwd, mono: true }] : []), { label: "Project", value: baseName(r.repo) }, { label: "State", value: stateText[runState(r)] }], hint: "Right-click for more" })}>
            <span class="dot {st}" aria-label={stateText[st]}></span>
            <span class="tname">{r.name}</span>
            <span class="trepo">{baseName(r.repo)}</span>
            <button class="icon-btn sm x" aria-label={r.running ? `Stop and close ${r.name}` : `Close ${r.name}`}
              onclick={(e) => { e.stopPropagation(); closeRunTab(r); }}>{@render xIcon()}</button>
          </div>
        {:else if t.shell}
          {@const s = t.shell}
          <div class="ptab" class:on={activeKey === t.key} role="tab" aria-selected={activeKey === t.key} tabindex="0"
            onclick={() => app.showShell(s.id)} onkeydown={(e) => e.key === "Enter" && app.showShell(s.id)} oncontextmenu={(e) => shellMenu(e, s)}
            use:hover={() => ({ title: s.title, rows: [{ label: "Project", value: baseName(s.repo) }, { label: "State", value: s.alive ? "Open" : "Exited" }], hint: "Right-click for more" })}>
            <span class="prompt" class:dead={!s.alive} aria-hidden="true">&gt;_</span>
            <span class="tname">{s.title}</span>
            <span class="trepo">{baseName(s.repo)}</span>
            <button class="icon-btn sm x" aria-label="Close {s.title}"
              onclick={(e) => { e.stopPropagation(); app.closeShellTab(s.id); }}>{@render xIcon()}</button>
          </div>
        {/if}
      {/each}
      <button class="icon-btn sm" onclick={() => app.openTerminal()} aria-label="New terminal"
        use:hover={() => ({ title: "New terminal", text: "Opens in this project.", hint: "Ctrl Shift `" })}>
        <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M8 3.5v9M3.5 8h9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
      </button>
    </div>
    <div class="tools">
      {#each urls.slice(0, 3) as u}
        <button class="chip url" onclick={() => openUrl(u)} use:hover={() => ({ title: "Open in browser", text: u })}>Open {u.replace(/^https?:\/\//, "").replace(/\/$/, "")}</button>
      {/each}
      {#if shell}
        {#if shell.repo !== app.current}
          <button class="btn ghost sm" onclick={() => app.select(shell.repo)} use:hover={() => ({ title: "Show project" })}>{baseName(shell.repo)}</button>
        {/if}
        <button class="btn ghost sm" onclick={() => clearShell(shell.id)}>Clear</button>
        <button class="btn ghost sm" onclick={() => copyShell(shell.id, shell.title)} use:hover={() => ({ title: "Copy output for agent", text: "The last 200 lines as plain text." })}>Copy output</button>
      {/if}
      {#if run}
        <span class="time mono">{elapsed(run)}</span>
        {#if run.repo !== app.current}
          <button class="btn ghost sm" onclick={() => app.select(run.repo)} use:hover={() => ({ title: "Show project" })}>{baseName(run.repo)}</button>
        {/if}
        <button class="btn ghost sm" onclick={() => clearRun(run.id)}>Clear</button>
        <button class="btn ghost sm" onclick={() => app.restartRun(run.id)}>Restart</button>
        <button class="btn ghost sm" onclick={() => app.copyRunOutput(run.id)} use:hover={() => ({ title: "Copy output for agent", text: "The last 200 lines as plain text." })}>Copy output</button>
        {#if run.running}<button class="btn ghost sm danger" onclick={() => app.stopRun(run.id)}>Stop</button>{/if}
      {/if}
      <span class="sep"></span>
      <button class="icon-btn sm" onclick={() => app.setPanel(false)} aria-label="Hide the panel" use:hover={() => ({ title: "Hide the panel", hint: "Ctrl J" })}>
        <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
    </div>
  </header>

  {#if shell}
    {#key shell.id}<ShellView id={shell.id} />{/key}
  {:else if run}
    <!-- A real terminal: type into it as you would in Windows Terminal. -->
    {#key run.id}<RunTermView id={run.id} />{/key}
  {:else}
    <div class="state">
      <span class="state-title">No tasks or terminals</span>
      <p>Run a task from the Tasks tab or with <span class="nw"><kbd>Ctrl Shift B</kbd>,</span> or open a terminal with <span class="nw"><kbd>Ctrl `</kbd>.</span></p>
      <div class="actions"><button class="btn sm" onclick={() => app.openTerminal()}>New terminal</button></div>
    </div>
  {/if}
</section>

<style>
  /* A key and the mark after it stay on one line. */
  .nw {
    white-space: nowrap;
  }
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
  /* The terminal stops a little above the panel edge, so its last line is never cut in half. */
  .panel > :global(.host) {
    padding-bottom: var(--s2);
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
  .phead {
    padding: 0 var(--s2) 0 var(--s1);
    gap: var(--s3);
  }
  .ptabs {
    display: flex;
    align-items: stretch;
    align-self: stretch;
    gap: var(--s1);
    overflow-x: auto;
    flex: 1;
    min-width: 0;
    scrollbar-width: none;
  }
  .ptabs > .icon-btn {
    align-self: center;
  }
  .ptab {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s2);
    padding: 0 var(--s1) 0 var(--s3);
    white-space: nowrap;
    font-size: var(--fs-sm);
    color: var(--muted);
    cursor: default;
    flex: 0 1 auto;
    min-width: 88px;
    max-width: 280px;
  }
  .ptab:hover {
    color: var(--text);
  }
  .ptab.on {
    color: var(--text);
  }
  /* Same marker as the shared tabs: an accent line under the chosen one. */
  .ptab.on::after {
    content: "";
    position: absolute;
    left: var(--s2);
    right: var(--s2);
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .tname,
  .trepo {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tname {
    flex: 0 1 auto;
  }
  .trepo {
    flex: 0 10 auto;
    color: var(--faint);
    font-size: var(--fs-xs);
  }
  .ptab > .x,
  .ptab > .dot,
  .ptab > .prompt {
    flex-shrink: 0;
  }
  .x {
    opacity: 0;
  }
  .ptab:hover .x,
  .ptab.on .x,
  .ptab:focus-within .x {
    opacity: 1;
  }
  .prompt {
    font: 700 var(--fs-xs) var(--mono);
    color: var(--add);
  }
  .prompt.dead {
    color: var(--faint);
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
    gap: var(--s1);
    flex-shrink: 0;
  }
  .time {
    font-size: var(--fs-xs);
    color: var(--faint);
    margin-right: var(--s1);
  }
  .url {
    color: var(--blue);
    cursor: pointer;
    font-family: var(--mono);
  }
  .url:hover {
    border-color: var(--blue);
  }
</style>
