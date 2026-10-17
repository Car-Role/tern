<script lang="ts">
  import { api } from "../lib/api";
  import { bytes as fmtBytes } from "../lib/format";
  import type { FileBytes } from "../lib/types";

  /** `rev` null reads the file on disk; otherwise the version in that commit. */
  let { repo, rel, rev = null }: { repo: string; rel: string; rev?: string | null } = $props();

  let file = $state<FileBytes | null>(null);
  let error = $state("");
  let fit = $state(true);
  let dims = $state<{ w: number; h: number } | null>(null);
  let broken = $state(false);

  $effect(() => {
    const [r, p, v] = [repo, rel, rev];
    file = null;
    error = "";
    dims = null;
    broken = false;
    api.fileBytes(r, p, v).then(
      (f) => {
        if (r === repo && p === rel && v === rev) file = f;
      },
      (e) => (error = String(e)),
    );
  });
</script>

<div class="imgview">
  {#if file?.dataUrl && !file.tooLarge && !broken}
    <div class="toolbar sub">
      <span class="meta">{#if dims}{dims.w} × {dims.h} · {/if}{fmtBytes(file.size)} · {file.mime}</span>
      <span class="grow"></span>
      <div class="seg" role="group" aria-label="Zoom">
        <button class:on={fit} onclick={() => (fit = true)}>Fit</button>
        <button class:on={!fit} onclick={() => (fit = false)}>Actual size</button>
      </div>
    </div>
  {/if}
  {#if error}
    <div class="state">
      <span class="state-title">Could not open this image</span>
      <pre>{error}</pre>
    </div>
  {:else if !file}
    <div class="loading skeleton" aria-label="Loading the image"></div>
  {:else if file.tooLarge}
    <div class="state">
      <span class="state-title">Too large to open here</span>
      <p>This image is {fmtBytes(file.size)}. Images over 40 MB do not open in Tern.</p>
    </div>
  {:else if !file.dataUrl}
    <div class="state"><p>This image does not exist in this version.</p></div>
  {:else if broken}
    <div class="state">
      <span class="state-title">Cannot draw this image</span>
      <p>This file ({file.mime}, {fmtBytes(file.size)}) is not an image the window can draw. It may be damaged.</p>
    </div>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="stage" class:fit>
      <img src={file.dataUrl} alt={rel} draggable="false"
        onload={(e) => { const im = e.currentTarget as HTMLImageElement; dims = { w: im.naturalWidth, h: im.naturalHeight }; }}
        onerror={() => (broken = true)}
        onclick={() => (fit = !fit)} title={fit ? "Click for actual size" : "Click to fit the window"} />
    </div>
  {/if}
</div>

<style>
  .imgview {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .stage {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    place-items: center;
    padding: var(--s6);
    /* A checkerboard shows where the image is transparent. */
    background-color: var(--bg);
    background-image: conic-gradient(color-mix(in srgb, var(--text) 5%, transparent) 25%, transparent 0 50%, color-mix(in srgb, var(--text) 5%, transparent) 0 75%, transparent 0);
    background-size: 16px 16px;
  }
  .stage img {
    image-rendering: auto;
    cursor: zoom-out;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.3);
  }
  .stage.fit img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    cursor: zoom-in;
  }
  .stage:not(.fit) {
    place-items: start;
  }
  .loading {
    margin: var(--s6);
    width: 240px;
    height: 160px;
  }
</style>
