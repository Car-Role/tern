<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { commands } from "../lib/commands";

  type Row = { label: string; keys: string };

  const listKeys: Row[] = [
    { keys: "↑ ↓ or J K", label: "Move through the list. Changes and files open as you move." },
    { keys: "Enter", label: "Open a pull request, commit, or folder" },
    { keys: "Space", label: "Select a file (Ctrl click and Shift click also work)" },
    { keys: "S", label: "Stage or unstage the file (or the selected files)" },
    { keys: "C", label: "Copy menu" },
    { keys: "Ctrl Enter", label: "Go to the commit box, then commit" },
    { keys: "Ctrl Shift Enter", label: "In the commit box: commit and push" },
    { keys: "← →", label: "Close or open a folder" },
    { keys: "Enter (Tasks tab)", label: "Run the task, or show it if it is running" },
    { keys: "R or X (Tasks tab)", label: "Restart or stop the task" },
    { keys: "Backspace", label: "Leave a pull request or commit" },
    { keys: "Ctrl F", label: "Search inside the open file" },
    { keys: "Ctrl S", label: "Save, while in quick edit" },
    { keys: "Esc", label: "Close, clear selection, or go back" },
    { keys: "Ctrl 1 … 7", label: "The 1st to 7th tab of this project (each space picks its tabs)" },
    { keys: "Right-click", label: "Menus on projects, files, commits, pull requests, issues, tasks, and code" },
    { keys: "Mouse back or forward", label: "Go back or forward (also Alt ← and Alt →)" },
    { keys: "Ctrl click or Shift click", label: "Pick several projects in the project list, then fetch, pull, push, hide, or move them together" },
  ];

  const groupNames = ["Go", "Project", "Spaces", "Copy", "Git", "Tasks", "Open", "View"] as const;
  const groups: { name: string; rows: Row[] }[] = [
    { name: "In the list", rows: listKeys },
    ...groupNames.map((g) => ({
      name: g as string,
      rows: [
        ...commands
          .filter((c) => c.group === g && c.keys && !c.hidden)
          .map((c) => ({ label: c.label, keys: (c.keys ?? "").replaceAll("+", " ") })),
        ...(g === "Spaces" ? [{ label: "Switch to space 1 to 9", keys: "Ctrl Alt 1–9" }] : []),
      ],
    })),
  ];

  let filter = $state("");
  let input: HTMLInputElement | undefined = $state();

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return groups.filter((g) => g.rows.length);
    return groups
      .map((g) => ({ name: g.name, rows: g.rows.filter((r) => r.label.toLowerCase().includes(q) || r.keys.toLowerCase().includes(q)) }))
      .filter((g) => g.rows.length);
  });

  onMount(() => input?.focus());
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="dialog lg" role="dialog" aria-modal="true" aria-labelledby="help-title">
    <div class="dialog-head">
      <h2 id="help-title">Keyboard shortcuts</h2>
      <span class="grow"></span>
      <input class="field filter" placeholder="Filter shortcuts" aria-label="Filter shortcuts" spellcheck="false" bind:this={input} bind:value={filter} />
    </div>
    <div class="dialog-body">
      {#if shown.length}
        <div class="cols">
          {#each shown as g (g.name)}
            <section>
              <h3 class="section-label">{g.name}</h3>
              {#each g.rows as r}
                <div class="row"><span>{r.label}</span><kbd>{r.keys}</kbd></div>
              {/each}
            </section>
          {/each}
        </div>
      {:else}
        <div class="state">
          <p>No shortcut matches "{filter.trim()}".</p>
          <div class="actions"><button class="btn sm" onclick={() => { filter = ""; input?.focus(); }}>Clear filter</button></div>
        </div>
      {/if}
      {#if !filter.trim()}
        <section class="legend">
          <h3 class="section-label">What the marks in the project list mean</h3>
          <div class="marks">
            <span><i class="d clean"></i>Clean: no local changes</span>
            <span><i class="d dirty"></i>Changes that are not committed</span>
            <span><i class="d red"></i>Merge conflict, or Tern could not read the project</span>
            <span><i class="d unknown"></i>Not read yet</span>
            <span><b class="green">working</b>Files changed in the last 2 minutes</span>
            <span><i class="d blue small"></i>Changed since you last looked</span>
            <span><b class="blue">↓3</b>Commits to pull</span>
            <span><b class="green">↑2</b>Commits to push</span>
            <span><b class="amber">5</b>Changed files</span>
            <span><b class="green">▶</b>A task is running</span>
          </div>
        </section>
      {/if}
    </div>
    <div class="dialog-foot">
      <span class="hint">Every action, with or without a shortcut, is in <kbd>Ctrl Shift P</kbd>.</span>
    </div>
  </div>
</div>

<style>
  .filter {
    width: 220px;
  }
  .section-label {
    margin: 0 0 var(--s1);
    padding: 0;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--s5) var(--s6);
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--s4);
    padding: var(--s1) 0;
    color: var(--muted);
  }
  .row kbd {
    flex-shrink: 0;
  }
  .legend {
    margin-top: var(--s5);
    padding-top: var(--s4);
    border-top: 1px solid var(--line);
  }
  .marks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--s2) var(--s6);
    color: var(--muted);
  }
  .marks span {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .marks b {
    font: 600 var(--fs-xs) var(--mono);
    min-width: 50px;
    text-align: center;
  }
  .d {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--faint);
    margin: 0 21px;
    flex-shrink: 0;
  }
  .d.dirty {
    background: var(--accent);
    border-color: var(--accent);
  }
  .d.red {
    background: var(--del);
    border-color: var(--del);
  }
  .d.unknown {
    border-style: dashed;
  }
  .d.blue {
    background: var(--blue);
    border-color: var(--blue);
  }
  .d.small {
    width: 6px;
    height: 6px;
    margin: 0 22px;
  }
  .green {
    color: var(--add);
  }
  .blue {
    color: var(--blue);
  }
  .amber {
    color: var(--accent);
  }
</style>
