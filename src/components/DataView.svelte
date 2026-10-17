<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { Compartment, EditorState } from "@codemirror/state";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
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
  const qKey = (c: DbConn) => `dbq.${fixed ? "file:" + c.file : c.id}`;

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
      { label: "Edit", run: () => (form = { conn: c }) },
      { label: "Refresh the schema", run: () => app.loadDbSchema(c.id) },
      { label: "Copy the schema for an agent", run: () => copySchema(c) },
      { label: "Disconnect", run: () => api.dbDisconnect(c.id).then(() => app.notify(`Disconnected from ${c.name}.`)) },
      { label: "Delete…", run: () => (form = { conn: c }) },
    ]);
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
      ...(conn.engine === "redis" ? [] : [{ label: "Copy its schema for an agent", run: () => copySchema(conn, [t]) }]),
    ]);
  }

  const set = $derived(result?.sets[tab] ?? null);

  function copyMenu(e: MouseEvent) {
    const s = set;
    const c = conn;
    if (!s || !c) return;
    app.showMenu(e, "Copy the results", [
      { label: "For an agent (query and a Markdown table)", run: () => app.copyValue(`${c.engine === "redis" ? "Redis" : engineLabel[c.engine]} query on ${c.name}:\n\n\`\`\`${c.engine === "redis" ? "" : "sql"}\n${text.trim()}\n\`\`\`\n\nResult${s.more ? ` (first ${s.rows.length.toLocaleString()} of ${(s.rows.length + s.more).toLocaleString()} rows)` : ""}:\n\n${toMarkdown(s)}`, "the query and results for an agent") },
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
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !form && !fixed && !(e.target as HTMLElement).closest(".cm-editor")) app.closeData();
  }

  const placeholder: Record<DbEngine, string> = {
    postgres: "SELECT * FROM … LIMIT 100;   Ctrl+Enter runs it. Select text to run only that.",
    mssql: "SELECT TOP 100 * FROM …;   Ctrl+Enter runs it. Select text to run only that.",
    sqlite: "SELECT * FROM … LIMIT 100;   Ctrl+Enter runs it.",
    redis: "One command per line, like redis-cli:  GET key   HGETALL user:1   SCAN 0 MATCH user:* COUNT 100",
  };
  const redisKind: Record<string, string> = { string: "str", hash: "hash", list: "list", set: "set", zset: "zset", stream: "strm", "ReJSON-RL": "json", TSDB_TYPE: "ts" };
  const ago = $derived(running ? ((now - started) / 1000).toFixed(1) : "");
  void tick;
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="data" class:fixed onkeydown={onKey}>
  <aside class="side">
    {#if !fixed}
      <div class="head">
        <h2>Databases</h2>
        <button class="icon" onclick={() => (form = { conn: null })} title="New connection" aria-label="New connection">
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
            {#if c.allowWrites}<span class="rw" title="Writes allowed">RW</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    {#if conn}
      <div class="schema-head" class:top={!!fixed}>
        <input class="filter" bind:value={filter} placeholder={conn.engine === "redis" ? "Filter keys" : "Filter tables and columns"} spellcheck="false" />
        <button class="icon" onclick={refreshSchema} title="Refresh the schema" aria-label="Refresh the schema" class:spin={schema?.loading}>
          <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.6M13 3v2.5h-2.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
      </div>
      <div class="tree">
        {#if schema?.error}
          <p class="err mono">{schema.error}</p>
          {#if !fixed}<button class="link" onclick={() => (form = { conn })}>Edit the connection</button>{/if}
        {:else if !schema?.data}
          <p class="faint pad">{schema?.loading ? "Connecting…" : ""}</p>
        {:else if !schema.data.tables.length}
          <p class="faint pad">{conn.engine === "redis" ? "No keys." : "No tables."}</p>
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
                      <span class="cn" class:key={c.key}>{c.key ? "🔑 " : ""}{c.name}</span>
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

  <section class="main">
    {#if !conn}
      <div class="empty">
        <h3>Look at your data without leaving Tern</h3>
        <p>Connect to SQLite, Postgres, SQL Server, or Redis. Connections are read-only unless you turn writes on, and passwords stay in your system keychain.</p>
        <button class="btn primary" onclick={() => (form = { conn: null })}>New connection</button>
      </div>
    {:else}
      <div class="bar">
        {#if !fixed}
          <span class="dot" style:background={conn.color || "var(--faint)"}></span>
          <b class="title">{conn.name}</b>
        {/if}
        <span class="faint ver">{schema?.data?.version ?? engineLabel[conn.engine]}</span>
        <span class="grow"></span>
        {#if conn.allowWrites}
          <button class="mode" class:writes onclick={() => (writes = !writes)} title={writes ? "Queries may change data. Tern asks first." : "Queries cannot change data."}>
            {writes ? "Writes on" : "Read-only"}
          </button>
        {:else}
          <span class="mode ro" title="Edit the connection to allow writes">Read-only</span>
        {/if}
        <button class="btn" onclick={() => copySchema()} title="Copy the schema as short text for an agent">Copy schema</button>
        <button class="btn primary" onclick={() => run()} disabled={running} title="Run (Ctrl+Enter). With text selected, runs only that.">
          {running ? `Running ${ago}s` : "Run"}
        </button>
      </div>
      <div class="editor" style:height="{editorH}px">
        <div class="cmhost" bind:this={host}></div>
        {#if !text}<span class="ph mono">{placeholder[conn.engine]}</span>{/if}
      </div>
      <div class="split" role="separator" aria-orientation="horizontal" onpointerdown={dragSplit}></div>
      <div class="results">
        {#if error}
          <pre class="qerr">{error.replace(/^Error: /, "")}{#if /read-only/i.test(error) && !writes}

{conn.allowWrites ? "To change data, click Read-only in the bar above to turn writes on." : fixed ? "Files opened from the file list are always read-only. Add the file as a connection to change it." : "To change data, edit the connection and turn on Allow writes."}{/if}</pre>
        {:else if result}
          <div class="rbar">
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
              <span class="faint">
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
              <span class="faint">No result · {result.ms.toLocaleString()} ms</span>
            {/if}
            {#if result.note && looksLikeWrite(conn.engine, lastQ)}<span class="note">{result.note}</span>{/if}
            <span class="grow"></span>
            {#if set?.columns.length}<button class="btn" onclick={copyMenu}>Copy ▾</button>{/if}
          </div>
          {#if set?.columns.length}
            {#if set.rows.length}
              <DbGrid {set} />
            {:else}
              <p class="faint pad">No rows. Columns: {set.columns.join(", ")}</p>
            {/if}
          {/if}
        {:else if !running}
          <p class="faint pad hint">
            {#if conn.engine === "redis"}Click a key on the left to see its value, or type a command and press <kbd>Ctrl Enter</kbd>.
            {:else}Click a table on the left for its first 100 rows, or write a query and press <kbd>Ctrl Enter</kbd>. Results show the first 1,000 rows.{/if}
          </p>
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
    grid-template-columns: 260px 1fr;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .data.fixed {
    position: absolute;
    inset: 0;
    grid-template-columns: 200px 1fr;
  }
  .side {
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--panel);
  }
  .head {
    display: flex;
    align-items: center;
    padding: 10px 10px 6px 14px;
  }
  h2 {
    flex: 1;
    margin: 0;
    font-size: 13px;
    font-weight: 650;
  }
  .icon {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    color: var(--muted);
  }
  .icon:hover {
    background: var(--hover);
    color: var(--text);
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
    padding: 0 6px 8px;
    max-height: 40%;
    overflow: auto;
  }
  .conn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 6px;
    text-align: left;
    font-size: 13px;
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
    font-size: 11px;
    color: var(--faint);
  }
  .rw {
    font: 600 9.5px var(--mono);
    color: var(--amber);
    border: 1px solid color-mix(in srgb, var(--amber) 50%, transparent);
    border-radius: 3px;
    padding: 0 3px;
  }
  .schema-head {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 8px 8px 6px 10px;
    border-top: 1px solid var(--line);
  }
  .schema-head.top {
    border-top: 0;
    background: var(--panel);
  }
  .filter {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 12px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 8px;
    outline: none;
  }
  .filter:focus {
    border-color: var(--amber);
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 2px 4px 12px;
    background: var(--panel);
  }
  .group {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    padding: 4px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    text-align: left;
  }
  .group .n {
    margin-left: auto;
    font-weight: 400;
    font-size: 11px;
  }
  .chev {
    width: 14px;
    flex-shrink: 0;
    color: var(--faint);
    font-size: 10px;
    text-align: center;
  }
  .trow {
    display: flex;
    align-items: center;
  }
  .trow.nested,
  .col.nested {
    padding-left: 12px;
  }
  .tname {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 6px;
    align-items: baseline;
    padding: 3px 6px;
    border-radius: 5px;
    font: 12.5px var(--mono);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tname:hover,
  .group:hover {
    background: var(--hover);
    border-radius: 5px;
  }
  .kind {
    font: 10px var(--ui);
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
    gap: 8px;
    padding: 1px 6px 1px 34px;
    font: 11.5px/18px var(--mono);
  }
  .cn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cn.key {
    color: var(--amber);
  }
  .ct {
    color: var(--faint);
    white-space: nowrap;
  }
  .err {
    margin: 8px;
    font-size: 11.5px;
    color: var(--del);
    white-space: pre-wrap;
    user-select: text;
  }
  .link {
    color: var(--blue);
    font-size: 12px;
    padding: 0 8px;
  }
  .link.more {
    padding: 4px 26px;
  }
  .pad {
    padding: 8px 12px;
    margin: 0;
  }
  .small {
    font-size: 11.5px;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line);
    min-height: 40px;
  }
  .title {
    font-size: 13px;
  }
  .ver {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .btn {
    font-size: 12px;
    padding: 3px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text);
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) {
    background: var(--hover);
  }
  .btn.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--on-accent);
    min-width: 54px;
  }
  .btn:disabled {
    opacity: 0.7;
  }
  .mode {
    font-size: 11.5px;
    padding: 2px 9px;
    border-radius: 10px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .mode.writes {
    color: var(--on-accent);
    background: var(--del);
    border-color: var(--del);
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
    top: 6px;
    left: 44px;
    right: 16px;
    color: var(--faint);
    font-size: 12px;
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
  .rbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    font-size: 12px;
    border-bottom: 1px solid var(--line);
    min-height: 34px;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    font-size: 12px;
    padding: 2px 9px;
    color: var(--muted);
  }
  .seg button.on {
    background: var(--raised);
    color: var(--text);
  }
  .note {
    font-size: 11.5px;
    color: var(--amber);
  }
  .qerr {
    margin: 12px;
    padding: 10px 12px;
    border-radius: 6px;
    font: 12px/1.5 var(--mono);
    white-space: pre-wrap;
    color: var(--del);
    background: var(--del-bg);
    user-select: text;
  }
  .hint {
    padding: 16px;
  }
  .empty {
    margin: auto;
    max-width: 420px;
    text-align: center;
    padding: 24px;
  }
  .empty h3 {
    margin: 0 0 8px;
    font-size: 15px;
  }
  .empty p {
    color: var(--muted);
    margin: 0 0 16px;
  }
</style>
