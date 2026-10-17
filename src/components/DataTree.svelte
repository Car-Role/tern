<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { childCount, children, isBranch, parseData, pathString, preview, typeOf, type DataKind, type Parsed } from "../lib/datatree";
  import { bytes as fmtBytes } from "../lib/format";

  /** `text` is null when the file was too big for the code view; then the tree reads it itself (up to 50 MB). */
  let { text, kind, repo, rel }: { text: string | null; kind: DataKind; repo: string; rel: string } = $props();

  const ROW = 22;
  const PAGE = 200;
  let parsed = $state<Parsed | null>(null);
  let loadNote = $state("");
  let open = $state(new Set<string>([""]));
  /** How many children a big node shows, by its path. */
  let shown = $state<Record<string, number>>({});
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(600);

  $effect(() => {
    const [t, k, r, p] = [text, kind, repo, rel];
    parsed = null;
    loadNote = "";
    open = new Set([""]);
    shown = {};
    (async () => {
      let src = t;
      if (src == null) {
        const big = await api.readText(r, p, 50 * 1024 * 1024);
        if (big.tooLarge || big.text == null) {
          loadNote = `This file is ${fmtBytes(big.size)}. The tree view opens files up to 50 MB.`;
          return;
        }
        src = big.text;
      }
      const res = await parseData(src, k);
      if (r === repo && p === rel) {
        parsed = res;
        // Open the first level of a document that has a single root.
        if (res.ok && isBranch(res.value) && childCount(res.value) <= 50) open = new Set(["", ...children(res.value, 0, 50).filter((c) => isBranch(c.value)).map((c) => "/" + c.key)]);
      }
    })().catch((e) => (loadNote = String(e)));
  });

  type Row = { id: string; depth: number; key: string | null; value: unknown; inArray: boolean; trail: { key: string; inArray: boolean }[] } | { id: string; depth: number; more: string; left: number };

  /** The rows on screen, from the open nodes only. */
  const rows = $derived.by((): Row[] => {
    if (!parsed?.ok) return [];
    const out: Row[] = [];
    const walk = (v: unknown, id: string, depth: number, trail: { key: string; inArray: boolean }[]) => {
      if (!isBranch(v) || !open.has(id)) return;
      const total = childCount(v);
      const limit = shown[id] ?? PAGE;
      const arr = Array.isArray(v);
      for (const c of children(v, 0, limit)) {
        const cid = `${id}/${c.key}`;
        const t2 = [...trail, { key: c.key, inArray: arr }];
        out.push({ id: cid, depth, key: c.key, value: c.value, inArray: arr, trail: t2 });
        if (out.length > 200_000) return;
        walk(c.value, cid, depth + 1, t2);
      }
      if (total > limit) out.push({ id: `${id}#more`, depth, more: id, left: total - limit });
    };
    out.push({ id: "", depth: 0, key: null, value: parsed.value, inArray: false, trail: [] });
    walk(parsed.value, "", 1, []);
    return out;
  });

  // After the list changes length, read the real scroll position again (the browser may have moved it).
  $effect(() => {
    rows.length;
    if (scroller) scrollTop = scroller.scrollTop;
  });
  // Clamped: when a filter makes the list shorter, the old scroll position can be past its end.
  const first = $derived(Math.max(0, Math.min(Math.floor(scrollTop / ROW), rows.length - Math.ceil(viewH / ROW)) - 10));
  const visible = $derived(rows.slice(first, first + Math.ceil(viewH / ROW) + 20));

  function toggle(id: string) {
    const next = new Set(open);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    open = next;
  }

  /** Opens every branch, up to a few thousand, so a huge file cannot freeze the window. */
  function expandAll() {
    if (!parsed?.ok) return;
    const next = new Set<string>([""]);
    const stack: [unknown, string][] = [[parsed.value, ""]];
    while (stack.length && next.size < 3000) {
      const [v, id] = stack.pop()!;
      for (const c of children(v, 0, PAGE)) {
        if (isBranch(c.value)) {
          next.add(`${id}/${c.key}`);
          stack.push([c.value, `${id}/${c.key}`]);
        }
      }
    }
    open = next;
  }

  function menu(e: MouseEvent, r: Row) {
    if ("more" in r) return;
    e.preventDefault();
    const path = pathString(r.trail);
    app.showMenu(e, path || "Document", [
      { label: "Copy the path", run: () => app.copyValue(path || "$") },
      { label: "Copy the value as JSON", run: () => app.copyValue(JSON.stringify(r.value, null, 2) ?? "null") },
      ...(typeOf(r.value) === "string" ? [{ label: "Copy the text", run: () => app.copyValue(String(r.value)) }] : []),
    ]);
  }
</script>

<div class="tree">
  <div class="bar">
    {#if parsed?.ok}
      <span class="faint">{kind.toUpperCase()}{parsed.docs > 1 ? ` · ${parsed.docs} documents` : ""} · {rows.length.toLocaleString()} rows shown</span>
      <span class="grow"></span>
      <button class="b" onclick={expandAll}>Expand all</button>
      <button class="b" onclick={() => (open = new Set([""]))}>Collapse all</button>
    {/if}
  </div>
  {#if loadNote}
    <p class="note">{loadNote}</p>
  {:else if !parsed}
    <p class="note">Reading…</p>
  {:else if !parsed.ok}
    <div class="note err">
      <p><b>This {kind.toUpperCase()} has a mistake{parsed.line ? ` on line ${parsed.line}${parsed.col ? `, column ${parsed.col}` : ""}` : ""}.</b></p>
      <p class="mono">{parsed.message}</p>
      <button class="b" onclick={() => app.mdPreview && app.toggleMdPreview()}>Show the source</button>
    </div>
  {:else}
    <div class="scroll" bind:this={scroller} bind:clientHeight={viewH} onscroll={() => (scrollTop = scroller!.scrollTop)}>
      <div style:height="{rows.length * ROW}px" class="space">
        {#each visible as r, i (r.id)}
          {@const top = (first + i) * ROW}
          {#if "more" in r}
            <button class="row more" style:top="{top}px" style:padding-left="{14 + r.depth * 16}px"
              onclick={() => (shown = { ...shown, [r.more]: (shown[r.more] ?? PAGE) + PAGE * 5 })}>
              Show {Math.min(r.left, PAGE * 5).toLocaleString()} more of {r.left.toLocaleString()}
            </button>
          {:else}
            {@const branch = isBranch(r.value) && childCount(r.value) > 0}
            {@const t = typeOf(r.value)}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div class="row" style:top="{top}px" style:padding-left="{14 + r.depth * 16}px" oncontextmenu={(e) => menu(e, r)}
              onclick={() => branch && toggle(r.id)} class:branch>
              <span class="chev">{branch ? (open.has(r.id) ? "▾" : "▸") : ""}</span>
              {#if r.key !== null}<span class="key" class:idx={r.inArray}>{r.inArray ? r.key : r.key}</span><span class="colon">:</span>{/if}
              <span class="val t-{t}">{preview(r.value)}</span>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .tree {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    min-height: 33px;
  }
  .grow {
    flex: 1;
  }
  .b {
    font-size: 12px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 1px 8px;
  }
  .b:hover {
    color: var(--text);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .space {
    position: relative;
    min-width: 100%;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 22px;
    display: flex;
    align-items: center;
    gap: 4px;
    font: 12.5px/22px var(--mono);
    white-space: nowrap;
    cursor: default;
    user-select: text;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.branch {
    cursor: pointer;
  }
  .row.more {
    color: var(--blue);
    font-family: var(--ui);
    font-size: 12px;
    text-align: left;
    justify-content: flex-start;
  }
  .chev {
    width: 12px;
    color: var(--faint);
    flex-shrink: 0;
  }
  .key {
    color: var(--syn-prop);
  }
  .key.idx {
    color: var(--faint);
  }
  .colon {
    color: var(--faint);
    margin-right: 4px;
  }
  .t-string {
    color: var(--syn-string);
  }
  .t-number {
    color: var(--syn-number);
  }
  .t-boolean,
  .t-null {
    color: var(--syn-keyword);
  }
  .t-date {
    color: var(--syn-type);
  }
  .t-object,
  .t-array {
    color: var(--faint);
  }
  .note {
    margin: 20px;
    color: var(--muted);
  }
  .note.err b {
    color: var(--del);
  }
  .note.err .mono {
    font-size: 12px;
  }
</style>
