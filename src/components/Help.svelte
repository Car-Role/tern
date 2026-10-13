<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { commands } from "../lib/commands";

  const groups = ["Go", "Spaces", "Copy", "Git", "Tasks", "Open", "View"] as const;
  const listKeys = [
    ["↑ ↓ or J K", "Move through the list. Changes and files open as you move."],
    ["Enter", "Open a pull request, commit, or folder"],
    ["Space", "Select a file (Ctrl+click and Shift+click also work)"],
    ["S", "Stage or unstage the file (or the selected files)"],
    ["C", "Copy menu"],
    ["Ctrl Enter", "Go to the commit box, then commit"],
    ["Ctrl Shift Enter", "In the commit box: commit and push"],
    ["← →", "Close or open a folder"],
    ["Enter (Tasks tab)", "Run the task, or show it if it is running"],
    ["R / X (Tasks tab)", "Restart / stop the task"],
    ["Backspace", "Leave a pull request or commit"],
    ["Ctrl F", "Search inside the open file"],
    ["Ctrl S", "Save, while in quick edit"],
    ["Esc", "Close, clear selection, or go back"],
    ["Ctrl 1 … 6", "The 1st to 6th tab of this project (each space picks its tabs)"],
    ["Right-click", "Copy menus on projects, files, commits, pull requests, issues, tasks, and code"],
    ["Mouse back / forward", "Go back or forward (also Alt ← and Alt →)"],
    ["Ctrl+click / Shift+click", "Pick several projects in the project list, then fetch, pull, push, hide, or move them together"],
  ];
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="help" role="dialog" aria-label="Keyboard shortcuts">
    <h2>Keyboard shortcuts</h2>
    <div class="cols">
      <section>
        <h3>In the list</h3>
        {#each listKeys as [k, what]}
          <div class="row"><span>{what}</span><kbd>{k}</kbd></div>
        {/each}
      </section>
      {#each groups as g}
        {@const cmds = commands.filter((c) => c.group === g && c.keys && !c.hidden)}
        <section>
          <h3>{g}</h3>
          {#each cmds as c}
            <div class="row"><span>{c.label}</span><kbd>{c.keys?.replaceAll("+", " ")}</kbd></div>
          {/each}
          {#if g === "Spaces"}<div class="row"><span>Switch to space 1 to 9</span><kbd>Ctrl Alt 1–9</kbd></div>{/if}
        </section>
      {/each}
    </div>
    <section class="legend">
      <h3>What the marks in the project list mean</h3>
      <div class="marks">
        <span><i class="d clean"></i>Clean: no local changes</span>
        <span><i class="d dirty"></i>Changes that are not committed</span>
        <span><i class="d red"></i>Merge conflict, or Tern could not read the repo</span>
        <span><i class="d unknown"></i>Not read yet</span>
        <span><b class="green">working</b>Files changed in the last 2 minutes</span>
        <span><i class="d blue small"></i>Changed since you last looked</span>
        <span><b class="blue">↓3</b>Commits to pull</span>
        <span><b class="green">↑2</b>Commits to push</span>
        <span><b class="amber">5</b>Changed files</span>
        <span><b class="green">▶</b>A task is running</span>
      </div>
    </section>
    <p class="faint">Every action, with or without a shortcut, is in <kbd>Ctrl Shift P</kbd>.</p>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    display: grid;
    place-items: center;
    z-index: 10;
    padding: 16px;
  }
  .help {
    width: min(900px, 100%);
    max-height: 86vh;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    padding: 20px 24px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
  }
  h2 {
    margin: 0 0 16px;
    font-size: 16px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 12px;
    color: var(--amber);
    font-weight: 600;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 18px 28px;
  }
  .row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 3px 0;
    color: var(--muted);
  }
  .legend {
    margin-top: 20px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .marks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 6px 28px;
    color: var(--muted);
  }
  .marks span {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .marks b {
    font: 600 11px var(--mono);
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
    background: var(--amber);
    border-color: var(--amber);
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
    color: var(--amber);
  }
  p {
    margin: 18px 0 0;
    font-size: 12px;
  }
</style>
