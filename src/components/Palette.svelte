<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app, type PaletteMode } from "../lib/store.svelte";
  import { commands, type Cmd } from "../lib/commands";
  import { rank } from "../lib/fuzzy";
  import { copyText } from "../lib/api";
  import { formatPaths, baseName, dirName } from "../lib/format";
  import type { RepoEntry, Task } from "../lib/types";
  import { THEMES } from "../lib/themes";
  import { hover } from "../lib/hover";

  type Item =
    | { kind: "project"; id: string; label: string; detail: string; repo: RepoEntry; hits: number[] }
    | { kind: "file"; id: string; label: string; detail: string; path: string; hits: number[] }
    | { kind: "cmd"; id: string; label: string; detail: string; cmd: Cmd; hits: number[] }
    | { kind: "task"; id: string; label: string; detail: string; task: Task | null; hits: number[] };

  let input: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();
  let query = $state("");
  let active = $state(0);
  let files = $state<string[]>([]);

  const modes: { id: PaletteMode; label: string; keys: string }[] = [
    { id: "projects", label: "Projects", keys: "Ctrl K" },
    { id: "files", label: "Files", keys: "Ctrl P" },
    { id: "commands", label: "Commands", keys: "Ctrl Shift P" },
    { id: "tasks", label: "Tasks", keys: "Ctrl Shift B" },
  ];

  const mode = $derived<PaletteMode>(query.startsWith(">") ? "commands" : app.paletteMode);

  /** Reads "src/app.ts:42", "src\app.ts", "@src/app.ts", or a full path inside the open project. */
  const parsed = $derived.by(() => {
    let q = query.trim().replace(/^@/, "");
    let line: number | undefined;
    const m = q.match(/^(.*?):(\d+)(?:[-:]\d+)?$/);
    if (m) {
      q = m[1];
      line = Number(m[2]);
    }
    q = q.replace(/\\/g, "/");
    const root = app.current?.replace(/\\/g, "/").toLowerCase();
    if (root && q.toLowerCase().startsWith(root + "/")) q = q.slice(root.length + 1);
    return { q, line };
  });

  const items = $derived.by((): Item[] => {
    if (mode === "commands") {
      const q = query.replace(/^>\s*/, "");
      const spaceCmds: Cmd[] = app.spaces.map((sp, i) => ({
        id: "space.go." + sp.id,
        group: "Spaces",
        label: `Switch to ${sp.name}`,
        keys: i < 9 ? `Ctrl+Alt+${i + 1}` : undefined,
        run: () => app.switchSpace(sp.id),
      }));
      const themeCmds: Cmd[] = THEMES.map((t) => ({
        id: "theme." + t.id,
        group: "View",
        label: `Theme: ${t.name}${app.theme === t.id ? " (current)" : ""}`,
        run: () => app.setTheme(t.id),
      }));
      const all = [...spaceCmds, ...commands.filter((c) => !c.hidden), ...themeCmds];
      return rank(all, q, (c) => `${c.group} ${c.label}`, 50).map(({ item, match }) => ({
        kind: "cmd",
        id: item.id,
        label: item.label,
        detail: item.keys ?? "",
        cmd: item,
        hits: match.hits.map((h) => h - item.group.length - 1).filter((h) => h >= 0),
      }));
    }
    if (mode === "tasks") {
      const list = app.current ? (app.tasks[app.current] ?? []) : [];
      const q = query.trim();
      const found: Item[] = rank(list, q, (t) => `${t.name} ${t.cwd}`, 50).map(({ item, match }) => ({
        kind: "task",
        id: item.id,
        label: item.name,
        detail: item.cwd ? `${item.cwd}: ${item.command}` : item.command,
        task: item,
        hits: match.hits.filter((h) => h < item.name.length),
      }));
      // Anything typed can also run as a command.
      if (q && app.current) found.push({ kind: "task", id: "run:" + q, label: `Run "${q}"`, detail: "as a command in this project", task: null, hits: [] });
      return found;
    }
    if (mode === "projects") {
      return rank(app.ordered, query.trim(), (r) => r.name, 50).map(({ item, match }) => ({
        kind: "project",
        id: item.path,
        label: item.name,
        detail: app.sums[item.path]?.branch ?? "",
        repo: item,
        hits: match.hits,
      }));
    }
    const exact = files.includes(parsed.q) ? [parsed.q] : [];
    const ranked = rank(files, parsed.q, (f) => f, 80).filter((r) => r.item !== parsed.q);
    return [
      ...exact.map((f) => ({ item: f, match: { hits: [] as number[], score: 0 } })),
      ...ranked,
    ].map(({ item, match }) => {
      const cut = item.length - baseName(item).length;
      return {
        kind: "file" as const,
        id: item,
        label: baseName(item),
        detail: dirName(item),
        path: item,
        hits: match.hits.filter((h) => h >= cut).map((h) => h - cut),
      };
    });
  });

  $effect(() => {
    items;
    active = 0;
  });

  $effect(() => {
    if (mode === "files" && app.current) app.loadFiles(app.current).then((f) => (files = f));
    if (mode === "tasks" && app.current) app.loadTasks(app.current);
  });

  onMount(() => {
    input?.focus();
  });

  function choose(item: Item | undefined, alt = false) {
    if (!item) return;
    app.overlay = null;
    if (item.kind === "project") {
      if (alt) copyText(item.repo.path).then(() => app.notify(`Copied ${item.repo.path}`, "ok"));
      else app.select(item.repo.path);
    } else if (item.kind === "file") {
      if (alt && app.current) {
        const text = formatPaths("mention", app.current, [item.path], null);
        copyText(text).then(() => app.notify(`Copied ${text}`, "ok"));
      } else app.openFile(item.path, parsed.line);
    } else if (item.kind === "task") {
      if (item.task) app.runTask(item.task);
      else app.runCommand(query);
    } else {
      item.cmd.run();
    }
  }

  async function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") active = Math.min(items.length - 1, active + 1);
    else if (e.key === "ArrowUp") active = Math.max(0, active - 1);
    else if (e.key === "PageDown") active = Math.min(items.length - 1, active + 10);
    else if (e.key === "PageUp") active = Math.max(0, active - 10);
    else if (e.key === "Enter") choose(items[active], e.shiftKey);
    else if (e.key === "Tab") {
      const i = modes.findIndex((m) => m.id === app.paletteMode);
      app.paletteMode = modes[(i + (e.shiftKey ? modes.length - 1 : 1)) % modes.length].id;
      if (query.startsWith(">")) query = query.slice(1);
    } else return;
    e.preventDefault();
    e.stopPropagation();
    await tick();
    listEl?.querySelector(".on")?.scrollIntoView({ block: "nearest" });
  }

  function marks(text: string, hits: number[]) {
    const set = new Set(hits);
    const out: { t: string; m: boolean }[] = [];
    for (let i = 0; i < text.length; i++) {
      const m = set.has(i);
      const last = out[out.length - 1];
      if (last && last.m === m) last.t += text[i];
      else out.push({ t: text[i], m });
    }
    return out;
  }

  const placeholder = $derived(
    {
      projects: "Go to a project",
      files: "Go to a file. Paste a path like src/app.ts:42",
      commands: "Run a Tern command",
      tasks: "Run a task, or type any command",
    }[mode],
  );
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="dialog palette" role="dialog" aria-modal="true" aria-label="Go to or run">
    <div class="head">
      <div class="tabs" role="tablist">
        {#each modes as m}
          <button role="tab" use:hover={() => ({ title: m.label, hint: m.keys })} class:on={mode === m.id} aria-selected={mode === m.id} onclick={() => { app.paletteMode = m.id; input?.focus(); }}>
            {m.label}
          </button>
        {/each}
      </div>
      <span class="grow"></span>
      <span class="hint"><kbd>Tab</kbd> switch</span>
    </div>
    <div class="query">
      <input class="field" bind:this={input} bind:value={query} onkeydown={onKey} {placeholder} spellcheck="false" aria-label={placeholder} />
    </div>
    <div class="results" bind:this={listEl} role="listbox">
      {#if (mode === "files" || mode === "tasks") && !app.current}
        <p class="empty">Open a project first. Press <kbd>Tab</kbd> to find a project.</p>
      {:else if !items.length}
        <p class="empty">No matches.</p>
      {/if}
      {#each items as item, i (item.kind + item.id)}
        <button class="item" class:on={i === active} role="option" aria-selected={i === active} onmousemove={() => (active = i)} onclick={() => choose(item)}>
          <span class="label">
            {#each marks(item.label, item.hits) as seg}{#if seg.m}<mark>{seg.t}</mark>{:else}{seg.t}{/if}{/each}
          </span>
          {#if item.kind === "cmd"}
            {#if item.detail}<kbd>{item.detail.replaceAll("+", " ")}</kbd>{/if}
          {:else}
            <span class="detail mono">{item.detail}</span>
          {/if}
        </button>
      {/each}
    </div>
    <div class="dialog-foot foot">
      <span class="hint"><kbd>↑</kbd> <kbd>↓</kbd> move · <kbd>Enter</kbd> open{#if mode === "files"} · <kbd>Shift Enter</kbd> copy as @ mention{/if}{#if mode === "projects"} · <kbd>Shift Enter</kbd> copy path{/if} · <kbd>Esc</kbd> close</span>
    </div>
  </div>
</div>

<style>
  .palette {
    width: min(640px, 100%);
    max-height: 70vh;
  }
  .head {
    display: flex;
    align-items: stretch;
    gap: var(--s3);
    height: 34px;
    padding: 0 var(--s4);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .head .hint {
    align-self: center;
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .query {
    display: flex;
    padding: var(--s3) var(--s4);
    flex-shrink: 0;
  }
  /* The palette's one input is its whole purpose, so it is a bar-height field. */
  .query .field {
    flex: 1;
    height: var(--h-bar);
    padding: 0 var(--s4);
    font-size: var(--fs-lg);
  }
  .results {
    overflow-y: auto;
    min-height: 0;
    padding: 0 var(--s2) var(--s2);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: var(--h-row);
    text-align: left;
    padding: 0 var(--s3);
    border-radius: var(--radius);
    white-space: nowrap;
  }
  .item.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .label {
    color: var(--text);
    flex-shrink: 0;
    max-width: 60%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
    flex: 1;
  }
  .item kbd {
    margin-left: auto;
  }
  mark {
    background: none;
    color: var(--accent);
    font-weight: 650;
  }
  .empty {
    color: var(--muted);
    padding: var(--s3);
    margin: 0;
  }
  .foot {
    flex-shrink: 0;
  }
</style>
