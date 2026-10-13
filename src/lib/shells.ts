// Built-in terminals. Each one is an xterm.js screen connected to a pseudo-terminal in the back end:
// shells (the >_ tabs) and tasks (npm scripts, builds, commands you type in the Tasks tab).
// xterm.js loads only when the first terminal opens, so Tern starts fast.

import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import { api, copyText, inTauri, openUrl, readClipboard } from "./api";

type Session = { term: Terminal; fit: FitAddon; el: HTMLDivElement; opened: boolean };

const sessions = new Map<number, Session>();
/** Task terminals, by run id. `replaying` is true while old output is written back into a new view. */
const runViews = new Map<number, Session & { replaying: boolean }>();
/** Raw task output (with escape codes), kept so a task's view can be built or rebuilt at any time. */
const runRaw = new Map<number, string>();
const RAW_MAX = 2_000_000;
let windowsBuild: Promise<number> | null = null;
/** Output that arrived before its terminal was set up. */
const early = new Map<number, string[]>();
let listening: Promise<void> | null = null;

function cssVar(name: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function theme() {
  const c = (n: string) => cssVar(`--t-${n}`);
  return {
    background: cssVar("--bg"),
    foreground: cssVar("--text"),
    cursor: cssVar("--amber"),
    cursorAccent: cssVar("--bg"),
    selectionBackground: cssVar("--amber-sel"),
    black: c("black"), red: c("red"), green: c("green"), yellow: c("yellow"),
    blue: c("blue"), magenta: c("magenta"), cyan: c("cyan"), white: c("white"),
    brightBlack: c("bright-black"), brightRed: c("bright-red"), brightGreen: c("bright-green"),
    brightYellow: c("bright-yellow"), brightBlue: c("bright-blue"), brightMagenta: c("bright-magenta"),
    brightCyan: c("bright-cyan"), brightWhite: c("bright-white"),
  };
}

/** Starts one listener for all terminals. `onExit` is called when a shell ends. */
function listen(onExit: (id: number, code: number | null) => void) {
  if (listening) return listening;
  listening = (async () => {
    if (!inTauri) return;
    const { listen } = await import("@tauri-apps/api/event");
    await listen<{ id: number; data: string }>("pty-output", (e) => {
      const s = sessions.get(e.payload.id);
      if (s) s.term.write(e.payload.data);
      else early.set(e.payload.id, [...(early.get(e.payload.id) ?? []), e.payload.data]);
    });
    await listen<{ id: number; code: number | null }>("pty-exit", (e) => {
      const s = sessions.get(e.payload.id);
      s?.term.write(`\r\n\x1b[2m[The shell ended${e.payload.code != null ? ` with code ${e.payload.code}` : ""}. Close this tab or open a new terminal.]\x1b[0m\r\n`);
      onExit(e.payload.id, e.payload.code);
    });
  })();
  return listening;
}

/** One xterm.js screen with Tern's keys: the setup shells and tasks share. */
async function makeTerminal() {
  const [{ Terminal }, { FitAddon }, { WebLinksAddon }, build] = await Promise.all([
    import("@xterm/xterm"),
    import("@xterm/addon-fit"),
    import("@xterm/addon-web-links"),
    (windowsBuild ??= api.osBuild().catch(() => 0)),
    import("@xterm/xterm/css/xterm.css"),
  ]);
  const term = new Terminal({
    fontFamily: cssVar("--mono"),
    fontSize: 13,
    lineHeight: 1.15,
    cursorBlink: true,
    cursorStyle: "bar",
    scrollback: 10000,
    theme: theme(),
    allowProposedApi: true,
    // Tells xterm.js the output comes from ConPTY, so lines wrap and reflow correctly on resize.
    ...(build ? { windowsPty: { backend: "conpty" as const, buildNumber: build } } : {}),
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new WebLinksAddon((_e, uri) => openUrl(uri)));

  const paste = async () => {
    const text = await readClipboard().catch(() => "");
    if (text) term.paste(text);
  };
  // Like Windows Terminal: Ctrl+C copies when text is selected (otherwise it goes to the program),
  // Ctrl+Shift+C copies, Ctrl+V and Ctrl+Shift+V paste.
  term.attachCustomKeyEventHandler((e) => {
    if (e.type !== "keydown") return true;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.code === "KeyC" && (e.shiftKey || term.hasSelection())) {
      if (term.hasSelection()) copyText(term.getSelection());
      term.clearSelection();
      return false;
    }
    if (ctrl && e.shiftKey && e.code === "KeyV") {
      e.preventDefault();
      paste();
      return false;
    }
    if (ctrl && e.code === "KeyV") return false; // the browser's paste event does the paste
    return true;
  });

  const el = document.createElement("div");
  el.className = "xterm-host";
  // Right-click copies the selection, or pastes when nothing is selected (as in Windows Terminal).
  el.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (term.hasSelection()) {
      copyText(term.getSelection());
      term.clearSelection();
    } else paste();
    term.focus();
  });
  return { term, fit, el };
}

export async function openShell(cwd: string, onExit: (id: number, code: number | null) => void) {
  if (!inTauri) throw new Error("Terminals work in the Tern app, not in a plain browser.");
  await listen(onExit);
  const { term, fit, el } = await makeTerminal();
  const { id, shell } = await api.ptyOpen(cwd, 120, 30);
  sessions.set(id, { term, fit, el, opened: false });
  term.onData((d) => api.ptyWrite(id, d).catch(() => {}));
  term.onResize(({ cols, rows }) => api.ptyResize(id, cols, rows).catch(() => {}));
  for (const chunk of early.get(id) ?? []) term.write(chunk);
  early.delete(id);
  return { id, shell };
}

/** Puts a terminal on screen inside `host` and sizes it to fit. */
export function attachShell(id: number, host: HTMLElement) {
  const s = sessions.get(id);
  if (!s) return;
  // One terminal on screen at a time. The others stay alive, just off screen.
  for (const child of [...host.children]) if (child !== s.el) child.remove();
  if (s.el.parentElement !== host) host.appendChild(s.el);
  if (!s.opened) {
    s.term.open(s.el);
    s.opened = true;
  }
  fitShell(id);
  s.term.focus();
}

export function fitShell(id: number) {
  const s = sessions.get(id);
  if (!s?.opened || !s.el.isConnected || s.el.clientWidth === 0) return;
  try {
    s.fit.fit();
  } catch {
    /* the panel is hidden */
  }
}

/** Gives open terminals the colors of a new theme. */
export function retheme() {
  const t = theme();
  for (const s of sessions.values()) s.term.options.theme = t;
  for (const s of runViews.values()) s.term.options.theme = t;
}

// Task terminals

/** Task output from the back end. It goes to the task's view when there is one, and is kept for later. */
export function feedRun(id: number, data: string) {
  let raw = (runRaw.get(id) ?? "") + data;
  if (raw.length > RAW_MAX) raw = raw.slice(raw.length - RAW_MAX / 2);
  runRaw.set(id, raw);
  runViews.get(id)?.term.write(data);
}

/** Puts a task's terminal on screen inside `host`. You type straight into it. */
export async function attachRun(id: number, host: HTMLElement) {
  let v = runViews.get(id);
  if (!v) {
    const made = await makeTerminal();
    if (runViews.has(id)) v = runViews.get(id)!;
    else {
      v = { ...made, opened: false, replaying: true };
      const view = v;
      runViews.set(id, view);
      // Old output may hold questions to the terminal (like "where is the cursor?"). The program
      // asked them long ago, so the answers xterm.js gives while replaying must not reach it.
      made.term.write(runRaw.get(id) ?? "", () => (view.replaying = false));
      made.term.onData((d) => {
        if (!view.replaying) api.runInput(id, d).catch(() => {});
      });
      made.term.onResize(({ cols, rows }) => api.runResize(id, cols, rows).catch(() => {}));
    }
  }
  if (!host.isConnected) return;
  for (const child of [...host.children]) if (child !== v.el) child.remove();
  if (v.el.parentElement !== host) host.appendChild(v.el);
  if (!v.opened) {
    v.term.open(v.el);
    v.opened = true;
  }
  fitRun(id);
  v.term.focus();
}

export function fitRun(id: number) {
  const v = runViews.get(id);
  if (!v?.opened || !v.el.isConnected || v.el.clientWidth === 0) return;
  try {
    v.fit.fit();
  } catch {
    /* the panel is hidden */
  }
}

export function clearRun(id: number) {
  runViews.get(id)?.term.clear();
}

/** Plain text of a task's last lines as the terminal shows them, or "" when it has no view yet. */
export function runText(id: number, lines = 200) {
  const v = runViews.get(id);
  if (!v) return "";
  const buf = v.term.buffer.active;
  const out: string[] = [];
  for (let i = Math.max(0, buf.length - lines); i < buf.length; i++) out.push(buf.getLine(i)?.translateToString(true) ?? "");
  while (out.length && !out[out.length - 1].trim()) out.pop();
  return out.join("\n");
}

/** Frees a closed task's terminal and output. */
export function disposeRun(id: number) {
  const v = runViews.get(id);
  runViews.delete(id);
  runRaw.delete(id);
  v?.term.dispose();
  v?.el.remove();
}

export function focusShell(id: number) {
  sessions.get(id)?.term.focus();
}

export function writeToShell(id: number, text: string) {
  return api.ptyWrite(id, text);
}

export function clearShell(id: number) {
  sessions.get(id)?.term.clear();
}

export function selectionOf(id: number) {
  return sessions.get(id)?.term.getSelection() ?? "";
}

/** Plain text of the last lines, for pasting to an agent. */
export function shellText(id: number, lines = 200) {
  const s = sessions.get(id);
  if (!s) return "";
  const buf = s.term.buffer.active;
  const out: string[] = [];
  for (let i = Math.max(0, buf.length - lines); i < buf.length; i++) out.push(buf.getLine(i)?.translateToString(true) ?? "");
  while (out.length && !out[out.length - 1].trim()) out.pop();
  return out.join("\n");
}

export function closeShell(id: number) {
  const s = sessions.get(id);
  sessions.delete(id);
  s?.term.dispose();
  s?.el.remove();
  api.ptyClose(id).catch(() => {});
}

export function isInsideTerminal(target: EventTarget | null) {
  return target instanceof Element && !!target.closest(".xterm");
}
