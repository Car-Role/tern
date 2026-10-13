<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/store.svelte";
  import { commandFor } from "./lib/commands";
  import { isInsideTerminal } from "./lib/shells";

  /** The only Tern keys that work while you type in a terminal. Everything else goes to the shell. */
  const TERMINAL_KEYS = new Set(["task.panel", "open.terminal", "term.new", "go.commands"]);
  import Rail from "./components/Rail.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import Tickets from "./components/Tickets.svelte";
  import ListPane from "./components/ListPane.svelte";
  import Viewer from "./components/Viewer.svelte";
  import Overview from "./components/Overview.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import Palette from "./components/Palette.svelte";
  import CopyMenu from "./components/CopyMenu.svelte";
  import Help from "./components/Help.svelte";
  import Settings from "./components/Settings.svelte";
  import SpaceMenu from "./components/SpaceMenu.svelte";
  import Confirm from "./components/Confirm.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import TaskPanel from "./components/TaskPanel.svelte";
  import HoverCard from "./components/HoverCard.svelte";
  import CloneDialog from "./components/CloneDialog.svelte";
  import BranchPicker from "./components/BranchPicker.svelte";
  import Search from "./components/Search.svelte";

  const load = (k: string, d: number) => {
    try {
      return Number(localStorage.getItem("scout." + k)) || d;
    } catch {
      return d;
    }
  };
  let railW = $state(load("railW", 250));
  let listW = $state(load("listW", 340));

  function drag(which: "rail" | "list") {
    return (e: PointerEvent) => {
      const startX = e.clientX;
      const start = which === "rail" ? railW : listW;
      const move = (ev: PointerEvent) => {
        const w = Math.min(560, Math.max(180, start + ev.clientX - startX));
        if (which === "rail") railW = w;
        else listW = w;
      };
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        try {
          localStorage.setItem("scout.railW", String(railW));
          localStorage.setItem("scout.listW", String(listW));
        } catch {}
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    };
  }

  function onKey(e: KeyboardEvent) {
    if (isInsideTerminal(e.target)) {
      const cmd = commandFor(e);
      if (cmd && TERMINAL_KEYS.has(cmd.id)) {
        e.preventDefault();
        e.stopPropagation();
        cmd.run();
      }
      return;
    }
    // The webview's own shortcuts (print, reload, find) must not fire.
    if ((e.ctrlKey && ["KeyP", "KeyR", "KeyJ", "KeyG"].includes(e.code)) || e.key === "F5") e.preventDefault();

    // The right-click menu handles its own keys.
    if (app.menu) return;
    if (e.key === "Escape") {
      if (app.overlay) app.overlay = null;
      else if (app.editing) app.editing = false;
      else if (app.selected.length) app.selected = [];
      else if (app.repoSel.length) app.clearRepoSel();
      else if (app.review) app.closeReview();
      else return;
      e.preventDefault();
      return;
    }
    // Overlays take their own keys; only a few global keys still work while one is open.
    if (app.overlay && !(e.ctrlKey && (e.code === "KeyK" || e.code === "KeyP" || (e.shiftKey && e.code === "KeyF")))) return;

    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    if (!typing && !e.ctrlKey && !e.altKey && (e.key === "?" || e.key === "F1")) {
      e.preventDefault();
      app.overlay = "help";
      return;
    }
    const cmd = commandFor(e);
    if (!cmd) return;
    // Inside the commit box, Ctrl+Shift+Enter means "commit and push". The box handles it.
    if (typing && cmd.id === "git.commit") return;
    // Let CodeMirror keep Ctrl+D (select next match) while editing.
    if (app.editing && cmd.id === "view.diffMode") return;
    e.preventDefault();
    e.stopPropagation();
    cmd.run();
  }

  // Mouse buttons 4 and 5 (the side buttons) go back and forward.
  function onMouse(e: MouseEvent) {
    if (e.button !== 3 && e.button !== 4) return;
    e.preventDefault();
    if (e.type !== "mouseup") return;
    if (e.button === 3) app.back();
    else app.forward();
  }

  onMount(() => {
    app.applyTheme();
    // Follow Windows when it switches between light and dark.
    matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => app.theme === "system" && app.applyTheme());
    app.init();
  });
</script>

<svelte:window onkeydowncapture={onKey} oncontextmenu={(e) => e.preventDefault()} onmousedown={onMouse} onmouseup={onMouse} />

<div class="shell" style:--rail-w="{railW}px" style:--list-w="{listW}px" class:overview={!app.current}>
  <TitleBar />
  <Rail />
  <div class="grip" role="separator" aria-orientation="vertical" onpointerdown={drag("rail")}></div>
  {#if app.ticketsOpen}
    <Tickets />
  {:else if app.current}
    <ListPane />
    <div class="grip" role="separator" aria-orientation="vertical" onpointerdown={drag("list")}></div>
    <Viewer />
  {:else}
    <Overview />
  {/if}
  {#if app.panelOpen}<TaskPanel />{/if}
  <StatusBar />
</div>

{#if app.overlay === "palette"}<Palette />{/if}
{#if app.overlay === "copy"}<CopyMenu />{/if}
{#if app.overlay === "help"}<Help />{/if}
{#if app.overlay === "settings"}<Settings />{/if}
{#if app.overlay === "spaces" || app.overlay === "move"}<SpaceMenu />{/if}
{#if app.overlay === "confirm"}<Confirm />{/if}
{#if app.menu}<ContextMenu />{/if}
{#if app.overlay === "clone"}<CloneDialog />{/if}
{#if app.overlay === "branches"}<BranchPicker />{/if}
{#if app.overlay === "search"}<Search />{/if}
<HoverCard />

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--rail-w) 1px var(--list-w) 1px minmax(0, 1fr);
    grid-template-rows: 40px minmax(0, 1fr) auto 26px;
    height: 100%;
  }
  /* The project list and its grip run down beside the task panel. */
  .shell > :global(.rail),
  .shell > .grip:first-of-type {
    grid-row: 2 / 4;
  }
  .shell.overview {
    grid-template-columns: var(--rail-w) 1px minmax(0, 1fr);
  }
  .grip {
    background: var(--line);
    cursor: col-resize;
    position: relative;
  }
  .grip::after {
    content: "";
    position: absolute;
    inset: 0 -3px;
  }
  .grip:hover {
    background: var(--line-strong);
  }
</style>
