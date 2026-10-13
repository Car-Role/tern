<script lang="ts">
  import type { GraphRow } from "../lib/graph";

  let { row, lanes, height, head = false }: { row: GraphRow; lanes: number; height: number; head?: boolean } = $props();

  const W = 12;
  const shown = $derived(Math.min(lanes, 12));
  const x = (i: number) => W / 2 + Math.min(i, 11) * W + 4;
  const mid = $derived(height / 2);
  const colors = ["var(--amber)", "var(--blue)", "var(--add)", "var(--syn-keyword)", "var(--t-cyan)", "var(--del)", "var(--syn-number)"];
  const color = (n: number) => colors[n % colors.length];

  function path(s: GraphRow["segments"][number]) {
    const a = x(s.from);
    const b = x(s.to);
    if (s.part === "through") return `M${a} 0 V${height}`;
    const [y1, y2] = s.part === "top" ? [0, mid] : [mid, height];
    if (a === b) return `M${a} ${y1} V${y2}`;
    // A soft bend between two lanes.
    const k = (y2 - y1) * 0.55;
    return `M${a} ${y1} C${a} ${y1 + k} ${b} ${y2 - k} ${b} ${y2}`;
  }
</script>

<svg class="graph" width={shown * W + 8} {height} aria-hidden="true">
  {#each row.segments as s}
    <path d={path(s)} stroke={color(s.color)} class:dash={row.fold && s.from === row.lane && s.to === row.lane} />
  {/each}
  {#if row.fold}
    <!-- A fold: many commits stand behind this row, so the line is dashed and has no dot. -->
  {:else if row.merge}
    <circle cx={x(row.lane)} cy={mid} r="3.5" fill="var(--bg)" stroke={color(row.color)} stroke-width="1.6" />
  {:else}
    <circle cx={x(row.lane)} cy={mid} r={head ? 4.5 : 3.5} fill={color(row.color)} />
    {#if head}<circle cx={x(row.lane)} cy={mid} r="7" fill="none" stroke={color(row.color)} stroke-opacity="0.35" stroke-width="1.5" />{/if}
  {/if}
</svg>

<style>
  .graph {
    flex-shrink: 0;
    display: block;
  }
  path.dash {
    stroke-dasharray: 2 3;
  }
  path {
    fill: none;
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>
