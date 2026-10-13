<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { formatPaths, type CopyFormat } from "../lib/format";

  let box: HTMLDivElement | undefined = $state();

  const rels = $derived(app.selected.length ? app.selected : app.view?.rel ? [app.view.rel] : []);
  const lines = $derived(app.view && rels.length === 1 && rels[0] === app.view.rel ? app.lines : null);
  const repo = $derived(app.current ?? "");

  const target = $derived(
    rels.length > 1 ? `${rels.length} selected files` : rels.length === 1 ? rels[0] : "the project folder",
  );

  const options = $derived.by(() => {
    const preview = (f: Exclude<CopyFormat, "diff">) =>
      rels.length ? formatPaths(f, repo, rels, lines).split("\n")[0] : f === "mention" ? "@" + repo : repo;
    const kind = app.review?.kind === "pr" ? "pull request" : "commit";
    const diffScope = app.review
      ? rels.length === 1 ? `this file in the ${kind}` : rels.length ? `${rels.length} files in the ${kind}` : `the whole ${kind}`
      :rels.length === 1 ? "the changes in this file" : rels.length ? `${rels.length} changed files` : "all local changes";
    return [
      { f: "relative" as const, key: "R", label: "Relative path", preview: preview("relative") },
      { f: "absolute" as const, key: "A", label: "Absolute path", preview: preview("absolute") },
      { f: "line" as const, key: "L", label: "Path with line numbers", preview: preview("line") },
      { f: "mention" as const, key: "M", label: "@ mention for Claude", preview: preview("mention") },
      { f: "diff" as const, key: "D", label: "Diff as text", preview: diffScope },
    ];
  });

  function onKey(e: KeyboardEvent) {
    const k = e.key.toUpperCase();
    const hit = options.find((o) => o.key === k) ?? (e.key === "@" ? options[3] : undefined);
    if (hit) app.copy(hit.f);
    else if (e.key === "Enter") app.copy(app.lastCopy);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  onMount(() => box?.focus());
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="menu" role="dialog" aria-label="Copy for an agent" tabindex="0" bind:this={box} onkeydown={onKey}>
    <p class="what">Copy <span class="mono">{target}</span> as</p>
    {#each options as o}
      <button class="opt" class:last={app.lastCopy === o.f} onclick={() => app.copy(o.f)}>
        <kbd>{o.key}</kbd>
        <span class="label">{o.label}</span>
        <span class="preview mono">{o.preview}</span>
      </button>
    {/each}
    <p class="foot faint"><kbd>Enter</kbd> repeats the last format. Select files with <kbd>Space</kbd> to copy many.</p>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 16vh;
    z-index: 10;
  }
  .menu {
    width: min(560px, calc(100vw - 32px));
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    padding: 8px;
    outline: none;
  }
  .what {
    margin: 4px 8px 8px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .what .mono {
    color: var(--text);
  }
  .opt {
    display: grid;
    grid-template-columns: 26px 170px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 7px 8px;
    border-radius: var(--radius);
  }
  .opt:hover {
    background: var(--raised);
  }
  .opt.last .label::after {
    content: "last used";
    margin-left: 8px;
    font-size: 11px;
    color: var(--amber);
  }
  .opt kbd {
    justify-self: center;
    color: var(--text);
  }
  .preview {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .foot {
    margin: 8px 8px 2px;
    font-size: 11px;
  }
</style>
