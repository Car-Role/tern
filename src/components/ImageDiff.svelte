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
</script>

<div class="idiff">
  <div class="bar">
    {#if both}
      <div class="seg" role="group" aria-label="Compare">
        <button class:on={mode === "side"} onclick={() => (mode = "side")}>Side by side</button>
        <button class:on={mode === "swipe"} onclick={() => (mode = "swipe")}>Swipe</button>
      </div>
      {#if mode === "swipe"}
        <input class="slider" type="range" min="0" max="100" bind:value={split} aria-label="Swipe between before and after" />
      {/if}
    {/if}
    <span class="grow"></span>
    {#if delta !== null}
      <span class="mono faint">{delta === 0 ? "same size" : `${delta > 0 ? "+" : "−"}${fmtBytes(Math.abs(delta))}`}</span>
    {/if}
  </div>

  {#if error}
    <p class="note err mono">{error}</p>
  {:else if !before || !after}
    <p class="note">Loading both versions…</p>
  {:else if mode === "swipe" && both}
    <div class="swipe">
      <div class="layer">
        <img src={before.dataUrl} alt="Before" draggable="false" />
      </div>
      <div class="layer top" style:clip-path="inset(0 0 0 {split}%)">
        <img src={after.dataUrl} alt="After" draggable="false" />
      </div>
      <div class="handle" style:left="{split}%"></div>
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
              <span class="mono faint">{dims[k as "a" | "b"] ?? ""} · {fmtBytes(file.size)}</span>
            {/if}
          </figcaption>
          <div class="pane">
            {#if file.tooLarge}
              <p class="note">Over 40 MB ({fmtBytes(file.size)}).</p>
            {:else if !file.dataUrl}
              <p class="note">{label === "Before" ? "New file: there was no image before." : "Deleted: there is no image now."}</p>
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
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .grow {
    flex: 1;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 2px 9px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .slider {
    width: 180px;
    accent-color: var(--amber);
  }
  .pane,
  .swipe {
    background-color: var(--bg);
    background-image: conic-gradient(var(--panel) 25%, transparent 0 50%, var(--panel) 0 75%, transparent 0);
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
    gap: 10px;
    align-items: baseline;
    padding: 6px 12px;
    font-size: 12px;
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
    padding: 16px;
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
  }
  .layer {
    position: absolute;
    inset: 16px;
    display: grid;
    place-items: center;
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
    background: var(--amber);
    pointer-events: none;
  }
  .tag {
    position: absolute;
    top: 10px;
    font-size: 11px;
    padding: 1px 7px;
    border-radius: 4px;
    background: var(--panel);
    border: 1px solid var(--line);
  }
  .tag.l {
    left: 12px;
    color: var(--del);
  }
  .tag.r {
    right: 12px;
    color: var(--add);
  }
  .note {
    margin: 16px;
    color: var(--muted);
    font-size: 12.5px;
    text-align: center;
  }
  .note.err {
    color: var(--del);
  }
</style>
