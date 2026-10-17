# Tern design system

This is how every screen in Tern looks and behaves. Read it before you change the UI. The goal: Tern is a calm
tool for reviewing code. Every screen uses the same parts, so nothing has to be learned twice.

## Principles

1. **Review first.** The work on screen (a diff, a file, a result) gets the most room. Chrome is quiet.
2. **One way to do a thing, in one place.** Each action has a home. A second entry point is fine only as a
   shortcut (a key or a right-click menu), never as a second, different-looking button.
3. **Everything is reachable three ways.** A visible control, a right-click menu, and the command palette
   (`Ctrl Shift P`). Common actions also have a key.
4. **Show what is hidden.** If a row has a right-click menu, it also has a `⋯` button on hover or focus. Keys
   appear in hover cards and menus.
5. **Same words everywhere.** See the word list below.

## Layout

```
┌──────────────────────────── title bar (38px) ─────────────────────────────┐
│ ← →  Space / Project / branch      [ Go to or run…  Ctrl K ]     🔔  ▶2  │
├──────────────┬────────────────────────────────────────────────────────────┤
│ Sidebar      │  Main area: Home, a project (list + viewer), Tickets, Data  │
│  Space ▾     │                                                            │
│  Home        │                                                            │
│  Tickets  5  │                                                            │
│  Data        │                                                            │
│  Search      │                                                            │
│  PROJECTS +  │                                                            │
│  filter      ├────────────────────────────────────────────────────────────┤
│  repo rows   │  Bottom panel: tasks and terminals (Ctrl J)                │
│  ⚙  counts   │                                                            │
├──────────────┴────────────────────────────────────────────────────────────┤
│ status bar (24px): project path · branch · toasts · undo · tasks          │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Sidebar** (`Rail.svelte`): the one place to go anywhere. Space switcher on top, then the destinations
  (Home, Tickets, Data, Search), then the projects. Settings sits in its footer. It can be resized, and
  collapsed with `Ctrl \`.
- **Title bar** (`TitleBar.svelte`): back and forward, where you are (a breadcrumb), the command bar,
  notifications, and running tasks. Window controls on Windows and Linux.
- **Main area**: one destination at a time.
  - **Home** (`Overview.svelte`): what needs you (changes, pushes, pulls, failing CI, tickets in progress),
    then every project in a table you can sort and filter.
  - **Project**: the list pane (`ListPane.svelte`) and the viewer (`Viewer.svelte`).
  - **Tickets** (`Tickets.svelte`) and **Data** (`DataView.svelte`): their own list and detail.
- **Bottom panel** (`TaskPanel.svelte`): tasks and terminals.
- **Overlays**: the palette, search, settings, and dialogs. All use the dialog parts below.

## Tokens (in `src/app.css`)

Themes change colors only. Sizes never change per theme.

| Group | Tokens |
| --- | --- |
| Surfaces | `--bg` (content), `--panel` (sidebar, bars, dialogs), `--raised` (chosen, cards), `--hover` |
| Lines | `--line`, `--line-strong` |
| Text | `--text`, `--muted`, `--faint` |
| Accent | `--accent` (same as `--amber`, which every theme sets), `--accent-soft`, `--amber-sel`, `--on-accent` |
| Status | `--add`, `--add-bg`, `--del`, `--del-bg`, `--blue` |
| Type | `--fs-xs` 11, `--fs-sm` 12, `--fs-md` 13, `--fs-lg` 15, `--fs-xl` 20; `--ui`, `--mono` |
| Space | `--s1` 4, `--s2` 6, `--s3` 8, `--s4` 12, `--s5` 16, `--s6` 24 |
| Shape | `--r-sm` 4, `--radius` 6, `--r-lg` 10 |
| Heights | `--h-ctl` 26 (buttons, inputs, groups), `--h-ctl-sm` 22, `--h-row` 28, `--h-bar` 40, `--h-title` 38, `--h-status` 24 |
| Depth | `--backdrop`, `--shadow-pop` |

Use only these sizes. Do not invent `13.5px` or `padding: 3px 11px`.

## Parts (global classes in `src/app.css`)

Components use these classes as they are. Do not restyle them inside a component. If a part is missing,
add it to `app.css` and to this file.

| Part | Class | Use |
| --- | --- | --- |
| Button | `.btn` | Bordered, for most actions. |
| | `.btn.ghost` | Quiet actions in toolbars and rows. |
| | `.btn.primary` | The one main action in a view or dialog. At most one per view. |
| | `.btn.danger`, `.btn.primary.danger` | Destructive actions. Confirm dialogs for destructive actions use the red primary. |
| | `.btn.on` | A toggle that is on (Hunks, Wrap). |
| | `.btn.sm` | Inside rows and dense bars. |
| Icon button | `.icon-btn` (`.sm`) | Square, quiet. Needs `aria-label` and a hover card. |
| Group | `.seg` | One choice out of 2 to 5 (Unified / Split, All / Errors). |
| Tabs | `.tabs` | Switches what a pane shows. Accent underline on the chosen tab. |
| Field | `.field` | Every `input`, `select`, and `textarea`. Filters use a field with a placeholder that starts with "Filter". |
| Checkbox | `.check` | A label around a checkbox. |
| Toolbar | `.toolbar` | Every pane header. 40px. `.title` and `.meta` on the left, `.grow`, actions on the right. `.toolbar.sub` (36px, panel color) for a second bar under it. |
| Count | `.badge` (`.accent`, `.add`, `.del`) | Counts in tabs and the sidebar. |
| Label | `.chip` | Tags and small labels. |
| States | `.state` (`.inline`) with `.state-title`, `p`, `.actions`, `pre` | Every empty, loading, and error message. Errors have a "Try again" action when one makes sense. |
| Loading | `.skeleton` | Shimmer blocks while a list loads. |
| Dialog | `.backdrop` > `.dialog` (`.sm` 440, default 600, `.lg` 820, `.xl` 960) > `.dialog-head`, `.dialog-body`, `.dialog-foot` | Every modal. 12vh from the top. Esc and a backdrop click close it. Main button on the right of the footer. |
| Section label | `.section-label` | A small muted heading above a group of rows. |
| Key | `kbd` | Shortcuts. Write keys with spaces: `Ctrl Shift P`. |

## Behavior rules

- **Toolbar buttons**: secondary actions in a toolbar are all `.btn.ghost` (or `.icon-btn`). The one main action
  is `.btn.primary`. Toggles are `.btn.ghost` that become `.btn.on`. A state (like "Read-only") is a `.chip`, not a button.
  Never mix bordered and ghost buttons in one bar. Bordered `.btn` is for dialogs, forms, and states.
- **Chosen row vs cursor**: the chosen (open) row uses `background: var(--raised)` only. The accent ring marks the
  keyboard cursor and shows only while the list has keyboard focus (`:focus-visible` on the list, or after an arrow key).
- **Accent use**: only the primary button, the chosen tab underline, focus rings, the active sidebar marker, and
  "needs you" signals (changed-file counts, dirty dots). Not for tags, hashes, headings, or "current" labels.
- **Code font**: code, diffs, hashes, and file paths in headers. Not for branch names in lists, meta lines, or table cells
  that are not code.
- **Narrow windows**: toolbars never clip. The title (path, name) shrinks with an ellipsis; lower-priority actions move
  into a `⋯` overflow menu (container queries).

- **Toolbars** are always at the top of their pane. Never at the bottom, never inside the scrolled content.
- **Rows** are 28px (one line) or about 44px (two lines). The chosen row uses `--raised`. The keyboard cursor
  uses an accent ring. Multi-selected rows use `--accent-soft`.
- **Right-click menus**: every list row has one. Destructive items are red at all times. Items with a key show it.
  "Destructive" means it cannot be undone from the screen (discard, drop, delete, remove). Hide and Unpin are not red.
- **Actions inside cards and rows** (Home cards, list rows) are `.btn.ghost.sm` or `.icon-btn.sm`, like toolbars.
- **Project tabs** rest on short words without icons ("PRs"), so the strip looks the same for 4 tabs or 7. They
  never clip: when words do not fit, counts go first, then only the chosen tab keeps its label, then icons only.
- **Hover cards** explain icons and dots. Use the `hover` action, not the native `title`, except on plain text.
- **Confirmations** list what will happen. Destructive ones use a red main button.
- **Loading** under 300 ms shows nothing. Longer lists show skeleton rows. Single values show "Loading…".
- **Motion**: 120 to 200 ms, `--ease-out`. Nothing moves when the system asks for reduced motion.
- **Focus**: every control can be reached with Tab and shows the accent focus ring.

## Words

| Say | Not |
| --- | --- |
| project | repo (in the UI) |
| Open in browser | Open on the web, Open in GitHub |
| Show in folder | Show in File Explorer, Show in Explorer |
| Copy for agent | Copy for an agent |
| Pull requests (tab: "PRs" is fine when narrow) | |
| Fetch all | Fetch all projects |
| Pin, Unpin | Pin to top, Pin to the top |
| Filter… (placeholder) | Search (when it only filters the list on screen) |
| Ctrl Shift P (keys with spaces) | Ctrl+Shift+P |

Sentences in the UI are short and plain. Buttons are verbs. Use "…" on a button only when it opens a dialog
that asks for more.
