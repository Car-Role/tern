<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { ago } from "../lib/format";
  import { hover } from "../lib/hover";
  import { bulkMenu, repoMenu } from "../lib/menus";
  import { repoCard } from "../lib/repoCard";

  const all = $derived(app.ordered.map((r) => app.sums[r.path]));
  const withChanges = $derived(all.filter((s) => s?.changes.length).length);
  const behind = $derived(all.filter((s) => s?.behind).length);
  const working = $derived(app.ordered.filter((r) => app.isWorking(r.path)).length);
</script>

<section class="overview" aria-label="All projects">
  <header>
    <div>
      <h1>{app.activeSpace?.name ?? "All projects"}</h1>
      <p class="muted">
        {app.ordered.length} projects. {withChanges} with local changes. {behind} behind their remote.
        {#if working}<span class="green">{working} being changed right now.</span>{/if}
      </p>
    </div>
    <div class="actions">
      <button class="btn" onclick={() => (app.overlay = "search")}>Search…</button>
      <button class="btn" onclick={() => (app.overlay = "clone")}>Clone a repo…</button>
      <button class="btn" onclick={() => app.fetchAll()}>Fetch all</button>
      <button class="btn" onclick={() => (app.overlay = "settings")}>Spaces and folders…</button>
    </div>
  </header>

  <div class="table" role="table">
    <div class="tr th" role="row">
      <span role="columnheader">Project</span>
      <span role="columnheader">Branch</span>
      <span role="columnheader" class="num">Changes</span>
      <span role="columnheader" class="num">Pull</span>
      <span role="columnheader" class="num">Push</span>
      <span role="columnheader">Activity</span>
      <span role="columnheader">Last commit</span>
      <span role="columnheader">Fetched</span>
    </div>
    {#each app.groups as g (g.space?.id ?? "none")}
      {#if !app.activeSpace}
        <div class="group" style:--c={g.space?.color ?? "var(--faint)"}><span class="sw"></span>{g.space?.name ?? "No space"}</div>
      {/if}
      {#each g.repos as r (r.path)}
        {@const s = app.sums[r.path]}
        <button class="tr" class:hidden={app.isHidden(r)} class:picked={app.repoSel.includes(r.path)} role="row"
          onclick={(e) => app.clickRepo(e, r.path)}
          oncontextmenu={(e) => (app.repoSel.length > 1 && app.repoSel.includes(r.path) ? bulkMenu(e, app.repoSel) : repoMenu(e, r.path))}
          use:hover={() => repoCard(r.path)}>
          <span class="name" class:child={!!r.parent}>{#if r.parent}<span class="faint">⑂ </span>{/if}{r.name}{#if app.nameHint[r.path]}<span class="faint hint"> {app.nameHint[r.path]}</span>{/if}</span>
          <span class="mono muted ell">{s?.branch ?? "—"}</span>
          <span class="num mono" class:amber={!!s?.changes.length}>{s?.changes.length || ""}</span>
          <span class="num mono blue">{s?.behind || ""}</span>
          <span class="num mono green">{s?.ahead || ""}</span>
          <span class="ell">
            {#if app.isWorking(r.path)}<span class="green">working</span>
            {:else if s?.activity}<span class:blue={app.isUnseen(r.path)} class:faint={!app.isUnseen(r.path)}>{ago(s.activity)}</span>{/if}
          </span>
          <span class="ell">
            {#if s?.error}<span class="red" title={s.error}>{app.errorText(s.error)}</span>
            {:else if s?.last}{s.last.subject} <span class="faint">{ago(s.last.time)}</span>{/if}
          </span>
          <span class="faint">{s?.remote ? (app.busy[r.path] ? `${app.busy[r.path]}…` : ago(s.fetchedAt)) : "no remote"}</span>
        </button>
      {/each}
    {/each}
  </div>
</section>

<style>
  .overview {
    container-type: inline-size;
    overflow-y: auto;
    padding: 28px 32px;
    min-width: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    margin-bottom: 20px;
    max-width: 1240px;
  }
  h1 {
    font-size: 22px;
    font-weight: 650;
    margin: 0 0 4px;
    letter-spacing: -0.01em;
  }
  p {
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .btn {
    padding: 4px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .table {
    max-width: 1240px;
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(90px, 1.2fr) minmax(70px, 1fr) 56px 40px 40px 72px minmax(0, 2.4fr) 76px;
    gap: 14px;
    align-items: center;
    padding: 7px 10px;
    width: 100%;
    text-align: left;
    border-radius: var(--radius);
  }
  .tr.picked {
    background: var(--amber-soft);
  }
  .tr.hidden {
    opacity: 0.5;
  }
  .tr > :global(*) {
    min-width: 0;
  }
  /* Narrow: drop Activity and Fetched. Narrower: also Pull and Push. The hover card still has them. */
  @container (max-width: 760px) {
    .tr {
      grid-template-columns: minmax(90px, 1.2fr) minmax(70px, 1fr) 56px 40px 40px minmax(0, 2.4fr);
    }
    .tr > :nth-child(6),
    .tr > :nth-child(8) {
      display: none;
    }
  }
  @container (max-width: 600px) {
    .tr {
      grid-template-columns: minmax(90px, 1.4fr) minmax(60px, 1fr) 56px minmax(0, 2fr);
    }
    .tr > :nth-child(4),
    .tr > :nth-child(5) {
      display: none;
    }
  }
  button.tr:hover {
    background: var(--hover);
  }
  .th {
    color: var(--faint);
    font-size: 12px;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
    margin-bottom: 4px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 10px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .sw {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--c);
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    font-weight: 400;
    margin-left: 8px;
  }
  .name.child {
    font-weight: 400;
    padding-left: 12px;
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    text-align: right;
  }
  .amber {
    color: var(--amber);
  }
  .blue {
    color: var(--blue);
  }
  .green {
    color: var(--add);
  }
  .red {
    color: var(--del);
  }
</style>
