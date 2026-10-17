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
  {#if error}
    <p class="note err mono">{error}</p>
  {:else if !file}
    <div class="loading" aria-label="Loading the image"></div>
  {:else if file.tooLarge}
    <p class="note">This image is {fmtBytes(file.size)}. Images over 40 MB do not open here.</p>
  {:else if !file.dataUrl}
    <p class="note">This image does not exist in this version.</p>
  {:else if broken}
    <p class="note">This file is not an image the window can draw ({file.mime}, {fmtBytes(file.size)}). It may be damaged.</p>
  {:else}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <div class="stage" class:fit>
      <img src={file.dataUrl} alt={rel} draggable="false"
        onload={(e) => { const im = e.currentTarget as HTMLImageElement; dims = { w: im.naturalWidth, h: im.naturalHeight }; }}
        onerror={() => (broken = true)}
        onclick={() => (fit = !fit)} title={fit ? "Click for actual size" : "Click to fit the window"} />
    </div>
    <div class="info mono">
      {#if dims}<span>{dims.w} × {dims.h}</span>{/if}
      <span>{fmtBytes(file.size)}</span>
      <span class="faint">{file.mime}</span>
      <span class="grow"></span>
      <button class="mode" onclick={() => (fit = !fit)}>{fit ? "Actual size" : "Fit"}</button>
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
    padding: 24px;
    /* A checkerboard shows where the image is transparent. */
    background-color: var(--bg);
    background-image: conic-gradient(var(--panel) 25%, transparent 0 50%, var(--panel) 0 75%, transparent 0);
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
  .info {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 14px;
    border-top: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--muted);
  }
  .grow {
    flex: 1;
  }
  .mode {
    font-size: 11.5px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 1px 8px;
  }
  .mode:hover {
    color: var(--text);
  }
  .note {
    margin: 24px;
    color: var(--muted);
  }
  .note.err {
    color: var(--del);
    font-size: 12px;
  }
  .loading {
    margin: 24px;
    width: 240px;
    height: 160px;
    border-radius: 8px;
    background: var(--raised);
    animation: fade 1.2s ease-in-out infinite;
  }
  @keyframes fade {
    50% {
      opacity: 0.45;
    }
  }
</style>
