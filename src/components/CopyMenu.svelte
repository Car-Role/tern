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

  // The highlighted row starts on the last-used format, so Enter repeats it until the arrows move.
  let active = $state(-1);
  const shown = $derived(active >= 0 ? active : Math.max(0, options.findIndex((o) => o.f === app.lastCopy)));

  function onKey(e: KeyboardEvent) {
    const k = e.key.toUpperCase();
    const hit = options.find((o) => o.key === k) ?? (e.key === "@" ? options[3] : undefined);
    if (hit) app.copy(hit.f);
    else if (e.key === "ArrowDown") active = (shown + 1) % options.length;
    else if (e.key === "ArrowUp") active = (shown - 1 + options.length) % options.length;
    else if (e.key === "Enter") app.copy(active >= 0 ? options[active].f : app.lastCopy);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  onMount(() => box?.focus());
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="dialog sm" role="dialog" aria-label="Copy for agent" tabindex="0" bind:this={box} onkeydown={onKey}>
    <div class="dialog-head"><h2 class="what">Copy <span class="mono">{target}</span> as</h2></div>
    <div class="dialog-body list" role="listbox" aria-label="Formats">
      {#each options as o, i}
        <button class="opt" role="option" aria-selected={i === shown} class:on={i === shown}
          onmousemove={() => (active = i)} onclick={() => app.copy(o.f)}>
          <kbd>{o.key}</kbd>
          <span class="label">{o.label}</span>
          <span class="preview mono">{o.preview}</span>
          <span class="last-used">{app.lastCopy === o.f ? "last used" : ""}</span>
        </button>
      {/each}
    </div>
    <div class="dialog-foot">
      <span class="hint"><kbd>↑</kbd> <kbd>↓</kbd> <kbd>Enter</kbd> copy the highlighted format. Select files with <kbd>Space</kbd> to copy many.</span>
    </div>
  </div>
</div>

<style>
  .what {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .what .mono {
    font-size: var(--fs-md);
    font-weight: 400;
  }
  .list {
    padding-left: var(--s3);
    padding-right: var(--s3);
    padding-bottom: 0;
  }
  .opt {
    display: grid;
    grid-template-columns: var(--h-ctl) 150px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: var(--h-row);
    text-align: left;
    padding: 0 var(--s3);
    border-radius: var(--radius);
  }
  .opt.on {
    background: var(--raised);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .last-used {
    font-size: var(--fs-xs);
    color: var(--muted);
    white-space: nowrap;
  }
  .opt kbd {
    justify-self: center;
    color: var(--text);
  }
  .label {
    white-space: nowrap;
  }
  .preview {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
