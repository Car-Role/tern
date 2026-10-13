// Small fuzzy matcher: letters must appear in order. Scores favor the file name,
// word starts, and letters next to each other.

export type Match = { score: number; hits: number[] };

const isBoundary = (c: string) => c === "/" || c === "\\" || c === "-" || c === "_" || c === "." || c === " ";

export function fuzzy(query: string, target: string): Match | null {
  if (!query) return { score: 0, hits: [] };
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  const nameStart = Math.max(t.lastIndexOf("/"), t.lastIndexOf("\\")) + 1;
  const hits: number[] = [];
  let score = 0;
  let ti = 0;
  let prev = -2;
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi];
    if (ch === " ") continue;
    const found = t.indexOf(ch, ti);
    if (found < 0) return null;
    let s = 1;
    if (found === prev + 1) s += 4;
    if (found === 0 || isBoundary(t[found - 1])) s += 3;
    if (target[found] !== t[found]) s += 1; // camelCase letter
    if (found >= nameStart) s += 2;
    score += s;
    hits.push(found);
    prev = found;
    ti = found + 1;
  }
  score -= target.length * 0.02;
  if (t.slice(nameStart).startsWith(q)) score += 8;
  return { score, hits };
}

export function rank<T>(items: T[], query: string, key: (item: T) => string, limit = 60) {
  const out: { item: T; match: Match }[] = [];
  for (const item of items) {
    const match = fuzzy(query, key(item));
    if (match) out.push({ item, match });
  }
  out.sort((a, b) => b.match.score - a.match.score);
  return out.slice(0, limit);
}
