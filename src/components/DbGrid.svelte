<script lang="ts">
  import { app } from "../lib/store.svelte";
  import type { DbResultSet } from "../lib/types";

  let { set }: { set: DbResultSet } = $props();

  const ROW = 26;
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(400);

  const show = (v: unknown) => (v === null || v === undefined ? "NULL" : typeof v === "string" ? v : JSON.stringify(v));
  const isNum = (v: unknown) => typeof v === "number" || (typeof v === "string" && v.length < 40 && /^-?\d+(\.\d+)?(e[+-]?\d+)?$/i.test(v));

  /** Column widths from the names and a sample of rows. */
  const widths = $derived.by(() => {
    const w = set.columns.map((c) => Math.min(c.length, 40) + 2);
    for (const r of set.rows.slice(0, 200)) r.forEach((v, i) => (w[i] = Math.max(w[i] ?? 4, Math.min(show(v).length, 48))));
    return w.map((n) => Math.max(60, Math.min(420, n * 7.3 + 22)));
  });
  /** Numbers line up on the right. A column counts as numbers when its first values are. */
  const numeric = $derived(set.columns.map((_, i) => {
    const vals = set.rows.slice(0, 50).map((r) => r[i]).filter((v) => v !== null && v !== undefined);
    return vals.length > 0 && vals.every(isNum);
  }));
  const numW = $derived(Math.max(40, String(set.rows.length).length * 8 + 18));
  const total = $derived(numW + widths.reduce((a, b) => a + b, 0));
  $effect(() => {
    set;
    if (scroller) {
      scroller.scrollTop = 0;
      scrollTop = 0;
    }
  });
  const first = $derived(Math.max(0, Math.min(Math.floor(scrollTop / ROW), set.rows.length - Math.ceil(viewH / ROW)) - 10));
  const visible = $derived(set.rows.slice(first, first + Math.ceil(viewH / ROW) + 20));

  function cellMenu(e: MouseEvent, row: unknown[], n: number, i: number) {
    e.preventDefault();
    app.showMenu(e, `Row ${n}, ${set.columns[i]}`, [
      { label: "Copy this value", run: () => app.copyValue(row[i] === null ? "" : show(row[i])) },
      { label: "Copy the row as JSON", run: () => app.copyValue(JSON.stringify(Object.fromEntries(set.columns.map((c, k) => [c, row[k] ?? null])), null, 2), "the row as JSON") },
      { label: "Copy the row (tab separated)", run: () => app.copyValue(row.map((v) => (v === null ? "" : show(v))).join("\t"), "the row") },
      { label: `Copy the column name`, run: () => app.copyValue(set.columns[i]) },
    ]);
  }
  function headMenu(e: MouseEvent, i: number) {
    e.preventDefault();
    app.showMenu(e, set.columns[i], [
      { label: "Copy the column name", run: () => app.copyValue(set.columns[i]) },
      { label: "Copy every value in this column", run: () => app.copyValue(set.rows.map((r) => (r[i] === null ? "" : show(r[i]))).join("\n"), "the column") },
    ]);
  }
</script>

<div class="grid" bind:this={scroller} bind:clientHeight={viewH} onscroll={() => (scrollTop = scroller!.scrollTop)}>
  <div class="inner" style:width="{total}px">
    <div class="thead" style:width="{total}px">
      <span class="num" style:width="{numW}px"></span>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      {#each set.columns as c, i}<span class="th" class:right={numeric[i]} style:width="{widths[i]}px" title={c} oncontextmenu={(e) => headMenu(e, i)}>{c}</span>{/each}
    </div>
    <div class="space" style:height="{set.rows.length * ROW}px">
      {#each visible as r, j (first + j)}
        {@const n = first + j + 1}
        <div class="tr" style:top="{(first + j) * ROW}px">
          <span class="num" style:width="{numW}px">{n}</span>
          {#each set.columns as _, i}
            {@const v = r[i]}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span class="td" class:null={v === null || v === undefined} class:right={numeric[i]} style:width="{widths[i]}px"
              title={show(v).length > 30 ? show(v).slice(0, 2000) : undefined} oncontextmenu={(e) => cellMenu(e, r, n, i)}>{show(v)}</span>
          {/each}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .grid {
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
  }
  .th {
    font-size: 12px;
    font-weight: 600;
  }
  .num {
    color: var(--faint);
    font: 11px/26px var(--mono);
    text-align: right;
    background: var(--panel);
    position: sticky;
    left: 0;
  }
  .td {
    border-bottom: 1px solid var(--line);
    font: 12px/26px var(--mono);
    user-select: text;
  }
  .right {
    text-align: right;
  }
  .td.null {
    color: var(--faint);
    font-style: italic;
  }
</style>
