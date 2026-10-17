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
    padding: var(--s3) var(--s4);
    background: var(--raised);
    border-radius: var(--radius);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-sm);
    pointer-events: none;
    animation: tern-rise-in 120ms var(--ease-out);
  }
  .title {
    margin: 0 0 var(--s2);
    font-weight: 600;
    color: var(--text);
    word-break: break-word;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--s1) var(--s4);
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
    font-size: var(--fs-xs);
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .amber {
    color: var(--accent);
  }
  .blue {
    color: var(--blue);
  }
  .text {
    margin: var(--s3) 0 0;
    padding-top: var(--s2);
    border-top: 1px solid var(--line);
    color: var(--muted);
    white-space: pre-wrap;
    max-height: 180px;
    overflow: hidden;
  }
  .hint {
    margin: var(--s2) 0 0;
    color: var(--faint);
    font-size: var(--fs-xs);
  }
</style>
