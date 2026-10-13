<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { hover } from "../lib/hover";
  import { bulkMenu, repoMenu } from "../lib/menus";
  import { repoCard } from "../lib/repoCard";
  import type { Summary } from "../lib/types";

  function stateOf(s: Summary | undefined) {
    if (!s) return "unknown";
    if (s.error) return "error";
    if (s.changes.some((c) => c.kind === "conflict")) return "conflict";
    if (s.changes.length) return "dirty";
    return "clean";
  }

  const dotText: Record<string, string> = {
    clean: "Clean: no local changes",
    dirty: "Has changes that are not committed",
    conflict: "Merge conflict",
    error: "Tern could not read this repo",
    unknown: "Not read yet",
  };

  const dirtyCount = $derived(app.visible.filter((r) => app.sums[r.path]?.changes.length).length);
  const workingCount = $derived(app.visible.filter((r) => app.isWorking(r.path)).length);
</script>

<nav class="rail" aria-label="Projects">
  <div class="list">
    {#if app.loadingRepos}
      <p class="empty">Looking for repos…</p>
    {:else if !app.repos.length}
      <div class="empty">
        <p>No git repos found.</p>
        <button class="link" onclick={() => (app.overlay = "settings")}>Add a folder to scan</button>
      </div>
    {:else if !app.visible.length}
      <div class="empty">
        <p>No projects in {app.activeSpace?.name ?? "this space"} yet.</p>
        <button class="link" onclick={() => (app.overlay = "settings")}>Edit the space rules</button>
      </div>
    {/if}
    {#each app.groups as g (g.space?.id ?? "none")}
      {#if !app.activeSpace}
        <div class="group" style:--c={g.space?.color ?? "var(--faint)"}>
          <span class="sw"></span>{g.space?.name ?? "No space"}
        </div>
      {/if}
    {#each g.repos as r (r.path)}
      {@const s = app.sums[r.path]}
      {@const st = stateOf(s)}
      {@const working = app.isWorking(r.path)}
      {@const folded = !!r.parent && !app.openTrees.includes(r.parent)}
      {@const trees = r.parent ? [] : g.repos.filter((x) => x.parent === r.path)}
      {#if !folded || app.showsFolded(r)}
      <button
        class="row"
        class:active={app.current === r.path}
        class:child={!!r.parent}
        class:hidden={app.isHidden(r)}
        class:picked={app.repoSel.includes(r.path)}
        aria-pressed={app.repoSel.includes(r.path)}
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
            {#if r.pinned}<span class="pin" aria-label="pinned">•</span>{/if}
            {#if app.isHidden(r)}<span class="hint">hidden</span>{/if}
          </span>
          <span class="branch mono">{s?.branch ?? (s ? "detached" : "")}</span>
        </span>
        <span class="meta mono">
          {#if app.busy[r.path]}<span class="busy">{app.busy[r.path]}…</span>{/if}
          {#if app.runningIn(r.path)}<span class="running" title="{app.runningIn(r.path)} tasks running">▶{app.runningIn(r.path) > 1 ? app.runningIn(r.path) : ""}</span>{/if}
          {#if working}<span class="working">working</span>
          {:else if app.isUnseen(r.path)}<span class="unseen"></span>{/if}
          {#if s?.behind}<span class="behind" title="{s.behind} commits to pull">↓{s.behind}</span>{/if}
          {#if s?.ahead}<span class="ahead" title="{s.ahead} commits to push">↑{s.ahead}</span>{/if}
          {#if s?.changes.length}<span class="count" title="{s.changes.length} changed files">{s.changes.length}</span>{/if}
        </span>
      </button>
      {/if}
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
    {/each}
  </div>

  {#if app.repoSel.length}
    <div class="bulk" role="toolbar" aria-label="Actions for the selected projects">
      <span class="count">{app.repoSel.length} selected</span>
      <button onclick={() => app.bulkFetch(app.repoSel)}>Fetch</button>
      <button onclick={() => app.bulkSync(app.repoSel, "pull")}>Pull</button>
      <button onclick={() => app.bulkSync(app.repoSel, "push")}>Push</button>
      <button onclick={(e) => { const b = e.currentTarget.getBoundingClientRect(); bulkMenu({ x: b.left, y: b.top - 8 }, app.repoSel); }} aria-label="More actions">⋯</button>
      <button class="x" onclick={() => app.clearRepoSel()} aria-label="Clear the selection">×</button>
    </div>
  {/if}

  <footer class="faint">
    <span class="count-line">
      {app.visible.length} projects{#if dirtyCount}, {dirtyCount} with changes{/if}{#if workingCount}, {workingCount} working{/if}
    </span>
    {#if app.hiddenCount}
      <button class="toggle-hidden" class:on={app.showHidden} onclick={() => app.setShowHidden(!app.showHidden)}
        title={app.showHidden ? "Stop showing hidden projects" : "Show hidden projects. Right-click one to unhide it."}>
        {app.showHidden ? "Hide hidden" : `${app.hiddenCount} hidden`}
      </button>
    {/if}
  </footer>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    background: var(--panel);
    min-height: 0;
  }
  .sw {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 10px 8px 3px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .group:first-child {
    padding-top: 2px;
  }
  .working {
    color: var(--add);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .working::before {
    content: "";
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--add);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }
  .row.picked {
    background: var(--amber-soft);
  }
  .row.picked::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: var(--radius);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--amber) 55%, transparent);
    pointer-events: none;
  }
  .bulk {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 0 6px 6px;
    padding: 4px 4px 4px 10px;
    border-radius: var(--radius);
    background: var(--amber-soft);
    border: 1px solid color-mix(in srgb, var(--amber) 40%, transparent);
    font-size: 12px;
  }
  .bulk .count {
    flex: 1;
    color: var(--text);
    white-space: nowrap;
  }
  .bulk button {
    padding: 2px 7px;
    border-radius: 4px;
    color: var(--amber);
  }
  .bulk button:hover {
    background: var(--amber-soft);
    color: var(--text);
  }
  .bulk .x {
    color: var(--muted);
  }
  .running {
    color: var(--add);
    font-size: 10px;
  }
  .unseen {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--blue);
    align-self: center;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 6px 8px;
  }
  .row {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    width: 100%;
    text-align: left;
    padding: 5px 8px 5px 6px;
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
    left: -6px;
    top: 6px;
    bottom: 6px;
    width: 2px;
    border-radius: 2px;
    background: var(--amber);
  }
  .row.child {
    padding-left: 20px;
    grid-template-columns: 14px minmax(0, 1fr) auto;
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
    color: var(--text);
  }
  .active .name {
    font-weight: 600;
  }
  .branch {
    color: var(--faint);
    font-size: 11px;
    line-height: 1.3;
  }
  .wt {
    color: var(--faint);
    margin-right: 4px;
  }
  .hint {
    color: var(--faint);
    margin-left: 6px;
    font-size: 11.5px;
  }
  .trees {
    display: block;
    width: 100%;
    text-align: left;
    padding: 1px 8px 4px 26px;
    font-size: 11.5px;
    color: var(--faint);
  }
  .trees:hover {
    color: var(--amber);
  }
  .pin {
    color: var(--amber);
    margin-left: 4px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    justify-self: center;
    border: 1.5px solid var(--faint);
  }
  .dot.dirty {
    background: var(--amber);
    border-color: var(--amber);
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
    gap: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .behind {
    color: var(--blue);
  }
  .ahead {
    color: var(--add);
  }
  .count {
    color: var(--amber);
  }
  .busy {
    color: var(--faint);
  }
  .empty {
    color: var(--muted);
    padding: 12px 10px;
    margin: 0;
  }
  .empty p {
    margin: 0 0 6px;
  }
  .link {
    color: var(--amber);
  }
  footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 14px;
    font-size: 11px;
    border-top: 1px solid var(--line);
  }
  .count-line {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toggle-hidden {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--muted);
    padding: 1px 7px;
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  .toggle-hidden:hover,
  .toggle-hidden.on {
    color: var(--text);
    border-color: var(--line-strong);
  }
  .row.hidden .name,
  .row.hidden .branch {
    opacity: 0.5;
  }
</style>
