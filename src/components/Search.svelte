<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { rank } from "../lib/fuzzy";
  import { absolutePath, ago, agoIso, baseName, dirName } from "../lib/format";
  import type { Branch, Commit, Issue, PullRequest, RepoEntry, SearchHit } from "../lib/types";
  import { definitionPattern, isCodeFile } from "../lib/defs";

  type Kind = "all" | "code" | "files" | "commits" | "branches" | "work";
  type Found = {
    code?: { hits: SearchHit[]; truncated: boolean };
    defs?: SearchHit[];
    files?: string[];
    commits?: Commit[];
    branches?: Branch[];
    prs?: PullRequest[];
    issues?: Issue[];
    error?: string;
  };
  type Set = { name: string; paths: string[] };
  type Row =
    | { t: "repo"; repo: string; id: string }
    | { t: "code"; repo: string; id: string; hit: SearchHit }
    | { t: "def"; repo: string; id: string; hit: SearchHit }
    | { t: "label"; repo: string; id: string; text: string }
    | { t: "file"; repo: string; id: string; rel: string }
    | { t: "commit"; repo: string; id: string; c: Commit }
    | { t: "branch"; repo: string; id: string; b: Branch }
    | { t: "pr"; repo: string; id: string; pr: PullRequest }
    | { t: "issue"; repo: string; id: string; issue: Issue }
    | { t: "more"; repo: string; id: string; kind: Kind; n: number };

  const load = <T,>(k: string, d: T): T => {
    try {
      return JSON.parse(localStorage.getItem("scout.search." + k) ?? "") as T;
    } catch {
      return d;
    }
  };
  const keep = (k: string, v: unknown) => {
    try {
      localStorage.setItem("scout.search." + k, JSON.stringify(v));
    } catch {}
  };

  // A preset comes from Ctrl+F, F12, Shift+F12, or a right-click on a name.
  const preset = app.searchPreset;
  app.searchPreset = null;
  let query = $state(preset?.query ?? load("query", ""));
  let regex = $state(preset?.defsFor ? false : load("regex", false));
  let matchCase = $state(preset?.defsFor ? true : load("case", false));
  let word = $state(preset?.word ?? load("word", false));
  let kind = $state<Kind>((preset?.kind as Kind) ?? (preset?.scope === "file" ? "code" : load("kind", "all")));
  /** The name whose definitions are shown first. Cleared when you type something else. */
  let defsFor = $state(preset?.defsFor ?? "");
  const viewRel = app.view?.rel || "";
  let sets = $state<Set[]>(load("sets", []));
  let picked = $state<string[]>(load("picked", []));
  // With a repo open, start on that repo. Otherwise start on the space.
  let scope = $state<string>(preset?.scope === "file" && viewRel ? "file" : preset?.scope && preset.scope !== "file" ? preset.scope : app.current ? "repo" : "space");
  let picking = $state(false);
  let pickFilter = $state("");
  let setName = $state("");
  let found = $state<Record<string, Found>>({});
  let pending = $state(0);
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();
  let seq = 0;
  const branchCache = new Map<string, Branch[]>();

  const mains = (list: RepoEntry[]) => list.filter((r) => !r.parent && !app.isHidden(r));
  const scopes = $derived([
    ...(viewRel && app.current ? [{ id: "file", label: `This file (${baseName(viewRel)})` }] : []),
    ...(app.current ? [{ id: "repo", label: app.repo?.name ?? "This repo" }] : []),
    { id: "space", label: app.activeSpace ? app.activeSpace.name : "This space" },
    { id: "all", label: "All" },
    ...sets.map((s) => ({ id: "set:" + s.name, label: s.name })),
    { id: "pick", label: picked.length ? `Picked (${picked.length})` : "Pick…" },
  ]);
  const repos = $derived.by((): string[] => {
    if ((scope === "repo" || scope === "file") && app.current) return [app.current];
    if (scope === "space") return mains(app.visible).map((r) => r.path);
    if (scope === "all") return mains(app.repos).map((r) => r.path);
    if (scope === "pick") return picked.filter((p) => app.repos.some((r) => r.path === p));
    const set = sets.find((s) => "set:" + s.name === scope);
    return (set?.paths ?? []).filter((p) => app.repos.some((r) => r.path === p));
  });
  const nameOf = (p: string) => app.repos.find((r) => r.path === p)?.name ?? baseName(p);

  function matcher(): ((s: string) => boolean) | null {
    const q = query.trim();
    if (!q) return null;
    if (regex) {
      try {
        const re = new RegExp(q, matchCase ? "" : "i");
        return (s) => re.test(s);
      } catch {
        return null;
      }
    }
    return matchCase ? (s) => s.includes(q) : (s) => s.toLowerCase().includes(q.toLowerCase());
  }

  async function searchRepo(repo: string, q: string, my: number) {
    const f: Found = {};
    const match = matcher();
    const paths = scope === "file" && viewRel ? [viewRel] : [];
    const code = api
      .searchTextWith(repo, q, { regex, caseSensitive: matchCase, word, paths }, scope === "file" ? 500 : 60)
      .then((r) => (f.code = r))
      .catch((e) => (f.error = String(e)));
    if (scope === "file") {
      // Only this file: other kinds do not apply.
      await code;
      if (my === seq) found = { ...found, [repo]: f };
      return;
    }
    const tasks: Promise<unknown>[] = [
      code,
      ...(defsFor && q === defsFor
        ? [
            api
              .searchTextWith(repo, definitionPattern(defsFor), { perl: true, caseSensitive: true }, 30)
              // Code files first, docs and notes after.
              .then((r) => (f.defs = [...r.hits].sort((a, b) => Number(isCodeFile(b.path)) - Number(isCodeFile(a.path)))))
              .catch(() => {}),
          ]
        : []),
      app.loadFiles(repo).then((files) => (f.files = rank(files, q, (x) => x, 40).map((x) => x.item))),
      api.searchCommits(repo, q, regex, matchCase).then((c) => (f.commits = c)).catch(() => {}),
      (branchCache.has(repo) ? Promise.resolve(branchCache.get(repo)!) : api.listBranches(repo).then((l) => (branchCache.set(repo, l.branches), l.branches)))
        .then((b) => (f.branches = match ? b.filter((x) => match(x.name)) : []))
        .catch(() => {}),
    ];
    // Pull requests and issues come from the network, so only for a small scope.
    const s = app.sums[repo];
    if (repos.length <= 10 && s && (s.provider === "github" || s.provider === "azdo")) {
      const num = q.replace(/^#/, "");
      const hitTitle = (title: string, id: number) => (match ? match(title) : false) || String(id) === num;
      tasks.push(app.loadPrs(repo).then(() => (f.prs = (app.prs[repo]?.items ?? []).filter((p) => hitTitle(p.title, p.id)))));
      if (s.provider === "github") tasks.push(app.loadIssues(repo).then(() => (f.issues = (app.issues[repo]?.items ?? []).filter((i) => hitTitle(i.title, i.number)))));
    }
    await Promise.all(tasks);
    if (my === seq) found = { ...found, [repo]: f };
  }

  let timer: ReturnType<typeof setTimeout> | undefined;
  function run() {
    clearTimeout(timer);
    const q = query.trim();
    const my = ++seq;
    found = {};
    if (q.length < 2) {
      pending = 0;
      return;
    }
    keep("query", query);
    timer = setTimeout(async () => {
      const list = [...repos];
      pending = list.length;
      let next = 0;
      const worker = async () => {
        while (next < list.length && my === seq) {
          const r = list[next++];
          await searchRepo(r, q, my);
          if (my === seq) pending--;
        }
      };
      await Promise.all(Array.from({ length: Math.min(8, list.length) }, worker));
    }, 220);
  }

  // Search again when the words, options, or scope change.
  $effect(() => {
    query;
    regex;
    matchCase;
    word;
    scope;
    repos;
    if (defsFor && query.trim() !== defsFor) defsFor = "";
    run();
  });

  const counts = $derived.by(() => {
    const c = { code: 0, files: 0, commits: 0, branches: 0, work: 0 };
    for (const f of Object.values(found)) {
      c.code += f.code?.hits.length ?? 0;
      c.files += f.files?.length ?? 0;
      c.commits += f.commits?.length ?? 0;
      c.branches += f.branches?.length ?? 0;
      c.work += (f.prs?.length ?? 0) + (f.issues?.length ?? 0);
    }
    return c;
  });

  const rows = $derived.by((): Row[] => {
    const out: Row[] = [];
    const cap = (k: Kind, n: number) => (kind === "all" ? n : 500);
    const order = repos.filter((r) => found[r]);
    for (const repo of order) {
      const f = found[repo];
      const group: Row[] = [];
      const add = (k: Kind, list: Row[], n: number) => {
        if (kind !== "all" && kind !== k) return;
        const c = cap(k, n);
        group.push(...list.slice(0, c));
        if (list.length > c) group.push({ t: "more", repo, id: `${repo}|more|${k}`, kind: k, n: list.length - c });
      };
      if (f.defs?.length && (kind === "all" || kind === "code")) {
        group.push({ t: "label", repo, id: `${repo}|dl`, text: "Definitions" });
        group.push(...f.defs.map((hit, i): Row => ({ t: "def", repo, id: `${repo}|d|${hit.path}|${hit.line}|${i}`, hit })));
        group.push({ t: "label", repo, id: `${repo}|ul`, text: "Used in" });
      }
      add("files", (f.files ?? []).map((rel) => ({ t: "file", repo, id: `${repo}|f|${rel}`, rel })), 4);
      add("code", (f.code?.hits ?? []).map((hit, i) => ({ t: "code", repo, id: `${repo}|c|${hit.path}|${hit.line}|${i}`, hit })), 12);
      add("commits", (f.commits ?? []).map((c) => ({ t: "commit", repo, id: `${repo}|m|${c.sha}`, c })), 4);
      add("branches", (f.branches ?? []).map((b) => ({ t: "branch", repo, id: `${repo}|b|${b.name}`, b })), 4);
      add("work", [
        ...(f.prs ?? []).map((pr): Row => ({ t: "pr", repo, id: `${repo}|p|${pr.id}`, pr })),
        ...(f.issues ?? []).map((issue): Row => ({ t: "issue", repo, id: `${repo}|i|${issue.number}`, issue })),
      ], 4);
      if (group.length) out.push({ t: "repo", repo, id: repo }, ...group);
    }
    return out;
  });

  const heading = (r: Row | undefined) => r?.t === "repo" || r?.t === "label";

  /** Moves the cursor by `d` rows, stepping over the repo name headings. */
  function step(from: number, d: number) {
    let i = Math.max(0, Math.min(rows.length - 1, from));
    const dir = d < 0 ? -1 : 1;
    while (heading(rows[i]) && i + dir >= 0 && i + dir < rows.length) i += dir;
    if (heading(rows[i])) {
      // At the very top or bottom: look the other way.
      while (heading(rows[i]) && i - dir >= 0 && i - dir < rows.length) i -= dir;
    }
    return i;
  }

  $effect(() => {
    rows;
    active = step(Math.min(active, rows.length - 1), 1);
  });

  function go(r: Row | undefined) {
    if (!r) return;
    if (r.t === "more") {
      kind = r.kind;
      keep("kind", kind);
      return;
    }
    app.overlay = null;
    app.select(r.repo);
    if (r.t === "code" || r.t === "def") app.openFile(r.hit.path, r.hit.line, true, { text: query.trim(), regex, matchCase });
    else if (r.t === "file") app.openFile(r.rel);
    else if (r.t === "commit") {
      if (app.visibleTabs.includes("history")) app.setTab("history");
      app.openCommit(r.c);
    } else if (r.t === "branch") app.openBranches("branches");
    else if (r.t === "pr") {
      if (app.visibleTabs.includes("prs")) app.setTab("prs");
      app.openPr(r.pr);
    } else if (r.t === "issue") {
      if (app.visibleTabs.includes("issues")) app.setTab("issues");
      app.openIssue(r.issue.number);
    }
  }

  function menu(e: MouseEvent, r: Row) {
    const rel = r.t === "code" || r.t === "def" ? r.hit.path : r.t === "file" ? r.rel : null;
    if (!rel) return;
    const at = r.t === "code" || r.t === "def" ? `${rel}:${r.hit.line}` : rel;
    app.showMenu(e, at, [
      { label: "Open", run: () => go(r) },
      "sep",
      { label: `Copy ${at}`, run: () => app.copyValue(at) },
      { label: `Copy @${rel}`, run: () => app.copyValue("@" + rel) },
      { label: "Copy the absolute path", run: () => app.copyValue(absolutePath(r.repo, rel)) },
      ...(r.t === "code" || r.t === "def" ? [{ label: "Copy the line", run: () => app.copyValue(r.hit.text.trim(), "the line") }] : []),
    ]);
  }

  function cycleScope(d: number) {
    const ids = scopes.map((s) => s.id);
    const i = ids.indexOf(scope);
    scope = ids[(i + d + ids.length) % ids.length];
    picking = scope === "pick" && !picked.length;
  }

  const kinds: { id: Kind; label: string }[] = [
    { id: "all", label: "All" },
    { id: "code", label: "Code" },
    { id: "files", label: "Files" },
    { id: "commits", label: "Commits" },
    { id: "branches", label: "Branches" },
    { id: "work", label: "PRs & issues" },
  ];

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") active = step(active + 1, 1);
    else if (e.key === "ArrowUp") active = step(active - 1, -1);
    else if (e.key === "PageDown") active = step(active + 12, 1);
    else if (e.key === "PageUp") active = step(active - 12, -1);
    else if (e.key === "Enter") go(rows[active]);
    else if (e.key === "Tab") cycleScope(e.shiftKey ? -1 : 1);
    else if (e.altKey && /^[1-6]$/.test(e.key)) {
      kind = kinds[Number(e.key) - 1].id;
      keep("kind", kind);
    } else if (e.altKey && e.key.toLowerCase() === "c") {
      matchCase = !matchCase;
      keep("case", matchCase);
    } else if (e.altKey && e.key.toLowerCase() === "w") {
      word = !word;
      keep("word", word);
    } else if (e.altKey && e.key.toLowerCase() === "r") {
      regex = !regex;
      keep("regex", regex);
    } else return;
    e.preventDefault();
    tick().then(() => listEl?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
  }

  // Highlights matches inside a line of code.
  function parts(text: string): { t: string; m: boolean }[] {
    const q = query.trim();
    if (!q) return [{ t: text, m: false }];
    let re: RegExp;
    try {
      re = new RegExp(regex ? q : q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), matchCase ? "g" : "gi");
    } catch {
      return [{ t: text, m: false }];
    }
    const out: { t: string; m: boolean }[] = [];
    let last = 0;
    for (const m of text.matchAll(re)) {
      if (!m[0]) break;
      out.push({ t: text.slice(last, m.index), m: false }, { t: m[0], m: true });
      last = m.index! + m[0].length;
    }
    out.push({ t: text.slice(last), m: false });
    return out;
  }

  // Picking repos
  const pickList = $derived(
    app.ordered.filter((r) => !app.isHidden(r) && (!pickFilter.trim() || r.name.toLowerCase().includes(pickFilter.trim().toLowerCase()))),
  );
  function togglePick(p: string) {
    picked = picked.includes(p) ? picked.filter((x) => x !== p) : [...picked, p];
    keep("picked", picked);
    scope = "pick";
  }
  function pickSpace(id: string | null) {
    const list = mains(app.repos).filter((r) => id === null || app.spaceOf(r) === id).map((r) => r.path);
    picked = [...new Set([...picked, ...list])];
    keep("picked", picked);
    scope = "pick";
  }
  function saveSet() {
    const name = setName.trim();
    if (!name || !picked.length) return;
    sets = [...sets.filter((s) => s.name !== name), { name, paths: [...picked] }];
    keep("sets", sets);
    scope = "set:" + name;
    setName = "";
    picking = false;
    app.notify(`Saved the set "${name}" with ${picked.length} repos`, "ok");
  }
  function deleteSet(name: string) {
    sets = sets.filter((s) => s.name !== name);
    keep("sets", sets);
    if (scope === "set:" + name) scope = "space";
  }
  function editSet(name: string) {
    const s = sets.find((x) => x.name === name);
    if (!s) return;
    picked = [...s.paths];
    keep("picked", picked);
    setName = name;
    scope = "pick";
    picking = true;
  }

  onMount(() => {
    input?.focus();
    input?.select();
  });
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.overlay = null)}>
  <div class="search" role="dialog" aria-label="Search across repos">
    <div class="bar">
      <input bind:this={input} bind:value={query} onkeydown={onKey} placeholder="Search code, files, commits, branches, PRs, and issues" spellcheck="false" aria-label="Search" />
      <button class="opt" class:on={matchCase} onclick={() => { matchCase = !matchCase; keep("case", matchCase); input?.focus(); }} title="Match case (Alt+C)" aria-pressed={matchCase}>Aa</button>
      <button class="opt" class:on={word} onclick={() => { word = !word; keep("word", word); input?.focus(); }} title="Whole word (Alt+W)" aria-pressed={word}><u>W</u></button>
      <button class="opt mono" class:on={regex} onclick={() => { regex = !regex; keep("regex", regex); input?.focus(); }} title="Regular expression (Alt+R)" aria-pressed={regex}>.*</button>
    </div>

    <div class="scopes" role="radiogroup" aria-label="Where to search">
      <span class="label">In</span>
      {#each scopes as s}
        <button role="radio" class="scope" class:on={scope === s.id} aria-checked={scope === s.id}
          onclick={() => { scope = s.id; picking = s.id === "pick"; input?.focus(); }}
          oncontextmenu={(e) => s.id.startsWith("set:") && app.showMenu(e, s.label, [
            { label: "Change this set", run: () => editSet(s.label) },
            { label: "Delete this set", run: () => deleteSet(s.label), danger: true },
          ])}>
          {s.label}
        </button>
      {/each}
      <span class="grow"></span>
      <span class="faint">{repos.length} {repos.length === 1 ? "repo" : "repos"}{#if pending} — searching {pending}…{/if}</span>
    </div>

    {#if picking}
      <div class="pick">
        <div class="picktop">
          <input bind:value={pickFilter} placeholder="Filter repos" spellcheck="false" aria-label="Filter repos" />
          {#each app.spaces as sp}<button class="mini" onclick={() => pickSpace(sp.id)}>+ {sp.name}</button>{/each}
          <button class="mini" onclick={() => { picked = []; keep("picked", picked); }}>None</button>
        </div>
        <div class="picklist">
          {#each pickList as r (r.path)}
            <label class="pickrow" class:child={!!r.parent}>
              <input type="checkbox" checked={picked.includes(r.path)} onchange={() => togglePick(r.path)} />
              <span>{r.name}</span>{#if app.nameHint[r.path]}<span class="faint">{app.nameHint[r.path]}</span>{/if}
            </label>
          {/each}
        </div>
        <form class="saveset" onsubmit={(e) => { e.preventDefault(); saveSet(); }}>
          <input bind:value={setName} placeholder="Name this set, for example Frontend + APIs" spellcheck="false" aria-label="Set name" />
          <button class="mini" type="submit" disabled={!setName.trim() || !picked.length}>Save set</button>
          <button class="mini" type="button" onclick={() => (picking = false)}>Done</button>
        </form>
      </div>
    {/if}

    <div class="kinds" role="tablist">
      {#each kinds as k, i}
        <button role="tab" class:on={kind === k.id} aria-selected={kind === k.id} onclick={() => { kind = k.id; keep("kind", kind); input?.focus(); }} title="Alt+{i + 1}">
          {k.label}{#if k.id !== "all" && counts[k.id as Exclude<Kind, "all">]}<span class="n">{counts[k.id as Exclude<Kind, "all">]}</span>{/if}
        </button>
      {/each}
    </div>

    <div class="results" bind:this={listEl} role="listbox">
      {#if query.trim().length < 2}
        <p class="note">Type at least 2 letters. <kbd>Tab</kbd> changes where to search. <kbd>Alt 1</kbd>…<kbd>6</kbd> pick a kind.</p>
      {:else if !rows.length && !pending}
        <p class="note">Nothing found in {repos.length} {repos.length === 1 ? "repo" : "repos"}.{#if scope === "repo"} Press <kbd>Tab</kbd> to search more repos.{/if}</p>
      {/if}
      {#each rows as r, i (r.id)}
        {#if r.t === "label"}
          <div class="sublabel">{r.text}</div>
        {:else if r.t === "repo"}
          <div class="group">{nameOf(r.repo)}{#if found[r.repo]?.code?.truncated}<span class="faint"> (more code matches; narrow the search)</span>{/if}</div>
        {:else}
          <div class="row {r.t}" class:on={i === active} role="option" aria-selected={i === active} tabindex="-1"
            onmousemove={() => (active = i)} onclick={() => go(r)} oncontextmenu={(e) => menu(e, r)} onkeydown={() => {}}>
            {#if r.t === "code" || r.t === "def"}
              {#if r.t === "def"}<span class="kindtag def">def</span>{/if}
              <span class="where mono">{baseName(r.hit.path)}<span class="faint">:{r.hit.line}</span></span>
              <span class="line mono">{#each parts(r.hit.text.trim()) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span>
              <span class="dir mono faint">{dirName(r.hit.path)}</span>
            {:else if r.t === "file"}
              <span class="kindtag">file</span>
              <span class="where mono">{baseName(r.rel)}</span>
              <span class="dir mono faint">{dirName(r.rel)}</span>
            {:else if r.t === "commit"}
              <span class="kindtag">commit</span>
              <span class="sha mono">{r.c.short}</span>
              <span class="line">{#each parts(r.c.subject) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span>
              <span class="dir faint">{r.c.author}, {ago(r.c.time)}</span>
            {:else if r.t === "branch"}
              <span class="kindtag">branch</span>
              <span class="where mono">{r.b.name}</span>
              <span class="dir faint">{r.b.subject}, {ago(r.b.time)}</span>
            {:else if r.t === "pr"}
              <span class="kindtag">PR</span>
              <span class="sha mono">#{r.pr.id}</span>
              <span class="line">{r.pr.title}</span>
              <span class="dir faint">{r.pr.author}, {agoIso(r.pr.updated)}</span>
            {:else if r.t === "issue"}
              <span class="kindtag">issue</span>
              <span class="sha mono">#{r.issue.number}</span>
              <span class="line">{r.issue.title}</span>
              <span class="dir faint">{r.issue.state}, {agoIso(r.issue.updated)}</span>
            {:else if r.t === "more"}
              <span class="more">{r.n} more — show all {kinds.find((k) => k.id === r.kind)?.label.toLowerCase()}</span>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
    <p class="foot faint"><kbd>Enter</kbd> open. Right-click a file or line to copy its path. Saved sets: right-click to change or delete.</p>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(8, 10, 14, 0.45);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 7vh 16px 16px;
    z-index: 10;
  }
  .search {
    width: min(900px, 100%);
    height: min(80vh, 760px);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }
  .bar {
    display: flex;
    gap: 6px;
    padding: 10px 10px 6px;
  }
  input {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 6px 9px;
    outline: none;
    font: inherit;
  }
  input:focus {
    border-color: var(--amber);
  }
  .bar input {
    flex: 1;
    font-size: 15px;
    padding: 8px 10px;
  }
  .opt {
    width: 36px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
    font-size: 12px;
  }
  .opt.on {
    color: var(--on-accent);
    background: var(--amber);
    border-color: var(--amber);
  }
  .scopes {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    padding: 0 10px 8px;
    font-size: 12px;
  }
  .label {
    color: var(--faint);
    margin-right: 2px;
  }
  .scope {
    padding: 2px 10px;
    border-radius: 12px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .scope.on {
    color: var(--text);
    border-color: var(--amber);
    background: var(--amber-soft);
  }
  .grow {
    flex: 1;
  }
  .pick {
    margin: 0 10px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px;
    background: var(--bg);
  }
  .picktop,
  .saveset {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .picktop input {
    width: 200px;
  }
  .saveset {
    margin-top: 8px;
  }
  .saveset input {
    flex: 1;
  }
  .picklist {
    columns: 3 200px;
    margin-top: 8px;
    max-height: 170px;
    overflow-y: auto;
  }
  .pickrow {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
    padding: 2px 0;
    break-inside: avoid;
  }
  .pickrow.child {
    padding-left: 16px;
  }
  .pickrow input {
    accent-color: var(--amber);
    margin: 0;
  }
  .mini {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .mini:hover:not(:disabled) {
    color: var(--text);
  }
  .mini:disabled {
    opacity: 0.45;
  }
  .kinds {
    display: flex;
    gap: 2px;
    padding: 0 10px;
    border-bottom: 1px solid var(--line);
  }
  .kinds button {
    padding: 5px 8px 6px;
    font-size: 12.5px;
    color: var(--muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    display: flex;
    gap: 5px;
    align-items: center;
  }
  .kinds button.on {
    color: var(--text);
    border-bottom-color: var(--amber);
  }
  .n {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
  .results {
    flex: 1;
    overflow-y: auto;
    padding: 4px 6px 8px;
  }
  .sublabel {
    padding: 6px 8px 2px 14px;
    font-size: 11px;
    color: var(--faint);
  }
  .kindtag.def {
    color: var(--amber);
    width: 26px;
  }
  .group {
    padding: 10px 8px 3px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 4px 8px;
    border-radius: 5px;
    white-space: nowrap;
    overflow: hidden;
    cursor: default;
  }
  .row.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--amber);
  }
  .where {
    color: var(--text);
    flex-shrink: 0;
    font-size: 12px;
  }
  .line {
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
    flex: 1;
    font-size: 12px;
    color: var(--muted);
  }
  .row.commit .line,
  .row.pr .line,
  .row.issue .line {
    font-size: 13px;
    color: var(--text);
  }
  .dir {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
    max-width: 40%;
  }
  .row.file .dir,
  .row.branch .dir {
    flex: 1;
    max-width: none;
  }
  .sha {
    color: var(--amber);
    font-size: 11.5px;
    flex-shrink: 0;
  }
  .kindtag {
    font-size: 10.5px;
    color: var(--faint);
    width: 44px;
    flex-shrink: 0;
  }
  mark {
    background: var(--amber-sel);
    color: var(--text);
    border-radius: 2px;
  }
  .more {
    color: var(--blue);
    font-size: 12px;
  }
  .note {
    margin: 16px 10px;
    color: var(--muted);
  }
  .foot {
    margin: 0;
    padding: 7px 12px;
    border-top: 1px solid var(--line);
    font-size: 11px;
  }
</style>
