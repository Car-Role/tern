// Path formats for pasting into agents, and small display helpers.

export type CopyFormat = "relative" | "absolute" | "line" | "mention" | "diff";

export type LineRange = { from: number; to: number };

export function absolutePath(repo: string, rel: string): string {
  const sep = repo.includes("\\") ? "\\" : "/";
  return repo.replace(/[\\/]+$/, "") + sep + rel.split("/").join(sep);
}

export function withLines(rel: string, lines: LineRange | null): string {
  if (!lines) return rel;
  return lines.from === lines.to ? `${rel}:${lines.from}` : `${rel}:${lines.from}-${lines.to}`;
}

/** Formats one or more repo-relative paths. Line numbers apply only to a single path. */
export function formatPaths(format: Exclude<CopyFormat, "diff">, repo: string, rels: string[], lines: LineRange | null): string {
  switch (format) {
    case "relative":
      return rels.join("\n");
    case "absolute":
      return rels.map((r) => absolutePath(repo, r)).join("\n");
    case "line":
      return rels.length === 1 ? withLines(rels[0], lines ?? { from: 1, to: 1 }) : rels.join("\n");
    case "mention":
      return rels.map((r) => "@" + r).join(" ");
  }
}

export function ago(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "never";
  const s = Math.max(0, Math.floor(Date.now() / 1000 - unixSeconds));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d ago`;
  return new Date(unixSeconds * 1000).toLocaleDateString();
}

export function agoIso(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? "" : ago(Math.floor(t / 1000));
}

/** "12s", "3m 04s", "1h 02m" */
export function duration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(s / 3600)}h ${String(Math.floor(s / 60) % 60).padStart(2, "0")}m`;
}

/** The web page of a repo, from its remote address. Works for GitHub and Azure DevOps. */
export function webUrl(remote: string | null | undefined): string | null {
  if (!remote) return null;
  let r = remote.trim().replace(/\.git$/, "");
  const gh = r.match(/github\.com[:/]([^/]+)\/([^/]+)$/);
  if (gh) return `https://github.com/${gh[1]}/${gh[2]}`;
  const ssh = r.match(/:v3\/([^/]+)\/([^/]+)\/([^/]+)$/);
  if (ssh) return `https://dev.azure.com/${ssh[1]}/${ssh[2]}/_git/${ssh[3]}`;
  r = r.replace(/^https:\/\/[^@/]+@/, "https://");
  if (/dev\.azure\.com|visualstudio\.com/.test(r)) return r;
  return null;
}

export function commitUrl(remote: string | null | undefined, sha: string): string | null {
  const web = webUrl(remote);
  return web ? `${web}/commit/${sha}` : null;
}

export function fileUrl(remote: string | null | undefined, branch: string, rel: string): string | null {
  const web = webUrl(remote);
  if (!web) return null;
  if (web.includes("github.com")) return `${web}/blob/${branch}/${rel}`;
  return `${web}?path=/${encodeURI(rel)}&version=GB${encodeURIComponent(branch)}`;
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function baseName(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

export function dirName(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i < 0 ? "" : p.slice(0, i);
}

export const kindLetter: Record<string, string> = {
  added: "A",
  modified: "M",
  deleted: "D",
  renamed: "R",
  untracked: "U",
  conflict: "!",
  copied: "C",
  typechange: "T",
};
