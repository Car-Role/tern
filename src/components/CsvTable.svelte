<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { parseCsv, type CsvResult } from "../lib/csv";
  import { bytes as fmtBytes } from "../lib/format";

  /** `text` is null when the file was too big for the code view; then the table reads it itself (up to 100 MB). */
  let { text, tsv, repo, rel }: { text: string | null; tsv: boolean; repo: string; rel: string } = $props();

  const ROW = 26;
  let data = $state<CsvResult | null>(null);
  let loadNote = $state("");
  let header = $state(true);
  let filter = $state("");
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(600);

  $effect(() => {
    const [t, k, r, p] = [text, tsv, repo, rel];
    data = null;
    loadNote = "";
    (async () => {
      let src = t;
      if (src == null) {
        const big = await api.readText(r, p, 100 * 1024 * 1024);
        if (big.tooLarge || big.text == null) {
          loadNote = `This file is ${fmtBytes(big.size)}. The table view opens files up to 100 MB.`;
          return;
        }
        src = big.text;
      }
      // Let the "Reading" note paint before a long parse.
      await new Promise((res) => setTimeout(res, 0));
      const res = parseCsv(src, k ? "\t" : undefined);
      if (r === repo && p === rel) data = res;
    })().catch((e) => (loadNote = String(e)));
  });

  const head = $derived(data && header ? (data.rows[0] ?? []) : []);
  const all = $derived.by(() => {
    if (!data) return [] as { n: number; cells: string[] }[];
    return data.rows.slice(header ? 1 : 0).map((cells, i) => ({ n: i + 1, cells }));
  });
  /** Each row as one lowercase line, made once, so filtering a big table stays quick. */
  const flatOf = new WeakMap<object, string[]>();
  /** The filter, applied 150 ms after the last key press. */
  let query = $state("");
  $effect(() => {
    const q = filter.trim().toLowerCase();
    const id = setTimeout(() => (query = q), all.length > 20_000 ? 150 : 0);
    return () => clearTimeout(id);
  });
  const body = $derived.by(() => {
    if (!query) return all;
    let f = flatOf.get(all);
    if (!f) flatOf.set(all, (f = all.map((r) => r.cells.join("\u0001").toLowerCase())));
    const q = query;
    return all.filter((_, i) => f[i].includes(q));
  });
  /** Column widths from a sample of rows, so a huge file does not have to be measured. */
  const widths = $derived.by(() => {
    if (!data) return [] as number[];
    const w = Array.from({ length: data.columns }, () => 4);
    for (const row of data.rows.slice(0, 300)) row.forEach((c, i) => (w[i] = Math.max(w[i], Math.min(c.length, 48))));
    return w.map((n) => Math.max(64, Math.min(380, n * 7.4 + 22)));
  });
  const numW = $derived(Math.max(44, String(body.length).length * 8 + 20));
  const total = $derived(numW + widths.reduce((a, b) => a + b, 0));
  // After the list changes length, read the real scroll position again (the browser may have moved it).
  $effect(() => {
    body.length;
    if (scroller) scrollTop = scroller.scrollTop;
  });
  // Clamped: when a filter makes the list shorter, the old scroll position can be past its end.
  const first = $derived(Math.max(0, Math.min(Math.floor(scrollTop / ROW), body.length - Math.ceil(viewH / ROW)) - 10));
  const visible = $derived(body.slice(first, first + Math.ceil(viewH / ROW) + 20));

  function cellMenu(e: MouseEvent, r: { n: number; cells: string[] }, i: number) {
    e.preventDefault();
    app.showMenu(e, `Row ${r.n}`, [
      { label: "Copy this cell", run: () => app.copyValue(r.cells[i] ?? "") },
      { label: "Copy the row", run: () => app.copyValue(r.cells.join("\t"), "the row") },
      ...(head.length ? [{ label: "Copy the row as JSON", run: () => app.copyValue(JSON.stringify(Object.fromEntries(head.map((h, k) => [h || `column ${k + 1}`, r.cells[k] ?? ""])), null, 2), "the row as JSON") }] : []),
    ]);
  }
</script>

<div class="csv">
  <div class="bar">
    {#if data}
      <span class="faint">
        {body.length.toLocaleString()}{filter ? ` of ${(data.rows.length - (header ? 1 : 0)).toLocaleString()}` : ""} rows · {data.columns} columns ·
        {data.delimiter === "\t" ? "tabs" : data.delimiter === ";" ? "semicolons" : data.delimiter === "|" ? "pipes" : "commas"}
      </span>
      <label class="hdr"><input type="checkbox" bind:checked={header} /> First row is the header</label>
      <span class="grow"></span>
      <input class="filter" bind:value={filter} placeholder="Filter rows" spellcheck="false" aria-label="Filter rows" />
    {/if}
  </div>
  {#if data?.problems.length}
    <p class="warn">{data.problems.join(" ")}</p>
  {/if}
  {#if loadNote}
    <p class="note">{loadNote}</p>
  {:else if !data}
    <p class="note">Reading the table…</p>
  {:else if !data.rows.length}
    <p class="note">This file has no rows.</p>
  {:else}
    <div class="scroll" bind:this={scroller} bind:clientHeight={viewH} onscroll={() => (scrollTop = scroller!.scrollTop)}>
      <div class="inner" style:width="{total}px">
        {#if head.length}
          <div class="thead" style:width="{total}px">
            <span class="num" style:width="{numW}px">#</span>
            {#each widths as w, i}<span class="th" style:width="{w}px" title={head[i]}>{head[i] ?? ""}</span>{/each}
          </div>
        {/if}
        <div class="space" style:height="{body.length * ROW}px">
          {#each visible as r, j (r.n)}
            <div class="tr" style:top="{(first + j) * ROW}px">
              <span class="num" style:width="{numW}px">{r.n}</span>
              {#each widths as w, i}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span class="td" class:missing={i >= r.cells.length} style:width="{w}px" title={r.cells[i]} oncontextmenu={(e) => cellMenu(e, r, i)}>{r.cells[i] ?? ""}</span>
              {/each}
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .csv {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 5px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    min-height: 34px;
  }
  .grow {
    flex: 1;
  }
  .hdr {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
  }
  .hdr input {
    accent-color: var(--amber);
  }
  .filter {
    width: 200px;
    font: inherit;
    font-size: 12px;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 3px 8px;
    outline: none;
  }
  .filter:focus {
    border-color: var(--amber);
  }
  .warn {
    margin: 0;
    padding: 6px 14px;
    font-size: 12px;
    color: var(--amber);
    background: var(--amber-soft);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .inner {
    position: relative;
    min-width: 100%;
  }
  .thead {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    background: var(--panel);
    border-bottom: 1px solid var(--line-strong);
  }
  .space {
    position: relative;
  }
  .tr {
    position: absolute;
    left: 0;
    height: 26px;
    display: flex;
  }
  .tr:hover .td {
    background: var(--hover);
  }
  .num,
  .th,
  .td {
    flex-shrink: 0;
    height: 26px;
    line-height: 26px;
    padding: 0 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-right: 1px solid var(--line);
    font-size: 12.5px;
  }
  .th {
    font-weight: 600;
  }
  .num {
    color: var(--faint);
    font: 11.5px/26px var(--mono);
    text-align: right;
    background: var(--panel);
    position: sticky;
    left: 0;
  }
  .td {
    border-bottom: 1px solid var(--line);
    user-select: text;
    font-family: var(--mono);
    font-size: 12px;
  }
  .td.missing {
    background: color-mix(in srgb, var(--panel) 60%, transparent);
  }
  .note {
    margin: 20px;
    color: var(--muted);
  }
</style>
