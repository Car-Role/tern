<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app, type MenuItem } from "../lib/store.svelte";

  const m = app.menu!;
  type Real = Exclude<MenuItem, "sep">;
  const items = m.items;
  const real = items.map((it, i) => (it === "sep" || it.disabled ? -1 : i)).filter((i) => i >= 0);

  let el: HTMLDivElement | undefined = $state();
  let x = $state(m.x);
  let y = $state(m.y);
  let active = $state(-1);

  const close = () => (app.menu = null);
  function choose(i: number) {
    const it = items[i] as Real;
    if (!it || it.disabled) return;
    close();
    it.run();
  }

  onMount(async () => {
    await tick();
    if (!el) return;
    // Keep the menu inside the window.
    const r = el.getBoundingClientRect();
    x = Math.max(4, Math.min(x, window.innerWidth - r.width - 6));
    y = Math.max(4, Math.min(y, window.innerHeight - r.height - 6));
    el.focus();
  });

  function onKey(e: KeyboardEvent) {
    const pos = real.indexOf(active);
    if (e.key === "ArrowDown") active = real[(pos + 1) % real.length];
    else if (e.key === "ArrowUp") active = real[(pos - 1 + real.length) % real.length];
    else if (e.key === "Enter" && active >= 0) choose(active);
    else if (e.key === "Escape" || e.key === "Tab") close();
    else if (e.key.length === 1) {
      // Jump to the next item that starts with this letter.
      const k = e.key.toLowerCase();
      const start = pos + 1;
      const order = [...real.slice(start), ...real.slice(0, start)];
      const hit = order.find((i) => (items[i] as Real).label.toLowerCase().startsWith(k));
      if (hit === undefined) return;
      active = hit;
    } else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div class="catcher" role="presentation" onmousedown={close} oncontextmenu={(e) => { e.preventDefault(); close(); }}></div>
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="menu" role="menu" tabindex="0" bind:this={el} style:left="{x}px" style:top="{y}px" onkeydown={onKey}>
  <p class="title">{m.title}</p>
  {#each items as it, i}
    {#if it === "sep"}
      {#if i > 0 && items[i - 1] !== "sep" && i < items.length - 1}<div class="sep" role="separator"></div>{/if}
    {:else}
      <button role="menuitem" class="item" class:on={i === active} class:danger={it.danger} disabled={it.disabled}
        onmousemove={() => (active = i)} onclick={() => choose(i)}>
        <span>{it.label}</span>{#if it.hint}<kbd>{it.hint}</kbd>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 15;
  }
  .menu {
    position: fixed;
    z-index: 16;
    min-width: 220px;
    max-width: 380px;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.45);
    padding: 4px;
    outline: none;
  }
  .title {
    margin: 0;
    padding: 5px 10px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    text-align: left;
    padding: 5px 10px;
    border-radius: 5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item.on {
    background: var(--raised);
  }
  .item.danger.on {
    color: var(--del);
  }
  .item:disabled {
    color: var(--faint);
    cursor: default;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 4px 6px;
  }
</style>
