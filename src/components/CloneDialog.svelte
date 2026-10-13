<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { rank } from "../lib/fuzzy";
  import { bytes, agoIso } from "../lib/format";
  import type { RemoteRepo } from "../lib/types";

  type Source = "azdo" | "github";

  // Guess the Azure DevOps organization from repos already on this machine.
  const knownOrg = (() => {
    for (const s of Object.values(app.sums)) {
      const m = s.remote?.match(/dev\.azure\.com\/([^/]+)\//i) ?? s.remote?.match(/\/\/([^.@/]+)\.visualstudio\.com/i);
      if (m) return decodeURIComponent(m[1]);
    }
    return "";
  })();

  const saved = (k: string, d: string) => {
    try {
      return localStorage.getItem("scout.clone." + k) ?? d;
    } catch {
      return d;
    }
  };
  const remember = (k: string, v: string) => {
    try {
      localStorage.setItem("scout.clone." + k, v);
    } catch {}
  };

  let source = $state<Source>((saved("source", app.activeSpace?.id === "personal" ? "github" : knownOrg ? "azdo" : "github") as Source));
  let org = $state(saved("org", knownOrg));
  let owner = $state(saved("owner", ""));
  let projects = $state<string[]>([]);
  let project = $state(saved("project", ""));
  let repos = $state<RemoteRepo[]>([]);
  let loading = $state("");
  let error = $state("");
  let filter = $state("");
  let active = $state(0);
  let filterEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  const roots = $derived([...(app.config?.roots ?? []), ...(app.activeSpace?.folders ?? [])].filter((v, i, a) => v && a.indexOf(v) === i));
  const defaultParent = () =>
    roots.find((r) => (source === "azdo" ? /work/i.test(r) : /coding|projects|personal/i.test(r))) ?? roots[0] ?? "";
  let parent = $state("");
  let name = $state("");

  // A remote address in one form, so local repos can be matched with the list.
  const norm = (u: string) => decodeURIComponent(u.trim().toLowerCase()).replace(/^https:\/\/[^@/]+@/, "https://").replace(/\.git$/, "").replace(/\/$/, "");
  const cloned = $derived(new Set(Object.values(app.sums).map((s) => (s.remote ? norm(s.remote) : "")).filter(Boolean)));

  const shown = $derived(filter.trim() ? rank(repos, filter.trim(), (r) => `${r.name} ${r.group}`, 300).map((x) => x.item) : repos);
  const pick = $derived(shown[active] as RemoteRepo | undefined);

  $effect(() => {
    // A new pick fills in the folder name.
    if (pick) name = pick.name;
  });

  async function loadProjects() {
    if (!org.trim()) return;
    loading = "Loading projects…";
    error = "";
    try {
      projects = await api.azdoProjects(org);
      remember("org", org);
      if (project && !projects.includes(project)) project = "";
      await loadRepos();
    } catch (e) {
      error = String(e);
    } finally {
      loading = "";
    }
  }

  async function loadRepos() {
    loading = "Loading repos…";
    error = "";
    try {
      if (source === "github") {
        repos = await api.githubRepos(owner);
        remember("owner", owner);
      } else {
        const list = project ? [project] : projects;
        const parts = await Promise.all(list.map((p) => api.azdoRepos(org, p).catch(() => [] as RemoteRepo[])));
        // One row per address, even if a repo shows up in more than one list.
        const seen = new Set<string>();
        repos = parts
          .flat()
          .filter((r) => !seen.has(norm(r.url)) && !!seen.add(norm(r.url)))
          .sort((a, b) => a.name.localeCompare(b.name));
        remember("project", project);
      }
      active = 0;
      await tick();
      filterEl?.focus();
    } catch (e) {
      error = String(e);
      repos = [];
    } finally {
      loading = "";
    }
  }

  function switchTo(s: Source) {
    source = s;
    remember("source", s);
    repos = [];
    parent = defaultParent();
    if (s === "azdo") loadProjects();
    else loadRepos();
  }

  async function clone() {
    if (!pick || !parent || !name.trim()) return;
    if (await app.cloneRepo(pick.url, parent, name.trim())) {
      remember("parent." + source, parent);
      app.overlay = null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") active = Math.min(shown.length - 1, active + 1);
    else if (e.key === "ArrowUp") active = Math.max(0, active - 1);
    else if (e.key === "Enter" && e.ctrlKey) clone();
    else return;
    e.preventDefault();
    tick().then(() => listEl?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
  }

  onMount(() => {
    parent = saved("parent." + source, defaultParent());
    if (source === "azdo") loadProjects();
    else loadRepos();
  });
</script>

<div class="backdrop fx-fade" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="dialog fx-fade" role="dialog" aria-label="Clone a repo">
    <header>
      <h2>Clone a repo</h2>
      <div class="seg" role="tablist">
        <button role="tab" class:on={source === "azdo"} aria-selected={source === "azdo"} onclick={() => switchTo("azdo")}>Azure DevOps</button>
        <button role="tab" class:on={source === "github"} aria-selected={source === "github"} onclick={() => switchTo("github")}>GitHub</button>
      </div>
    </header>

    <div class="where">
      {#if source === "azdo"}
        <label>Organization <input bind:value={org} spellcheck="false" placeholder="your-org" onkeydown={(e) => e.key === "Enter" && loadProjects()} /></label>
        <label>Project
          <select bind:value={project} onchange={loadRepos} disabled={!projects.length}>
            <option value="">All projects</option>
            {#each projects as p}<option value={p}>{p}</option>{/each}
          </select>
        </label>
      {:else}
        <label>Owner <input bind:value={owner} spellcheck="false" placeholder="you (leave empty) or an organization" onkeydown={(e) => e.key === "Enter" && loadRepos()} /></label>
        <button class="btn" onclick={loadRepos}>Load</button>
      {/if}
    </div>

    <input class="filter" bind:this={filterEl} bind:value={filter} onkeydown={onKey} placeholder="Filter repos" spellcheck="false" aria-label="Filter repos" />

    <div class="list" bind:this={listEl} role="listbox">
      {#if loading}
        <p class="note">{loading}</p>
      {:else if error}
        <p class="note err">{error}</p>
        <p class="note">{source === "azdo" ? "Check that az is signed in: run az login in a terminal." : "Check that gh is signed in: run gh auth login in a terminal."}</p>
      {:else if !shown.length}
        <p class="note">{repos.length ? "No repos match." : "No repos found."}</p>
      {/if}
      {#each shown as r, i (r.url)}
        {@const have = cloned.has(norm(r.url))}
        <button class="repo" class:on={i === active} role="option" aria-selected={i === active} onclick={() => (active = i)} ondblclick={clone}>
          <span class="rname">{r.name}</span>
          {#if have}<span class="have">on this machine</span>{/if}
          {#if r.disabled}<span class="faint">{source === "github" ? "archived" : "disabled"}</span>{/if}
          <span class="grow"></span>
          <span class="faint">{r.group}</span>
          {#if r.size}<span class="faint mono">{bytes(r.size)}</span>{/if}
          {#if r.updated}<span class="faint mono">{agoIso(r.updated)}</span>{/if}
        </button>
      {/each}
    </div>

    {#if pick}
      <div class="dest">
        <label>Into
          <select bind:value={parent}>
            {#each roots as root}<option value={root}>{root}</option>{/each}
          </select>
        </label>
        <span class="sep">\</span>
        <input class="name" bind:value={name} spellcheck="false" aria-label="Folder name" />
      </div>
      {#if pick.description}<p class="desc muted">{pick.description}</p>{/if}
    {/if}

    <footer>
      <span class="faint">↑ ↓ choose. <kbd>Ctrl Enter</kbd> clones.</span>
      <button class="btn" onclick={() => (app.overlay = null)}>Cancel</button>
      <button class="btn primary" disabled={!pick || !parent || !name.trim()} onclick={clone}>
        {pick ? `Clone ${pick.name}` : "Clone"}
      </button>
    </footer>
  </div>
</div>

<style>
  /* Opened with the mouse, now and then: a short entrance. Closing stays instant. */
  .backdrop {
    animation: tern-fade-in 150ms var(--ease-out);
  }
  .dialog {
    animation: tern-dialog-in 200ms var(--ease-out);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 9vh 16px 16px;
    z-index: 10;
  }
  .dialog {
    width: min(720px, 100%);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    padding: 16px 18px 14px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 3px 12px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .where,
  .dest {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 10px;
  }
  label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  input,
  select {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px 8px;
    outline: none;
    font: inherit;
  }
  input:focus,
  select:focus {
    border-color: var(--amber);
  }
  .where input {
    width: 220px;
  }
  .filter {
    width: 100%;
    font-size: 14px;
    padding: 7px 10px;
    margin-bottom: 8px;
  }
  .list {
    flex: 1;
    min-height: 160px;
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px;
  }
  .repo {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 5px 8px;
    border-radius: 5px;
    white-space: nowrap;
  }
  .repo:hover {
    background: var(--hover);
  }
  .repo.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--amber);
  }
  .rname {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .have {
    font-size: 11px;
    color: var(--add);
  }
  .grow {
    flex: 1;
  }
  .repo .faint {
    font-size: 11.5px;
  }
  .note {
    margin: 8px;
    color: var(--muted);
  }
  .note.err {
    color: var(--del);
    white-space: pre-wrap;
    font-family: var(--mono);
    font-size: 12px;
    user-select: text;
  }
  .dest {
    margin: 10px 0 0;
  }
  .dest select {
    max-width: 380px;
  }
  .sep {
    color: var(--faint);
  }
  .name {
    flex: 1;
    font-family: var(--mono);
    font-size: 12px;
  }
  .desc {
    margin: 6px 0 0;
    font-size: 12px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  footer .faint {
    flex: 1;
    font-size: 11.5px;
  }
  .btn {
    padding: 5px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  .btn:hover:not(:disabled) {
    color: var(--text);
  }
  .btn.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--on-accent);
    font-weight: 600;
  }
  .btn.primary:disabled {
    background: var(--raised);
    border-color: var(--line);
    color: var(--faint);
    cursor: default;
  }
</style>
