// Log lines: split growing text into lines, strip terminal colors, and tell errors and warnings apart.

export type Level = "error" | "warn" | "info" | "debug" | null;

// Terminal color and cursor codes (ESC [ ... letter), and OSC titles (ESC ] ... BEL).
const ANSI = /\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(\x07|\x1b\\)|\x1b[@-Z\\-_]/g;

export const stripAnsi = (s: string) => s.replace(ANSI, "");

export function levelOf(line: string): Level {
  // Word boundaries keep "terror" or "Errors: 0" style counters from matching too eagerly.
  if (/\b(fatal|panic|exception|traceback|error|err|failed|failure|crit(ical)?)\b|\bERR!/i.test(line) && !/\b0 errors?\b|\berrors?: 0\b/i.test(line)) return "error";
  if (/\b(warn|warning|deprecated)\b/i.test(line)) return "warn";
  if (/\b(debug|trace|verbose)\b/i.test(line)) return "debug";
  if (/\binfo\b/i.test(line)) return "info";
  return null;
}

/** Keeps the lines of a growing log. Text that does not end in a line break waits for the rest. */
export class LineBuffer {
  lines: string[] = [];
  private carry = "";
  /** Lines dropped from the top to stay under `max`. */
  dropped = 0;

  constructor(private max = 50_000) {}

  reset() {
    this.lines = [];
    this.carry = "";
    this.dropped = 0;
  }

  /** Adds text and returns how many full lines it made. */
  push(text: string): number {
    if (!text) return 0;
    const parts = (this.carry + text).split(/\r?\n/);
    this.carry = parts.pop() ?? "";
    const fresh = parts.map((l) => stripAnsi(l.replace(/\r/g, "")));
    for (const l of fresh) this.lines.push(l);
    if (this.lines.length > this.max) {
      const cut = this.lines.length - this.max;
      this.lines.splice(0, cut);
      this.dropped += cut;
    }
    return fresh.length;
  }

  /** The last line without its line break yet, shown at the end. */
  get pending() {
    return stripAnsi(this.carry);
  }
}

/** Lines that match a filter: plain text (case-insensitive) or /regex/flags. Bad regexes match as plain text. */
export function makeFilter(q: string): (line: string) => boolean {
  const t = q.trim();
  if (!t) return () => true;
  const re = /^\/(.+)\/([gimsuy]*)$/.exec(t);
  if (re) {
    try {
      const rx = new RegExp(re[1], re[2].replace("g", ""));
      return (l) => rx.test(l);
    } catch {
      // Not a valid regex: search for the text between the slashes.
      const low = re[1].toLowerCase();
      return (l) => l.toLowerCase().includes(low);
    }
  }
  const low = t.toLowerCase();
  return (l) => l.toLowerCase().includes(low);
}
