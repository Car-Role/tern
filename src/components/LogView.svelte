<script lang="ts">
  import { tick, untrack } from "svelte";
  import { api } from "../lib/api";
  import { LineBuffer, levelOf, makeFilter, type Level } from "../lib/logs";
  import { bytes as fmtBytes } from "../lib/format";

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
  function toEnd() {
    follow = true;
    tick().then(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
  }
</script>

<div class="logv">
  <div class="bar">
    <span class="live" class:on={!paused} title={paused ? "Paused" : "Reading new lines every second"}></span>
    <span class="faint">{fmtBytes(size)} · {counts.total.toLocaleString()} lines{cut ? " (the start is not shown)" : ""}</span>
    <div class="seg" role="group" aria-label="Show">
      <button class:on={only === "all"} onclick={() => (only = "all")}>All</button>
      <button class:on={only === "error"} onclick={() => (only = "error")}>Errors <span class="cnt e">{counts.e}</span></button>
      <button class:on={only === "warn"} onclick={() => (only = "warn")}>Warnings <span class="cnt w">{counts.w}</span></button>
    </div>
    <span class="grow"></span>
    <input class="filter" bind:value={filter} placeholder="Filter, or /regex/" spellcheck="false" aria-label="Filter lines" />
    <button class="b" onclick={() => (paused = !paused)}>{paused ? "Resume" : "Pause"}</button>
  </div>
  {#if error}
    <p class="note err mono">{error}</p>
  {:else}
    <div class="scroll" bind:this={scroller} bind:clientHeight={viewH} onscroll={onScroll}>
      <div class="space" style:height="{(lines.length + 1) * ROW}px">
        {#each visible as l, i (l.n)}
          <div class="ln {l.level ?? ''}" style:top="{(first + i) * ROW}px"><span class="n">{l.n}</span><span class="t">{l.text}</span></div>
        {/each}
        {#if version && buf.pending && !filter && only === "all"}
          <div class="ln pending" style:top="{lines.length * ROW}px"><span class="n"></span><span class="t">{buf.pending}</span></div>
        {/if}
      </div>
      {#if !lines.length}<p class="note">{filter || only !== "all" ? "No lines match." : "The log is empty."}</p>{/if}
    </div>
    {#if !follow}
      <button class="jump" onclick={toEnd}>Follow new lines ↓</button>
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
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 5px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    min-height: 34px;
  }
  .grow {
    flex: 1;
  }
  .live {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
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
  .cnt {
    font-size: 11px;
  }
  .cnt.e {
    color: var(--del);
  }
  .cnt.w {
    color: var(--amber);
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
  .ln {
    position: absolute;
    left: 0;
    right: 0;
    height: 19px;
    display: flex;
    font: 12px/19px var(--mono);
    white-space: pre;
    user-select: text;
  }
  .ln .n {
    width: 64px;
    flex-shrink: 0;
    padding-right: 12px;
    text-align: right;
    color: var(--faint);
    user-select: none;
  }
  .ln.error {
    background: var(--del-bg);
  }
  .ln.error .t {
    color: var(--del);
  }
  .ln.warn .t {
    color: var(--amber);
  }
  .ln.debug .t {
    color: var(--muted);
  }
  .ln.pending .t {
    color: var(--muted);
  }
  .jump {
    position: absolute;
    right: 18px;
    bottom: 14px;
    font-size: 12px;
    padding: 4px 12px;
    border-radius: 14px;
    color: var(--on-accent);
    background: var(--amber);
  }
  .note {
    margin: 20px;
    color: var(--muted);
  }
  .note.err {
    color: var(--del);
    font-size: 12px;
  }
</style>
