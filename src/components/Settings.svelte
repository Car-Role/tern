<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import type { Config, TabId } from "../lib/types";
  import { ALL_TABS } from "../lib/store.svelte";
  import { THEMES } from "../lib/themes";

  const tabNames: Record<TabId, string> = {
    changes: "Changes", files: "Files", prs: "Pull requests", issues: "Issues", history: "History", ci: "CI", tasks: "Tasks",
  };

  function toggleTab(sp: { tabs: TabId[] }, t: TabId) {
    const now = sp.tabs.length ? sp.tabs : ALL_TABS;
    const next = now.includes(t) ? now.filter((x) => x !== t) : ALL_TABS.filter((x) => x === t || now.includes(x));
    // At least one tab stays on.
    if (next.length) sp.tabs = next;
  }

  let draft = $state<Config>(
    structuredClone($state.snapshot(app.config)) ?? {
      roots: [], extra: [], hidden: [], pinned: [], scanDepth: 3, pollSeconds: 8,
      spaces: [], assign: {}, defaultSpace: null, activeSpace: null, tasks: {}, pinnedTasks: {},
    },
  );
  const colors = ["#6fa8ff", "#f0b54a", "#5fbf8b", "#c79bf2", "#e0716f", "#6fc9d6", "#a0a9b8"];
  const lines = (v: string) => v.split("\n");

  function addSpace() {
    const used = new Set(draft.spaces.map((x) => x.color));
    const color = colors.find((c) => !used.has(c)) ?? colors[0];
    draft.spaces = [...draft.spaces, { id: "s" + Date.now().toString(36), name: "New space", color, remotes: [], folders: [], tabs: [] }];
  }

  function removeSpace(id: string) {
    draft.spaces = draft.spaces.filter((x) => x.id !== id);
    if (draft.defaultSpace === id) draft.defaultSpace = null;
    if (draft.activeSpace === id) draft.activeSpace = null;
    draft.assign = Object.fromEntries(Object.entries(draft.assign).filter(([, v]) => v !== id));
  }

  function moveSpace(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= draft.spaces.length) return;
    const next = [...draft.spaces];
    [next[i], next[j]] = [next[j], next[i]];
    draft.spaces = next;
  }

  const assigned = $derived(Object.entries(draft.assign));
  let newRoot = $state("");
  let newRepo = $state("");
  let path = $state("");
  let saving = $state(false);

  onMount(async () => {
    path = await api.configPath();
  });

  const clean = (p: string) => p.trim().replace(/^"(.*)"$/, "$1").replace(/[\\/]+$/, "");

  function add(list: "roots" | "extra", value: string) {
    const v = clean(value);
    if (!v || draft[list].includes(v)) return;
    draft[list] = [...draft[list], v];
    if (list === "roots") newRoot = "";
    else newRepo = "";
  }

  function remove(list: "roots" | "extra" | "hidden" | "pinned", value: string) {
    draft[list] = draft[list].filter((x) => x !== value);
  }

  async function save() {
    saving = true;
    const snap = $state.snapshot(draft);
    for (const sp of snap.spaces) {
      sp.name = sp.name.trim() || "Untitled";
      sp.remotes = sp.remotes.map((x) => x.trim()).filter(Boolean);
      sp.folders = sp.folders.map((x) => clean(x)).filter(Boolean);
    }
    try {
      await app.saveConfig(snap);
      app.overlay = null;
      app.notify("Settings saved", "ok");
    } catch (e) {
      app.notify(`Could not save settings: ${e}`, "err");
    } finally {
      saving = false;
    }
  }
</script>

<div class="backdrop fx-fade" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="settings fx-fade" role="dialog" aria-label="Settings">
    <h2>Settings</h2>

    <section>
      <h3>Theme</h3>
      <p class="hint">Changes at once. It is saved on this computer.</p>
      <div class="themes" role="radiogroup" aria-label="Theme">
        {#each THEMES as t}
          <button class="theme" class:on={app.theme === t.id} role="radio" aria-checked={app.theme === t.id} onclick={() => app.setTheme(t.id)}>
            <span class="preview" style:background={t.swatch[0]}>
              <span class="bar" style:background={t.swatch[1]}></span>
              <span class="dots">
                {#each t.swatch.slice(2) as c}<i style:background={c}></i>{/each}
              </span>
            </span>
            <span class="tname">{t.name}</span>
            <span class="tnote">{t.note}</span>
          </button>
        {/each}
      </div>
    </section>

    <section>
      <h3>Spaces</h3>
      <p class="hint">
        A space shows only its own projects. Folder rules are checked first, then remote rules, top to bottom.
        The number is the <kbd>Ctrl Alt</kbd> shortcut for each space.
      </p>
      {#each draft.spaces as sp, i (sp.id)}
        <div class="space" style:--c={sp.color}>
          <div class="shead">
            <kbd>{i + 1}</kbd>
            <input class="sname" bind:value={sp.name} aria-label="Space name" />
            <div class="colors" role="radiogroup" aria-label="Color">
              {#each colors as c}
                <button
                  class="color"
                  class:on={sp.color === c}
                  style:background={c}
                  role="radio"
                  aria-checked={sp.color === c}
                  aria-label="Color {c}"
                  onclick={() => (sp.color = c)}
                ></button>
              {/each}
            </div>
            <span class="grow"></span>
            <button class="mini" onclick={() => moveSpace(i, -1)} disabled={i === 0} aria-label="Move up">↑</button>
            <button class="mini" onclick={() => moveSpace(i, 1)} disabled={i === draft.spaces.length - 1} aria-label="Move down">↓</button>
            <button class="mini danger" onclick={() => removeSpace(sp.id)}>Remove</button>
          </div>
          <div class="rules">
            <label>
              <span>Remote contains <span class="faint">(one per line)</span></span>
              <textarea
                rows="2"
                spellcheck="false"
                placeholder="dev.azure.com/your-org"
                bind:value={() => sp.remotes.join("\n"), (v) => (sp.remotes = lines(v))}
              ></textarea>
            </label>
            <label>
              <span>Folder starts with <span class="faint">(one per line)</span></span>
              <textarea
                rows="2"
                spellcheck="false"
                placeholder="C:\Work"
                bind:value={() => sp.folders.join("\n"), (v) => (sp.folders = lines(v))}
              ></textarea>
            </label>
          </div>
          <div class="tabs-row">
            <span>Tabs</span>
            {#each ALL_TABS as t}
              {@const on = !sp.tabs.length || sp.tabs.includes(t)}
              <button class="tabpill" class:on onclick={() => toggleTab(sp, t)} aria-pressed={on}>{tabNames[t]}</button>
            {/each}
          </div>
          <label class="default">
            <input type="radio" name="default-space" checked={draft.defaultSpace === sp.id} onchange={() => (draft.defaultSpace = sp.id)} />
            Also put projects that match no rule here
          </label>
        </div>
      {/each}
      <div class="srow">
        <button class="addspace" onclick={addSpace}>Add a space</button>
        {#if draft.defaultSpace}
          <button class="mini" onclick={() => (draft.defaultSpace = null)}>Leave unmatched projects out of every space</button>
        {/if}
      </div>
      {#if assigned.length}
        <p class="hint sub">Moved by hand (these win over the rules):</p>
        {#each assigned as [path, id]}
          <div class="item">
            <span class="mono">{path} <span class="faint">in {draft.spaces.find((x) => x.id === id)?.name ?? id}</span></span>
            <button onclick={() => { const { [path]: _, ...rest } = draft.assign; draft.assign = rest; }}>Use rules</button>
          </div>
        {/each}
      {/if}
    </section>

    <section>
      <h3>Folders to scan</h3>
      <p class="hint">Every git repo inside these folders shows up, down to {draft.scanDepth} levels deep.</p>
      {#each draft.roots as r}
        <div class="item"><span class="mono">{r}</span><button onclick={() => remove("roots", r)}>Remove</button></div>
      {/each}
      <form class="add" onsubmit={(e) => { e.preventDefault(); add("roots", newRoot); }}>
        <input bind:value={newRoot} placeholder="Paste a folder path, for example C:\Users\you\Code" spellcheck="false" />
        <button type="submit">Add</button>
      </form>
    </section>

    <section>
      <h3>Single repos</h3>
      <p class="hint">Add a repo that is outside the folders above.</p>
      {#each draft.extra as r}
        <div class="item"><span class="mono">{r}</span><button onclick={() => remove("extra", r)}>Remove</button></div>
      {/each}
      <form class="add" onsubmit={(e) => { e.preventDefault(); add("extra", newRepo); }}>
        <input bind:value={newRepo} placeholder="Paste a repo path" spellcheck="false" />
        <button type="submit">Add</button>
      </form>
    </section>

    {#if draft.hidden.length}
      <section>
        <h3>Hidden repos</h3>
        {#each draft.hidden as r}
          <div class="item"><span class="mono">{r}</span><button onclick={() => remove("hidden", r)}>Show again</button></div>
        {/each}
      </section>
    {/if}

    {#if draft.pinned.length}
      <section>
        <h3>Pinned repos</h3>
        {#each draft.pinned as r}
          <div class="item"><span class="mono">{r}</span><button onclick={() => remove("pinned", r)}>Unpin</button></div>
        {/each}
      </section>
    {/if}

    <section class="nums">
      <label>Scan depth <input type="number" min="1" max="8" bind:value={draft.scanDepth} /></label>
      <label>Refresh every <input type="number" min="3" max="300" bind:value={draft.pollSeconds} /> seconds</label>
    </section>

    <footer>
      <span class="faint mono path" title={path}>{path}</span>
      <button class="btn" onclick={() => (app.overlay = null)}>Cancel</button>
      <button class="btn primary" disabled={saving} onclick={save}>{saving ? "Saving…" : "Save"}</button>
    </footer>
  </div>
</div>

<style>
  /* Opened with the mouse, now and then: a short entrance. Closing stays instant. */
  .backdrop {
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .settings {
    animation: tern-dialog-in 200ms var(--ease-out);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    display: grid;
    place-items: center;
    z-index: 10;
    padding: 16px;
  }
  .settings {
    width: min(760px, 100%);
    max-height: 86vh;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    padding: 20px 24px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    user-select: text;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 16px;
  }
  h3 {
    margin: 0 0 2px;
    font-size: 13px;
  }
  section {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .hint {
    margin: 0 0 8px;
    color: var(--muted);
    font-size: 12px;
  }
  .item {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 0;
  }
  .item span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item button {
    color: var(--muted);
    font-size: 12px;
    flex-shrink: 0;
  }
  .item button:hover {
    color: var(--del);
  }
  .add {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  input {
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 5px 8px;
    outline: none;
  }
  input:focus {
    border-color: var(--amber);
  }
  .add input {
    flex: 1;
  }
  .add button {
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
    margin-top: 6px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: 8px;
    text-align: left;
  }
  .theme:hover {
    border-color: var(--line-strong);
  }
  .theme.on {
    border-color: var(--amber);
    box-shadow: 0 0 0 1px var(--amber);
  }
  .preview {
    position: relative;
    height: 44px;
    border-radius: 5px;
    overflow: hidden;
    border: 1px solid rgba(128, 128, 128, 0.25);
  }
  .bar {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 32%;
  }
  .dots {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: flex;
    gap: 4px;
  }
  .dots i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .tname {
    font-weight: 600;
    font-size: 12.5px;
  }
  .tnote {
    font-size: 11px;
    color: var(--faint);
  }
  .space {
    border: 1px solid var(--line);
    border-left: 3px solid var(--c);
    border-radius: var(--radius);
    padding: 8px 10px;
    margin: 8px 0;
  }
  .shead {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sname {
    width: 160px;
    font-weight: 600;
  }
  .colors {
    display: flex;
    gap: 4px;
  }
  .color {
    width: 14px;
    height: 14px;
    border-radius: 4px;
    opacity: 0.55;
  }
  .color.on {
    opacity: 1;
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .grow {
    flex: 1;
  }
  .mini {
    font-size: 12px;
    color: var(--muted);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .mini:hover:not(:disabled) {
    color: var(--text);
    background: var(--hover);
  }
  .mini:disabled {
    opacity: 0.35;
  }
  .mini.danger:hover {
    color: var(--del);
  }
  .rules {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 8px;
  }
  .rules label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--muted);
  }
  textarea {
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 5px 8px;
    outline: none;
    color: var(--text);
    font: 12px/1.4 var(--mono);
    resize: vertical;
  }
  textarea:focus {
    border-color: var(--amber);
  }
  .tabs-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .tabs-row > span {
    margin-right: 4px;
  }
  .tabpill {
    font-size: 11.5px;
    padding: 1px 9px;
    border-radius: 10px;
    border: 1px solid var(--line);
    color: var(--faint);
  }
  .tabpill.on {
    color: var(--text);
    border-color: var(--c);
    background: var(--raised);
  }
  .default {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .default input {
    accent-color: var(--amber);
    margin: 0;
  }
  .srow {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .addspace {
    font-size: 12px;
    padding: 4px 10px;
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .addspace:hover {
    color: var(--text);
  }
  .hint.sub {
    margin-top: 12px;
  }
  .nums {
    display: flex;
    gap: 24px;
    border-bottom: 0;
  }
  .nums input {
    width: 64px;
    margin: 0 6px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-top: 12px;
  }
  .path {
    flex: 1;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn {
    padding: 5px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--on-accent);
    font-weight: 600;
  }
</style>
