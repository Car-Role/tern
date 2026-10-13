// The hover card for a project: where it is, its branch, sync state, changes, and activity.

import { app } from "./store.svelte";
import { ago } from "./format";
import type { HoverData } from "./hover";

export function repoCard(path: string): HoverData {
  const repo = app.repos.find((r) => r.path === path);
  const s = app.sums[path];
  const rows: NonNullable<HoverData["rows"]> = [{ label: "Folder", value: path, mono: true }];
  if (repo?.parent) rows.push({ label: "Worktree of", value: repo.parent, mono: true });
  if (s?.error) return { title: repo?.name, rows: [...rows, { label: "Problem", value: s.error, tone: "del" }], hint: "Right-click for more" };
  if (s) {
    rows.push({ label: "Branch", value: s.branch ? (s.upstream ? `${s.branch}, tracks ${s.upstream}` : `${s.branch}, not on a remote yet`) : "detached HEAD", mono: true });
    const sync = [s.behind ? `${s.behind} to pull` : "", s.ahead ? `${s.ahead} to push` : ""].filter(Boolean).join(", ");
    if (s.upstream) rows.push({ label: "Sync", value: sync || "Up to date as of the last fetch", tone: s.behind ? "blue" : s.ahead ? "add" : undefined });
    const staged = s.changes.filter((c) => c.staged).length;
    rows.push({
      label: "Changes",
      value: s.changes.length ? `${s.changes.length} ${s.changes.length === 1 ? "file" : "files"}${staged ? `, ${staged} staged` : ""}` : "None",
      tone: s.changes.length ? "amber" : undefined,
    });
    if (s.last) rows.push({ label: "Last commit", value: `${s.last.short} ${s.last.subject} (${ago(s.last.time)}, ${s.last.author})` });
    if (s.activity) rows.push({ label: "Last change", value: app.isWorking(path) ? `${ago(s.activity)}, an agent may be working` : ago(s.activity), tone: app.isWorking(path) ? "add" : undefined });
    if (s.remote) rows.push({ label: "Fetched", value: ago(s.fetchedAt) });
  }
  const running = app.runningIn(path);
  if (running) rows.push({ label: "Tasks", value: `${running} running`, tone: "add" });
  const space = repo ? app.spaces.find((x) => x.id === app.spaceOf(repo)) : undefined;
  if (space) rows.push({ label: "Space", value: space.name });
  return { title: repo?.name, rows, hint: "Right-click for more" };
}
