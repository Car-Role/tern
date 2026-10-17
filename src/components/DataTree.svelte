<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { childCount, children, isBranch, parseData, pathString, preview, typeOf, type DataKind, type Parsed } from "../lib/datatree";
  import { bytes as fmtBytes } from "../lib/format";

  /** `text` is null when the file was too big for the code view; then the tree reads it itself (up to 50 MB). */
  let { text, kind, repo, rel }: { text: string | null; kind: DataKind; repo: string; rel: string } = $props();

  const ROW = 22;
  const PAGE = 200;
  /** The filter stops after this many nodes or matches, so a huge file stays responsive. */
  const MAX_VISIT = 400_000;
  const MAX_HITS = 5_000;
  let parsed = $state<Parsed | null>(null);
  let loadNote = $state("");
  let open = $state(new Set<string>([""]));
  /** How many children a big node shows, by its path. */
  let shown = $state<Record<string, number>>({});
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(600);
  let filter = $state("");
  /** The filter, applied 150 ms after the last key press. */
  let query = $state("");
  /** While filtering, the branches the user closed (the others that lead to a match stay open). */
  let fclosed = $state(new Set<string>());
  /** The keyboard cursor, by row id. */
  let cursor = $state<string | null>(null);

  $effect(() => {
    const [t, k, r, p] = [text, kind, repo, rel];
    parsed = null;
    loadNote = "";
    open = new Set([""]);
    shown = {};
    cursor = null;
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

  $effect(() => {
    const q = filter.trim().toLowerCase();
    const id = setTimeout(() => {
      query = q;
      fclosed = new Set();
    }, q ? 150 : 0);
    return () => clearTimeout(id);
  });

  type Trail = { key: string; inArray: boolean }[];
  type Hits = { keep: Set<string>; anc: Set<string>; count: number; capped: boolean };

  /** What the filter finds: the matching nodes, and the branches that lead to them. */
  const hits = $derived.by((): Hits | null => {
    if (!query || !parsed?.ok) return null;
    const q = query;
    // A query that looks like a path ("jobs.build", "[2]") also matches against the full path.
    const pathy = /[.[\]$]/.test(q);
    const keep = new Set<string>();
    const anc = new Set<string>();
    let visited = 0;
    let capped = false;
    const text = (v: unknown) => (typeOf(v) === "date" ? (v as Date).toISOString() : String(v));
    const walk = (v: unknown, id: string, trail: Trail): boolean => {
      let any = false;
      const arr = Array.isArray(v);
      for (const c of children(v, 0, Infinity)) {
        if (++visited > MAX_VISIT || keep.size >= MAX_HITS) {
          capped = true;
          return any;
        }
        const cid = `${id}/${c.key}`;
        const t2 = pathy ? [...trail, { key: c.key, inArray: arr }] : trail;
        const branch = isBranch(c.value);
        const hit =
          (!arr && c.key.toLowerCase().includes(q)) ||
          (!branch && text(c.value).toLowerCase().includes(q)) ||
          (pathy && pathString(t2).toLowerCase().includes(q));
        if (hit) keep.add(cid);
        const below = branch && walk(c.value, cid, t2);
        if (below) anc.add(cid);
        if (hit || below) any = true;
      }
      return any;
    };
    if (isBranch(parsed.value) && walk(parsed.value, "", [])) anc.add("");
    return { keep, anc, count: keep.size, capped };
  });

  const isOpen = (id: string) => (hits?.anc.has(id) ? !fclosed.has(id) : open.has(id));

  type Row =
    | { id: string; parent: string; depth: number; key: string | null; value: unknown; inArray: boolean; trail: Trail }
    | { id: string; parent: string; depth: number; more: string; left: number };

  /** The rows on screen, from the open nodes only. While filtering, only matches and the branches that lead to them. */
  const rows = $derived.by((): Row[] => {
    if (!parsed?.ok) return [];
    const out: Row[] = [];
    const h = hits;
    const walk = (v: unknown, id: string, depth: number, trail: Trail) => {
      if (!isBranch(v) || !isOpen(id)) return;
      const total = childCount(v);
      const limit = shown[id] ?? PAGE;
      const arr = Array.isArray(v);
      for (const c of children(v, 0, limit)) {
        const cid = `${id}/${c.key}`;
        const t2 = [...trail, { key: c.key, inArray: arr }];
        out.push({ id: cid, parent: id, depth, key: c.key, value: c.value, inArray: arr, trail: t2 });
        if (out.length > 200_000) return;
        walk(c.value, cid, depth + 1, t2);
      }
      if (total > limit) out.push({ id: `${id}#more`, parent: id, depth, more: id, left: total - limit });
    };
    // Filtering: walk only the branches that lead to a match. A matching branch the user opens shows all of it.
    const walkHits = (v: unknown, id: string, depth: number, trail: Trail) => {
      if (!h || !isBranch(v) || !isOpen(id)) return;
      const arr = Array.isArray(v);
      for (const c of children(v, 0, Infinity)) {
        const cid = `${id}/${c.key}`;
        if (!h.keep.has(cid) && !h.anc.has(cid)) continue;
        const t2 = [...trail, { key: c.key, inArray: arr }];
        out.push({ id: cid, parent: id, depth, key: c.key, value: c.value, inArray: arr, trail: t2 });
        if (out.length > 200_000) return;
        if (h.anc.has(cid)) walkHits(c.value, cid, depth + 1, t2);
        else walk(c.value, cid, depth + 1, t2);
      }
    };
    out.push({ id: "", parent: "", depth: 0, key: null, value: parsed.value, inArray: false, trail: [] });
    if (h) walkHits(parsed.value, "", 1, []);
    else walk(parsed.value, "", 1, []);
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
  const cursorIdx = $derived(cursor === null ? -1 : rows.findIndex((r) => r.id === cursor));

  function toggle(id: string) {
    if (hits?.anc.has(id)) {
      const next = new Set(fclosed);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      fclosed = next;
      return;
    }
    const next = new Set(open);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    open = next;
  }

  function showMore(id: string) {
    shown = { ...shown, [id]: (shown[id] ?? PAGE) + PAGE * 5 };
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
    fclosed = new Set();
  }

  function collapseAll() {
    open = new Set([""]);
    fclosed = new Set([...(hits?.anc ?? [])].filter((id) => id !== ""));
  }

  const hasKids = (r: Row) => !("more" in r) && isBranch(r.value) && childCount(r.value) > 0;

  /** Keeps the cursor row on screen. */
  function reveal(idx: number) {
    if (!scroller) return;
    const top = idx * ROW;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + viewH) scroller.scrollTop = top + ROW - viewH;
    scrollTop = scroller.scrollTop;
  }

  function moveTo(idx: number) {
    if (!rows.length) return;
    const i = Math.max(0, Math.min(rows.length - 1, idx));
    cursor = rows[i].id;
    reveal(i);
  }

  /** ↑ ↓ move, → opens (or steps into), ← closes (or steps out), Enter toggles. */
  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const i = cursorIdx;
    const r = i >= 0 ? rows[i] : null;
    const page = Math.max(1, Math.floor(viewH / ROW) - 1);
    switch (e.key) {
      case "ArrowDown":
        moveTo(i < 0 ? 0 : i + 1);
        break;
      case "ArrowUp":
        moveTo(i < 0 ? 0 : i - 1);
        break;
      case "PageDown":
        moveTo(i + page);
        break;
      case "PageUp":
        moveTo(i - page);
        break;
      case "Home":
        moveTo(0);
        break;
      case "End":
        moveTo(rows.length - 1);
        break;
      case "ArrowRight":
        if (!r) moveTo(0);
        else if (hasKids(r)) {
          if (!isOpen(r.id)) toggle(r.id);
          else moveTo(i + 1);
        }
        break;
      case "ArrowLeft":
        if (!r) moveTo(0);
        else if (hasKids(r) && isOpen(r.id) && r.id !== "") toggle(r.id);
        else if (r.id !== "") moveTo(rows.findIndex((x) => x.id === r.parent));
        break;
      case "Enter":
      case " ":
        if (!r) moveTo(0);
        else if ("more" in r) showMore(r.more);
        else if (hasKids(r)) toggle(r.id);
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  function onFilterKey(e: KeyboardEvent) {
    if (e.key === "Escape" && filter) {
      e.preventDefault();
      filter = "";
      query = "";
    } else if (e.key === "ArrowDown" || e.key === "Enter") {
      e.preventDefault();
      scroller?.focus();
      if (cursorIdx < 0) moveTo(hits ? 1 : 0);
    }
  }

  function menu(e: MouseEvent, r: Row) {
    if ("more" in r) return;
    e.preventDefault();
    cursor = r.id;
    const path = pathString(r.trail);
    app.showMenu(e, path || "Document", [
      { label: "Copy the path", run: () => app.copyValue(path || "$") },
      { label: "Copy the value as JSON", run: () => app.copyValue(JSON.stringify(r.value, null, 2) ?? "null") },
      ...(typeOf(r.value) === "string" ? [{ label: "Copy the text", run: () => app.copyValue(String(r.value)) }] : []),
    ]);
  }
</script>

<div class="tree">
  <div class="toolbar sub">
    {#if parsed?.ok}
      <span class="meta">
        {kind.toUpperCase()}{parsed.docs > 1 ? ` · ${parsed.docs} documents` : ""} ·
        {#if hits}{hits.count.toLocaleString()}{hits.capped ? "+" : ""} {hits.count === 1 ? "match" : "matches"}{hits.capped ? " (stopped early, type more to narrow it)" : ""}{:else}{rows.length.toLocaleString()} rows shown{/if}
      </span>
      <span class="grow"></span>
      <input class="field filter" bind:value={filter} onkeydown={onFilterKey} placeholder="Filter keys and values" spellcheck="false" aria-label="Filter keys and values" />
      <button class="btn sm ghost" onclick={expandAll}>Expand all</button>
      <button class="btn sm ghost" onclick={collapseAll}>Collapse all</button>
    {/if}
  </div>
  {#if loadNote}
    <div class="state">
      <span class="state-title">Cannot show this tree</span>
      <p>{loadNote}</p>
    </div>
  {:else if !parsed}
    <div class="state delay"><p>Reading…</p></div>
  {:else if !parsed.ok}
    <div class="state">
      <span class="state-title">This {kind.toUpperCase()} has a mistake{parsed.line ? ` on line ${parsed.line}${parsed.col ? `, column ${parsed.col}` : ""}` : ""}</span>
      <pre>{parsed.message}</pre>
      <div class="actions"><button class="btn" onclick={() => app.mdPreview && app.toggleMdPreview()}>Show the source</button></div>
    </div>
  {:else if hits && !hits.count}
    <div class="state">
      <span class="state-title">Nothing matches “{filter.trim()}”</span>
      <div class="actions"><button class="btn" onclick={() => ((filter = ""), (query = ""))}>Clear the filter</button></div>
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="scroll" bind:this={scroller} bind:clientHeight={viewH} onscroll={() => (scrollTop = scroller!.scrollTop)}
      tabindex="0" role="tree" aria-label="Data tree" onkeydown={onKeydown}>
      <div style:height="{rows.length * ROW}px" class="space">
        {#each visible as r, i (r.id)}
          {@const top = (first + i) * ROW}
          {#if "more" in r}
            <button class="row more" class:cur={r.id === cursor} style:top="{top}px" style:padding-left="{14 + r.depth * 16}px" tabindex="-1"
              onclick={() => { cursor = r.id; showMore(r.more); }}>
              Show {Math.min(r.left, PAGE * 5).toLocaleString()} more of {r.left.toLocaleString()}
            </button>
          {:else}
            {@const branch = hasKids(r)}
            {@const t = typeOf(r.value)}
            {@const hit = !!hits?.keep.has(r.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div class="row" class:cur={r.id === cursor} class:hit style:top="{top}px" style:padding-left="{14 + r.depth * 16}px" oncontextmenu={(e) => menu(e, r)}
              role="treeitem" tabindex="-1" aria-selected={r.id === cursor} aria-expanded={branch ? isOpen(r.id) : undefined}
              onclick={() => { cursor = r.id; if (branch) toggle(r.id); }} class:branch>
              <span class="chev">{branch ? (isOpen(r.id) ? "▾" : "▸") : ""}</span>
              {#if r.key !== null}<span class="key" class:idx={r.inArray}>{r.key}</span><span class="colon">:</span>{/if}
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
  .filter {
    width: 200px;
  }
  .state.delay {
    animation: tern-fade-in 150ms var(--ease-out) 300ms both;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
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
    gap: var(--s1);
    font: var(--fs-sm) / 22px var(--mono);
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
  .row.cur {
    background: var(--raised);
  }
  /* The keyboard cursor: an accent ring while the tree has focus. */
  .scroll:focus .row.cur {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .row.hit .key,
  .row.hit .val {
    background: var(--accent-soft);
    border-radius: 2px;
  }
  .row.more {
    color: var(--blue);
    font-family: var(--ui);
    font-size: var(--fs-sm);
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
    margin-right: var(--s1);
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
</style>
