<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { ago } from "../lib/format";
  import { hover } from "../lib/hover";
  import { bulkMenu, repoMenu } from "../lib/menus";
  import { repoCard } from "../lib/repoCard";
  import type { RepoEntry } from "../lib/types";
  import Icon from "./Icon.svelte";

  const all = $derived(app.ordered.map((r) => ({ r, s: app.sums[r.path] })));
  const withChanges = $derived(all.filter((x) => x.s?.changes.length).length);
  const behindN = $derived(all.filter((x) => x.s?.behind).length);
  const workingN = $derived(app.ordered.filter((r) => app.isWorking(r.path)).length);

  type Item = { r: RepoEntry; detail: string; tone?: "add" | "del" | "blue" | "accent"; action?: { label: string; run: () => void } };
  type Card = { id: string; title: string; hint: string; icon: string; items: Item[] };

  /** What needs you: one card per kind of work, built from data Tern already has. */
  const cards = $derived.by((): Card[] => {
    const working: Item[] = [];
    const commit: Item[] = [];
    const push: Item[] = [];
    const pull: Item[] = [];
    const ci: Item[] = [];
    const problems: Item[] = [];
    for (const { r, s } of all) {
      if (!s) continue;
      if (s.error) {
        problems.push({ r, detail: app.errorText(s.error), tone: "del" });
        continue;
      }
      const conflicts = s.changes.filter((c) => c.kind === "conflict").length;
      if (conflicts) problems.push({ r, detail: `${conflicts} ${conflicts === 1 ? "file has" : "files have"} a merge conflict`, tone: "del" });
      if (app.isWorking(r.path)) working.push({ r, detail: `${s.changes.length} changed · ${s.activity ? ago(s.activity) : "now"}`, tone: "add" });
      else if (s.changes.length) commit.push({ r, detail: `${s.changes.length} ${s.changes.length === 1 ? "file" : "files"} not committed`, tone: "accent" });
      if (s.ahead) push.push({ r, detail: `${s.ahead} to push${s.branch ? ` on ${s.branch}` : ""}`, tone: "add", action: { label: "Push…", run: () => app.action(r.path, "push") } });
      if (s.behind) pull.push({ r, detail: `${s.behind} to pull${s.branch ? ` into ${s.branch}` : ""}`, tone: "blue", action: { label: "Pull…", run: () => app.action(r.path, "pull") } });
      const runs = app.ci[r.path]?.items ?? [];
      const mine = runs.find((x) => !s.branch || x.branch === s.branch || x.branch.endsWith(`/${s.branch}`));
      if (mine && mine.status === "completed" && mine.result === "failed") ci.push({ r, detail: `${mine.name} failed · ${ago(Date.parse(mine.finished || mine.started) / 1000)}`, tone: "del" });
    }
    const out: Card[] = [
      { id: "problems", title: "Problems", hint: "Tern could not read these, or they have merge conflicts.", icon: "alert", items: problems },
      { id: "working", title: "Agents working", hint: "Files changed in the last 2 minutes.", icon: "play", items: working },
      { id: "ci", title: "CI failing", hint: "The newest run on the current branch failed. Open the project's CI tab for details.", icon: "x", items: ci },
      { id: "commit", title: "Ready to review", hint: "Changes that are not committed, and nobody is changing them now.", icon: "check", items: commit },
      { id: "push", title: "Ready to push", hint: "Commits on this computer that the remote does not have.", icon: "upload", items: push },
      { id: "pull", title: "Behind", hint: "The remote has commits you do not have yet, as of the last fetch.", icon: "download", items: pull },
    ];
    return out.filter((c) => c.items.length);
  });

  /** Tickets that are in progress, from the Tickets screen's data. */
  const activeTickets = $derived(app.tickets.items.filter((t) => /progress|active|doing|committed|review|testing/i.test(t.state)).slice(0, 6));

  // The projects table: sort by any column, filter by name or branch.
  type Col = "name" | "branch" | "changes" | "behind" | "ahead" | "activity" | "commit" | "fetched";
  const load = (k: string, d: string) => {
    try {
      return localStorage.getItem("tern." + k) || d;
    } catch {
      return d;
    }
  };
  let sortBy = $state<Col | "">(load("homeSort", "") as Col | "");
  let desc = $state(load("homeDesc", "1") === "1");
  let filter = $state("");
  function sort(c: Col) {
    if (sortBy === c) desc = !desc;
    else {
      sortBy = c;
      desc = c !== "name" && c !== "branch";
    }
    try {
      localStorage.setItem("tern.homeSort", sortBy);
      localStorage.setItem("tern.homeDesc", desc ? "1" : "0");
    } catch {}
  }
  const keyOf = (r: RepoEntry, c: Col): string | number => {
    const s = app.sums[r.path];
    switch (c) {
      case "name": return r.name.toLowerCase();
      case "branch": return (s?.branch ?? "").toLowerCase();
      case "changes": return s?.changes.length ?? 0;
      case "behind": return s?.behind ?? 0;
      case "ahead": return s?.ahead ?? 0;
      case "activity": return s?.activity ?? 0;
      case "commit": return s?.last?.time ?? 0;
      case "fetched": return s?.fetchedAt ?? 0;
    }
  };
  const sorted = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = q ? app.ordered.filter((r) => `${r.name} ${app.sums[r.path]?.branch ?? ""}`.toLowerCase().includes(q)) : null;
    if (!sortBy && !list) return null;
    const rows = [...(list ?? app.ordered)];
    if (sortBy) {
      const c = sortBy;
      rows.sort((a, b) => {
        const x = keyOf(a, c), y = keyOf(b, c);
        const d = x < y ? -1 : x > y ? 1 : 0;
        return desc ? -d : d;
      });
    }
    return rows;
  });
  const cols: { id: Col; label: string; num?: boolean }[] = [
    { id: "name", label: "Project" },
    { id: "branch", label: "Branch" },
    { id: "changes", label: "Changes", num: true },
    { id: "behind", label: "Pull", num: true },
    { id: "ahead", label: "Push", num: true },
    { id: "activity", label: "Activity" },
    { id: "commit", label: "Last commit" },
    { id: "fetched", label: "Fetched" },
  ];
</script>

{#snippet projectRow(r: RepoEntry)}
  {@const s = app.sums[r.path]}
  <button class="tr" class:hidden={app.isHidden(r)} class:picked={app.repoSel.includes(r.path)}
    onclick={(e) => app.clickRepo(e, r.path)}
    oncontextmenu={(e) => (app.repoSel.length > 1 && app.repoSel.includes(r.path) ? bulkMenu(e, app.repoSel) : repoMenu(e, r.path))}
    use:hover={() => repoCard(r.path)}>
    <span class="name" class:child={!!r.parent && !sorted}>{#if r.parent}<span class="faint">⑂ </span>{/if}{r.name}{#if app.nameHint[r.path]}<span class="faint hint"> {app.nameHint[r.path]}</span>{/if}</span>
    <span class="muted ell">{s?.branch ?? "—"}</span>
    <span class="num mono" class:accent={!!s?.changes.length}>{s?.changes.length || ""}</span>
    <span class="num mono blue">{s?.behind || ""}</span>
    <span class="num mono green">{s?.ahead || ""}</span>
    <span class="ell">
      {#if app.isWorking(r.path)}<span class="green">working</span>
      {:else if s?.activity}<span class:blue={app.isUnseen(r.path)} class:faint={!app.isUnseen(r.path)}>{ago(s.activity)}</span>{/if}
    </span>
    <span class="ell">
      {#if s?.error}<span class="red">{app.errorText(s.error)}</span>
      {:else if s?.last}{s.last.subject} <span class="faint">{ago(s.last.time)}</span>{/if}
    </span>
    <span class="faint ell">{s?.remote ? (app.busy[r.path] ? `${app.busy[r.path]}…` : ago(s.fetchedAt)) : "no remote"}</span>
  </button>
{/snippet}

<section class="home" aria-label="Home">
  <div class="inner">
    <header>
      <h1>{app.activeSpace?.name ?? "Home"}</h1>
      <p class="muted">
        {app.ordered.length} {app.ordered.length === 1 ? "project" : "projects"} · {withChanges} with changes · {behindN} behind
        {#if workingN} · <span class="green">{workingN} being changed now</span>{/if}
      </p>
    </header>

    {#if app.loadingRepos}
      <div class="cards">
        {#each [0, 1, 2] as i (i)}<div class="card skel"><span class="skeleton" style:width="40%"></span><span class="skeleton"></span><span class="skeleton" style:width="70%"></span></div>{/each}
      </div>
    {:else if !app.repos.length}
      <div class="state">
        <span class="state-title">Welcome to Tern</span>
        <p>Tern shows every git project under the folders you choose. Add a folder, or clone a project to start.</p>
        <div class="actions">
          <button class="btn primary" onclick={() => app.openSettings("folders")}>Add a folder…</button>
          <button class="btn" onclick={() => (app.overlay = "clone")}>Clone a project…</button>
        </div>
      </div>
    {:else}
      <h2 class="section-label">Needs you</h2>
      {#if cards.length || activeTickets.length}
        <div class="cards">
          {#each cards as c (c.id)}
            <div class="card">
              <div class="card-head" use:hover={() => ({ title: c.title, text: c.hint })}>
                <span class="card-ic {c.id}"><Icon name={c.icon} size={13} stroke={1.8} /></span>
                <b>{c.title}</b>
                <span class="badge">{c.items.length}</span>
              </div>
              {#each c.items.slice(0, 5) as it (it.r.path)}
                <div class="item">
                  <button class="item-main" onclick={() => app.select(it.r.path)} oncontextmenu={(e) => repoMenu(e, it.r.path)} use:hover={() => repoCard(it.r.path)}>
                    <span class="item-name">{it.r.name}</span>
                    <span class="item-detail {it.tone ?? ''}">{it.detail}</span>
                  </button>
                  {#if it.action}
                    {@const act = it.action}
                    <button class="btn ghost sm" disabled={!!app.busy[it.r.path]} onclick={act.run}>{app.busy[it.r.path] ? `${app.busy[it.r.path]}…` : act.label}</button>
                  {/if}
                </div>
              {/each}
              {#if c.items.length > 5}<p class="more">and {c.items.length - 5} more in the table below</p>{/if}
            </div>
          {/each}
          {#if activeTickets.length}
            <div class="card">
              <div class="card-head">
                <span class="card-ic tickets"><Icon name="ticket" size={13} stroke={1.8} /></span>
                <b>Tickets in progress</b>
                <span class="badge">{activeTickets.length}</span>
              </div>
              {#each activeTickets as t (t.key)}
                <div class="item">
                  <button class="item-main" onclick={() => { app.selectTicket(t.key); app.openTickets(); }}>
                    <span class="item-name">#{t.number} {t.title}</span>
                    <span class="item-detail">{t.state}{t.project ? ` · ${t.project}` : ""}</span>
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="clear">
          <Icon name="check" size={16} stroke={2} />
          <span>All clear. Nothing is waiting for review, push, or pull{app.ordered.some((r) => app.sums[r.path]?.remote) ? ", as of the last fetch" : ""}.</span>
        </div>
      {/if}

      <div class="table-head">
        <h2 class="section-label">Projects</h2>
        <span class="grow"></span>
        <input class="field" bind:value={filter} placeholder="Filter projects" spellcheck="false" aria-label="Filter projects"
          onkeydown={(e) => { if (e.key === "Escape" && filter) { e.stopPropagation(); filter = ""; } }} />
      </div>
      <div class="table" role="table" aria-label="Projects">
        <div class="tr th" role="row">
          {#each cols as c (c.id)}
            <button role="columnheader" class:num={c.num} class:on={sortBy === c.id} aria-sort={sortBy === c.id ? (desc ? "descending" : "ascending") : "none"} onclick={() => sort(c.id)}>
              {c.label}{#if sortBy === c.id}<span class="arrow">{desc ? "↓" : "↑"}</span>{/if}
            </button>
          {/each}
        </div>
        {#if sorted}
          {#each sorted as r (r.path)}{@render projectRow(r)}{/each}
          {#if !sorted.length}<p class="none">No projects match “{filter}”.</p>{/if}
        {:else}
          {#each app.groups as g (g.space?.id ?? "none")}
            {#if !app.activeSpace}
              <div class="group" style:--c={g.space?.color ?? "var(--faint)"}><span class="sw"></span>{g.space?.name ?? "No space"}</div>
            {/if}
            {#each g.repos as r (r.path)}{@render projectRow(r)}{/each}
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</section>

<style>
  .home {
    container-type: inline-size;
    overflow-y: auto;
    min-width: 0;
    min-height: 0;
  }
  .inner {
    max-width: 1180px;
    margin: 0 auto;
    padding: var(--s6) 32px 48px;
  }
  header {
    margin-bottom: var(--s5);
  }
  h1 {
    font-size: var(--fs-xl);
    font-weight: 650;
    margin: 0 0 var(--s1);
    letter-spacing: -0.01em;
  }
  header p {
    margin: 0;
    font-size: var(--fs-md);
  }
  h2.section-label {
    margin: 0;
    padding: 0;
  }
  /* Columns that fill top to bottom, so short and tall cards pack without holes. */
  .cards {
    columns: 300px 3;
    column-gap: var(--s4);
    margin: var(--s2) 0 var(--s6);
  }
  .card {
    break-inside: avoid;
    margin-bottom: var(--s4);
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--s3);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    min-width: 0;
    animation: tern-rise-in 200ms var(--ease-out);
  }
  .card.skel {
    gap: var(--s3);
    padding: var(--s4);
  }
  .card.skel .skeleton {
    height: 10px;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s1) var(--s1) var(--s2);
    font-size: var(--fs-md);
  }
  .card-head b {
    flex: 1;
    font-weight: 600;
  }
  .card-ic {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    color: var(--muted);
    background: var(--raised);
  }
  .card-ic.problems,
  .card-ic.ci {
    color: var(--del);
    background: var(--del-bg);
  }
  .card-ic.working,
  .card-ic.push {
    color: var(--add);
    background: var(--add-bg);
  }
  .card-ic.commit,
  .card-ic.tickets {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .card-ic.pull {
    color: var(--blue);
    background: color-mix(in srgb, var(--blue) 14%, transparent);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s2);
  }
  .item-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    padding: var(--s1) var(--s2);
    border-radius: var(--radius);
    text-align: left;
  }
  .item-main:hover {
    background: var(--hover);
  }
  .item-name {
    font-size: var(--fs-md);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item-detail {
    font-size: var(--fs-xs);
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item-detail.add {
    color: var(--add);
  }
  .item-detail.del {
    color: var(--del);
  }
  .item-detail.blue {
    color: var(--blue);
  }
  .item-detail.accent {
    color: var(--accent);
  }
  /* The card's action: quiet, but clearly a button. */
  .item .btn {
    color: var(--text);
    background: var(--hover);
  }
  .item .btn:hover:not(:disabled) {
    background: var(--raised);
  }
  .more {
    margin: var(--s1) var(--s2) 0;
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .clear {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin: var(--s2) 0 var(--s6);
    padding: var(--s4) var(--s4);
    border: 1px dashed var(--line-strong);
    border-radius: var(--r-lg);
    color: var(--muted);
    font-size: var(--fs-md);
  }
  .clear :global(svg) {
    color: var(--add);
  }
  .table-head {
    display: flex;
    align-items: center;
    gap: var(--s3);
    margin-bottom: var(--s2);
  }
  .table-head .field {
    width: 220px;
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(90px, 1.2fr) minmax(70px, 1fr) 64px 48px 48px 80px minmax(0, 2.4fr) 80px;
    gap: var(--s4);
    align-items: center;
    padding: 0 var(--s3);
    min-height: 34px;
    width: 100%;
    text-align: left;
    border-radius: var(--radius);
    font-size: var(--fs-md);
  }
  .tr.picked {
    background: var(--accent-soft);
  }
  .tr.hidden {
    opacity: 0.5;
  }
  .tr > :global(*) {
    min-width: 0;
  }
  button.tr:hover {
    background: var(--hover);
  }
  .th {
    min-height: 30px;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
    margin-bottom: var(--s1);
  }
  .th button {
    display: flex;
    align-items: center;
    gap: 3px;
    height: 100%;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--faint);
    text-align: left;
  }
  .th button.num {
    justify-content: flex-end;
  }
  .th button:hover,
  .th button.on {
    color: var(--text);
  }
  .arrow {
    color: var(--accent);
  }
  @container (max-width: 760px) {
    .tr {
      grid-template-columns: minmax(90px, 1.2fr) minmax(70px, 1fr) 64px 48px 48px minmax(0, 2.4fr);
    }
    .tr > :nth-child(6),
    .tr > :nth-child(8) {
      display: none;
    }
  }
  @container (max-width: 600px) {
    .inner {
      padding: var(--s5);
    }
    .tr {
      grid-template-columns: minmax(90px, 1.4fr) minmax(60px, 1fr) 64px minmax(0, 2fr);
    }
    .tr > :nth-child(4),
    .tr > :nth-child(5) {
      display: none;
    }
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--s3);
    padding: var(--s4) var(--s3) var(--s1);
    font-size: var(--fs-xs);
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
    margin-left: var(--s3);
  }
  .name.child {
    font-weight: 400;
    padding-left: var(--s4);
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    text-align: right;
  }
  .none {
    margin: 0;
    padding: var(--s4) var(--s3);
    color: var(--muted);
  }
  .accent {
    color: var(--accent);
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
