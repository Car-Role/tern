// Every action with a name and an optional shortcut. The palette, the help screen,
// and the keyboard handler all read this list.

import { app, type PaletteMode } from "./store.svelte";
import { repoMenu } from "./menus";

export type Cmd = {
  id: string;
  label: string;
  group: "Go" | "Spaces" | "Copy" | "Git" | "Tasks" | "Open" | "View" | "Project";
  /** Hidden from the palette (the palette lists these with real names instead). */
  hidden?: boolean;
  keys?: string;
  run: () => void;
};

const palette = (mode: PaletteMode) => () => {
  app.paletteMode = mode;
  app.overlay = "palette";
};
const inRepo = (fn: (repo: string) => void) => () => {
  if (app.current) fn(app.current);
  else app.notify("Open a project first.", "info");
};

export const commands: Cmd[] = [
  { id: "go.projects", group: "Go", label: "Go to project", keys: "Ctrl+K", run: palette("projects") },
  { id: "go.files", group: "Go", label: "Go to file", keys: "Ctrl+P", run: palette("files") },
  { id: "go.search", group: "Go", label: "Search code, files, commits, and more across projects", keys: "Ctrl+Shift+F", run: () => app.openSearch({}) },
  { id: "go.searchHere", group: "Go", label: "Search in this file", keys: "Ctrl+F", run: () => app.searchHere() },
  { id: "go.data", group: "Go", label: "Databases: SQLite, Postgres, SQL Server, Redis", keys: "Ctrl+Shift+D", run: () => (app.dataOpen ? app.closeData() : app.openData()) },
  { id: "go.tickets", group: "Go", label: "Show my tickets", keys: "Ctrl+Shift+T", run: () => (app.ticketsOpen ? app.closeTickets() : app.openTickets()) },
  { id: "go.definition", group: "Go", label: "Go to definition", keys: "F12", run: () => app.goToDefinition() },
  { id: "go.usages", group: "Go", label: "Find where it is used", keys: "Shift+F12", run: () => app.findUsages() },
  { id: "go.commands", group: "Go", label: "Run a command", keys: "Ctrl+Shift+P", run: palette("commands") },
  { id: "go.overview", group: "Go", label: "Go home (all projects)", keys: "Ctrl+0", run: () => app.select(null) },
  { id: "go.back", group: "Go", label: "Go back", keys: "Alt+←", run: () => app.back() },
  { id: "go.forward", group: "Go", label: "Go forward", keys: "Alt+→", run: () => app.forward() },
  { id: "go.next", group: "Go", label: "Next project", keys: "Alt+↓", run: () => app.step(1) },
  { id: "go.prev", group: "Go", label: "Previous project", keys: "Alt+↑", run: () => app.step(-1) },
  // Ctrl+1 to Ctrl+7 pick the Nth tab the project shows, so they follow each space's own tabs.
  ...Array.from({ length: 7 }, (_, i): Cmd => ({
    id: `tab.${i + 1}`,
    group: "Go",
    label: `Show tab ${i + 1}`,
    keys: `Ctrl+${i + 1}`,
    hidden: true,
    run: () => app.tabByNumber(i + 1),
  })),
  { id: "tab.changes", group: "Go", label: "Show changes", run: () => app.setTab("changes") },
  { id: "tab.files", group: "Go", label: "Show files", run: () => app.setTab("files") },
  { id: "tab.prs", group: "Go", label: "Show pull requests", run: () => app.setTab("prs") },
  { id: "tab.issues", group: "Go", label: "Show issues", run: () => app.setTab("issues") },
  { id: "tab.history", group: "Go", label: "Show history", run: () => app.setTab("history") },
  { id: "tab.ci", group: "Go", label: "Show CI runs", run: () => app.setTab("ci") },
  { id: "tab.tasks", group: "Go", label: "Show tasks", run: () => app.setTab("tasks") },
  { id: "go.clone", group: "Go", label: "Clone a project…", keys: "Ctrl+Shift+O", run: () => (app.overlay = "clone") },

  { id: "space.pick", group: "Spaces", label: "Switch space…", keys: "Ctrl+Shift+S", run: () => (app.overlay = "spaces") },
  { id: "space.move", group: "Spaces", label: "Move this project to a space…", run: inRepo(() => (app.overlay = "move")) },
  { id: "space.all", group: "Spaces", label: "Show all spaces", keys: "Ctrl+Alt+0", run: () => app.switchSpace(null) },
  ...Array.from({ length: 9 }, (_, i): Cmd => ({
    id: `space.${i + 1}`,
    group: "Spaces",
    label: `Switch to space ${i + 1}`,
    keys: `Ctrl+Alt+${i + 1}`,
    hidden: true,
    run: () => {
      const sp = app.spaces[i];
      if (sp) app.switchSpace(sp.id);
    },
  })),

  { id: "copy.menu", group: "Copy", label: "Copy for agent…", keys: "Ctrl+Shift+C", run: () => (app.overlay = "copy") },
  { id: "copy.relative", group: "Copy", label: "Copy relative path", keys: "Ctrl+Alt+R", run: () => app.copy("relative") },
  { id: "copy.absolute", group: "Copy", label: "Copy absolute path", keys: "Ctrl+Alt+A", run: () => app.copy("absolute") },
  { id: "copy.line", group: "Copy", label: "Copy path with line numbers", keys: "Ctrl+Alt+L", run: () => app.copy("line") },
  { id: "copy.mention", group: "Copy", label: "Copy as @ mention", keys: "Ctrl+Alt+M", run: () => app.copy("mention") },
  { id: "copy.diff", group: "Copy", label: "Copy diff", keys: "Ctrl+Alt+D", run: () => app.copy("diff") },

  {
    id: "git.commit",
    group: "Git",
    label: "Write a commit message",
    keys: "Ctrl+Shift+Enter",
    run: inRepo(() => {
      app.setTab("changes");
      setTimeout(() => (document.querySelector(".commit textarea") as HTMLTextAreaElement | null)?.focus(), 30);
    }),
  },
  {
    id: "git.review",
    group: "Git",
    label: "Review this branch since main (commits and uncommitted work)",
    run: inRepo(() => app.openBranchReview()),
  },
  { id: "git.branches", group: "Git", label: "Branches…", keys: "Ctrl+B", run: () => app.openBranches("branches") },
  { id: "git.worktrees", group: "Git", label: "Worktrees…", run: () => app.openBranches("worktrees") },
  { id: "git.stageAll", group: "Git", label: "Stage all changes", run: inRepo(() => app.stage("all")) },
  { id: "git.unstageAll", group: "Git", label: "Unstage all changes", run: inRepo(() => app.unstage("all")) },
  { id: "app.update", group: "Open", label: "Check for updates", run: () => app.checkForUpdate(true) },
  { id: "git.newPr", group: "Git", label: "Create a pull request…", run: inRepo(() => app.openNewPr()) },
  { id: "view.hunks", group: "View", label: "Stage by hunk (show the hunks of this file)", keys: "Ctrl+Shift+H", run: () => app.toggleHunks() },
  { id: "git.amend", group: "Git", label: "Amend the last commit", run: inRepo((r) => (app.canAmend(r) ? app.setAmend(r, !app.amend[r]) : app.notify("The last commit is pushed, so it can't be amended.", "info"))) },
  { id: "git.stashAll", group: "Git", label: "Stash all changes", run: inRepo(() => app.stashChanges("all")) },
  { id: "git.discardAll", group: "Git", label: "Discard all changes…", run: inRepo(() => app.discardChanges("all")) },
  { id: "git.undoDiscard", group: "Git", label: "Undo the last discard", run: () => app.undoDiscard() },
  { id: "git.undoCommit", group: "Git", label: "Undo the last commit (keep its changes)…", run: inRepo(() => app.undoLastCommit()) },
  { id: "git.fetchAll", group: "Git", label: "Fetch all", keys: "Ctrl+Alt+F", run: () => app.fetchAll() },
  { id: "git.fetch", group: "Git", label: "Fetch this project", run: inRepo((r) => app.action(r, "fetch")) },
  { id: "git.pull", group: "Git", label: "Pull this project (fast-forward only)", run: inRepo((r) => app.action(r, "pull")) },
  { id: "git.push", group: "Git", label: "Push this project", run: inRepo((r) => app.action(r, "push")) },
  { id: "view.reload", group: "Git", label: "Reload status and files", keys: "F5", run: () => app.reloadAll() },

  { id: "task.run", group: "Tasks", label: "Run a task…", keys: "Ctrl+Shift+B", run: palette("tasks") },
  { id: "task.panel", group: "Tasks", label: "Show or hide the task panel", keys: "Ctrl+J", run: () => app.setPanel(!app.panelOpen) },
  {
    id: "task.restart",
    group: "Tasks",
    label: "Restart the task in the panel",
    run: () => (app.activeRun ? app.restartRun(app.activeRun) : app.notify("No task in the panel.", "info")),
  },
  {
    id: "task.stop",
    group: "Tasks",
    label: "Stop the task in the panel",
    run: () => app.activeRun && app.stopRun(app.activeRun),
  },
  { id: "task.stopAll", group: "Tasks", label: "Stop all tasks", run: () => app.stopAll() },
  {
    id: "task.copy",
    group: "Tasks",
    label: "Copy task output for agent",
    run: () => (app.activeRun ? app.copyRunOutput(app.activeRun) : app.notify("No task in the panel.", "info")),
  },
  { id: "open.terminal", group: "Open", label: "Open this project's terminal", keys: "Ctrl+`", run: () => app.toggleTerminal() },
  { id: "term.new", group: "Open", label: "New terminal in this project", keys: "Ctrl+Shift+`", run: () => app.openTerminal() },
  { id: "open.agent", group: "Open", label: "Start Claude Code here", run: () => app.openTerminal(app.current, { agent: true }) },
  { id: "open.external", group: "Open", label: "Open an external terminal here", run: () => app.open("terminal") },
  { id: "open.externalAgent", group: "Open", label: "Start Claude Code in an external terminal", run: () => app.open("agent") },
  { id: "open.editor", group: "Open", label: "Open in VS Code", keys: "Ctrl+Shift+E", run: () => app.open("editor") },
  { id: "open.explorer", group: "Open", label: "Show in folder", run: () => app.open("explorer") },
  { id: "open.pr", group: "Open", label: "Open pull request in browser", run: () => app.openPrInBrowser() },

  {
    id: "view.mdPreview",
    group: "View",
    label: "Switch between the page view and the source (Markdown, data, tables, logs)",
    keys: "Ctrl+Shift+V",
    run: () => app.toggleMdPreview(),
  },
  {
    id: "view.diffMode",
    group: "View",
    label: "Switch between split and unified diff",
    keys: "Ctrl+D",
    run: () => app.setDiffMode(app.diffMode === "split" ? "unified" : "split"),
  },
  {
    id: "view.edit",
    group: "View",
    label: "Quick edit this file",
    keys: "Ctrl+E",
    run: () => {
      if (app.view?.kind === "file") app.editing = !app.editing;
      else app.notify("Quick edit works on files. Open the file from the Files tab or Ctrl+P.", "info");
    },
  },
  {
    id: "view.sortRecent",
    group: "View",
    label: "Sort changes by most recent",
    run: () => app.setSortRecent(!app.sortRecent),
  },
  { id: "view.sidebar", group: "View", label: "Show or hide the sidebar", keys: "Ctrl+\\", run: () => app.toggleSidebar() },
  { id: "view.notices", group: "View", label: "Show notifications", run: () => app.toggleNotices() },
  { id: "view.help", group: "View", label: "Keyboard shortcuts", keys: "F1", run: () => (app.overlay = "help") },
  { id: "view.theme", group: "View", label: "Choose a theme…", run: () => (app.overlay = "settings") },
  { id: "view.settings", group: "View", label: "Settings", keys: "Ctrl+,", run: () => (app.overlay = "settings") },

  { id: "project.pin", group: "Project", label: "Pin or unpin this project", run: inRepo((r) => app.togglePin(r)) },
  { id: "project.hide", group: "Project", label: "Hide this project", run: inRepo((r) => app.hide(r)) },
  { id: "project.unhide", group: "Project", label: "Unhide this project", run: inRepo((r) => app.unhide(r)) },
  {
    id: "project.showHidden",
    group: "Project",
    label: "Show or hide hidden projects",
    run: () => app.setShowHidden(!app.showHidden),
  },
  {
    id: "project.menu",
    group: "Project",
    label: "Project menu",
    keys: "Shift+F10",
    run: inRepo((r) => {
      const el = document.querySelector(".rail .row.active") as HTMLElement | null;
      const box = el?.getBoundingClientRect();
      repoMenu({ x: box ? box.left + 24 : 80, y: box ? box.bottom : 80 }, r);
    }),
  },
];

const codeNames: Record<string, string> = {
  Backquote: "`",
  Comma: ",",
  Enter: "Enter",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

/** Turns a key event into the same text used in `keys`, for example "Ctrl+Shift+C". */
export function keyName(e: KeyboardEvent): string {
  let k: string;
  if (e.code.startsWith("Key")) k = e.code.slice(3);
  else if (e.code.startsWith("Digit")) k = e.code.slice(5);
  else k = codeNames[e.code] ?? e.key;
  const mods = [e.ctrlKey || e.metaKey ? "Ctrl" : "", e.shiftKey ? "Shift" : "", e.altKey ? "Alt" : ""].filter(Boolean);
  return [...mods, k].join("+");
}

const byKeys = new Map<string, Cmd>();
for (const c of commands) {
  if (c.keys) byKeys.set(c.keys, c);
}

export function commandFor(e: KeyboardEvent): Cmd | undefined {
  return byKeys.get(keyName(e));
}

export function run(id: string) {
  commands.find((c) => c.id === id)?.run();
}
