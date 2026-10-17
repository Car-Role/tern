<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { baseName } from "../lib/format";
  import Icon from "./Icon.svelte";

  let box: HTMLDivElement | undefined = $state();

  const when = (at: number) => {
    const s = Math.max(0, Math.round((Date.now() - at) / 1000));
    if (s < 60) return "now";
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    return `${Math.floor(s / 3600)}h`;
  };

  onMount(() => {
    const down = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (box && !box.contains(t) && !t.closest(".bell")) app.toggleNotices(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        app.toggleNotices(false);
      }
    };
    window.addEventListener("mousedown", down, true);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("mousedown", down, true);
      window.removeEventListener("keydown", key, true);
    };
  });
</script>

<div class="pop" bind:this={box} role="dialog" aria-label="Notifications">
  <div class="head">
    <b>Notifications</b>
    <span class="grow"></span>
    {#if app.notices.length}<button class="btn sm ghost" onclick={() => app.clearNotices()}>Clear</button>{/if}
  </div>
  <div class="list">
    {#each app.notices as n (n.id)}
      <div class="item {n.kind}">
        <span class="ic">
          {#if n.kind === "err"}<Icon name="alert" size={13} />{:else if n.kind === "ok"}<Icon name="check" size={13} />{:else}<Icon name="circle" size={8} stroke={3} />{/if}
        </span>
        <span class="text">
          {n.text}
          {#if n.repo}<button class="repo" onclick={() => { app.select(n.repo); app.toggleNotices(false); }}>{baseName(n.repo)}</button>{/if}
        </span>
        <span class="time">{when(n.at)}</span>
      </div>
    {:else}
      <div class="state"><p>No notifications yet. Tern lists what it tells you here: finished runs, failed actions, and agents that went quiet.</p></div>
    {/each}
  </div>
</div>

<style>
  .pop {
    position: fixed;
    top: calc(var(--h-title) + 4px);
    right: 12px;
    z-index: 25;
    width: min(380px, calc(100vw - 24px));
    max-height: min(480px, 70vh);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-pop);
    animation: tern-dialog-in 140ms var(--ease-out);
    user-select: text;
  }
  .head {
    display: flex;
    align-items: center;
    height: var(--h-bar);
    padding: 0 var(--s3) 0 var(--s4);
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-md);
  }
  .list {
    overflow: auto;
    padding: var(--s1) 0;
  }
  .item {
    display: grid;
    grid-template-columns: 18px 1fr auto;
    gap: var(--s3);
    align-items: start;
    padding: var(--s2) var(--s4);
    font-size: var(--fs-sm);
  }
  .item + .item {
    border-top: 1px solid var(--line);
  }
  .ic {
    display: grid;
    place-items: center;
    height: 18px;
    color: var(--faint);
  }
  .err .ic {
    color: var(--del);
  }
  .ok .ic {
    color: var(--add);
  }
  .text {
    color: var(--text);
    line-height: 1.45;
    overflow-wrap: anywhere;
  }
  .repo {
    margin-left: var(--s2);
    color: var(--muted);
    font-size: var(--fs-xs);
  }
  .repo:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .time {
    color: var(--faint);
    font-size: var(--fs-xs);
    line-height: 18px;
  }
</style>
