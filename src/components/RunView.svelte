<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { api, openUrl } from "../lib/api";
  import { agoIso, duration } from "../lib/format";
  import { popOnDone } from "../lib/motion";
  import type { CiRun, CiStep } from "../lib/types";
  import { hover } from "../lib/hover";
  import Icon from "./Icon.svelte";

  let { repo, run }: { repo: string; run: CiRun } = $props();

  let steps = $state<CiStep[]>([]);
  let error = $state("");
  let loading = $state(true);
  let showAll = $state(false);
  let open = $state<Record<string, boolean>>({});
  // The newest copy of this run from the list, so status and result stay current.
  const live = $derived(app.ci[repo]?.items.find((r) => r.id === run.id) ?? run);

  async function load() {
    try {
      steps = await api.ciDetail(repo, run.id);
      error = "";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Load now, then every 10 seconds while the run is going.
  $effect(() => {
    load();
    const t = setInterval(() => {
      if (live.status !== "completed") {
        load();
        app.loadCi(repo, true);
      }
    }, 10_000);
    return () => clearInterval(t);
  });

  const took = (a: string, b: string) => (a && b ? duration(Date.parse(b) - Date.parse(a)) : a ? duration(Date.now() - Date.parse(a)) : "");
  const problems = $derived(steps.filter((s) => s.issues.length || s.result === "failed"));
  const shown = $derived(showAll ? steps : steps.filter((s) => s.kind !== "step" || s.result === "failed" || s.issues.length || s.status === "running"));

  const icon = (status: string, result: string) =>
    status === "running" ? "●" : status === "queued" ? "○" : { succeeded: "✓", partial: "◐", failed: "✗", canceled: "■", skipped: "–", "": "·" }[result] ?? "·";
  const word = (status: string, result: string) =>
    status === "running" ? "Running" : status === "queued" ? "Waiting" : { succeeded: "Succeeded", partial: "Finished with warnings", failed: "Failed", canceled: "Canceled", skipped: "Skipped", "": "Finished" }[result] ?? result;

  function retry() {
    loading = true;
    error = "";
    load();
  }

  function copyForAgent() {
    const r = live;
    const lines = [
      `CI run ${r.name} #${r.number} on ${r.branch} (${word(r.status, r.result)}). ${r.url}`,
      r.sha ? `Commit: ${r.sha}` : "",
      "",
      ...problems.flatMap((s) => [
        `${s.kind} "${s.name}": ${word(s.status, s.result)}`,
        ...s.issues.map((i) => `  ${i.kind}: ${i.message}`),
      ]),
    ].filter((l, i) => l || i < 3);
    app.copyValue(lines.join("\n"), "the CI problems for an agent");
  }

  /** The narrow-window menu with the toolbar's actions. */
  function moreMenu(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    app.showMenu({ x: r.left, y: r.bottom + 4 }, `Run #${live.number}`, [
      { label: "Copy for agent", disabled: !problems.length, run: copyForAgent },
      { label: "Copy link", run: () => app.copyValue(live.url) },
      { label: "Open in browser", run: () => openUrl(live.url) },
    ]);
  }
</script>

<div class="runview">
  <header class="toolbar">
    <span class="icon r-{live.result} s-{live.status}" use:popOnDone={live.status}>{icon(live.status, live.result)}</span>
    <span class="title">{live.name} <span class="num mono">#{live.number}</span></span>
    <span class="chip st r-{live.result} s-{live.status}">{word(live.status, live.result)}</span>
    <span class="meta">{live.branch}</span>
    <span class="grow"></span>
    <button class="btn sm ghost lo" onclick={copyForAgent} disabled={!problems.length}
      use:hover={() => ({ title: problems.length ? "Copy the problems in this run for an agent" : "No problems to copy" })}>Copy for agent</button>
    <button class="btn sm ghost lo" onclick={() => app.copyValue(live.url)}>Copy link</button>
    <button class="btn sm ghost lo" onclick={() => openUrl(live.url)}
      use:hover={() => ({ title: `Open in ${live.provider === "azdo" ? "Azure DevOps" : "GitHub"}` })}>Open in browser</button>
    <button class="icon-btn sm more" onclick={moreMenu} aria-label="More actions" aria-haspopup="menu"
      use:hover={() => ({ title: "More actions", text: "Copy for agent, Copy link, Open in browser" })}><Icon name="more" /></button>
  </header>
  {#if steps.length}
    <div class="toolbar sub">
      <span class="meta">{problems.length ? `${problems.length} ${problems.length === 1 ? "step has" : "steps have"} problems` : "No problems"}</span>
      <span class="grow"></span>
      <label class="check"><input type="checkbox" bind:checked={showAll} /> Show every step</label>
    </div>
  {/if}

  <div class="scroll">
    <div class="head">
      <p class="info">
        {#if live.sha}<button class="sha mono" onclick={() => app.copyValue(live.sha.slice(0, 7), `commit ID ${live.sha.slice(0, 7)}`)}
          use:hover={() => ({ title: "Copy commit ID", rows: [{ label: "Commit", value: live.sha, mono: true }] })}>{live.sha.slice(0, 7)}</button>{/if}
        <span>{live.by}</span>
        <span class="faint">started {agoIso(live.started)}{#if took(live.started, live.finished)}, took {took(live.started, live.finished)}{/if}</span>
      </p>
      {#if live.title}<p class="msg">{live.title}</p>{/if}
    </div>

    {#if loading && !steps.length}
      <div class="state delay"><p>Loading stages and jobs…</p></div>
    {:else if error}
      <div class="state">
        <span class="state-title">Could not load this run</span>
        <pre>{error}</pre>
        <div class="actions"><button class="btn" onclick={retry}>Try again</button></div>
      </div>
    {:else if !steps.length}
      <div class="state"><p>This run has no stages or jobs yet.</p></div>
    {:else}
      <div class="steps">
        {#each shown as st}
          <div class="step k-{st.kind}" style:padding-left="{12 + st.depth * 18}px">
            <span class="icon r-{st.result} s-{st.status}">{icon(st.status, st.result)}</span>
            <span class="sname">{st.name}</span>
            <span class="faint mono">{took(st.started, st.finished)}</span>
          </div>
          {@const key = st.depth + st.name}
          {@const sorted = [...st.issues].sort((a, b) => Number(b.kind === "error") - Number(a.kind === "error"))}
          {@const cut = open[key] ? sorted : sorted.filter((is, i) => is.kind === "error" || i < 3)}
          {#each cut as is}
            <div class="issue {is.kind}" style:padding-left="{36 + st.depth * 18}px">
              <span class="ikind">{is.kind}</span><span class="imsg mono">{is.message}</span>
            </div>
          {/each}
          {#if cut.length < sorted.length}
            <button class="link more" style:margin-left="{100 + st.depth * 18}px" onclick={() => (open[key] = true)}>Show {sorted.length - cut.length} more warnings</button>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .runview {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: var(--s4) var(--s6) 40px;
    user-select: text;
  }
  .scroll > * {
    flex-shrink: 0;
  }
  .toolbar:not(.sub) {
    container: runbar / inline-size;
  }
  .toolbar > :global(*) {
    flex-shrink: 0;
  }
  .toolbar > .title {
    flex-shrink: 1;
  }
  /* The branch gives way before the run name. */
  .toolbar > .meta {
    flex-shrink: 1000;
  }
  .more {
    display: none;
  }
  @container runbar (max-width: 640px) {
    .lo {
      display: none;
    }
    .more {
      display: inline-flex;
    }
  }
  .toolbar > .icon {
    font-size: var(--fs-md);
  }
  .num {
    color: var(--muted);
    font-weight: 400;
  }
  .chip.st {
    color: var(--muted);
  }
  .chip.st.r-succeeded {
    color: var(--add);
  }
  .chip.st.r-partial {
    color: var(--amber);
  }
  .chip.st.r-failed {
    color: var(--del);
  }
  .chip.st.s-running {
    color: var(--blue);
  }
  .state.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
  .head {
    max-width: 900px;
    padding-bottom: var(--s4);
    border-bottom: 1px solid var(--line);
  }
  .info {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s2) var(--s4);
    align-items: center;
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .msg {
    margin: var(--s3) 0 0;
  }
  .sha {
    color: var(--muted);
  }
  .sha:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .steps {
    max-width: 900px;
    padding-top: var(--s3);
  }
  .step {
    display: flex;
    gap: var(--s3);
    align-items: center;
    min-height: var(--h-row);
    padding: 0 var(--s4);
    border-radius: var(--r-sm);
  }
  .step.k-stage {
    font-weight: 600;
    margin-top: var(--s2);
  }
  .step:hover {
    background: var(--hover);
  }
  .sname {
    flex: 1;
  }
  .issue {
    display: flex;
    gap: var(--s3);
    padding: 2px var(--s4) var(--s1);
    font-size: var(--fs-sm);
  }
  .more {
    font-size: var(--fs-sm);
    padding: 2px 0 var(--s2);
  }
  .ikind {
    flex-shrink: 0;
    width: 56px;
    color: var(--amber);
  }
  .issue.error .ikind {
    color: var(--del);
  }
  .imsg {
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text);
  }
  .icon {
    width: 14px;
    text-align: center;
    color: var(--faint);
    flex-shrink: 0;
  }
  .r-succeeded {
    color: var(--add);
  }
  .r-partial {
    color: var(--amber);
  }
  .r-failed {
    color: var(--del);
  }
  .s-running {
    color: var(--blue);
  }
  .icon.s-running {
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
</style>
