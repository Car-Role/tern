<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { Compartment, EditorState } from "@codemirror/state";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { ago as agoSecs } from "../lib/format";
  import { hover } from "../lib/hover";
  import { queryExtensions } from "../lib/cm";
  import { completionSchema, engineLabel, looksLikeWrite, openQuery, schemaText, tableName, toCsv, toJson, toMarkdown, toTsv } from "../lib/dbtext";
  import type { DbConn, DbEngine, DbQueryResult, DbSchema, DbTable } from "../lib/types";
  import DbGrid from "./DbGrid.svelte";
  import DbConnForm from "./DbConnForm.svelte";

  /** A fixed connection (an SQLite file opened from the file list) hides the saved connections. */
  let { fixed = null }: { fixed?: DbConn | null } = $props();

  const conn = $derived(fixed ?? app.dbConns.find((c) => c.id === app.dbSel) ?? null);
  let localSchema = $state<{ loading: boolean; error: string | null; data: DbSchema | null } | null>(null);
  const schema = $derived(fixed ? localSchema : conn ? app.dbSchemas[conn.id] : undefined);

  let form = $state<{ conn: DbConn | null } | null>(null);
  let filter = $state("");
  let openGroups = $state(new Set<string>());
  let openTables = $state(new Set<string>());
  let shownPerGroup = $state<Record<string, number>>({});

  let result = $state<DbQueryResult | null>(null);
  let error = $state("");
  let running = $state(false);
  let started = $state(0);
  let now = $state(0);
  let tab = $state(0);
  let writes = $state(false);
  let editorH = $state(200);
  /** The query that made the result on screen. */
  let lastQ = $state("");

  let host: HTMLDivElement | undefined = $state();
  let editor: EditorView | null = null;
  const language = new Compartment();
  let text = $state("");

  const store = {
    get: (k: string) => {
      try {
        return localStorage.getItem("scout." + k);
      } catch {
        return null;
      }
    },
    set: (k: string, v: string) => {
      try {
        localStorage.setItem("scout." + k, v);
      } catch {}
    },
  };
  const connId = (c: DbConn) => (fixed ? "file:" + c.file : c.id);
  const qKey = (c: DbConn) => `dbq.${connId(c)}`;

  // Sizes you set by dragging, remembered across restarts.
  const clamp = (n: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, n));
  const SIDE_MIN = 200;
  const SIDE_MAX = 480;
  let sideW = $state(clamp(Number(store.get("dbSideW")) || (untrack(() => fixed) ? 200 : 260), SIDE_MIN, SIDE_MAX));
  editorH = clamp(Number(store.get("dbEditorH")) || 200, 80, 2000);

  // Query history: the last 50 queries run on each connection, newest first.
  type HistItem = { q: string; at: number };
  const HIST_MAX = 50;
  const histKey = (c: DbConn) => `dbhist.${connId(c)}`;
  function readHistory(c: DbConn): HistItem[] {
    try {
      const v = JSON.parse(store.get(histKey(c)) ?? "[]");
      return Array.isArray(v) ? v.filter((x) => x && typeof x.q === "string" && typeof x.at === "number") : [];
    } catch {
      return [];
    }
  }
  function remember(c: DbConn, q: string) {
    const list = [{ q, at: Date.now() }, ...readHistory(c).filter((h) => h.q !== q)].slice(0, HIST_MAX);
    store.set(histKey(c), JSON.stringify(list));
  }
  function historyMenu(e: MouseEvent) {
    const c = conn;
    if (!c) return;
    const list = readHistory(c);
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const at = { x: r.left, y: r.bottom + 4 };
    if (!list.length) {
      app.showMenu(at, "History", [{ label: "No queries run yet", disabled: true, run: () => {} }]);
      return;
    }
    app.showMenu(at, `History · ${c.name}`, [
      ...list.map((h) => {
        const first = h.q.split("\n").find((l) => l.trim())?.trim() ?? h.q;
        const label = first.length > 60 ? first.slice(0, 59) + "…" : h.q.includes("\n") ? first + " …" : first;
        return { label, hint: agoSecs(Math.floor(h.at / 1000)), run: () => (setText(h.q), editor?.focus()) };
      }),
      "sep" as const,
      { label: "Clear history", danger: true, run: () => store.set(histKey(c), "[]") },
    ]);
  }

  async function loadFixedSchema() {
    if (!fixed) return;
    localSchema = { loading: true, error: null, data: localSchema?.data ?? null };
    try {
      localSchema = { loading: false, error: null, data: await api.dbSchema($state.snapshot(fixed) as DbConn) };
    } catch (e) {
      localSchema = { loading: false, error: String(e), data: null };
    }
  }
  const refreshSchema = () => (fixed ? loadFixedSchema() : conn && app.loadDbSchema(conn.id));

  // A new connection: its saved query, a fresh result, and writes off again.
  const connKey = $derived(conn ? [conn.id, conn.engine, conn.file, conn.host, conn.port, conn.database, conn.user].join("|") : "");
  $effect(() => {
    connKey;
    const c = untrack(() => conn);
    untrack(() => {
      result = null;
      error = "";
      writes = false;
      tab = 0;
      filter = "";
      if (fixed) loadFixedSchema();
      if (c) setText(store.get(qKey(c)) ?? "");
    });
  });

  // The editor, made once.
  $effect(() => {
    if (!host || editor) return;
    untrack(() => {
      editor = new EditorView({
        parent: host!,
        state: EditorState.create({
          doc: text,
          extensions: queryExtensions({
            language,
            run: () => run(),
            change: (t) => {
              text = t;
              if (conn) store.set(qKey(conn), t);
            },
          }),
        }),
      });
    });
  });
  onDestroy(() => editor?.destroy());

  function setText(t: string) {
    text = t;
    if (editor && editor.state.doc.toString() !== t) editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: t } });
  }

  // SQL for the engine, with table and column names to complete.
  $effect(() => {
    const engine = conn?.engine;
    const s = schema?.data;
    if (!editor || !engine) return;
    untrack(async () => {
      if (engine === "redis") {
        editor!.dispatch({ effects: language.reconfigure([]) });
        return;
      }
      const m = await import("@codemirror/lang-sql");
      const dialect = engine === "postgres" ? m.PostgreSQL : engine === "mssql" ? m.MSSQL : m.SQLite;
      editor?.dispatch({ effects: language.reconfigure(m.sql({ dialect, upperCaseKeywords: true, schema: s ? completionSchema(engine, s) : undefined })) });
    });
  });

  // The clock while a query runs.
  $effect(() => {
    if (!running) return;
    const id = setInterval(() => (now = Date.now()), 100);
    return () => clearInterval(id);
  });

  /** Runs the selection, or the whole editor. */
  async function run(sqlText?: string) {
    const c = conn;
    if (!c || running) return;
    const sel = editor?.state.selection.main;
    const q = (sqlText ?? (sel && !sel.empty ? editor!.state.sliceDoc(sel.from, sel.to) : text)).trim();
    if (!q) return;
    const write = writes && c.allowWrites;
    if (write && looksLikeWrite(c.engine, q)) {
      const ok = await app.ask({
        title: `Change data in ${c.name}?`,
        body: "Writes are on for this connection. Tern will run this as written:",
        code: q.length > 600 ? q.slice(0, 600) + "…" : q,
        confirm: "Run it",
      });
      if (!ok) return;
    }
    running = true;
    lastQ = q;
    remember(c, q);
    started = now = Date.now();
    error = "";
    try {
      result = await api.dbQuery($state.snapshot(c) as DbConn, q, write);
      tab = Math.max(0, result.sets.findIndex((s) => s.columns.length > 0));
      if (write && looksLikeWrite(c.engine, q)) refreshSchema();
    } catch (e) {
      error = String(e);
      result = null;
    } finally {
      running = false;
    }
  }

  function openTable(t: DbTable) {
    if (!conn) return;
    const q = openQuery(conn.engine, t);
    setText(q);
    run(q);
  }

  // The tree: tables by schema (or Redis keys by prefix), filtered.
  const groups = $derived.by(() => {
    const tables = schema?.data?.tables ?? [];
    const q = filter.trim().toLowerCase();
    const m = new Map<string, DbTable[]>();
    for (const t of tables) {
      if (q && !t.name.toLowerCase().includes(q) && !t.columns.some((c) => c.name.toLowerCase().includes(q))) continue;
      const g = t.schema || (conn?.engine === "redis" ? "(no prefix)" : "");
      m.set(g, [...(m.get(g) ?? []), t]);
    }
    return [...m.entries()];
  });
  const oneGroup = $derived(groups.length === 1);
  const tkey = (t: DbTable) => `${t.schema}.${t.name}`;
  const toggle = (set: Set<string>, k: string) => {
    const n = new Set(set);
    if (n.has(k)) n.delete(k);
    else n.add(k);
    return n;
  };

  function connMenu(e: MouseEvent, c: DbConn) {
    e.preventDefault();
    app.showMenu(e, c.name, [
      { label: "Edit…", run: () => (form = { conn: c }) },
      { label: "Refresh the schema", run: () => app.loadDbSchema(c.id) },
      { label: "Copy schema for agent", run: () => copySchema(c) },
      { label: "Disconnect", run: () => api.dbDisconnect(c.id).then(() => app.notify(`Disconnected from ${c.name}.`)) },
      "sep",
      { label: "Delete", danger: true, run: () => removeConn(c) },
    ]);
  }

  async function removeConn(c: DbConn) {
    const ok = await app.ask({
      title: `Delete ${c.name}?`,
      body: "This removes the saved connection and its password from the keychain. The database itself is not touched.",
      confirm: "Delete",
      danger: true,
    });
    if (ok) await app.removeDbConn(c.id);
  }

  function copySchema(c: DbConn | null = conn, only?: DbTable[]) {
    const s = c && (fixed ? localSchema?.data : app.dbSchemas[c.id]?.data);
    if (!c || !s) return app.notify("Open the connection first, so Tern has its schema.", "err");
    app.copyValue(schemaText(c.engine, s, only), only ? `the schema of ${only.map((t) => t.name).join(", ")}` : `the schema of ${c.name}`);
  }

  function tableMenu(e: MouseEvent, t: DbTable) {
    e.preventDefault();
    if (!conn) return;
    const name = conn.engine === "redis" ? t.name : tableName(conn.engine, t);
    app.showMenu(e, t.name, [
      { label: conn.engine === "redis" ? "Show the value" : "Show the first 100 rows", run: () => openTable(t) },
      ...(conn.engine === "redis" ? [] : [{ label: "Count the rows", run: () => { const q = `SELECT COUNT(*) AS rows FROM ${name};`; setText(q); run(q); } }]),
      { label: "Copy the name", run: () => app.copyValue(name) },
      ...(conn.engine === "redis" ? [] : [{ label: "Copy schema for agent", run: () => copySchema(conn, [t]) }]),
    ]);
  }

  const set = $derived(result?.sets[tab] ?? null);

  function copyMenu(e: MouseEvent) {
    const s = set;
    const c = conn;
    if (!s || !c) return;
    app.showMenu(e, "Copy the results", [
      { label: "For agent (query and a Markdown table)", run: () => app.copyValue(`${c.engine === "redis" ? "Redis" : engineLabel[c.engine]} query on ${c.name}:\n\n\`\`\`${c.engine === "redis" ? "" : "sql"}\n${text.trim()}\n\`\`\`\n\nResult${s.more ? ` (first ${s.rows.length.toLocaleString()} of ${(s.rows.length + s.more).toLocaleString()} rows)` : ""}:\n\n${toMarkdown(s)}`, "the query and results for an agent") },
      { label: "As tab-separated text (pastes into Excel)", run: () => app.copyValue(toTsv(s), "the results") },
      { label: "As CSV", run: () => app.copyValue(toCsv(s), "the results as CSV") },
      { label: "As JSON", run: () => app.copyValue(toJson(s), "the results as JSON") },
      { label: "As a Markdown table", run: () => app.copyValue(toMarkdown(s, 10_000), "the results as Markdown") },
    ]);
  }

  /** Drags the line between the editor and the results. */
  function dragSplit(e: PointerEvent) {
    const startY = e.clientY;
    const startH = editorH;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => (editorH = Math.max(80, Math.min(window.innerHeight - 260, startH + ev.clientY - startY)));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      store.set("dbEditorH", String(Math.round(editorH)));
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  /** Drags the line between the sidebar and the editor. */
  function dragSide(e: PointerEvent) {
    const startX = e.clientX;
    const start = sideW;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => (sideW = clamp(start + ev.clientX - startX, SIDE_MIN, SIDE_MAX));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      store.set("dbSideW", String(Math.round(sideW)));
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !form && !fixed && !(e.target as HTMLElement).closest(".cm-editor")) app.closeData();
  }

  const placeholder: Record<DbEngine, string> = {
    postgres: "SELECT * FROM … LIMIT 100;   Ctrl Enter runs it. Select text to run only that.",
    mssql: "SELECT TOP 100 * FROM …;   Ctrl Enter runs it. Select text to run only that.",
    sqlite: "SELECT * FROM … LIMIT 100;   Ctrl Enter runs it.",
    redis: "One command per line, like redis-cli:  GET key   HGETALL user:1   SCAN 0 MATCH user:* COUNT 100",
  };
  const redisKind: Record<string, string> = { string: "str", hash: "hash", list: "list", set: "set", zset: "zset", stream: "strm", "ReJSON-RL": "json", TSDB_TYPE: "ts" };
  const ago = $derived(running ? ((now - started) / 1000).toFixed(1) : "");
  void tick;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="data" class:fixed onkeydown={onKey} style:--sw="{sideW}px">
  <aside class="side">
    {#if !fixed}
      <div class="toolbar">
        <span class="title">Databases</span>
        <span class="grow"></span>
        <button class="icon-btn" onclick={() => (form = { conn: null })} aria-label="New connection" use:hover={() => ({ title: "New connection" })}>
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
        </button>
      </div>
      <div class="conns" role="listbox" aria-label="Connections">
        {#each app.dbConns as c (c.id)}
          <button class="conn" class:on={c.id === app.dbSel} role="option" aria-selected={c.id === app.dbSel}
            onclick={() => app.selectDb(c.id)} ondblclick={() => (form = { conn: c })} oncontextmenu={(e) => connMenu(e, c)}>
            <span class="dot" style:background={c.color || "var(--faint)"}></span>
            <span class="cname">{c.name}</span>
            <span class="eng">{engineLabel[c.engine]}</span>
            {#if c.allowWrites}<span class="chip" use:hover={() => ({ title: "Writes allowed", text: "Turn writes on in the bar to change data. Tern asks first." })}>RW</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    {#if conn}
      <div class="toolbar sub schema-head" class:top={!!fixed}>
        <input class="field grow" bind:value={filter} placeholder={conn.engine === "redis" ? "Filter keys…" : "Filter tables and columns…"} spellcheck="false" aria-label="Filter the schema" />
        <button class="icon-btn sm" onclick={refreshSchema} aria-label="Refresh the schema" class:spin={schema?.loading} use:hover={() => ({ title: "Refresh the schema" })}>
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.6M13 3v2.5h-2.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
      </div>
      <div class="tree">
        {#if schema?.error}
          <div class="state inline">
            <span class="state-title">Could not connect</span>
            <pre>{schema.error}</pre>
            <div class="actions">
              <button class="btn sm" onclick={refreshSchema}>Try again</button>
              {#if !fixed}<button class="btn sm ghost" onclick={() => (form = { conn })}>Edit the connection…</button>{/if}
            </div>
          </div>
        {:else if !schema?.data}
          {#if schema?.loading}
            <div class="skel" aria-hidden="true">
              {#each [0, 1, 2, 3, 4, 5] as i}<span class="skeleton" style:width="{75 - i * 8}%"></span>{/each}
            </div>
          {/if}
        {:else if !schema.data.tables.length}
          <div class="state"><span class="state-title">{conn.engine === "redis" ? "No keys" : "No tables"}</span><p>{conn.engine === "redis" ? "This database has no keys yet." : "This database has no tables yet."}</p></div>
        {:else}
          {#each groups as [g, tables] (g)}
            {@const gopen = oneGroup || filter || openGroups.has(g) || ((groups.length < 4 || (schema.data.tables.length <= 60)) && !openGroups.has("!" + g))}
            {#if !oneGroup}
              <button class="group" onclick={() => (openGroups = gopen ? toggle(toggle(openGroups, "!" + g), g) : toggle(openGroups, g))}>
                <span class="chev">{gopen ? "▾" : "▸"}</span>{g || "(default)"}<span class="faint n">{tables.length}</span>
              </button>
            {/if}
            {#if gopen}
              {@const limit = shownPerGroup[g] ?? 300}
              {#each tables.slice(0, limit) as t (tkey(t))}
                {@const topen = openTables.has(tkey(t))}
                <div class="trow" class:nested={!oneGroup}>
                  {#if t.columns.length}
                    <button class="chev tbtn-x" onclick={() => (openTables = toggle(openTables, tkey(t)))} aria-label={topen ? "Hide columns" : "Show columns"}>{topen ? "▾" : "▸"}</button>
                  {:else}
                    <span class="chev"></span>
                  {/if}
                  <button class="tname" onclick={() => openTable(t)} oncontextmenu={(e) => tableMenu(e, t)} title={conn.engine === "redis" ? `${t.name} (${t.kind})` : `Show the first 100 rows of ${t.name}`}>
                    <span class="kind k-{t.kind}">{conn.engine === "redis" ? (redisKind[t.kind] ?? t.kind.slice(0, 4)) : t.kind === "view" ? "view" : ""}</span>{conn.engine === "redis" && t.schema ? t.name.slice(t.schema.length + 1) || t.name : t.name}
                  </button>
                </div>
                {#if topen}
                  {#each t.columns as c}
                    <div class="col" class:nested={!oneGroup}>
                      <span class="cn" class:key={c.key} title={c.key ? "Primary key" : undefined}>{c.name}{#if c.key}<span class="pk">PK</span>{/if}</span>
                      <span class="ct">{c.kind}{c.nullable ? "" : " ·"}</span>
                    </div>
                  {/each}
                {/if}
              {/each}
              {#if tables.length > limit}
                <button class="link more" onclick={() => (shownPerGroup = { ...shownPerGroup, [g]: limit + 1000 })}>Show {Math.min(1000, tables.length - limit)} more</button>
              {/if}
            {/if}
          {/each}
          {#if schema.data.truncated}<p class="faint pad small">{conn.engine === "redis" ? "Only the first 2,000 keys are listed. Use SCAN with MATCH to find others." : "The schema is cut short: it has very many columns."}</p>{/if}
        {/if}
      </div>
    {/if}
  </aside>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="vsplit" role="separator" aria-orientation="vertical" aria-label="Resize the sidebar" onpointerdown={dragSide}
    ondblclick={() => ((sideW = fixed ? 200 : 260), store.set("dbSideW", String(sideW)))}></div>

  <section class="main">
    {#if !conn}
      <div class="state">
        <span class="state-title">Look at your data without leaving Tern</span>
        <p>Connect to SQLite, Postgres, SQL Server, or Redis. Connections are read-only unless you turn writes on, and passwords stay in your system keychain.</p>
        <div class="actions"><button class="btn primary" onclick={() => (form = { conn: null })}>New connection…</button></div>
      </div>
    {:else}
      <div class="toolbar">
        {#if !fixed}
          <span class="dot" style:background={conn.color || "var(--faint)"}></span>
          <span class="title">{conn.name}</span>
        {/if}
        <span class="meta">{schema?.data?.version ?? engineLabel[conn.engine]}</span>
        <span class="grow"></span>
        {#if conn.allowWrites}
          <button class="btn ghost sm" class:danger={writes} class:on={writes} aria-pressed={writes} onclick={() => (writes = !writes)}
            use:hover={() => ({ title: writes ? "Writes on" : "Read-only", text: writes ? "Queries may change data. Tern asks first." : "Queries cannot change data.", hint: "Click to switch" })}>
            {writes ? "Writes on" : "Read-only"}
          </button>
        {:else}
          <span class="chip" use:hover={() => ({ title: "Read-only", text: "Edit the connection to allow writes." })}>Read-only</span>
        {/if}
        <span class="sep"></span>
        <button class="btn ghost sm" onclick={historyMenu} aria-haspopup="menu" use:hover={() => ({ title: "History", text: "The last 50 queries run on this connection." })}>
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><circle cx="8" cy="8" r="5.75" fill="none" stroke="currentColor" stroke-width="1.4" /><path d="M8 5v3.2l2 1.3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
          History
        </button>
        <button class="btn ghost sm" onclick={() => copySchema()} use:hover={() => ({ title: "Copy schema", text: "The tables and columns as short text for an agent." })}>Copy schema</button>
        <button class="btn primary sm run" onclick={() => run()} disabled={running} use:hover={() => ({ title: "Run", text: "With text selected, runs only that.", hint: "Ctrl Enter" })}>
          {running ? `Running ${ago}s` : "Run"}
        </button>
      </div>
      <div class="editor" style:height="{editorH}px">
        <div class="cmhost" bind:this={host}></div>
        {#if !text}<span class="ph mono">{placeholder[conn.engine]}</span>{/if}
      </div>
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="split" role="separator" aria-orientation="horizontal" aria-label="Resize the editor" onpointerdown={dragSplit}
        ondblclick={() => ((editorH = 200), store.set("dbEditorH", "200"))}></div>
      <div class="results">
        {#if error}
          <div class="state inline">
            <span class="state-title">The query failed</span>
            <pre>{error.replace(/^Error: /, "")}{#if /read-only/i.test(error) && !writes}

{conn.allowWrites ? "To change data, click Read-only in the bar above to turn writes on." : fixed ? "Files opened from the file list are always read-only. Add the file as a connection to change it." : "To change data, edit the connection and turn on Allow writes."}{/if}</pre>
          </div>
        {:else if result}
          <div class="toolbar sub">
            {#if result.sets.length > 1}
              <div class="seg" role="tablist">
                {#each result.sets as s, i}
                  <button class:on={tab === i} role="tab" aria-selected={tab === i} onclick={() => (tab = i)}>
                    {s.columns.length ? `Result ${i + 1}` : `Statement ${i + 1}`}
                  </button>
                {/each}
              </div>
            {/if}
            {#if set}
              <span class="meta">
                {#if set.columns.length}
                  {set.more ? `First ${set.rows.length.toLocaleString()} of ${(set.rows.length + set.more).toLocaleString()} rows` : `${set.rows.length.toLocaleString()} ${set.rows.length === 1 ? "row" : "rows"}`}
                {:else if set.affected !== null}
                  {set.affected.toLocaleString()} {set.affected === 1 ? "row" : "rows"} changed
                {:else}
                  Done
                {/if}
                · {result.ms.toLocaleString()} ms
              </span>
            {:else}
              <span class="meta">No result · {result.ms.toLocaleString()} ms</span>
            {/if}
            {#if result.note && looksLikeWrite(conn.engine, lastQ)}<span class="note">{result.note}</span>{/if}
            <span class="grow"></span>
            {#if set?.columns.length}<button class="btn ghost sm" onclick={copyMenu} aria-haspopup="menu">Copy ▾</button>{/if}
          </div>
          {#if set?.columns.length}
            {#if set.rows.length}
              <DbGrid {set} />
            {:else}
              <div class="state">
                <span class="state-title">No rows</span>
                <p>Columns: {set.columns.join(", ")}</p>
              </div>
            {/if}
          {/if}
        {:else if !running}
          <div class="state">
            <span class="state-title">{conn.engine === "redis" ? "Run a command" : "Run a query"}</span>
            <p>
              {#if conn.engine === "redis"}Click a key on the left to see its value, or type a command and press <kbd>Ctrl Enter</kbd>.
              {:else}Click a table on the left for its first 100 rows, or write a query and press <kbd>Ctrl Enter</kbd>. Results show the first 1,000 rows.{/if}
            </p>
          </div>
        {/if}
      </div>
    {/if}
  </section>
</div>

{#if form}
  <DbConnForm conn={form.conn} onclose={() => (form = null)} />
{/if}

<style>
  .data {
    grid-column: 3 / -1;
    grid-row: 2;
    display: grid;
    grid-template-columns: var(--sw, 260px) 1px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .data.fixed {
    position: absolute;
    inset: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--panel);
  }
  .vsplit {
    position: relative;
    background: var(--line);
    cursor: col-resize;
  }
  .vsplit::after {
    content: "";
    position: absolute;
    inset: 0 -3px;
    z-index: 1;
  }
  .vsplit:hover {
    background: var(--line-strong);
  }
  .spin svg {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .conns {
    display: flex;
    flex-direction: column;
    padding: var(--s1) var(--s2) var(--s3);
    max-height: 40%;
    overflow: auto;
  }
  .conn {
    display: flex;
    align-items: center;
    gap: var(--s3);
    height: var(--h-row);
    padding: 0 var(--s3);
    border-radius: var(--radius);
    text-align: left;
    font-size: var(--fs-md);
    flex-shrink: 0;
  }
  .conn:hover {
    background: var(--hover);
  }
  .conn.on {
    background: var(--raised);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .cname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .eng {
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .schema-head {
    gap: var(--s1);
    border-top: 1px solid var(--line);
  }
  .schema-head.top {
    border-top: 0;
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--s1) var(--s1) var(--s4);
  }
  .skel {
    display: flex;
    flex-direction: column;
    gap: var(--s3);
    padding: var(--s3) var(--s4);
  }
  .skel .skeleton {
    height: 10px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--s1);
    width: 100%;
    padding: var(--s1) var(--s2);
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--muted);
    text-align: left;
  }
  .group .n {
    margin-left: auto;
    font-weight: 400;
    font-size: var(--fs-xs);
  }
  .chev {
    width: 14px;
    flex-shrink: 0;
    color: var(--faint);
    font-size: var(--fs-xs);
    text-align: center;
  }
  .trow {
    display: flex;
    align-items: center;
  }
  .trow.nested {
    padding-left: var(--s4);
  }
  .col.nested {
    padding-left: calc(var(--s6) + var(--s3) + var(--s4));
  }
  .tname {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: var(--s2);
    align-items: baseline;
    padding: var(--s1) var(--s2);
    border-radius: var(--r-sm);
    font: var(--fs-sm) var(--ui);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tname:hover,
  .group:hover {
    background: var(--hover);
    border-radius: var(--r-sm);
  }
  .kind {
    font: var(--fs-xs) var(--ui);
    color: var(--faint);
    min-width: 0;
  }
  .kind.k-view {
    color: var(--blue);
  }
  .kind.k-hash {
    color: var(--syn-prop);
  }
  .kind.k-zset,
  .kind.k-set {
    color: var(--syn-type);
  }
  .kind.k-list {
    color: var(--syn-number);
  }
  .kind.k-string {
    color: var(--syn-string);
  }
  .col {
    display: flex;
    gap: var(--s3);
    padding: 1px var(--s2) 1px calc(var(--s6) + var(--s3));
    font: var(--fs-xs) / 18px var(--ui);
  }
  .cn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cn.key {
    color: var(--text);
    font-weight: 600;
  }
  .pk {
    margin-left: var(--s1);
    padding: 0 var(--s1);
    font-weight: 600;
    color: var(--muted);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
  }
  .ct {
    color: var(--faint);
    font-family: var(--mono);
    white-space: nowrap;
  }
  .link.more {
    padding: var(--s1) var(--s6);
    font-size: var(--fs-sm);
  }
  .pad {
    padding: var(--s3) var(--s4);
    margin: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .run {
    min-width: 54px;
  }
  .editor {
    position: relative;
    min-height: 80px;
    flex-shrink: 0;
  }
  .cmhost {
    position: absolute;
    inset: 0;
    overflow: auto;
  }
  .cmhost :global(.cm-editor) {
    height: 100%;
  }
  .ph {
    position: absolute;
    top: var(--s2);
    left: 44px;
    right: var(--s5);
    color: var(--faint);
    font-size: var(--fs-sm);
    pointer-events: none;
  }
  .split {
    height: 5px;
    flex-shrink: 0;
    cursor: row-resize;
    border-top: 1px solid var(--line);
    background: var(--panel);
  }
  .split:hover {
    background: var(--hover);
  }
  .results {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .results > .state.inline {
    padding: var(--s4);
  }
  .note {
    font-size: var(--fs-xs);
    color: var(--muted);
  }
</style>
