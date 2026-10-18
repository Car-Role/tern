# Everything Tern does

This is the complete reference: every screen, every action, every key, and every right-click menu.
For a short introduction, see the [README](../README.md). For how the screens are built, see the
[design system](design.md).

Contents:
[Layout](#layout) ·
[Home](#home) ·
[The sidebar](#the-sidebar) ·
[A project](#a-project) ·
[The viewer](#the-viewer) ·
[File views](#file-views) ·
[Tickets](#tickets) ·
[Data](#data) ·
[Search](#search) ·
[Command palette](#command-palette) ·
[Copy for agent](#copy-for-agent) ·
[Branches and worktrees](#branches-and-worktrees) ·
[Tasks and terminals](#tasks-and-terminals) ·
[Clone a project](#clone-a-project) ·
[Spaces](#spaces) ·
[Notifications and undo](#notifications-and-undo) ·
[Settings](#settings) ·
[Problems and bug reports](#problems-and-bug-reports) ·
[Updates](#updates) ·
[Keyboard reference](#keyboard-reference) ·
[Right-click menus](#right-click-menus) ·
[Where Tern keeps things](#where-tern-keeps-things)

---

## Layout

| Area | What it holds |
| --- | --- |
| **Title bar** | The Tern mark (goes Home), Back and Forward, where you are (space / project / branch), the command bar (`Ctrl K`), running tasks, Fetch all, notifications, and the window buttons. |
| **Sidebar** | The space switcher, then Home, Tickets, Data, and Search, then your projects. Settings and "hide the sidebar" are in its footer. Drag its edge to resize it; double-click the edge to reset it. `Ctrl \` hides and shows it. |
| **Main area** | One screen at a time: Home, a project (list and viewer), Tickets, or Data. |
| **Bottom panel** | Tasks and terminals (`Ctrl J`). Drag its top edge to resize it; double-click to reset it. |
| **Status bar** | The project path (click to copy), its pull and push counts, the latest message (click to see all), the update button, Undo discard, and a link to the shortcuts. |

Back and Forward (`Alt ←` / `Alt →`, the mouse side buttons, or the title-bar arrows) move through the
projects, tabs, files, reviews, and screens you visited.

## Home

`Ctrl 0`, the Tern mark, or **Home** in the sidebar.

- **Needs you**: cards for the work that waits on you, built from what Tern already knows:
  - **Problems**: projects Tern could not read, and merge conflicts.
  - **Agents working**: files changed in the last 2 minutes.
  - **CI failing**: the newest run on the current branch failed (for projects whose CI tab has been loaded).
  - **Ready to review**: changes nobody is making right now.
  - **Ready to push** and **Behind**, each with a **Push…** or **Pull…** button that shows what happens first.
  - **Tickets in progress**: click one to open it in Tickets.
  When nothing waits, Home says "All clear".
- **Projects**: every project in the space, with branch, changes, pull and push counts, activity, last commit,
  and when it was last fetched. Click a column to sort by it (click again to flip). Type in **Filter projects**
  to narrow the list. Click a row to open it; `Ctrl` click and `Shift` click select many; right-click for the
  project menu.

## The sidebar

- **Space switcher** (top): pick a space, or All spaces (`Ctrl Shift S`, `Ctrl Alt 0`–`9`).
- **Home, Tickets, Data, Search**: the places you can go. Tickets shows how many are assigned to you.
- **Projects**: grouped by space in the All spaces view. Click a group name to fold it. **+** clones a project
  or adds a folder to scan. **Drag** a project to change its place; drag it into another space, or onto a space's
  name, to move it there. Dropped next to a pinned project, it is pinned too. Drag several selected projects at once.
  `Alt Shift ↑` / `Alt Shift ↓` moves the focused project. `Esc` cancels a drag. **Filter projects** appears when you have more than six; Enter opens the first match.
- Each row shows:

  | Mark | Meaning |
  | --- | --- |
  | Hollow dot | No local changes |
  | Accent dot | Changes that are not committed |
  | Red dot | Merge conflict, or Tern could not read it |
  | Dashed dot | Not read yet |
  | Pulsing green dot | Files changed in the last 2 minutes (an agent may be working) |
  | Blue dot | Changed since you last looked |
  | ▶ | A task is running |
  | ↓3 / ↑2 | Commits to pull / to push |
  | Accent number | Changed files |
  | Pin | Pinned to the top |
  | ⑂ | A worktree (shown under its main project) |

- Hover a row for the details. Arrow keys move between rows, Enter opens one, and `Shift F10` or the menu key
  opens its menu.
- **Many at once**: `Ctrl` click or `Shift` click to select projects. A bar appears with Fetch, Pull…, Push…, and more.
- **Worktrees** fold under their project ("+2 worktrees"). A folded worktree still shows while it is open,
  working, or changed.
- **Hidden projects**: "N hidden" in the footer shows them, dimmed. Right-click one to unhide it.

## A project

The list pane has a toolbar, tabs, a filter row, and the list.

**Toolbar**: the project name (hover for its path), the branch (click, or `Ctrl B`, for branches and
worktrees), and **Fetch**, **Pull ↓N**, **Push ↑N**. Pull and push always show what will happen first:
the commits, and warnings about local changes, a split from the remote, no upstream, or pushing to main.
Right-click the toolbar for the project menu.

**Tabs** (`Ctrl 1`–`Ctrl 7`, in the order your space shows them): Changes, Files, Pull requests, Issues,
History, CI, Tasks. A narrow list shows icons only. Each space can turn tabs off in Settings.

**Filter**: every tab has a filter. `/` jumps to it, Esc clears it, `↓` goes back to the list.

**List keys**: `↑` `↓` or `J` `K` move (changes and files open as you move), `PageUp` `PageDown`, `Home` `End`,
Enter opens, Space selects, `S` stages or unstages, `C` opens Copy for agent, `→` `←` open and close folders,
Backspace closes a review. Every row has a right-click menu, and a `⋯` button that opens the same menu.

### Changes

- Staged and unstaged files, with the kind of change (A, M, D, R, U, !). A green dot marks files saved in
  the last 2 minutes. "Sort changes by most recent" is in the command palette.
- **Stage** (`S`, the + button, "Stage all") and **Unstage**. Stage single blocks with **Hunks** in the viewer.
- **Commit box**: write a message, `Ctrl Enter` commits (with nothing staged, it commits everything),
  `Ctrl Shift Enter` commits and pushes. **Push after** remembers that choice. **Amend** (only when the last
  commit is not pushed) adds to the last commit and lets you change its message.
- **Uncommitted / Since main**: switch to a review of everything this branch did since it left main
  (commits and uncommitted work). **Copy for agent** copies a summary of it.
- **Stash** and **Discard**: right-click files, or use `⋯` on the Staged and Changes headings. Discard asks
  first, moves new files to the Recycle Bin, and can be undone for 3 minutes from the status bar.
- **Stashes** at the bottom: Enter shows what one holds; **Pop** puts it back; right-click to apply and keep,
  copy its name, or drop it.

### Files

Every file in the project as a tree. Type in the filter for a fuzzy search (top 200). `→` `←` open and close folders.

### Pull requests

Open pull requests from GitHub (`gh`) and Azure DevOps (`az`). Enter reviews one here: Tern fetches both
branches and diffs the head with the merge base. **New pull request** opens the form: pick the target branch,
edit the title and description (filled in from your commits), choose draft. Tern pushes the branch first when needed.

### Issues

GitHub issues, Open, Closed, or All. Enter reads one here with its comments.

### History

The commit graph with branch and tag labels. **All branches** shows every branch. The strip at the top says how
your branch compares with its remote; commits you have not pulled show in blue (fold or unfold them), and
**Review incoming** shows one diff of everything a pull would bring. Enter shows the files a commit changed.
Right-click the last commit (when it is not pushed) to undo it; its changes come back staged.

### CI

Pipeline runs from Azure DevOps or GitHub Actions, newest first. The tab shows the result for your branch.
Enter shows stages, jobs, and steps with their errors and warnings, and **Copy problems for agent**. Running
pipelines refresh on their own, and Tern tells you when one finishes.

### Tasks

Tern finds npm, pnpm, yarn, and bun scripts (also in sub-folders), VS Code tasks, .NET, Gradle, Cargo, Make,
and Docker Compose. Enter runs a task, `R` restarts it, `X` stops it, `P` pins it. Type any command in the field
to run it, and **Save** keeps it for the project. Tasks that look like a deploy or a database change ask first.

## The viewer

The toolbar shows the file path (click to copy it), the change kind, and the line range you selected (click to
copy `path:lines`). Then the modes and actions:

- **Diffs**: **Unified / Split** (`Ctrl D`), **Wrap** (`Alt Z`), previous and next change (`Shift F7` / `F7`, with
  "Change 2 of 7"), **Hunks** (`Ctrl Shift H`) to stage, unstage, or discard single blocks, and **File** to see
  the whole file. Unchanged lines fold away; click a fold to open it.
- **Files**: the page view or the source (`Ctrl Shift V`), **Wrap** (`Alt Z`), **Diff** when the file has changes,
  **Edit** (`Ctrl E`; `Ctrl S` saves, **Done** leaves), **VS Code** (`Ctrl Shift E`), and **Copy for agent**
  (`Ctrl Shift C`).
- **Go to definition** (`F12` or `Ctrl` click) and **Find where it is used** (`Shift F12`). There is no language
  server: Tern searches for the usual ways code defines a name. One clear match jumps there; otherwise Search
  opens with the definitions first.
- Right-click code for definition, usages, search, and copy actions. `Ctrl F` searches in the file.
- Click line numbers to select lines; `Shift` click extends.
- In a narrow window, the less used actions move into the `⋯` menu at the end of the toolbar.

## File views

| Files | View | What you can do |
| --- | --- | --- |
| `.md`, `.markdown`, `.mdx` | Page | Headings, tables, task lists, code. Links to files open them in Tern. |
| `.png`, `.jpg`, `.gif`, `.webp`, `.bmp`, `.ico`, `.avif`, `.svg` | Image | Click to switch between fit and actual size. A changed image shows **Before** and **After**, side by side or with **Swipe** (drag the line). |
| `.pdf` | PDF | Shown in place, up to 40 MB. |
| `.json`, `.yaml`, `.toml` (and `package.json`, `Cargo.lock`, …) | Tree | Fold and unfold (arrow keys too), filter, right-click to copy a path or value. Mistakes show their line and column. |
| `.csv`, `.tsv` | Table | Fixed header and row numbers, separator detection, filter, right-click to copy a cell, row, or column. Up to 100 MB. |
| `.log`, `app.log.1` | Log | Follows the file as it grows. Filter by text or `/regex/`, show only errors or warnings, jump between errors (`n` / `N`), pause. Right-click a line to copy it with its context. |
| `.db`, `.sqlite`, `.sqlite3` | Database | Opens in the [Data](#data) view, read-only. |

## Tickets

`Ctrl Shift T`, or **Tickets** in the sidebar. Azure DevOps work items and GitHub issues assigned to you that
are not closed. Grouped by state (in progress first). Filter with `/`, and switch between All, Azure DevOps,
and GitHub. Drag the line between the list and the details to resize.

The details show the project, sprint, people, dates, tags, description, links, comments, and images (click an
image to zoom). Actions: **Open in** Azure DevOps or GitHub, **Copy for agent**, **Copy branch name** (for
example `bug/4790-grid-loses-sort`), **Copy link**. Keys: `↑` `↓` or `J` `K`, Enter opens in the browser,
`C` copies for an agent, `B` copies a branch name. Right-click a ticket for the same actions.

## Data

`Ctrl Shift D`, or **Data** in the sidebar. SQLite, Postgres, SQL Server (and Azure SQL), and Redis.

- **+** adds a connection. **Test** checks it before you save. Passwords go to the system keychain, never to a file.
  SQL Server: **Sign in with** a SQL login, your Windows account, or **Microsoft Entra**. Entra uses your Azure CLI
  sign-in (run `az login` once); Tern stores no password, and the server needs a login for you or your group.
  Write `server\instance` for a named instance.
  A color dot tells production from development.
- **Read-only by default**, enforced by each database: SQLite opens the file read-only, Postgres refuses writes,
  SQL Server rolls back every batch, and Redis allows only commands that read. To change data, turn on
  **Allow writes** in the connection, then the **Read-only** button in the bar. Tern still asks before a query
  that changes data.
- Click a table for its first 100 rows, or a Redis key for its value and when it expires (the second result). Postgres marks **foreign** tables: they read a
  file or another server each time, so they fail when that file is gone (for example an old server log). The editor completes table and column names.
  `Ctrl Enter` runs the query (or only the selected text). Several statements give several results. Results show
  the first 1,000 rows.
- **History** lists the last 50 queries for the connection. **Copy schema** copies every table for an agent.
  **Copy ▾** copies a result for an agent, or as TSV, CSV, JSON, or Markdown.
- The sidebar width and the editor height are remembered.

## Search

`Ctrl Shift F`, **Search** in the sidebar, or `Ctrl F` (this file). Searches code, file names, commits, branches,
pull requests, and issues.

- **Where**: this file, this project, this space, all projects, a saved set, or projects you pick. `Tab` cycles.
- **What**: All, Code, Files, Commits, Branches, PRs & issues (`Alt 1`–`6`).
- **Match case** (`Alt C`), **Whole word** (`Alt W`), **Regular expression** (`Alt R`).
- Right-click a result to copy its path or line. Save a set of projects to search them together.

## Command palette

`Ctrl K` projects, `Ctrl P` files, `Ctrl Shift P` commands, `Ctrl Shift B` tasks. `Tab` switches modes; `>`
switches to commands.

- **Files**: paste a path like `src/app.ts:42` to jump to a line.
- **Shift Enter** copies instead of opening: a project's path, or a file's `@mention`.
- **Tasks**: type any command to run it in the project.
- Every Tern action is a command, including themes and spaces.

## Copy for agent

`Ctrl Shift C`, or `C` in the list. Copies the selected files (or the open file, or the project):

| Key | Format | Example |
| --- | --- | --- |
| `R` | Relative path | `src/app.ts` |
| `A` | Absolute path | `C:\Code\web\src\app.ts` |
| `L` | Path with lines | `src/app.ts:42-60` |
| `M` or `@` | @ mention | `@src/app.ts` |
| `D` | Diff as text | the changes, the review, or the commit |

Enter repeats the last format. Direct keys: `Ctrl Alt R`, `A`, `L`, `M`, `D`. Other agent copies: a commit, a pull
request, an issue, a ticket, a CI run's problems, task output, a branch review summary, a database schema or
result, and code with its place (right-click code).

## Branches and worktrees

`Ctrl B`, or click the branch. **Branches**: find a branch or type a new name; Enter switches (or creates it);
`Alt Enter` opens it in a new worktree. Rows show ahead and behind, "merged", "in a worktree", and "remote gone".
Delete local branches that are merged. **Worktrees**: open, review, or remove each one; **Clean up missing**;
**New worktree…** (pick a branch, a base, a folder, and optionally start Claude Code in it).

## Tasks and terminals

The bottom panel (`Ctrl J`). Tasks run on real terminals, so prompts, colors, and full-screen tools work.
`Ctrl `` ` opens this project's terminal, `Ctrl Shift `` ` a new one. Tabs show the status. The tools show
the time, links to local servers ("Open localhost:5173"), **Clear**, **Restart**, **Stop**, and
**Copy output**. Closing a running task's tab asks first, then stops it. Right-click a tab for more. Double-click the panel's top edge to reset its height. In a terminal, `Ctrl C` copies when text is selected, `Ctrl V`
pastes, and right-click copies or pastes.

## Clone a project

`Ctrl Shift O`, or **+** in the sidebar. Browse Azure DevOps (organization and project) or GitHub (an owner),
or paste a git URL (https, ssh, or git). Pick the folder to clone into (or **Browse…**); Tern clones as a task and opens the project when it is done.

## Spaces

Spaces group projects, for example Work and Personal. A project joins a space by folder rule, then remote rule,
or by hand ("Move to a space…" in its menu). A space shows only its projects, and polling and Fetch all cover
only that space. Each space picks its tabs. Set them up in Settings.

## Notifications and undo

Messages appear in the status bar for a few seconds; errors stay longer. The bell in the title bar lists every
message from this session (finished CI runs, agents that went quiet, failed actions), with a red dot for new
problems. Click a message's project name to open it.

Undo: **Undo discard** in the status bar (3 minutes), "Undo the last commit" (when not pushed), and hunk discards.

## Settings

`Ctrl ,`, or the gear in the sidebar. Sections: **Appearance** (13 themes, including Follow Windows and
High contrast), **Spaces** (rules, colors, tabs), **Folders** (folders to scan and single projects, with
**Browse…**), **Projects** (hidden and pinned), **Updates** (version and links), **Advanced** (scan depth and
refresh interval). Changes need **Save** (`Ctrl S`). A theme you pick shows at once as a preview; Cancel puts the old one back. Closing with unsaved changes asks first.

## Problems and bug reports

**Copy diagnostics** (Settings › Updates, or "Copy diagnostics" in `Ctrl Shift P`) copies a report for a bug:
Tern's version, the system, the window engine, the git, gh, az, and VS Code versions, project counts, recent
errors, and the end of Tern's log. Check it for anything private before you share it. **Show the log folder**
opens the folder with `tern.log`. Tern writes errors and crashes there; the log starts over at 1 MB and keeps
one older copy. "Report a problem" opens a bug form that asks for the diagnostics.

## Updates

Tern checks for a new version a little after it starts and every 6 hours. The status bar shows **Restart to
update** when one is ready. Updates are signed; Tern only installs ones from this project.

## Keyboard reference

Keys are written with spaces: `Ctrl Shift P` means hold Ctrl and Shift, then press P.

### Go

| Keys | Action |
| --- | --- |
| `Ctrl K` | Go to a project |
| `Ctrl P` | Go to a file |
| `Ctrl Shift P` | Run a command |
| `Ctrl Shift F` | Search everywhere |
| `Ctrl F` | Search in this file |
| `Ctrl 0` | Home |
| `Ctrl Shift T` | Tickets |
| `Ctrl Shift D` | Data |
| `Ctrl 1` – `Ctrl 7` | Project tabs |
| `Alt ←` / `Alt →` | Back / forward |
| `Alt ↑` / `Alt ↓` | Previous / next project |
| `Alt Shift ↑` / `Alt Shift ↓` | Move the focused project up or down (sidebar) |
| `F12` / `Shift F12` | Go to definition / find usages |
| `Ctrl Shift O` | Clone a project |
| `Ctrl \` | Show or hide the sidebar |

### Spaces

| Keys | Action |
| --- | --- |
| `Ctrl Shift S` | Switch space |
| `Ctrl Alt 0` | All spaces |
| `Ctrl Alt 1` – `9` | Space 1 to 9 |

### Copy

| Keys | Action |
| --- | --- |
| `Ctrl Shift C` | Copy for agent |
| `Ctrl Alt R` / `A` / `L` / `M` / `D` | Relative path / absolute path / with lines / @ mention / diff |

### Git

| Keys | Action |
| --- | --- |
| `Ctrl Shift Enter` | Write a commit message (in the box: commit and push) |
| `Ctrl Enter` | Commit (in the box), or go to the commit box (in the list) |
| `Ctrl B` | Branches and worktrees |
| `Ctrl Alt F` | Fetch all |
| `F5` | Reload status and files |

### View

| Keys | Action |
| --- | --- |
| `Ctrl D` | Split or unified diff |
| `Alt Z` | Wrap long lines |
| `F7` / `Shift F7` | Next / previous change |
| `Ctrl Shift H` | Hunks |
| `Ctrl Shift V` | Page view or source |
| `Ctrl E` | Quick edit (`Ctrl S` saves) |
| `Ctrl Shift E` | Open in VS Code |
| `Ctrl J` | Task panel |
| ``Ctrl ` `` / ``Ctrl Shift ` `` | This project's terminal / a new terminal |
| `Ctrl Shift B` | Run a task |
| `Ctrl ,` | Settings |
| `F1` or `?` | Keyboard shortcuts |
| `Shift F10` | Project menu |

### In lists

| Keys | Action |
| --- | --- |
| `↑` `↓` or `J` `K` | Move |
| `PageUp` `PageDown` `Home` `End` | Move further |
| `Enter` | Open |
| `Space` | Select (files) |
| `S` | Stage or unstage |
| `C` | Copy for agent |
| `/` | Filter |
| `→` `←` | Open or close a folder |
| `P` / `R` / `X` | Pin / restart / stop a task (Tasks tab) |
| `Backspace` | Close a review |
| `Esc` | Close a dialog, leave edit, clear the selection, or close a review |

Dialogs have their own keys, shown at their bottom: the palette (`Tab`, `Shift Enter`), Search (`Tab`, `Alt 1`–`6`,
`Alt C` `W` `R`), branches (`Alt Enter`), Copy for agent (`R` `A` `L` `M` `D`), spaces (`0`–`9`), and the database
editor (`Ctrl Enter`).

## Right-click menus

| Where | Items |
| --- | --- |
| Project (sidebar, Home) | Open; copy path, @ mention, remote address, branch name; Fetch, Pull…, Push…; terminal, Claude Code, external terminal, VS Code, Show in folder; Branches…, Worktrees…, Remove this worktree…, Review everything since main; Pin or Unpin, Move to a space…, Hide or Unhide |
| Several projects | Fetch, Pull…, Push…; copy paths, @ mentions; open a terminal in each; Pin or Unpin, Move…, Hide or Unhide; clear the selection |
| File or change | Copy relative path, absolute path, @ mention, path with lines, diff; show the file or the diff; Stage or Unstage, Stash, Discard…; VS Code, Show in folder, Open in browser |
| Commit | Show its files, Undo this commit…; copy ID, full ID, message, for agent; Open in browser |
| Pull request | Review the changes, Open in browser; copy link, number, branch, for agent |
| CI run | Show stages and jobs, Open in browser; copy link, run number, commit ID |
| Issue | Read it here, Open in browser; copy link, number, title, for agent |
| Stash | Show what it holds; Pop, Apply; copy its name, message; Drop… |
| Staged or Changes heading (`⋯`) | Stash all, stash this group; discard this group…, discard all… |
| Task | Run or show the output, Restart, Stop, Pin or Unpin; copy the command, copy the last output for agent; save as my own command, remove a saved command |
| Code | Go to definition, find usages, search for the name; copy, copy `path:line`, copy `@path`, copy the selection with its place; open in VS Code at this line |
| Ticket | Open in browser, copy for agent, copy branch name, copy link, copy number |
| Task tab | Restart, Stop, copy command, copy output for agent, show project, close |
| Log line | Copy line, copy with context, copy the shown lines for agent |
| Tree row | Copy path, copy value as JSON, copy text |
| Table cell / header | Copy cell, row, row as JSON; copy column name, column values |
| Database connection | Edit, refresh schema, copy schema for agent, disconnect, delete… |
| Database table | Show the first 100 rows (or the value), count rows, copy name, copy schema for agent |

## Where Tern keeps things

- **Settings file**: folders, single projects, hidden and pinned projects, spaces, saved tasks and pins, scan depth,
  refresh interval. Settings shows its path at the bottom.
- **Database connections**: `databases.json` next to the settings file. Passwords are in the system keychain.
- **Log**: `tern.log` in Tern's log folder (Settings › Updates › Show the log folder).
- **On this computer only** (browser storage, names start with `tern.`): theme, layout widths, the open project and tab, diff layout,
  page-view choice, search options and saved sets, query text and history, and other view choices.
