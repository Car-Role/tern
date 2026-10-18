<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api, inTauri, openUrl } from "../lib/api";
  import type { Config, TabId } from "../lib/types";
  import { ALL_TABS } from "../lib/store.svelte";
  import { THEMES } from "../lib/themes";
  import Icon from "./Icon.svelte";

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
    confirmingClose = false;
    saving = true;
    const snap = $state.snapshot(draft);
    for (const sp of snap.spaces) {
      sp.name = sp.name.trim() || "Untitled";
      sp.remotes = sp.remotes.map((x) => x.trim()).filter(Boolean);
      sp.folders = sp.folders.map((x) => clean(x)).filter(Boolean);
    }
    try {
      await app.saveConfig(snap);
      saved = true;
      app.overlay = null;
      app.notify("Settings saved", "ok");
    } catch (e) {
      app.notify(`Could not save settings: ${e}`, "err");
    } finally {
      saving = false;
    }
  }

  // Sections: one shows at a time, picked in the nav on the left.
  type SectionId = "appearance" | "spaces" | "folders" | "projects" | "updates" | "advanced";
  const sections: { id: SectionId; label: string }[] = [
    { id: "appearance", label: "Appearance" },
    { id: "spaces", label: "Spaces" },
    { id: "folders", label: "Folders" },
    { id: "projects", label: "Projects" },
    { id: "updates", label: "Updates" },
    { id: "advanced", label: "Advanced" },
  ];

  /** Another screen can ask Settings to open on a section by setting this key just before opening it. */
  function initialSection(): SectionId {
    try {
      const want = localStorage.getItem("tern.settingsSection");
      localStorage.removeItem("tern.settingsSection");
      const hit = sections.find((x) => x.id === want);
      if (hit) return hit.id;
    } catch {
      /* Storage can be blocked. The first section is fine. */
    }
    return "appearance";
  }
  let section = $state<SectionId>(initialSection());
  let content: HTMLDivElement | undefined = $state();

  function show(id: SectionId) {
    section = id;
    content?.scrollTo({ top: 0 });
  }

  // The theme previews at once. Save keeps it; Cancel puts the old one back.
  const startTheme = app.theme;
  const startDraft = JSON.stringify($state.snapshot(draft));
  const dirty = $derived(app.theme !== startTheme || JSON.stringify($state.snapshot(draft)) !== startDraft);
  let saved = false;
  let confirmingClose = $state(false);

  /** Closes, or first asks to discard unsaved changes. */
  function requestClose() {
    if (dirty && !confirmingClose) {
      confirmingClose = true;
      return;
    }
    discard();
  }

  function discard() {
    confirmingClose = false;
    app.overlay = null;
  }

  // Lets the shell ask before Esc closes the dialog. It takes effect once the store has `overlayGuard`.
  const guarded = app as typeof app & { overlayGuard?: (() => boolean) | null };
  guarded.overlayGuard = () => {
    if (!dirty || confirmingClose) return false;
    confirmingClose = true;
    return true;
  };

  onDestroy(() => {
    guarded.overlayGuard = null;
    if (!saved && app.theme !== startTheme) app.setTheme(startTheme);
  });

  async function browse(list: "roots" | "extra") {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") add(list, picked);
    } catch (e) {
      app.notify(`Could not open the folder picker: ${e}`, "err");
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.code === "KeyS") {
      e.preventDefault();
      if (!saving) save();
    }
  }
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && requestClose()}>
  <div class="dialog xl settings" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" onkeydown={onKey}>
    <div class="dialog-head">
      <h2 id="settings-title">Settings</h2>
    </div>

    <div class="layout">
      <nav class="nav" aria-label="Settings sections">
        {#each sections as x}
          <button class="navitem" class:on={section === x.id} aria-current={section === x.id ? "page" : undefined} onclick={() => show(x.id)}>
            <span>{x.label}</span>
            {#if x.id === "spaces" && draft.spaces.length}<span class="badge">{draft.spaces.length}</span>{/if}
            {#if x.id === "folders" && draft.roots.length + draft.extra.length}<span class="badge">{draft.roots.length + draft.extra.length}</span>{/if}
            {#if x.id === "updates" && app.update}<span class="badge accent">1</span>{/if}
          </button>
        {/each}
      </nav>

      <div class="content" bind:this={content}>
        {#if section === "appearance"}
          <section>
            <h3>Theme</h3>
            <p class="hint">Changes at once so you can see it. Save keeps it, Cancel puts the old one back.</p>
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
        {:else if section === "spaces"}
          <section>
            <h3>Spaces</h3>
            <p class="hint">
              A space shows only its own projects. Folder rules are checked first, then remote rules, top to bottom.
              The number is the <kbd>Ctrl Alt</kbd> shortcut for each space.
            </p>
            {#if !draft.spaces.length}
              <p class="none">No spaces yet. Every project shows in one list.</p>
            {/if}
            {#each draft.spaces as sp, i (sp.id)}
              <div class="space" style:--c={sp.color}>
                <div class="shead">
                  <kbd>{i + 1}</kbd>
                  <input class="field sname" bind:value={sp.name} aria-label="Space name" />
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
                  <button class="icon-btn sm" onclick={() => moveSpace(i, -1)} disabled={i === 0} aria-label="Move up">↑</button>
                  <button class="icon-btn sm" onclick={() => moveSpace(i, 1)} disabled={i === draft.spaces.length - 1} aria-label="Move down">↓</button>
                  <button class="btn ghost sm danger" onclick={() => removeSpace(sp.id)}>Remove</button>
                </div>
                <div class="rules">
                  <label>
                    <span>Remote contains <span class="faint">(one per line)</span></span>
                    <textarea
                      class="field mono"
                      rows="2"
                      spellcheck="false"
                      placeholder="dev.azure.com/your-org"
                      bind:value={() => sp.remotes.join("\n"), (v) => (sp.remotes = lines(v))}
                    ></textarea>
                  </label>
                  <label>
                    <span>Folder starts with <span class="faint">(one per line)</span></span>
                    <textarea
                      class="field mono"
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
                    <button class="btn sm tabtoggle" class:on class:ghost={!on} onclick={() => toggleTab(sp, t)} aria-pressed={on}>
                      {#if on}<Icon name="check" size={12} stroke={2} />{/if}{tabNames[t]}
                    </button>
                  {/each}
                </div>
                <label class="check default">
                  <input type="radio" name="default-space" checked={draft.defaultSpace === sp.id} onchange={() => (draft.defaultSpace = sp.id)} />
                  Also put projects that match no rule here
                </label>
              </div>
            {/each}
            <div class="srow">
              <button class="btn" onclick={addSpace}>Add a space</button>
              {#if draft.defaultSpace}
                <button class="btn ghost" onclick={() => (draft.defaultSpace = null)}>Leave unmatched projects out of every space</button>
              {/if}
            </div>
            {#if assigned.length}
              <h3 class="sub">Moved by hand</h3>
              <p class="hint">These win over the rules.</p>
              {#each assigned as [path, id]}
                <div class="item">
                  <span class="mono">{path} <span class="faint">in {draft.spaces.find((x) => x.id === id)?.name ?? id}</span></span>
                  <button class="btn ghost sm" onclick={() => { const { [path]: _, ...rest } = draft.assign; draft.assign = rest; }}>Use rules</button>
                </div>
              {/each}
            {/if}
          </section>
        {:else if section === "folders"}
          <section>
            <h3>Folders to scan</h3>
            <p class="hint">Every git project inside these folders shows up, down to {draft.scanDepth} {draft.scanDepth === 1 ? "level" : "levels"} deep.</p>
            {#each draft.roots as r}
              <div class="item"><span class="mono">{r}</span><button class="btn ghost sm" onclick={() => remove("roots", r)}>Remove</button></div>
            {/each}
            <form class="add" onsubmit={(e) => { e.preventDefault(); add("roots", newRoot); }}>
              <input class="field grow" bind:value={newRoot} placeholder="Paste a folder path, for example C:\Users\you\Code" spellcheck="false" aria-label="Folder to scan" />
              {#if inTauri}<button type="button" class="btn" onclick={() => browse("roots")}>Browse…</button>{/if}
              <button type="submit" class="btn" disabled={!newRoot.trim()}>Add</button>
            </form>
          </section>
          <section>
            <h3>Single projects</h3>
            <p class="hint">Add a project that is outside the folders above.</p>
            {#each draft.extra as r}
              <div class="item"><span class="mono">{r}</span><button class="btn ghost sm" onclick={() => remove("extra", r)}>Remove</button></div>
            {/each}
            <form class="add" onsubmit={(e) => { e.preventDefault(); add("extra", newRepo); }}>
              <input class="field grow" bind:value={newRepo} placeholder="Paste a project path" spellcheck="false" aria-label="Single project" />
              {#if inTauri}<button type="button" class="btn" onclick={() => browse("extra")}>Browse…</button>{/if}
              <button type="submit" class="btn" disabled={!newRepo.trim()}>Add</button>
            </form>
          </section>
        {:else if section === "projects"}
          <section>
            <h3>Pinned projects</h3>
            <p class="hint">Pinned projects stay at the top of the list.</p>
            {#each draft.pinned as r}
              <div class="item"><span class="mono">{r}</span><button class="btn ghost sm" onclick={() => remove("pinned", r)}>Unpin</button></div>
            {:else}
              <p class="none">No pinned projects. Right-click a project and choose Pin.</p>
            {/each}
          </section>
          <section>
            <h3>Hidden projects</h3>
            <p class="hint">Hidden projects do not show in the list.</p>
            {#each draft.hidden as r}
              <div class="item"><span class="mono">{r}</span><button class="btn ghost sm" onclick={() => remove("hidden", r)}>Show again</button></div>
            {:else}
              <p class="none">No hidden projects.</p>
            {/each}
          </section>
        {:else if section === "updates"}
          <section class="about">
            <div class="aboutrow">
              <span class="mark" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="16" height="16"><path d="M3.5 9.5c3 0 5.5 1.5 8.5 5 3-3.5 5.5-5 8.5-5" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
              </span>
              <div class="aboutname">
                <b>Tern {app.version}</b>
                <span class="faint">
                  {#if app.update}Version {app.update.info.version} is available.
                  {:else if app.updateCheck.state === "checking"}Checking for updates…
                  {:else if app.updateCheck.state === "current"}Up to date.
                  {:else if app.updateCheck.state === "failed"}Could not check: {app.updateCheck.error}
                  {:else}Source-available under the Business Source License 1.1.{/if}
                </span>
              </div>
              {#if app.update}
                <button class="btn primary" onclick={() => app.installUpdate()} disabled={app.update.state === "downloading"}>
                  {app.update.state === "downloading" ? `Updating… ${Math.round(app.update.progress * 100)}%` : `Update to ${app.update.info.version}`}
                </button>
              {:else}
                <button class="btn" onclick={() => app.checkForUpdate(true)} disabled={app.updateCheck.state === "checking"}>Check for updates</button>
              {/if}
            </div>
            <p class="links">
              <button class="link" onclick={() => openUrl("https://car-role.github.io/tern/")}>Website</button>
              <button class="link" onclick={() => openUrl("https://github.com/Car-Role/tern/releases")}>What's new</button>
              <button class="link" onclick={() => openUrl("https://github.com/Car-Role/tern/blob/main/docs/guide.md")}>Guide</button>
              <button class="link" onclick={() => openUrl("https://github.com/Car-Role/tern/issues/new?template=bug.yml")}>Report a problem</button>
              <button class="link" onclick={() => openUrl("https://github.com/Car-Role/tern/blob/main/LICENSE")}>License</button>
            </p>
          </section>
          <section>
            <h3>Problems</h3>
            <p class="hint">Something went wrong? Copy the diagnostics and paste them into your bug report. They hold versions, counts, recent errors, and the end of Tern's log. Check them for anything private before you share them.</p>
            <div class="diag">
              <button class="btn" onclick={() => app.copyDiagnostics()}>Copy diagnostics</button>
              <button class="btn ghost" onclick={() => app.openLogFolder()}>Show the log folder</button>
            </div>
          </section>
        {:else}
          <section>
            <h3>Scanning</h3>
            <div class="nums">
              <label class="num">Scan depth <input class="field" type="number" min="1" max="8" bind:value={draft.scanDepth} /> <span class="faint">levels</span></label>
              <label class="num">Refresh every <input class="field" type="number" min="3" max="300" bind:value={draft.pollSeconds} /> <span class="faint">seconds</span></label>
            </div>
          </section>
          <section>
            <h3>Settings file</h3>
            <p class="hint">Everything in Settings except the theme is saved in this file.</p>
            <p class="mono path">{path || "Loading…"}</p>
          </section>
        {/if}
      </div>
    </div>

    <div class="dialog-foot foot">
      {#if confirmingClose}
        <span class="discard">Discard changes?</span>
        <button class="btn" onclick={() => (confirmingClose = false)}>Keep editing</button>
        <button class="btn primary danger" onclick={discard}>Discard</button>
      {:else}
        <span class="hint">{dirty ? "Unsaved changes" : ""}</span>
        <button class="btn" onclick={requestClose}>Cancel</button>
        <button class="btn primary" disabled={saving} onclick={save}>{saving ? "Saving…" : "Save"}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .diag {
    display: flex;
    gap: var(--s3);
  }
  .settings {
    height: min(680px, calc(88vh - var(--s5)));
    user-select: text;
  }
  .layout {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    width: 180px;
    flex-shrink: 0;
    padding: var(--s3);
    border-right: 1px solid var(--line);
  }
  .navitem {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s3);
    height: var(--h-row);
    padding: 0 var(--s3);
    border-radius: var(--radius);
    color: var(--muted);
    text-align: left;
  }
  .navitem:hover {
    background: var(--hover);
    color: var(--text);
  }
  .navitem.on {
    background: var(--raised);
    color: var(--text);
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 0 var(--s6);
  }
  h3 {
    margin: 0 0 var(--s1);
    font-size: var(--fs-md);
    font-weight: 600;
  }
  h3.sub {
    margin-top: var(--s5);
  }
  section {
    padding: var(--s5) 0;
  }
  section + section {
    border-top: 1px solid var(--line);
  }
  section .hint {
    margin: 0 0 var(--s3);
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .none {
    margin: 0;
    color: var(--faint);
    font-size: var(--fs-sm);
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s4);
    min-height: var(--h-row);
  }
  .item > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .add {
    display: flex;
    gap: var(--s2);
    margin-top: var(--s2);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: var(--s3);
    margin-top: var(--s2);
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    padding: var(--s2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    text-align: left;
  }
  .theme:hover {
    border-color: var(--line-strong);
  }
  .theme.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .preview {
    position: relative;
    height: 44px;
    border-radius: var(--r-sm);
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
    right: var(--s3);
    bottom: var(--s3);
    display: flex;
    gap: var(--s1);
  }
  .dots i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .tname {
    font-weight: 600;
    font-size: var(--fs-sm);
  }
  .tnote {
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .space {
    border: 1px solid var(--line);
    border-left: 3px solid var(--c);
    border-radius: var(--radius);
    padding: var(--s3) var(--s4);
    margin: var(--s3) 0;
  }
  .shead {
    display: flex;
    align-items: center;
    gap: var(--s3);
  }
  .sname {
    width: 180px;
    font-weight: 600;
  }
  .colors {
    display: flex;
    gap: var(--s1);
  }
  .color {
    width: 14px;
    height: 14px;
    border-radius: var(--r-sm);
    opacity: 0.55;
  }
  .color.on {
    opacity: 1;
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }
  .rules {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s3);
    margin-top: var(--s3);
  }
  .rules label {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .tabs-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s1);
    margin-top: var(--s3);
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .tabs-row > span {
    margin-right: var(--s1);
  }
  .tabtoggle.on {
    font-weight: 600;
  }

  .default {
    margin-top: var(--s3);
  }
  .srow {
    display: flex;
    gap: var(--s3);
    align-items: center;
    margin-top: var(--s3);
  }
  .nums {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s6);
    margin-top: var(--s3);
  }
  .num {
    display: inline-flex;
    align-items: center;
    gap: var(--s2);
    font-size: var(--fs-sm);
  }
  .num .field {
    width: 72px;
  }
  .path {
    margin: 0;
    word-break: break-all;
    color: var(--muted);
  }
  .about {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
  }
  .aboutrow {
    display: flex;
    align-items: center;
    gap: var(--s4);
  }
  .mark {
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--panel);
    flex-shrink: 0;
  }
  .aboutname {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: var(--fs-sm);
  }
  .aboutname b {
    font-size: var(--fs-md);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1) var(--s4);
    margin: 0;
    font-size: var(--fs-sm);
  }
  .discard {
    margin-right: auto;
    font-weight: 600;
  }
</style>
