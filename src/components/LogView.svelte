<script lang="ts">
  import { tick, untrack } from "svelte";
  import { api } from "../lib/api";
  import { LineBuffer, levelOf, makeFilter, type Level } from "../lib/logs";
  import { bytes as fmtBytes } from "../lib/format";
  import { app } from "../lib/store.svelte";
  import { hover } from "../lib/hover";

  let { repo, rel }: { repo: string; rel: string } = $props();

  const ROW = 19;
  const FIRST_READ = 4 * 1024 * 1024;
  const buf = new LineBuffer(50_000);
  /** Goes up when the lines change, so the view redraws. */
  let version = $state(0);
  let offset = 0;
  let size = $state(0);
  let cut = $state(false);
  let error = $state("");
  let follow = $state(true);
  let paused = $state(false);
  let filter = $state("");
  let only = $state<"all" | "error" | "warn">("all");
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(600);

  async function read() {
    try {
      const t = await api.readTail(repo, rel, offset, offset === 0 ? FIRST_READ : 2 * 1024 * 1024);
      if (t.reset) {
        buf.reset();
        cut = t.cut;
      }
      offset = t.offset;
      size = t.size;
      error = "";
      if (buf.push(t.text) || t.reset) {
        version++;
        if (follow) tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
      }
    } catch (e) {
      error = String(e);
    }
  }

  // Read now, then every second while the window is on screen and the view is not paused.
  $effect(() => {
    const [r, p] = [repo, rel];
    void r, p;
    // untrack: this effect must not watch `version`, or each change starts it again.
    untrack(() => {
      offset = 0;
      buf.reset();
      version++;
      read();
    });
    const id = setInterval(() => {
      if (!paused && document.visibilityState === "visible") read();
    }, 1000);
    return () => clearInterval(id);
  });

  type Line = { n: number; text: string; level: Level };

  /**
   * The level word in a line (ERROR, WARN, INFO, ...), so only that word is colored. Upper-case words count
   * anywhere; lower-case ones only in brackets, after "level=", or before a colon, so prose stays plain.
   */
  const LEVEL_TOKEN =
    /\b(?:FATAL|ERROR|ERR|WARNING|WARN|DEBUG|INFO|TRACE)\b|(?<=\[|level=)(?:fatal|error|err|warning|warn|debug|info|trace)\b|\b(?:[Ff]atal|[Ee]rror|[Ww]arning|[Ww]arn)(?=:)/;
  function tokenKind(t: string): "error" | "warn" | "info" | "debug" {
    const u = t.toUpperCase();
    return u === "FATAL" || u.startsWith("ERR") ? "error" : u.startsWith("WARN") ? "warn" : u === "INFO" ? "info" : "debug";
  }
  function splitLevel(text: string): { pre: string; tok: string; kind: string; post: string } | null {
    const m = LEVEL_TOKEN.exec(text);
    if (!m) return null;
    return { pre: text.slice(0, m.index), tok: m[0], kind: tokenKind(m[0]), post: text.slice(m.index + m[0].length) };
  }
  const lines = $derived.by((): Line[] => {
    version;
    const match = makeFilter(filter);
    const out: Line[] = [];
    const base = buf.dropped;
    for (let i = 0; i < buf.lines.length; i++) {
      const text = buf.lines[i];
      const level = levelOf(text);
      if (only === "error" && level !== "error") continue;
      if (only === "warn" && level !== "error" && level !== "warn") continue;
      if (!match(text)) continue;
      out.push({ n: base + i + 1, text, level });
    }
    return out;
  });
  const counts = $derived.by(() => {
    version;
    let e = 0, w = 0;
    for (const l of buf.lines) {
      const lv = levelOf(l);
      if (lv === "error") e++;
      else if (lv === "warn") w++;
    }
    return { e, w, total: buf.lines.length + buf.dropped };
  });
  // After the list changes length, read the real scroll position again (the browser may have moved it).
  $effect(() => {
    lines.length;
    if (scroller) scrollTop = scroller.scrollTop;
  });
  // Clamped: when a filter makes the list shorter, the old scroll position can be past its end.
  const first = $derived(Math.max(0, Math.min(Math.floor(scrollTop / ROW), lines.length - Math.ceil(viewH / ROW)) - 20));
  const visible = $derived(lines.slice(first, first + Math.ceil(viewH / ROW) + 40));

  function onScroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    // Scrolling up to read stops following; scrolling back to the bottom starts it again.
    follow = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - ROW * 2;
  }
  /** The line found by Next error or Previous error, by line number. */
  let mark = $state<number | null>(null);

  /** Index range of the lines on screen. */
  function onScreen() {
    const from = Math.max(0, Math.floor(scrollTop / ROW));
    const to = Math.min(lines.length, Math.ceil((scrollTop + viewH) / ROW));
    return { from, to };
  }

  /** Moves to the next (1) or previous (-1) error among the shown lines, wrapping at the ends. */
  function stepError(dir: 1 | -1) {
    if (!lines.length || !scroller) return;
    const { from, to } = onScreen();
    const markIdx = mark === null ? -1 : lines.findIndex((l) => l.n === mark);
    // Start from the found line while it is on screen; otherwise from the middle of the screen.
    const onMark = markIdx >= from && markIdx < to;
    const refIdx = onMark ? markIdx : Math.max(0, Math.min(lines.length - 1, Math.floor((scrollTop + viewH / 2) / ROW)));
    const refN = lines[refIdx].n + (onMark ? 0 : dir === 1 ? -0.5 : 0.5);
    let idx = -1;
    if (dir === 1) {
      idx = lines.findIndex((l) => l.level === "error" && l.n > refN);
      if (idx < 0) idx = lines.findIndex((l) => l.level === "error");
    } else {
      for (let i = lines.length - 1; i >= 0; i--) if (lines[i].level === "error" && lines[i].n < refN) { idx = i; break; }
      if (idx < 0) for (let i = lines.length - 1; i >= 0; i--) if (lines[i].level === "error") { idx = i; break; }
    }
    if (idx < 0) {
      app.notify("No errors in the shown lines");
      return;
    }
    mark = lines[idx].n;
    follow = false;
    scroller.scrollTop = Math.max(0, idx * ROW - viewH / 2 + ROW / 2);
    scrollTop = scroller.scrollTop;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "n" || e.key === "N") {
      e.preventDefault();
      stepError(e.key === "n" ? 1 : -1);
    }
  }

  const FENCE = "`".repeat(3);

  function lineMenu(e: MouseEvent, l: Line) {
    e.preventDefault();
    app.showMenu(e, `Line ${l.n}`, [
      { label: "Copy line", run: () => app.copyValue(l.text, `line ${l.n}`) },
      {
        label: "Copy line and 20 lines around it",
        run: () => {
          const i = l.n - 1 - buf.dropped;
          const a = Math.max(0, i - 20);
          const b = Math.min(buf.lines.length, i + 21);
          app.copyValue(buf.lines.slice(a, b).join("\n"), `lines ${a + buf.dropped + 1}–${b + buf.dropped}`);
        },
      },
      { label: "Copy the shown lines for agent", run: copyShown },
    ]);
  }

  /** The lines on screen, fenced, with the file name. Filtered lines keep their line numbers. */
  function copyShown() {
    const { from, to } = onScreen();
    const part = lines.slice(from, to);
    if (!part.length) return;
    const q = filter.trim();
    const filtered = !!q || only !== "all";
    const what = filtered
      ? `${rel}, lines matching ${[only !== "all" ? (only === "error" ? "errors" : "warnings") : "", q ? `"${q}"` : ""].filter(Boolean).join(" and ")}`
      : `${rel}, lines ${part[0].n}–${part[part.length - 1].n}`;
    const body = part.map((l) => (filtered ? `${l.n}: ${l.text}` : l.text)).join("\n");
    app.copyValue(`From ${what}:\n${FENCE}\n${body}\n${FENCE}`, `${part.length} log lines for an agent`);
  }

  function toEnd() {
    follow = true;
    tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
  }
</script>

<div class="logv">
  <div class="toolbar sub bar">
    <span class="live" class:on={!paused} use:hover={() => ({ title: paused ? "Paused" : "Reading new lines every second" })}></span>
    <span class="meta">{fmtBytes(size)} · {counts.total.toLocaleString()} lines{cut ? " (the start is not shown)" : ""}</span>
    <div class="seg" role="group" aria-label="Show">
      <button class:on={only === "all"} onclick={() => (only = "all")}>All</button>
      <button class:on={only === "error"} onclick={() => (only = "error")}>Errors <span class="badge" class:del={counts.e > 0}>{counts.e}</span></button>
      <button class:on={only === "warn"} onclick={() => (only = "warn")}>Warnings <span class="badge">{counts.w}</span></button>
    </div>
    <div class="nav" role="group" aria-label="Errors">
      <button class="icon-btn sm" onclick={() => stepError(-1)} disabled={!counts.e} aria-label="Previous error"
        use:hover={() => ({ title: "Previous error", hint: "Shift N" })}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 10l4-4 4 4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
      <button class="icon-btn sm" onclick={() => stepError(1)} disabled={!counts.e} aria-label="Next error"
        use:hover={() => ({ title: "Next error", hint: "N" })}>
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </button>
    </div>
    <span class="grow"></span>
    <input class="field filter" bind:value={filter} placeholder="Filter, or /regex/" spellcheck="false" aria-label="Filter lines"
      onkeydown={(e) => { if (e.key === "Escape" && filter) { e.preventDefault(); filter = ""; } }} />
    <button class="btn sm" onclick={() => (paused = !paused)}>{paused ? "Resume" : "Pause"}</button>
  </div>
  {#if error}
    <div class="state">
      <span class="state-title">Could not read this log</span>
      <pre>{error}</pre>
      <div class="actions"><button class="btn" onclick={read}>Try again</button></div>
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div class="scroll" bind:this={scroller} bind:clientHeight={viewH} onscroll={onScroll} tabindex="0" role="log" aria-label="Log lines" onkeydown={onKeydown}>
      <div class="space" style:height="{(lines.length + 1) * ROW}px">
        {#each visible as l, i (l.n)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="ln {l.level ?? ''}" class:mark={l.n === mark} style:top="{(first + i) * ROW}px" oncontextmenu={(e) => lineMenu(e, l)}><span class="n">{l.n}</span><span class="t">{#if l.level === "debug"}{l.text}{:else}{@const p = splitLevel(l.text)}{#if p}{p.pre}<span class="lv {p.kind}">{p.tok}</span>{p.post}{:else}{l.text}{/if}{/if}</span></div>
        {/each}
        {#if version && buf.pending && !filter && only === "all"}
          <div class="ln pending" style:top="{lines.length * ROW}px"><span class="n"></span><span class="t">{buf.pending}</span></div>
        {/if}
      </div>
      {#if !lines.length}
        <div class="state empty">
          {#if filter || only !== "all"}
            <span class="state-title">No lines match</span>
            <div class="actions"><button class="btn" onclick={() => ((filter = ""), (only = "all"))}>Show every line</button></div>
          {:else}
            <span class="state-title">The log is empty</span>
            <p>New lines show here as they are written.</p>
          {/if}
        </div>
      {/if}
    </div>
    {#if !follow}
      <button class="btn sm primary jump" onclick={toEnd}>Follow new lines ↓</button>
    {/if}
  {/if}
</div>

<style>
  .logv {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .live {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
    flex-shrink: 0;
  }
  .live.on {
    background: var(--add);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .nav {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }
  /* Narrow panes: the filter gives way first, then the size and line count go. */
  .bar {
    container: logbar / inline-size;
  }
  .bar > :global(*) {
    flex-shrink: 0;
  }
  .bar > .filter {
    flex: 0 1 200px;
    min-width: 120px;
  }
  @container logbar (max-width: 620px) {
    .bar > .meta {
      display: none;
    }
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    position: relative;
    outline: none;
  }
  .space {
    position: relative;
    min-width: 100%;
  }
  .ln {
    position: absolute;
    left: 0;
    right: 0;
    height: 19px;
    display: flex;
    font: var(--fs-sm) / 19px var(--mono);
    white-space: pre;
    user-select: text;
  }
  .ln .n {
    width: 64px;
    flex-shrink: 0;
    padding-right: var(--s4);
    text-align: right;
    color: var(--faint);
    user-select: none;
  }
  /* Only the level word is colored. Error rows keep a faint tint so they can be found while scrolling. */
  .ln.error {
    background: color-mix(in srgb, var(--del-bg) 45%, transparent);
  }
  .ln.debug .t {
    color: var(--muted);
  }
  .lv.error {
    color: var(--del);
    font-weight: 600;
  }
  .lv.warn {
    color: var(--warn);
    font-weight: 600;
  }
  .lv.info {
    color: var(--blue);
  }
  .lv.debug {
    color: var(--muted);
  }
  .ln.pending .t {
    color: var(--muted);
  }
  .ln.mark {
    box-shadow: inset 2px 0 0 var(--accent), inset 0 0 0 1px var(--accent);
  }
  .state.empty {
    position: absolute;
    inset: 0;
  }
  .jump {
    position: absolute;
    right: 18px;
    bottom: 14px;
    border-radius: 11px;
  }
</style>
