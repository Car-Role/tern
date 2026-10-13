<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { api, openUrl } from "../lib/api";
  import { agoIso, duration } from "../lib/format";
  import { popOnDone } from "../lib/motion";
  import type { CiRun, CiStep } from "../lib/types";

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
</script>

<div class="runview">
  <header>
    <h1><span class="icon r-{live.result} s-{live.status}" use:popOnDone={live.status}>{icon(live.status, live.result)}</span> {live.name} <span class="num mono">#{live.number}</span></h1>
    <p class="meta">
      <span class="state r-{live.result} s-{live.status}">{word(live.status, live.result)}</span>
      <span class="mono">{live.branch}</span>
      {#if live.sha}<button class="sha mono" onclick={() => app.copyValue(live.sha.slice(0, 7), `commit ID ${live.sha.slice(0, 7)}`)}>{live.sha.slice(0, 7)}</button>{/if}
      <span>{live.by}</span>
      <span class="faint">started {agoIso(live.started)}{#if took(live.started, live.finished)}, took {took(live.started, live.finished)}{/if}</span>
    </p>
    {#if live.title}<p class="title">{live.title}</p>{/if}
    <div class="actions">
      <button class="btn" onclick={copyForAgent} disabled={!problems.length}>Copy problems for an agent</button>
      <button class="btn" onclick={() => app.copyValue(live.url)}>Copy link</button>
      <button class="btn" onclick={() => openUrl(live.url)}>Open in {live.provider === "azdo" ? "Azure DevOps" : "GitHub"}</button>
    </div>
  </header>

  {#if loading && !steps.length}
    <p class="note">Loading stages and jobs…</p>
  {:else if error}
    <p class="note err">{error}</p>
  {:else}
    <div class="steps">
      <label class="all"><input type="checkbox" bind:checked={showAll} /> Show every step (otherwise only steps with problems)</label>
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
          <button class="more" style:margin-left="{100 + st.depth * 18}px" onclick={() => (open[key] = true)}>Show {sorted.length - cut.length} more warnings</button>
        {/if}
      {/each}
      {#if !steps.length}<p class="note">This run has no stages or jobs yet.</p>{/if}
    </div>
  {/if}
</div>

<style>
  .runview {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 20px 26px 40px;
    user-select: text;
  }
  header {
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line);
    max-width: 900px;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 17px;
    font-weight: 650;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .num {
    color: var(--muted);
    font-weight: 400;
    font-size: 14px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    align-items: center;
    margin: 0;
    color: var(--muted);
    font-size: 12.5px;
  }
  .title {
    margin: 8px 0 0;
  }
  .sha {
    color: var(--amber);
    font-size: 12px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .btn {
    font-size: 12px;
    padding: 3px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
  }
  .btn:disabled {
    opacity: 0.45;
  }
  .steps {
    max-width: 900px;
    padding-top: 10px;
  }
  .all {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
    margin: 0 0 8px;
  }
  .all input {
    accent-color: var(--amber);
    margin: 0;
  }
  .step {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 12px;
    border-radius: 5px;
  }
  .step.k-stage {
    font-weight: 600;
    margin-top: 6px;
  }
  .step:hover {
    background: var(--hover);
  }
  .sname {
    flex: 1;
  }
  .issue {
    display: flex;
    gap: 8px;
    padding: 2px 12px 4px;
    font-size: 12px;
  }
  .more {
    font-size: 12px;
    color: var(--blue);
    padding: 2px 0 6px;
  }
  .more:hover {
    text-decoration: underline;
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
  .note {
    color: var(--muted);
    margin: 16px 0;
  }
  .note.err {
    color: var(--del);
    font-family: var(--mono);
    font-size: 12px;
    white-space: pre-wrap;
  }
</style>
