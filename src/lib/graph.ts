// Lays out a commit graph. Commits come newest first, children before parents (git log --date-order).
// Each commit gets a lane (column). Each row lists the line pieces to draw in its top and bottom half.

import type { Commit } from "./types";

export type Segment = {
  /** Lane where the piece starts (top edge or dot) and ends (dot or bottom edge). */
  from: number;
  to: number;
  /** "top": from the row's top edge to the dot. "bottom": from the dot to the bottom edge. "through": top to bottom. */
  part: "top" | "bottom" | "through";
  color: number;
};

export type GraphRow = { lane: number; color: number; segments: Segment[]; width: number; merge: boolean; fold?: boolean };

/** A history row: a commit, or a fold that stands for many hidden commits. */
export type HistItem = Commit & { fold?: number };

export const FOLD = "fold";

/**
 * Hides the older commits a pull would bring, all but the newest `keep`, behind one fold row.
 * The commits next to the fold get it as their parent or child, so the graph lines stay joined.
 * `base` is where the branch split from its remote branch; the fold always leads there.
 */
export function foldIncoming(commits: Commit[], incoming: Set<string>, base: string, keep = 3, min = 4): HistItem[] {
  const inc = commits.filter((c) => incoming.has(c.sha));
  if (inc.length - keep < min) return commits;
  const hidden = new Set(inc.slice(keep).map((c) => c.sha));
  const index = new Map(commits.map((c, i) => [c.sha, i]));
  const exits = new Set<string>();
  for (const c of commits) {
    if (!hidden.has(c.sha)) continue;
    for (const p of c.parents) if (!hidden.has(p) && index.has(p)) exits.add(p);
  }
  if (base && index.has(base)) exits.add(base);
  const parents = [...exits].sort((a, b) => index.get(a)! - index.get(b)!);
  const out: HistItem[] = [];
  let placed = false;
  for (const c of commits) {
    if (hidden.has(c.sha)) {
      if (!placed) {
        out.push({ ...c, sha: FOLD, short: "", subject: "", refs: "", body: "", parents, fold: hidden.size });
        placed = true;
      }
      continue;
    }
    if (!c.parents.some((p) => hidden.has(p))) out.push(c);
    else out.push({ ...c, parents: [...new Set(c.parents.map((p) => (hidden.has(p) ? FOLD : p)))] });
  }
  return out;
}

export function layoutGraph(commits: HistItem[]): { rows: GraphRow[]; lanes: number } {
  // lanes[i] = the commit id this lane is waiting for, or null when the lane is free.
  let lanes: (string | null)[] = [];
  let colors: number[] = [];
  let nextColor = 0;
  let maxLanes = 1;
  const known = new Set(commits.map((c) => c.sha));
  const rows: GraphRow[] = [];

  for (const c of commits) {
    const segments: Segment[] = [];
    let lane = lanes.indexOf(c.sha);
    if (lane < 0) {
      // A branch tip: nothing points here yet. Use the first free lane.
      lane = lanes.indexOf(null);
      if (lane < 0) lane = lanes.length;
      lanes[lane] = c.sha;
      colors[lane] = nextColor++;
    } else {
      segments.push({ from: lane, to: lane, part: "top", color: colors[lane] });
    }
    const color = colors[lane];

    // Other lanes waiting for this commit join into it here (the ends of merged branches).
    for (let i = 0; i < lanes.length; i++) {
      if (i !== lane && lanes[i] === c.sha) {
        segments.push({ from: i, to: lane, part: "top", color: colors[i] });
        lanes[i] = null;
      }
    }
    // Lanes that only pass by.
    for (let i = 0; i < lanes.length; i++) {
      if (i !== lane && lanes[i] !== null) segments.push({ from: i, to: i, part: "through", color: colors[i] });
    }

    const parents = c.parents.filter((p) => known.has(p));
    // The first parent continues this commit's lane.
    if (parents.length) {
      lanes[lane] = parents[0];
      segments.push({ from: lane, to: lane, part: "bottom", color });
    } else {
      lanes[lane] = null;
    }
    // More parents (a merge): join a lane already waiting for that parent, or open a new lane.
    for (const p of parents.slice(1)) {
      let target = lanes.indexOf(p);
      if (target < 0) {
        target = lanes.indexOf(null);
        if (target < 0) target = lanes.length;
        lanes[target] = p;
        colors[target] = nextColor++;
      }
      segments.push({ from: lane, to: target, part: "bottom", color: colors[target] });
    }

    while (lanes.length && lanes[lanes.length - 1] === null) {
      lanes.pop();
      colors.length = lanes.length;
    }
    const width = Math.max(lane + 1, ...segments.map((s) => Math.max(s.from, s.to) + 1));
    maxLanes = Math.max(maxLanes, width);
    rows.push({ lane, color, segments, width, merge: c.parents.length > 1 && !c.fold, fold: !!c.fold });
  }
  return { rows, lanes: maxLanes };
}

/** Branch and tag names from `%D`, sorted: HEAD first, then local branches, remotes, tags. */
export function parseRefs(refs: string) {
  const out: { name: string; kind: "head" | "branch" | "remote" | "tag" }[] = [];
  for (const raw of refs.split(",").map((r) => r.trim()).filter(Boolean)) {
    if (raw.startsWith("HEAD -> ")) out.push({ name: raw.slice(8), kind: "head" });
    else if (raw === "HEAD") out.push({ name: "HEAD", kind: "head" });
    else if (raw.startsWith("tag: ")) out.push({ name: raw.slice(5), kind: "tag" });
    else if (raw.includes("/") && /^(origin|upstream)\//.test(raw)) out.push({ name: raw, kind: "remote" });
    else out.push({ name: raw, kind: "branch" });
  }
  const order = { head: 0, branch: 1, remote: 2, tag: 3 };
  return out.filter((r) => !r.name.endsWith("/HEAD")).sort((a, b) => order[a.kind] - order[b.kind]);
}
