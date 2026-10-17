<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { hover } from "../lib/hover";
  import { inTauri } from "../lib/api";

  // macOS keeps its own traffic-light buttons on the left; Windows and Linux get Tern's buttons.
  const mac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);
  let maximized = $state(false);
  let focused = $state(true);
  const fetching = $derived(Object.values(app.busy).some((a) => a === "fetch"));

  type Win = Awaited<ReturnType<typeof getWin>>;
  let win: Win | null = null;
  async function getWin() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }

  onMount(() => {
    const offs: (() => void)[] = [];
    const onFocus = () => (focused = true);
    const onBlur = () => (focused = false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    focused = document.hasFocus();
    if (inTauri) {
      getWin().then(async (w) => {
        win = w;
        maximized = await w.isMaximized();
        offs.push(await w.onResized(async () => (maximized = await w.isMaximized())));
      });
    }
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      offs.forEach((off) => off());
    };
  });

  const openGoTo = () => {
    app.paletteMode = "projects";
    app.overlay = "palette";
  };
</script>

<header class="titlebar" class:mac class:blurred={!focused} data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <button class="brand" onclick={() => app.select(null)} use:hover={() => ({ title: "All projects", hint: "Ctrl+0" })} aria-label="Tern: all projects">
      <!-- The app icon in small: a tile with the tern mark. -->
      <span class="mark" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="14" height="14">
          <path d="M3.5 9.5c3 0 5.5 1.5 8.5 5 3-3.5 5.5-5 8.5-5" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
      <span>Tern</span>
    </button>

    {#if app.spaces.length}
      <div class="spaces" role="tablist" aria-label="Spaces">
        <button role="tab" class="space" class:on={!app.activeSpace} aria-selected={!app.activeSpace} onclick={() => app.switchSpace(null)}
          use:hover={() => ({ title: "All spaces", hint: "Ctrl+Alt+0" })}>All</button>
        {#each app.spaces as sp, i (sp.id)}
          <button role="tab" class="space" class:on={app.activeSpace?.id === sp.id} aria-selected={app.activeSpace?.id === sp.id}
            style:--c={sp.color} onclick={() => app.switchSpace(sp.id)} use:hover={() => ({ title: sp.name, hint: `Ctrl+Alt+${i + 1}` })}>
            <span class="sw"></span>{sp.name}
          </button>
        {/each}
      </div>
      <!-- Narrow windows: one button that opens the space picker. -->
      <button class="space-pick" onclick={() => (app.overlay = "spaces")} aria-label="Switch space" use:hover={() => ({ title: "Switch space", hint: "Ctrl+Shift+S" })}>
        <span class="sw" style:--c={app.activeSpace?.color ?? "var(--faint)"}></span>{app.activeSpace?.name ?? "All"}
        <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
    {/if}
  </div>

  <div class="center" data-tauri-drag-region>
    <button class="goto" onclick={openGoTo} aria-label="Go to a project, file, or command">
      <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.6" /><path d="M10.4 10.4 14 14" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
      <span class="goto-text">Go to a project, file, or command</span>
      <kbd>Ctrl K</kbd>
    </button>
    <button class="search tix" class:on={app.ticketsOpen} onclick={() => (app.ticketsOpen ? app.closeTickets() : app.openTickets())}
      use:hover={() => ({ title: "My tickets", text: "Azure DevOps work items and GitHub issues assigned to you.", hint: "Ctrl+Shift+T" })}>
      Tickets{#if app.tickets.items.length}<span class="badge">{app.tickets.items.length}</span>{/if}
    </button>
    <button class="search tix" class:on={app.dataOpen} onclick={() => (app.dataOpen ? app.closeData() : app.openData())}
      use:hover={() => ({ title: "Databases", text: "Look at SQLite, Postgres, SQL Server, and Redis. Read-only unless you allow writes.", hint: "Ctrl+Shift+D" })}>
      Data
    </button>
    <button class="search" onclick={() => (app.overlay = "search")}
      use:hover={() => ({ title: "Search", text: "Code, file names, commits, branches, PRs, and issues, in this repo, this space, or any repos you pick.", hint: "Ctrl+Shift+F" })}>
      Search
    </button>
  </div>

  <div class="right" data-tauri-drag-region>
    <div class="tools">
      <button class="icon" onclick={() => (app.overlay = "clone")} use:hover={() => ({ title: "Clone a repo", text: "From Azure DevOps or GitHub.", hint: "Ctrl+Shift+O" })} aria-label="Clone a repo">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
      </button>
      <button class="icon" class:spin={fetching} onclick={() => app.fetchAll()} use:hover={() => ({ title: fetching ? "Fetching…" : "Fetch all projects", hint: "Ctrl+Alt+F" })} aria-label="Fetch all projects">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v2.8h-2.8" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
      <button class="icon" onclick={() => (app.overlay = "settings")} use:hover={() => ({ title: "Settings", hint: "Ctrl+," })} aria-label="Settings">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path d="M2.5 4.5h7M12.5 4.5h1M2.5 11.5h1M6.5 11.5h7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          <circle cx="11" cy="4.5" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
          <circle cx="5" cy="11.5" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
        </svg>
      </button>
    </div>

    {#if !mac && inTauri}
      <div class="controls">
        <button class="ctl" onclick={() => win?.minimize()} aria-label="Minimize" tabindex="-1">
          <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M0 5.5h10" stroke="currentColor" /></svg>
        </button>
        <button class="ctl" onclick={() => win?.toggleMaximize()} aria-label={maximized ? "Restore" : "Maximize"} tabindex="-1">
          {#if maximized}
            <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z" fill="none" stroke="currentColor" /></svg>
          {:else}
            <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><rect x=".5" y=".5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
          {/if}
        </button>
        <button class="ctl close" onclick={() => win?.close()} aria-label="Close" tabindex="-1">
          <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M.5.5l9 9M9.5.5l-9 9" stroke="currentColor" stroke-width="1.05" /></svg>
        </button>
      </div>
    {/if}
  </div>
</header>

<style>
  .titlebar {
    grid-column: 1 / -1;
    height: 40px;
    display: flex;
    align-items: center;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    container-type: inline-size;
    user-select: none;
    transition: color 150ms ease-out;
  }
  .titlebar.mac {
    /* Room for the macOS window buttons. */
    padding-left: 76px;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    min-width: 0;
    height: 100%;
  }
  .left {
    flex: none;
    gap: 14px;
    padding-left: 12px;
  }
  .right {
    flex: none;
    justify-content: flex-end;
    gap: 6px;
  }
  /* The middle takes the free space, so it shrinks instead of running under its neighbors. */
  .center {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 16px;
    height: 100%;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 6px;
    margin-left: -6px;
    border-radius: var(--radius);
    font-weight: 650;
    font-size: 13px;
    color: var(--text);
  }
  .mark {
    width: 20px;
    height: 20px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--panel);
    transition: background-color 150ms ease-out;
  }
  .brand:hover {
    background: var(--hover);
  }

  .spaces {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .space {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 9px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--muted);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .space:hover {
    color: var(--text);
  }
  .space.on {
    color: var(--text);
    background: var(--raised);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.18);
  }
  .sw {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  .space-pick {
    display: none;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    font-size: 12px;
    color: var(--text);
  }
  .space-pick svg {
    color: var(--muted);
  }
  .space-pick:hover {
    background: var(--hover);
  }

  .goto {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 0 1 460px;
    min-width: 0;
    height: 28px;
    padding: 0 6px 0 10px;
    border-radius: 7px;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 12.5px;
    transition: border-color 150ms ease-out, color 150ms ease-out, transform 160ms var(--ease-out);
  }
  .goto:hover {
    border-color: var(--line-strong);
    color: var(--text);
  }
  .goto:focus-visible,
  .search:focus-visible,
  .brand:focus-visible,
  .space:focus-visible,
  .icon:focus-visible {
    outline: 2px solid var(--amber);
    outline-offset: 1px;
  }
  .goto-text {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .goto kbd {
    flex-shrink: 0;
  }
  .search {
    flex: none;
    height: 28px;
    padding: 0 10px;
    border-radius: 7px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .search:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tix {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tix.on {
    background: var(--raised);
    color: var(--text);
  }
  .badge {
    font-size: 10.5px;
    line-height: 16px;
    min-width: 16px;
    padding: 0 5px;
    border-radius: 8px;
    background: var(--amber-soft);
    color: var(--amber);
    text-align: center;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: 4px;
  }
  /* A thin line between Tern's buttons and the window buttons. */
  .tools:has(+ .controls)::after {
    content: "";
    width: 1px;
    height: 16px;
    margin-left: 8px;
    background: var(--line-strong);
  }
  .icon {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    color: var(--muted);
  }
  .icon:hover {
    background: var(--hover);
    color: var(--text);
  }
  .spin svg {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .controls {
    display: flex;
    height: 100%;
    align-self: flex-start;
  }
  .ctl {
    width: 46px;
    height: 39px;
    display: grid;
    place-items: center;
    color: var(--text);
    border-radius: 0;
    transition: background-color 120ms ease-out, color 120ms ease-out;
  }
  .ctl svg {
    shape-rendering: crispEdges;
  }
  .ctl:hover {
    background: var(--hover);
  }
  .ctl:active {
    background: var(--raised);
  }
  .ctl.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .ctl.close:active {
    background: #b22a1b;
    color: #fff;
  }

  /* An inactive window looks quieter, like native title bars. */
  .blurred .brand,
  .blurred .ctl,
  .blurred .space.on {
    color: var(--muted);
  }
  .blurred .mark {
    background: var(--muted);
  }

  @container (max-width: 1000px) {
    .goto-text {
      font-size: 0;
    }
    .goto-text::before {
      content: "Go to…";
      font-size: 12.5px;
    }
  }
  @container (max-width: 1160px) {
    .spaces {
      display: none;
    }
    .space-pick {
      display: flex;
    }
  }
  @container (max-width: 760px) {
    .search,
    .brand span {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin svg {
      animation-duration: 2.4s;
    }
    .space,
    .goto,
    .ctl {
      transition: none;
    }
  }
</style>
