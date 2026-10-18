<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api, inTauri } from "../lib/api";
  import { rank } from "../lib/fuzzy";
  import { bytes, agoIso } from "../lib/format";
  import type { RemoteRepo } from "../lib/types";

  type Source = "azdo" | "github" | "url";

  /** The path separator shown between the folder and the new name. */
  const SEP = typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent) ? "\\" : "/";

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
      return localStorage.getItem("tern.clone." + k) ?? d;
    } catch {
      return d;
    }
  };
  const remember = (k: string, v: string) => {
    try {
      localStorage.setItem("tern.clone." + k, v);
    } catch {}
  };

  const savedSource = saved("source", app.activeSpace?.id === "personal" ? "github" : knownOrg ? "azdo" : "github");
  let source = $state<Source>(savedSource === "azdo" || savedSource === "github" || savedSource === "url" ? savedSource : "github");
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
  let urlEl: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  const roots = $derived([...(app.config?.roots ?? []), ...(app.activeSpace?.folders ?? [])].filter((v, i, a) => v && a.indexOf(v) === i));
  const defaultParent = () =>
    source === "url"
      ? (roots[0] ?? "")
      : (roots.find((r) => (source === "azdo" ? /work/i.test(r) : /coding|projects|personal/i.test(r))) ?? roots[0] ?? "");
  let parent = $state("");
  let name = $state("");
  /** The folders offered in "Into": the scan folders, plus one picked with Browse. */
  const parents = $derived(parent && !roots.includes(parent) ? [parent, ...roots] : roots);

  // A remote address in one form, so local repos can be matched with the list.
  const norm = (u: string) => decodeURIComponent(u.trim().toLowerCase()).replace(/^https:\/\/[^@/]+@/, "https://").replace(/\.git$/, "").replace(/\/$/, "");
  const cloned = $derived(new Set(Object.values(app.sums).map((s) => (s.remote ? norm(s.remote) : "")).filter(Boolean)));

  const shown = $derived(filter.trim() ? rank(repos, filter.trim(), (r) => `${r.name} ${r.group}`, 300).map((x) => x.item) : repos);
  const pick = $derived(shown[active] as RemoteRepo | undefined);

  $effect(() => {
    // A new pick fills in the folder name.
    if (pick) name = pick.name;
  });

  // Clone from a URL: https, ssh://, or scp-style user@host:path.
  let url = $state("");
  const urlOk = $derived(/^(https?:\/\/[^/\s]+\/\S+|ssh:\/\/\S+|git:\/\/\S+|[\w.-]+@[\w.-]+:\S+)$/i.test(url.trim()));
  /** The folder name from a git URL: its last path segment without ".git". */
  const nameFromUrl = (u: string) =>
    u
      .trim()
      .replace(/[?#].*$/, "")
      .replace(/[\\/]+$/, "")
      .split(/[/:]/)
      .pop()
      ?.replace(/\.git$/i, "") ?? "";

  $effect(() => {
    if (source === "url") name = nameFromUrl(url);
  });

  const cloneUrl = $derived(source === "url" ? (urlOk ? url.trim() : "") : (pick?.url ?? ""));
  const canClone = $derived(!!cloneUrl && !!parent && !!name.trim());

  async function loadProjects() {
    if (!org.trim()) return;
    loading = "Loading Azure DevOps projects…";
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
    loading = "Loading the list…";
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

  async function switchTo(s: Source) {
    source = s;
    remember("source", s);
    repos = [];
    error = "";
    parent = saved("parent." + s, defaultParent());
    if (s === "azdo") loadProjects();
    else if (s === "github") loadRepos();
    else {
      await tick();
      urlEl?.focus();
    }
  }

  async function browseParent() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked === "string") parent = picked;
    } catch (e) {
      app.notify(`Could not open the folder picker: ${e}`, "err");
    }
  }

  async function clone() {
    if (!canClone) return;
    if (await app.cloneRepo(cloneUrl, parent, name.trim())) {
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

  /** Ctrl Enter clones from anywhere in the dialog. */
  function onDialogKey(e: KeyboardEvent) {
    if (e.key === "Enter" && e.ctrlKey && !e.defaultPrevented) {
      e.preventDefault();
      clone();
    }
  }

  onMount(() => {
    parent = saved("parent." + source, defaultParent());
    if (source === "azdo") loadProjects();
    else if (source === "github") loadRepos();
    else urlEl?.focus();
  });
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="dialog lg clone" role="dialog" aria-modal="true" aria-labelledby="clone-title" tabindex="-1" onkeydown={onDialogKey}>
    <div class="dialog-head">
      <h2 id="clone-title">Clone a project</h2>
    </div>

    <div class="dialog-body body">
      <div class="seg source" role="tablist" aria-label="Clone from">
        <button role="tab" class:on={source === "azdo"} aria-selected={source === "azdo"} onclick={() => switchTo("azdo")}>Azure DevOps</button>
        <button role="tab" class:on={source === "github"} aria-selected={source === "github"} onclick={() => switchTo("github")}>GitHub</button>
        <button role="tab" class:on={source === "url"} aria-selected={source === "url"} onclick={() => switchTo("url")}>URL</button>
      </div>
      {#if source === "url"}
        <label class="lbl">Git URL
          <input class="field mono" bind:this={urlEl} bind:value={url} spellcheck="false" placeholder="https://github.com/owner/name.git or git@github.com:owner/name.git"
            onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); clone(); } }} />
        </label>
        {#if url.trim() && !urlOk}
          <p class="warn">That does not look like a git URL. Use an https or ssh address.</p>
        {:else}
          <p class="help">Any git host works. The folder name comes from the end of the URL.</p>
        {/if}
      {:else}
        <div class="where">
          {#if source === "azdo"}
            <label class="lbl row">Organization <input class="field org" bind:value={org} spellcheck="false" placeholder="your-org" onkeydown={(e) => e.key === "Enter" && loadProjects()} /></label>
            <label class="lbl row">Project
              <select class="field" bind:value={project} onchange={loadRepos} disabled={!projects.length}>
                <option value="">All projects</option>
                {#each projects as p}<option value={p}>{p}</option>{/each}
              </select>
            </label>
          {:else}
            <label class="lbl row">Owner <input class="field org" bind:value={owner} spellcheck="false" placeholder="you (leave empty) or an organization" onkeydown={(e) => e.key === "Enter" && loadRepos()} /></label>
            <button class="btn" onclick={loadRepos}>Load</button>
          {/if}
        </div>

        <input class="field filter" bind:this={filterEl} bind:value={filter} onkeydown={onKey} placeholder="Filter by name" spellcheck="false" aria-label="Filter by name" />

        <div class="list" bind:this={listEl} role="listbox">
          {#if loading}
            <p class="note">{loading}</p>
          {:else if error}
            <div class="state inline">
              <span class="state-title">Could not load the list</span>
              <pre>{error}</pre>
              <p>{source === "azdo" ? "Check that az is signed in: run az login in a terminal." : "Check that gh is signed in: run gh auth login in a terminal."}</p>
              <div class="actions"><button class="btn sm" onclick={() => (source === "azdo" ? loadProjects() : loadRepos())}>Try again</button></div>
            </div>
          {:else if !shown.length}
            <p class="note">{repos.length ? "Nothing matches." : "Nothing found."}</p>
          {/if}
          {#each shown as r, i (r.url)}
            {@const have = cloned.has(norm(r.url))}
            <button class="repo" class:on={i === active} role="option" aria-selected={i === active} onclick={() => (active = i)} ondblclick={clone}>
              <span class="rname">{r.name}</span>
              {#if have}<span class="have">on this machine</span>{/if}
              {#if r.disabled}<span class="faint">{source === "github" ? "archived" : "disabled"}</span>{/if}
              <span class="grow"></span>
              <span class="faint">{r.group}</span>
              {#if r.size}<span class="faint num">{bytes(r.size)}</span>{/if}
              {#if r.updated}<span class="faint mono">{agoIso(r.updated)}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}

      {#if source === "url" || pick}
        <div class="dest">
          <label class="lbl row">Into
            <select class="field parent" bind:value={parent}>
              {#each parents as root}<option value={root}>{root}</option>{/each}
            </select>
          </label>
          {#if inTauri}<button class="btn" onclick={browseParent}>Browse…</button>{/if}
          <span class="sep">{SEP}</span>
          <input class="field mono name" bind:value={name} spellcheck="false" aria-label="Folder name" />
        </div>
        {#if !parents.length}<p class="help">Pick a folder to clone into.</p>{/if}
        {#if source !== "url" && pick?.description}<p class="desc">{pick.description}</p>{/if}
      {/if}
    </div>

    <div class="dialog-foot">
      <span class="hint">{#if source !== "url"}<kbd>↑</kbd> <kbd>↓</kbd> choose · {/if}<kbd>Ctrl Enter</kbd> clones</span>
      <button class="btn" onclick={() => (app.overlay = null)}>Cancel</button>
      <button class="btn primary" disabled={!canClone} onclick={clone}>
        {source !== "url" && pick ? `Clone ${pick.name}` : source === "url" && name.trim() ? `Clone ${name.trim()}` : "Clone"}
      </button>
    </div>
  </div>
</div>

<style>
  .num {
    font-variant-numeric: tabular-nums;
  }
  .source {
    align-self: flex-start;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    flex: 1;
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: var(--s1);
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .lbl.row {
    flex-direction: row;
    align-items: center;
    gap: var(--s3);
  }
  .where,
  .dest {
    display: flex;
    align-items: center;
    gap: var(--s4);
  }
  .dest {
    gap: var(--s2);
    margin-top: var(--s1);
  }
  .org {
    width: 240px;
  }
  .list {
    flex: 1;
    min-height: 160px;
    overflow-y: auto;
    background: var(--bg);
    border-radius: var(--radius);
    padding: var(--s1);
  }
  .repo {
    display: flex;
    align-items: center;
    gap: var(--s3);
    width: 100%;
    height: var(--h-row);
    text-align: left;
    padding: 0 var(--s3);
    border-radius: var(--r-sm);
    white-space: nowrap;
  }
  .repo:hover {
    background: var(--hover);
  }
  .repo.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .rname {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .have {
    font-size: var(--fs-xs);
    color: var(--add);
  }
  .repo .faint {
    font-size: var(--fs-xs);
  }
  .note {
    margin: var(--s3);
    color: var(--muted);
  }
  .parent {
    max-width: 380px;
  }
  .sep {
    color: var(--faint);
    font-family: var(--mono);
  }
  .name {
    flex: 1;
  }
  .help,
  .desc {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--muted);
  }
  .warn {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--del);
  }
</style>
