<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { hover } from "../lib/hover";
  import { bulkMenu, repoMenu } from "../lib/menus";
  import { repoCard } from "../lib/repoCard";
  import type { RepoEntry, Summary } from "../lib/types";
  import Icon from "./Icon.svelte";

  function stateOf(s: Summary | undefined) {
    if (!s) return "unknown";
    if (s.error) return "error";
    if (s.changes.some((c) => c.kind === "conflict")) return "conflict";
    if (s.changes.length) return "dirty";
    return "clean";
  }

  const dotText: Record<string, string> = {
    clean: "No local changes",
    dirty: "Has changes that are not committed",
    conflict: "Merge conflict",
    error: "Tern could not read this project",
    unknown: "Not read yet",
  };

  const dirtyCount = $derived(app.visible.filter((r) => app.sums[r.path]?.changes.length).length);
  const workingCount = $derived(app.visible.filter((r) => app.isWorking(r.path)).length);
  const home = $derived(!app.current && !app.ticketsOpen && !app.dataOpen);

  // The project filter: a flat list of matches, ignoring folds and groups.
  let filter = $state("");
  let filterEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();
  const matches = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return null;
    return app.ordered.filter((r) => `${r.name} ${app.sums[r.path]?.branch ?? ""} ${app.nameHint[r.path] ?? ""}`.toLowerCase().includes(q));
  });

  // Space groups can be folded (All spaces view). Remembered on this computer.
  const loadFolded = () => {
    try {
      return JSON.parse(localStorage.getItem("tern.foldedGroups") ?? "[]") as string[];
    } catch {
      return [];
    }
  };
  let folded = $state<string[]>(loadFolded());
  function toggleGroup(id: string) {
    folded = folded.includes(id) ? folded.filter((x) => x !== id) : [...folded, id];
    try {
      localStorage.setItem("tern.foldedGroups", JSON.stringify(folded));
    } catch {}
  }

  function addMenu(e: MouseEvent) {
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    app.showMenu({ x: b.left, y: b.bottom + 4 }, "Add a project", [
      { label: "Clone a project…", hint: "Ctrl Shift O", run: () => (app.overlay = "clone") },
      { label: "Add a folder to scan…", run: () => app.openSettings("folders") },
      { label: "Add a single project…", run: () => app.openSettings("folders") },
    ]);
  }

  /** Arrow keys move between project rows; Enter opens one (the button handles Enter itself). */
  function onListKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const rows = [...(listEl?.querySelectorAll<HTMLButtonElement>("button.row") ?? [])];
    const i = rows.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    e.preventDefault();
    rows[Math.max(0, Math.min(rows.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))]?.focus();
  }

  function onFilterKey(e: KeyboardEvent) {
    if (e.key === "Escape" && filter) {
      e.preventDefault();
      e.stopPropagation();
      filter = "";
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      tick().then(() => listEl?.querySelector<HTMLButtonElement>("button.row")?.focus());
    } else if (e.key === "Enter" && matches?.length) {
      e.preventDefault();
      app.select(matches[0].path);
      filter = "";
    }
  }

  const spaceLabel = $derived(app.activeSpace?.name ?? "All spaces");
</script>

{#snippet repoRow(r: RepoEntry, flat: boolean)}
  {@const s = app.sums[r.path]}
  {@const st = stateOf(s)}
  {@const working = app.isWorking(r.path)}
  {@const running = app.runningIn(r.path)}
  <button
    class="row"
    class:active={app.current === r.path}
    class:child={!!r.parent && !flat}
    class:hidden={app.isHidden(r)}
    class:picked={app.repoSel.includes(r.path)}
    aria-pressed={app.repoSel.includes(r.path)}
    aria-current={app.current === r.path ? "page" : undefined}
    onclick={(e) => app.clickRepo(e, r.path)}
    oncontextmenu={(e) => (app.repoSel.length > 1 && app.repoSel.includes(r.path) ? bulkMenu(e, app.repoSel) : repoMenu(e, r.path))}
    use:hover={() => repoCard(r.path)}
    onkeydown={(e) => {
      if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
        e.preventDefault();
        e.stopPropagation();
        const b = e.currentTarget.getBoundingClientRect();
        repoMenu({ x: b.left + 24, y: b.bottom }, r.path);
      }
    }}
  >
    <span class="dot {st}" aria-label={dotText[st]}></span>
    <span class="main">
      <span class="name">
        {#if r.parent}<span class="wt" aria-label="worktree">⑂</span>{/if}{r.name}
        {#if app.nameHint[r.path]}<span class="hint">{app.nameHint[r.path]}</span>{/if}
        {#if r.pinned}<span class="pin" aria-label="pinned"><Icon name="pin" size={11} /></span>{/if}
        {#if app.isHidden(r)}<span class="hint">hidden</span>{/if}
      </span>
      <span class="branch">{s?.branch ?? (s ? "detached" : "")}</span>
    </span>
    <span class="meta mono">
      {#if app.busy[r.path]}<span class="busy">{app.busy[r.path]}…</span>
      {:else if working}<span class="working" aria-label="An agent may be working"></span>
      {:else if app.isUnseen(r.path)}<span class="unseen" aria-label="Changed since you last looked"></span>{/if}
      {#if running}<span class="running" aria-label="{running} tasks running"><Icon name="play" size={9} stroke={2} />{running > 1 ? running : ""}</span>{/if}
      {#if s?.behind}<span class="behind">↓{s.behind}</span>{/if}
      {#if s?.ahead}<span class="ahead">↑{s.ahead}</span>{/if}
      {#if s?.changes.length}<span class="count">{s.changes.length}</span>{/if}
    </span>
  </button>
{/snippet}

<nav class="rail" aria-label="Sidebar">
  <div class="top">
    <button class="space" onclick={() => (app.overlay = "spaces")} use:hover={() => ({ title: "Switch space", text: "Spaces group your projects, for example Work and Personal.", hint: "Ctrl Shift S" })}>
      <span class="sw" style:--c={app.activeSpace?.color ?? "var(--faint)"}></span>
      <span class="space-name">{spaceLabel}</span>
      <Icon name="chevronDown" size={12} />
    </button>
  </div>

  <div class="nav">
    <button class="nav-item" class:on={home} aria-current={home ? "page" : undefined} onclick={() => app.select(null)}
      use:hover={() => ({ title: "Home", text: "What needs you, and every project.", hint: "Ctrl 0" })}>
      <Icon name="home" /><span>Home</span>
    </button>
    <button class="nav-item" class:on={app.ticketsOpen} aria-current={app.ticketsOpen ? "page" : undefined} onclick={() => app.openTickets()}
      use:hover={() => ({ title: "My tickets", text: "Azure DevOps work items and GitHub issues assigned to you.", hint: "Ctrl Shift T" })}>
      <Icon name="ticket" /><span>Tickets</span>
      {#if app.tickets.items.length}<span class="badge">{app.tickets.items.length}</span>{/if}
    </button>
    <button class="nav-item" class:on={app.dataOpen} aria-current={app.dataOpen ? "page" : undefined} onclick={() => app.openData()}
      use:hover={() => ({ title: "Data", text: "SQLite, Postgres, SQL Server, and Redis. Read-only unless you allow writes.", hint: "Ctrl Shift D" })}>
      <Icon name="database" /><span>Data</span>
    </button>
    <button class="nav-item" onclick={() => app.openSearch({})}
      use:hover={() => ({ title: "Search", text: "Code, file names, commits, branches, pull requests, and issues, in one project or many.", hint: "Ctrl Shift F" })}>
      <Icon name="search" /><span>Search</span>
    </button>
  </div>

  <div class="section-label projects-head">
    <span>Projects</span>
    <span class="grow"></span>
    <button class="icon-btn sm" onclick={addMenu} aria-label="Add a project" use:hover={() => ({ title: "Add a project", text: "Clone one, or add a folder that Tern scans." })}>
      <Icon name="plus" size={13} />
    </button>
  </div>
  {#if app.repos.length > 6}
    <div class="filter-row">
      <input class="field" bind:this={filterEl} bind:value={filter} placeholder="Filter projects" spellcheck="false" aria-label="Filter projects" onkeydown={onFilterKey} />
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="list" bind:this={listEl} onkeydown={onListKey}>
    {#if app.loadingRepos}
      {#each [0, 1, 2, 3, 4] as i (i)}<div class="skel"><span class="skeleton" style:width="{60 + ((i * 17) % 30)}%"></span></div>{/each}
    {:else if !app.repos.length}
      <div class="state inline">
        <p>No git projects found yet.</p>
        <button class="link" onclick={() => app.openSettings("folders")}>Add a folder to scan</button>
      </div>
    {:else if !app.visible.length}
      <div class="state inline">
        <p>No projects in {app.activeSpace?.name ?? "this space"} yet.</p>
        <button class="link" onclick={() => app.openSettings("spaces")}>Edit the space rules</button>
      </div>
    {:else if matches}
      {#each matches as r (r.path)}{@render repoRow(r, true)}{/each}
      {#if !matches.length}
        <div class="state inline">
          <p>No projects match “{filter}”.</p>
          <button class="link" onclick={() => (filter = "")}>Clear the filter</button>
        </div>
      {/if}
    {:else}
      {#each app.groups as g (g.space?.id ?? "none")}
        {@const gid = g.space?.id ?? "none"}
        {@const gfolded = !app.activeSpace && folded.includes(gid)}
        {#if !app.activeSpace}
          <button class="group" style:--c={g.space?.color ?? "var(--faint)"} onclick={() => toggleGroup(gid)} aria-expanded={!gfolded}>
            <span class="chev" class:closed={gfolded}><Icon name="chevronDown" size={10} stroke={1.8} /></span>
            <span class="sw"></span>{g.space?.name ?? "No space"}
            <span class="gcount">{g.repos.filter((r) => !r.parent).length}</span>
          </button>
        {/if}
        {#if !gfolded}
          {#each g.repos as r (r.path)}
            {@const hiddenTree = !!r.parent && !app.openTrees.includes(r.parent)}
            {@const trees = r.parent ? [] : g.repos.filter((x) => x.parent === r.path)}
            {#if !hiddenTree || app.showsFolded(r)}{@render repoRow(r, false)}{/if}
            {#if trees.length}
              {@const open = app.openTrees.includes(r.path)}
              {@const hiddenN = open ? 0 : trees.filter((t) => !app.showsFolded(t)).length}
              {#if open || hiddenN}
                <button class="trees" onclick={() => app.toggleTrees(r.path)} aria-expanded={open}>
                  {open ? "Fold worktrees" : `+${hiddenN} ${hiddenN === 1 ? "worktree" : "worktrees"}`}
                </button>
              {/if}
            {/if}
          {/each}
        {/if}
      {/each}
    {/if}
  </div>

  {#if app.repoSel.length}
    <div class="bulk" role="toolbar" aria-label="Actions for the selected projects">
      <span class="bcount">{app.repoSel.length} selected</span>
      <button class="btn sm ghost" onclick={() => app.bulkFetch(app.repoSel)}>Fetch</button>
      <button class="btn sm ghost" onclick={() => app.bulkSync(app.repoSel, "pull")}>Pull…</button>
      <button class="btn sm ghost" onclick={() => app.bulkSync(app.repoSel, "push")}>Push…</button>
      <button class="icon-btn sm" onclick={(e) => { const b = e.currentTarget.getBoundingClientRect(); bulkMenu({ x: b.left, y: b.top - 8 }, app.repoSel); }} aria-label="More actions"><Icon name="more" /></button>
      <button class="icon-btn sm" onclick={() => app.clearRepoSel()} aria-label="Clear the selection"><Icon name="x" size={12} /></button>
    </div>
  {/if}

  <footer>
    <button class="icon-btn" onclick={() => app.openSettings()} aria-label="Settings" use:hover={() => ({ title: "Settings", hint: "Ctrl ," })}>
      <Icon name="gear" />
    </button>
    <span class="count-line" use:hover={() => ({ title: "Projects", rows: [
      { label: "In view", value: String(app.visible.length) },
      { label: "With changes", value: String(dirtyCount) },
      { label: "Working now", value: String(workingCount) },
    ] })}>
      {app.visible.length} projects{#if workingCount} · <span class="live">{workingCount} working</span>{/if}
    </span>
    {#if app.hiddenCount}
      <button class="btn sm ghost" class:on={app.showHidden} onclick={() => app.setShowHidden(!app.showHidden)}
        use:hover={() => ({ title: app.showHidden ? "Stop showing hidden projects" : "Show hidden projects", text: "Right-click a hidden project to unhide it." })}>
        {app.showHidden ? "Hide hidden" : `${app.hiddenCount} hidden`}
      </button>
    {/if}
    <button class="icon-btn" onclick={() => app.toggleSidebar(false)} aria-label="Hide the sidebar" use:hover={() => ({ title: "Hide the sidebar", hint: "Ctrl \\" })}>
      <Icon name="sidebar" />
    </button>
  </footer>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    background: var(--panel);
    min-height: 0;
    min-width: 0;
  }
  .top {
    padding: var(--s3) var(--s3) var(--s1);
  }
  .space {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: 30px;
    padding: 0 var(--s3);
    border-radius: var(--radius);
    font-size: var(--fs-md);
    font-weight: 600;
    color: var(--text);
    text-align: left;
  }
  .space:hover {
    background: var(--hover);
  }
  .space :global(svg) {
    color: var(--faint);
  }
  .space-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sw {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: var(--s1) var(--s3) var(--s3);
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: var(--h-row);
    padding: 0 var(--s3);
    border-radius: var(--radius);
    font-size: var(--fs-md);
    color: var(--muted);
    text-align: left;
  }
  .nav-item span:not(.badge) {
    flex: 1;
  }
  .nav-item:hover {
    background: var(--hover);
    color: var(--text);
  }
  .nav-item.on {
    background: var(--raised);
    color: var(--text);
    font-weight: 600;
  }
  .projects-head {
    padding-right: var(--s3);
    border-top: 1px solid var(--line);
    padding-top: var(--s2);
    height: 32px;
  }
  .filter-row {
    padding: 0 var(--s3) var(--s2);
    display: flex;
  }
  .filter-row .field {
    flex: 1;
    height: var(--h-ctl-sm);
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--s3) var(--s3);
  }
  .skel {
    height: 40px;
    display: flex;
    align-items: center;
    padding: 0 var(--s3) 0 26px;
  }
  .skel .skeleton {
    height: 10px;
  }
  .none {
    margin: 0;
    padding: var(--s3);
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--s2);
    width: 100%;
    padding: var(--s3) var(--s2) var(--s1) 2px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--muted);
    text-align: left;
  }
  .group:hover {
    color: var(--text);
  }
  .chev {
    color: var(--faint);
    transition: transform 150ms var(--ease-out);
  }
  .chev.closed {
    transform: rotate(-90deg);
  }
  .gcount {
    margin-left: auto;
    font-weight: 400;
    color: var(--faint);
  }
  .row {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--s2);
    width: 100%;
    min-height: 40px;
    text-align: left;
    padding: var(--s1) var(--s3) var(--s1) var(--s2);
    border-radius: var(--radius);
    position: relative;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.active {
    background: var(--raised);
  }
  .row.active::before {
    content: "";
    position: absolute;
    left: calc(-1 * var(--s3));
    top: 9px;
    bottom: 9px;
    width: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .row.child {
    padding-left: 20px;
  }
  .row.picked {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 50%, transparent);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name,
  .branch {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    display: flex;
    align-items: center;
    color: var(--text);
    font-size: var(--fs-md);
  }
  .active .name {
    font-weight: 600;
  }
  .branch {
    color: var(--faint);
    font-size: var(--fs-xs);
    line-height: 1.35;
  }
  .wt {
    color: var(--faint);
    margin-right: var(--s1);
  }
  .hint {
    color: var(--faint);
    margin-left: var(--s2);
    font-size: var(--fs-xs);
    font-weight: 400;
  }
  .pin {
    color: var(--accent);
    margin-left: var(--s1);
    display: inline-flex;
  }
  .row.hidden .name,
  .row.hidden .branch {
    opacity: 0.5;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    justify-self: center;
    border: 1.5px solid var(--faint);
  }
  .dot.dirty {
    background: var(--accent);
    border-color: var(--accent);
  }
  .dot.conflict,
  .dot.error {
    background: var(--del);
    border-color: var(--del);
  }
  .dot.unknown {
    border-style: dashed;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-xs);
    color: var(--muted);
  }
  .working,
  .unseen {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .working {
    background: var(--add);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .unseen {
    background: var(--blue);
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }
  .running {
    display: inline-flex;
    align-items: center;
    gap: 1px;
    color: var(--add);
  }
  .behind {
    color: var(--blue);
  }
  .ahead {
    color: var(--add);
  }
  .count {
    color: var(--accent);
  }
  .busy {
    color: var(--faint);
  }
  .trees {
    display: block;
    width: 100%;
    text-align: left;
    padding: 0 var(--s3) var(--s1) 34px;
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .trees:hover {
    color: var(--accent);
  }
  .bulk {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 0 var(--s3) var(--s3);
    padding: 3px 3px 3px 10px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .bcount {
    flex: 1;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  footer {
    display: flex;
    align-items: center;
    gap: var(--s1);
    height: 36px;
    padding: 0 var(--s2);
    border-top: 1px solid var(--line);
  }
  .count-line {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .live {
    color: var(--add);
  }
</style>
