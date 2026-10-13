<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/store.svelte";

  let el: HTMLDivElement | undefined = $state();
  let x = $state(0);
  let y = $state(0);

  const card = $derived(app.hoverCard);

  // Place the card to the right of the element, or below it when there is no room. Keep it in the window.
  $effect(() => {
    const c = card;
    if (!c) return;
    tick().then(() => {
      if (!el) return;
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      const r = c.rect;
      let left = r.right + 10;
      let top = r.top;
      if (left + w > window.innerWidth - 8) {
        left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8));
        top = r.bottom + 6;
        if (top + h > window.innerHeight - 8) top = r.top - h - 6;
      }
      x = left;
      y = Math.max(8, Math.min(top, window.innerHeight - h - 8));
    });
  });

  function hide() {
    if (app.hoverCard) app.hoverCard = null;
  }
</script>

<svelte:window onkeydown={hide} onwheel={hide} onblur={hide} />

{#if card}
  <div class="card" bind:this={el} style:left="{x}px" style:top="{y}px" role="tooltip">
    {#if card.data.title}<p class="title">{card.data.title}</p>{/if}
    {#if card.data.rows?.length}
      <dl>
        {#each card.data.rows as r}
          <dt>{r.label}</dt>
          <dd class:mono={r.mono} class={r.tone ?? ""}>{r.value}</dd>
        {/each}
      </dl>
    {/if}
    {#if card.data.text}<p class="text">{card.data.text}</p>{/if}
    {#if card.data.hint}<p class="hint">{card.data.hint}</p>{/if}
  </div>
{/if}

<style>
  .card {
    position: fixed;
    z-index: 30;
    max-width: 380px;
    min-width: 180px;
    padding: 9px 11px;
    background: var(--raised);
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.35);
    font-size: 12px;
    pointer-events: none;
    animation: in 90ms ease-out;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
  }
  .title {
    margin: 0 0 6px;
    font-weight: 600;
    color: var(--text);
    word-break: break-word;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 3px 12px;
    margin: 0;
  }
  dt {
    color: var(--faint);
  }
  dd {
    margin: 0;
    color: var(--muted);
    word-break: break-word;
  }
  dd.mono {
    font-family: var(--mono);
    font-size: 11.5px;
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .amber {
    color: var(--amber);
  }
  .blue {
    color: var(--blue);
  }
  .text {
    margin: 8px 0 0;
    padding-top: 7px;
    border-top: 1px solid var(--line);
    color: var(--muted);
    white-space: pre-wrap;
    max-height: 180px;
    overflow: hidden;
  }
  .hint {
    margin: 7px 0 0;
    color: var(--faint);
    font-size: 11px;
  }
</style>
