<script lang="ts">
  import { api } from "../lib/api";
  import { bytes as fmtBytes } from "../lib/format";
  import type { FileBytes } from "../lib/types";

  /**
   * Before and after for a changed image.
   * `base`: the commit for "before" (HEAD for local changes). `head`: the commit for "after", or null for the file on disk.
   */
  let { repo, rel, oldRel = null, base, head }: { repo: string; rel: string; oldRel?: string | null; base: string; head: string | null } = $props();

  let before = $state<FileBytes | null>(null);
  let after = $state<FileBytes | null>(null);
  let error = $state("");
  let mode = $state<"side" | "swipe">("side");
  let split = $state(50);
  let dims = $state<{ a?: string; b?: string }>({});

  $effect(() => {
    const key = `${repo}|${rel}|${base}|${head}`;
    before = after = null;
    error = "";
    dims = {};
    Promise.all([api.fileBytes(repo, oldRel ?? rel, base), api.fileBytes(repo, rel, head)]).then(
      ([a, b]) => {
        if (key !== `${repo}|${rel}|${base}|${head}`) return;
        before = a;
        after = b;
      },
      (e) => (error = String(e)),
    );
  });

  const both = $derived(!!before?.dataUrl && !!after?.dataUrl);
  const delta = $derived(before && after && before.dataUrl && after.dataUrl ? after.size - before.size : null);
  const dimOf = (e: Event) => {
    const img = e.currentTarget as HTMLImageElement;
    return `${img.naturalWidth} × ${img.naturalHeight}`;
  };

  /** Drag anywhere on the swipe view to move the line between before and after. */
  let dragging = $state(false);
  function splitAt(e: PointerEvent) {
    const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
    if (box.width > 0) split = Math.round(Math.max(0, Math.min(100, ((e.clientX - box.left) / box.width) * 100)) * 10) / 10;
  }
  function onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = true;
    splitAt(e);
  }
  function onMove(e: PointerEvent) {
    if (dragging) splitAt(e);
  }
  function onUp(e: PointerEvent) {
    dragging = false;
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }
</script>

<div class="idiff">
  <div class="toolbar sub">
    {#if both}
      <div class="seg" role="group" aria-label="Compare">
        <button class:on={mode === "side"} onclick={() => (mode = "side")}>Side by side</button>
        <button class:on={mode === "swipe"} onclick={() => (mode = "swipe")}>Swipe</button>
      </div>
      {#if mode === "swipe"}
        <input class="slider" type="range" min="0" max="100" step="0.1" bind:value={split} aria-label="Swipe between before and after" />
        <span class="meta">Drag on the image to compare</span>
      {/if}
    {/if}
    <span class="grow"></span>
    {#if delta !== null}
      <span class="meta">{delta === 0 ? "same size" : `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`}</span>
    {/if}
  </div>

  {#if error}
    <div class="state">
      <span class="state-title">Could not read the images</span>
      <pre>{error}</pre>
    </div>
  {:else if !before || !after}
    <div class="state delay"><p>Loading both versions…</p></div>
  {:else if mode === "swipe" && both}
    <!-- The range input in the bar is the keyboard way to do the same. -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="swipe" class:dragging
      onpointerdown={onDown} onpointermove={onMove} onpointerup={onUp} onpointercancel={onUp}>
      <div class="layer">
        <img src={before.dataUrl} alt="Before" draggable="false" />
      </div>
      <div class="layer top" style:clip-path="inset(0 0 0 {split}%)">
        <img src={after.dataUrl} alt="After" draggable="false" />
      </div>
      <div class="handle" style:left="{split}%"><span class="knob"></span></div>
      <span class="tag l">Before</span><span class="tag r">After</span>
    </div>
  {:else}
    <div class="side">
      {#each [["Before", before, "a"], ["After", after, "b"]] as [label, f, k]}
        {@const file = f as FileBytes}
        <figure>
          <figcaption>
            <b class:del={label === "Before"} class:add={label === "After"}>{label}</b>
            {#if file.dataUrl}
              <span class="faint">{dims[k as "a" | "b"] ?? ""} · {fmtBytes(file.size)}</span>
            {/if}
          </figcaption>
          <div class="pane">
            {#if file.tooLarge}
              <div class="state"><p>Over 40 MB ({fmtBytes(file.size)}).</p></div>
            {:else if !file.dataUrl}
              <div class="state"><p>{label === "Before" ? "New file: there was no image before." : "Deleted: there is no image now."}</p></div>
            {:else}
              <img src={file.dataUrl} alt={label as string} draggable="false" onload={(e) => (dims = { ...dims, [k as string]: dimOf(e) })} />
            {/if}
          </div>
        </figure>
      {/each}
    </div>
  {/if}
</div>

<style>
  .idiff {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .slider {
    width: 160px;
    accent-color: var(--accent);
  }
  .state.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
  .pane,
  .swipe {
    background-color: var(--bg);
    background-image: conic-gradient(color-mix(in srgb, var(--text) 5%, transparent) 25%, transparent 0 50%, color-mix(in srgb, var(--text) 5%, transparent) 0 75%, transparent 0);
    background-size: 16px 16px;
  }
  .side {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  figure {
    margin: 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  figure + figure {
    border-left: 1px solid var(--line);
  }
  figcaption {
    display: flex;
    gap: var(--s3);
    align-items: baseline;
    padding: var(--s2) var(--s4);
    font-size: var(--fs-sm);
    border-bottom: 1px solid var(--line);
  }
  .del {
    color: var(--del);
  }
  .add {
    color: var(--add);
  }
  .pane {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    place-items: center;
    padding: var(--s5);
  }
  .pane img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.3);
  }
  .swipe {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
    cursor: ew-resize;
    touch-action: none;
    outline: none;
  }
  /* The layers fill the view (the gap is padding), so the clip and the handle use the same width. */
  .layer {
    position: absolute;
    inset: 0;
    padding: var(--s5);
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .layer img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--accent);
    pointer-events: none;
  }
  .knob {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 14px;
    height: 28px;
    transform: translate(-50%, -50%);
    border-radius: 7px;
    background: var(--accent);
    box-shadow: 0 0 0 2px var(--bg);
    transition: transform 120ms var(--ease-out);
  }
  .swipe.dragging .knob {
    transform: translate(-50%, -50%) scale(1.1);
  }
  .tag {
    position: absolute;
    top: var(--s3);
    font-size: var(--fs-xs);
    padding: 1px 7px;
    border-radius: var(--r-sm);
    background: var(--panel);
    border: 1px solid var(--line);
    pointer-events: none;
  }
  .tag.l {
    left: var(--s4);
    color: var(--del);
  }
  .tag.r {
    right: var(--s4);
    color: var(--add);
  }
</style>
