# Tern guide

Everything Tern does, in detail. For the overview, see the [README](../README.md).

## Features

- Finds every git repo under the folders in Settings, plus repos you add by hand.
  Linked worktrees (for example the ones agents make) show under their main repo.
- Shows branch, changed files, and commits to pull or push for every repo. Status refreshes
  every few seconds while the window has focus, and at once when you come back to the window.
- Changes, Files, Pull requests, and History tabs for the open project.
- Diffs in unified or split layout, with unchanged lines folded away.
- Pull requests from GitHub (`gh`) and Azure DevOps (`az repos`). The diff is made on your machine:
  Tern fetches both branches and compares the PR head with the merge base.
- Fetch, pull (fast-forward only), and push for one repo, or fetch for all repos at once.
- Copy paths for agents: relative, absolute, with line numbers (`src/app.ts:42-60`),
  as an `@src/app.ts` mention, or the diff as text. Works on many selected files at once.
- Spaces (for example Work and Personal). A space shows only its projects, and polling and
  "Fetch all" cover only that space. A repo joins a space by folder rule, then remote rule, or by hand
  ("Move this project to a space…"). Repos that match no rule go to the default space.
- Agent activity. A repo is "working" while its changed files were saved in the last 2 minutes.
  A blue dot means it changed since you last looked. When a working repo goes quiet, Tern says so
  in the status bar. Files changed in the last 2 minutes get a green dot.
- Stage (`S`) and commit (`Ctrl Enter`, or `Ctrl Shift Enter` to also push) from the Changes tab.
  With nothing staged, the button commits all changes.
- Stash and discard. Right-click a file (or several selected files), or use the `⋯` on the Staged and Changes
  headers: **Stash** saves the changes away, new files included. **Discard** puts changed files back to the
  last commit and moves new files to the Recycle Bin. It asks first, and the status bar offers **Undo discard**
  for a few minutes (changed files come back; new files are restored from the Recycle Bin).
- Stashes show at the bottom of the Changes tab, with their branch, file count, and age. Enter shows what a
  stash holds as a review, new files included. **Pop** puts it back and removes it; right-click to apply it
  and keep it, copy its name, or drop it.
- Stage by hunk: in a diff of your local changes, **Hunks** (or `Ctrl Shift H`) opens a panel with each block of
  changes. **Stage** or **Discard** a block that is not staged yet, or **Unstage** a staged one. Click a block's line
  number to scroll the diff there. Discarding a block asks first and can be undone from the status bar.
- Amend: when the last commit is not pushed, the commit box has an **Amend** box. It puts the last message back in
  the box; the button then adds the staged changes to that commit and replaces its message. With nothing staged, it
  changes only the message.
- Create a pull request: **Create one** on the empty PRs tab, or "Create a pull request" in `Ctrl Shift P`. Pick the
  branch to merge into, edit the title and description (filled in from your commits), and choose a draft if you like.
  Tern pushes the branch first when the remote does not have all of it. Works with GitHub (`gh`) and Azure DevOps (`az`).
- Updates: Tern checks GitHub Releases a little after it starts and every 6 hours. When a new version is out, the status
  bar shows **Restart to update**. Settings shows your version, with **Check for updates** and links to the release notes.
  Updates are signed, and Tern only installs ones signed by this project. (Linux: the AppImage updates itself; for
  .deb and .rpm, install the new file.)
- Undo the last commit (right-click it in History, or "Undo the last commit" in `Ctrl Shift P`) when it is
  not pushed yet. Its changes come back staged and its message goes back in the commit box.
- Pull and push always ask first. The dialog lists the commits that come in or go out, and warns about
  local changes, a branch that has split from the remote, no upstream, or pushing straight to main.
  "Commit and push" asks too.
- Right-click a project (or `Shift F10`) for its menu: open, copy path, fetch, pull, push, terminal,
  VS Code, pin, move to a space, hide. The "N hidden" button under the list shows hidden projects,
  dimmed; right-click one to unhide it.
- Tasks tab (`Ctrl 5`, or `Ctrl Shift B` from anywhere). Tern finds npm scripts (also in sub-folders
  like `cdk/` and `web/`, with the right package manager), VS Code `tasks.json`, .NET build/test/run/watch,
  Gradle, Cargo, Make, and Docker Compose. You can run any command and save it for the project.
  Tasks that look like a deploy, publish, release, or database change ask first.
  Pin a task (`P`, the pin button on hover, or right-click) to put it in a "Pinned" group at the top.
  Pins are kept per project (each worktree has its own) in the settings file.
- The bottom panel (`Ctrl J`) shows tasks as real terminals: type straight into a running task (prompts,
  arrow keys, and `Ctrl C` work), with Clear, Stop, Restart, Open buttons for local addresses the task prints,
  and "Copy output" for agents. Tern stops every task and terminal it started when it closes.
- Built-in terminals (`Ctrl `` ` for the project's terminal, `Ctrl Shift `` ` for a new one). Tasks and terminals
  run on pseudo-terminals (ConPTY), so full-screen tools like Claude Code work. Inside a terminal, all keys go
  to the program except `Ctrl J`, `Ctrl `` `, `Ctrl Shift `` `, and `Ctrl Shift P`. As in Windows Terminal:
  `Ctrl C` copies when text is selected, `Ctrl Shift C` copies, `Ctrl V` and `Ctrl Shift V` paste, and
  right-click copies the selection or pastes when nothing is selected.
- Tickets (`Ctrl Shift T`, or Tickets in the title bar): your open Azure DevOps work items (from the organizations
  your repos use, through `az boards`) and GitHub issues assigned to you (through `gh`), grouped by state.
  The detail shows the description, repro steps, acceptance criteria, linked commits and PRs, and comments.
  Images in tickets show inline (click one to see it full-size). Tern downloads them with your `az` or `gh`
  sign-in, and sends each token only to its own service, never to another host on a redirect.
  Keys: `↑` `↓` move, `Enter` opens it in the browser, `C` copies it for an agent, `B` copies a branch name
  like `bug/4790-grid-loses-its-sort`, `/` filters. Read only: Tern does not change tickets.
- Hover cards: rest the mouse on a project, file, commit, pull request, issue, task, or the Pull/Push
  buttons to see more (full commit ID, author, dates, sync state, last run). They close as soon as you move on.
- Right-click menus with copy options on projects, files, commits, pull requests, issues, tasks, and code.
- History shows a commit graph with branch and tag labels. "All branches" includes every local and
  remote branch. Click a commit ID to copy it.
- Each space picks its tabs (Settings). `Ctrl 1`–`6` follow the tabs the project shows.
- Issues tab for GitHub repos (through `gh`), with the full text and comments and "Copy for an agent".
- Clone a repo (`Ctrl Shift O`, or `+` above the project list) from Azure DevOps (all projects in the
  organization) or GitHub. Repos already on this machine are marked. Progress shows in the panel.
- Many projects at once: `Ctrl+click` and `Shift+click` in the project list (or the overview), then
  Fetch, Pull, Push, or the `⋯` menu (copy paths, open terminals, pin, move to a space, hide).
  Pull and push ask once, with one line per project saying what will happen or why it is skipped.
- History reloads by itself when a branch, tag, or HEAD moves (commit, pull, fetch, checkout,
  or a commit in a worktree). Tern sees this from git's branch files, without running git.
- Branch review: on a worktree or a non-main branch, the Changes tab has **Uncommitted | Since main**.
  "Since main" shows everything the branch did since it split from its base: its commits plus
  uncommitted and new files, compared file by file with the split point. Tern picks the base that
  split most recently (so a stale local `main` does not flood the list); change it in the header.
  "Copy for an agent" copies the commits and `@` file mentions. The review updates as the agent works.
- Branches and worktrees (`Ctrl B`, or click the branch name under the project name):
  - Branches: newest first, with ahead/behind, which worktree has each, "merged" (into the remote
    default branch), and "remote gone". Enter switches (asks first when you have uncommitted work).
    Type a new name to make a branch. `Alt Enter` opens a branch in a new worktree. Delete uses
    `git branch -d`, which refuses unmerged work.
  - Worktrees: open, review since main, remove, or make a new one (new or existing branch, with an
    option to start Claude Code in it). New worktrees go in `.claude\worktrees` when that folder
    exists, otherwise in `<repo>.worktrees` beside the repo. Remove asks first and says what is lost.
    Git cannot remove worktrees with submodules or broken ones; Tern moves those folders to the
    Recycle Bin and then prunes. "Clean up missing" forgets worktrees whose folder is gone.
- Themes (Settings, or "theme" in `Ctrl Shift P`): Follow Windows, Slate, Daylight, Forest, Pine,
  Moss, Ocean, Plum, Ink, Fjord, Ember, Sand, High contrast. Code, diffs, the graph, and terminals follow the theme.
- Back and forward: mouse side buttons, or `Alt ←` / `Alt →`.
- Tern draws its own title bar in the theme colors: the space switcher, a "Go to" field (`Ctrl K`), Search,
  Clone, Fetch all, Settings, and the window buttons. Drag the empty parts to move the window; double-click
  to maximize. On macOS the system's window buttons stay on the left.
- Worktrees fold under their main repo. One still shows while it is working, unseen, or open.
- Search across repos (`Ctrl Shift F`, from anywhere): code (`git grep`, includes new files, skips
  ignored ones), file names, commit messages, branches, and PRs and issues (for 10 repos or fewer).
  Scope: the open repo (the default when one is open), the space, all repos, a picked list, or a
  saved set (Pick…, then "Save set"; right-click a set to change or delete it). `Tab` changes scope,
  `Alt 1`–`6` the kind, `Alt C` match case, `Alt R` regular expression. Enter opens the file at the line.
  Fetch all moved to `Ctrl Alt F`. `Ctrl F` opens the same search on "This file" (with the word at the
  cursor filled in). `Alt W` matches whole words only.
- Go to definition (`F12`, `Ctrl+click`, or right-click a name) and Find where it is used (`Shift F12`).
  There is no language server: Tern searches for the usual ways code defines a name (`class X`,
  `function X`, `const X =`, C#/Java method and property lines, and so on). One clear match jumps there;
  otherwise the search opens with "Definitions" first and then every use (whole word).
- Paste a path from an agent into `Ctrl P` (for example `src/app.ts:42`) to jump there.
- Quick edit (`Ctrl E`, then `Ctrl S`) for small fixes.
- Markdown files show as a page (headings, tables, task lists, code). **Page | Source** in the header,
  or `Ctrl Shift V`, switches. Links to other files open them in Tern. Web links open in the browser.
- Other files get their own view too. `Ctrl Shift V` switches between that view and the source, as for Markdown.
  - **Images** (PNG, JPG, GIF, WebP, BMP, ICO, AVIF, and SVG): click to switch between fit and actual size.
    A changed image in a diff shows **Before** and **After** side by side, or **Swipe** to slide between them.
  - **PDFs** show in place, up to 40 MB.
  - **JSON, YAML, and TOML** show as a tree. Click a row to fold or unfold it. Right-click to copy its path
    or value. Big lists show 200 items at a time. A file with a mistake shows the line and column.
  - **CSV and TSV** show as a table with a fixed header and row numbers. Tern finds the separator (comma,
    semicolon, tab, or pipe). Filter rows, or right-click a cell to copy it or the row. Tables up to 100 MB open.
  - **Logs** (`.log`, and rotated logs like `app.log.1`) follow the file as it grows. Filter by text or
    `/regex/`, show only errors or warnings, and pause. Scroll up to stop following; the button at the bottom
    starts it again. If the log is cleared or rotated, the view starts over.
- CI tab: pipeline runs from Azure DevOps (`az pipelines`) or GitHub Actions (`gh run`) for the repo,
  newest first, with the result, branch, commit, and time. The tab shows the newest result for your
  branch. Enter opens a run: stages, jobs, and steps, with the errors and warnings. "Copy problems for an
  agent" copies the failed steps and their messages. Running pipelines refresh every 10 to 20 seconds,
  and the status bar says when one finishes. Turn the tab on per space in Settings.

## Shortcuts

| Keys | Action |
| --- | --- |
| `Ctrl K` / `Ctrl P` / `Ctrl Shift P` | Find a project / a file / a command |
| `Alt ↑` `Alt ↓` | Previous / next project |
| `Ctrl Alt 1`–`9` / `Ctrl Alt 0` | Switch to a space / all spaces |
| `Ctrl Shift S` | Space picker |
| `S` | Stage or unstage the file under the cursor (or the selected files) |
| `Ctrl Enter` | Go to the commit box; in the box, commit |
| `Ctrl 0` | All projects |
| `Ctrl 1`–`4` | Changes, Files, Pull requests, History |
| `↑` `↓` or `J` `K` | Move through the list; changes and files open as you move |
| `Space` | Select a file (Ctrl+click and Shift+click work too) |
| `Ctrl Shift C` or `C` | Copy menu, then `R` `A` `L` `M` `D` |
| `Ctrl Alt R/A/L/M/D` | Copy in one format directly |
| `Ctrl D` | Split or unified diff |
| `Ctrl Shift F` / `Ctrl F` | Search everywhere / search this file |
| `Ctrl Alt F` | Fetch all projects |
| `Ctrl Shift V` | Page view (Markdown, tree, table, image, log) or source |
| `` Ctrl ` `` / `` Ctrl Shift ` `` | Terminal here / Claude Code here |
| `Ctrl Shift E` | Open in VS Code |
| `F5` | Reload |
| `F1` | All shortcuts |

