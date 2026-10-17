<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { hover } from "../lib/hover";
  import { inTauri } from "../lib/api";
  import { baseName } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Notices from "./Notices.svelte";

  // macOS keeps its own traffic-light buttons on the left; Windows and Linux get Tern's buttons.
  const mac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);
  let maximized = $state(false);
  let focused = $state(true);
  const fetching = $derived(Object.values(app.busy).some((a) => a === "fetch"));
  const running = $derived(app.runs.filter((r) => r.running).length);

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

  /** Where you are, for the breadcrumb. */
  const place = $derived(app.ticketsOpen ? "Tickets" : app.dataOpen ? "Data" : app.current ? null : "Home");
  const repoName = $derived(app.current ? baseName(app.current) : "");
</script>

<header class="titlebar" class:mac class:blurred={!focused} data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    <button class="mark" onclick={() => app.select(null)} use:hover={() => ({ title: "Tern", text: "Home", hint: "Ctrl 0" })} aria-label="Tern: home">
      <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
        <path d="M3.5 9.5c3 0 5.5 1.5 8.5 5 3-3.5 5.5-5 8.5-5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
    {#if app.sidebarCollapsed}
      <button class="icon-btn" onclick={() => app.toggleSidebar(true)} aria-label="Show the sidebar" use:hover={() => ({ title: "Show the sidebar", hint: "Ctrl \\" })}>
        <Icon name="sidebar" />
      </button>
    {/if}
    <div class="history">
      <button class="icon-btn" disabled={!app.canBack} onclick={() => app.back()} aria-label="Back" use:hover={() => ({ title: "Back", hint: "Alt ←" })}><Icon name="back" /></button>
      <button class="icon-btn" disabled={!app.canForward} onclick={() => app.forward()} aria-label="Forward" use:hover={() => ({ title: "Forward", hint: "Alt →" })}><Icon name="forward" /></button>
    </div>
    <nav class="crumbs" aria-label="Where you are" data-tauri-drag-region>
      {#if app.activeSpace}
        <button class="crumb faint" onclick={() => (app.overlay = "spaces")}>
          <span class="sw" style:--c={app.activeSpace.color}></span>{app.activeSpace.name}
        </button>
        <span class="slash" aria-hidden="true">/</span>
      {/if}
      {#if place}
        <span class="crumb now">{place}</span>
      {:else}
        <button class="crumb now" onclick={() => app.review && app.closeReview()} title={app.current ?? ""}>{repoName}</button>

      {/if}
    </nav>
  </div>

  <div class="center" data-tauri-drag-region>
    <button class="goto" onclick={openGoTo} aria-label="Go to a project, file, or command">
      <Icon name="search" size={13} />
      <span class="goto-text">Go to a project, file, or command</span>
      <kbd>Ctrl K</kbd>
    </button>
  </div>

  <div class="right" data-tauri-drag-region>
    <div class="tools">
      {#if app.runs.length}
        <button class="btn sm ghost tasks" class:live={running > 0} onclick={() => app.setPanel(!app.panelOpen)}
          use:hover={() => ({ title: "Tasks and terminals", text: running ? `${running} running` : "Nothing running", hint: "Ctrl J" })}>
          <Icon name="play" size={10} stroke={2} />{running ? `${running} running` : "Tasks"}
        </button>
      {/if}
      <button class="icon-btn" class:spin={fetching} onclick={() => app.fetchAll()} aria-label="Fetch all"
        use:hover={() => ({ title: fetching ? "Fetching…" : "Fetch all", text: "Fetch every project in view, so pull and push counts are fresh.", hint: "Ctrl Alt F" })}>
        <Icon name="refresh" />
      </button>
      <button class="icon-btn bell" class:on={app.noticesOpen} onclick={() => app.toggleNotices()} aria-label="Notifications"
        use:hover={() => ({ title: "Notifications", text: app.unreadNotices ? `${app.unreadNotices} new` : "Everything Tern told you this session." })}>
        <Icon name="bell" />
        {#if app.unreadNotices}<span class="dot"></span>{/if}
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
{#if app.noticesOpen}<Notices />{/if}

<style>
  .titlebar {
    grid-column: 1 / -1;
    height: var(--h-title);
    display: flex;
    align-items: center;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    container-type: inline-size;
    user-select: none;
  }
  .titlebar.mac {
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
    flex: 1 1 0;
    gap: var(--s2);
    padding-left: var(--s3);
  }
  .right {
    flex: 1 1 0;
    justify-content: flex-end;
    gap: var(--s2);
  }
  .center {
    flex: 0 1 440px;
    min-width: 120px;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }
  .mark {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--panel);
    flex-shrink: 0;
    transition: transform 160ms var(--ease-out), background-color 150ms;
  }
  .mark:active {
    transform: scale(0.94);
  }
  .history {
    display: flex;
    gap: 0;
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: var(--s1);
    min-width: 0;
    overflow: hidden;
    font-size: var(--fs-md);
  }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: var(--s2);
    height: var(--h-ctl);
    padding: 0 var(--s2);
    border-radius: var(--radius);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  button.crumb:hover {
    background: var(--hover);
  }
  .crumb.now {
    color: var(--text);
    font-weight: 600;
  }
  .slash {
    color: var(--faint);
  }
  .sw {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--c);
    flex-shrink: 0;
  }
  .goto {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    min-width: 0;
    height: 28px;
    padding: 0 var(--s2) 0 var(--s3);
    border-radius: 7px;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: var(--fs-sm);
    transition: border-color 120ms, color 120ms;
  }
  .goto:hover {
    border-color: var(--line-strong);
    color: var(--text);
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
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: var(--s2);
  }
  .tools:has(+ .controls)::after {
    content: "";
    width: 1px;
    height: 16px;
    margin-left: var(--s3);
    background: var(--line-strong);
  }
  .tasks.live {
    color: var(--add);
  }
  .bell {
    position: relative;
  }
  .bell .dot {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--del);
    box-shadow: 0 0 0 2px var(--panel);
  }
  .spin :global(svg) {
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
    height: calc(var(--h-title) - 1px);
    display: grid;
    place-items: center;
    color: var(--text);
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
  .blurred .ctl,
  .blurred .crumb.now {
    color: var(--muted);
  }
  .blurred .mark {
    background: var(--muted);
  }
  @container (max-width: 900px) {
    .goto-text {
      font-size: 0;
    }
    .goto-text::before {
      content: "Go to…";
      font-size: var(--fs-sm);
    }
  }
  @container (max-width: 720px) {
    .crumbs .faint,
    .crumbs .slash {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin :global(svg) {
      animation-duration: 2.4s;
    }
  }
</style>
