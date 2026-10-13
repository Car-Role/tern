// Turns task output (text with ANSI color codes) into lines of HTML.
// It handles colors, bold, carriage-return progress lines, and "erase line". It ignores cursor jumps
// between lines, so full-screen programs will look messy here. Use a real terminal for those.

type Style = { fg?: string; bg?: string; bold?: boolean; dim?: boolean; italic?: boolean; underline?: boolean };
type Cell = { ch: string; s: Style };

const BASE = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
const LOCAL_URL = /\bhttps?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\]|[a-z0-9-]+\.localhost)(?::\d+)?(?:\/[^\s'"`<>)\]]*)?/gi;

function color256(n: number): string {
  if (n < 8) return `var(--t-${BASE[n]})`;
  if (n < 16) return `var(--t-bright-${BASE[n - 8]})`;
  if (n < 232) {
    const i = n - 16;
    const v = (x: number) => (x === 0 ? 0 : 55 + x * 40);
    return `rgb(${v(Math.floor(i / 36))},${v(Math.floor(i / 6) % 6)},${v(i % 6)})`;
  }
  const g = 8 + (n - 232) * 10;
  return `rgb(${g},${g},${g})`;
}

const esc = (t: string) => t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

function css(s: Style): string {
  const out: string[] = [];
  if (s.fg) out.push(`color:${s.fg}`);
  if (s.bg) out.push(`background:${s.bg}`);
  if (s.bold) out.push("font-weight:700");
  if (s.dim) out.push("opacity:.6");
  if (s.italic) out.push("font-style:italic");
  if (s.underline) out.push("text-decoration:underline");
  return out.join(";");
}

function lineHtml(cells: Cell[]): string {
  let html = "";
  let i = 0;
  while (i < cells.length) {
    const s = cells[i].s;
    let text = "";
    while (i < cells.length && cells[i].s === s) text += cells[i++].ch;
    const style = css(s);
    html += style ? `<span style="${style}">${esc(text)}</span>` : esc(text);
  }
  return html;
}

export class Term {
  /** Finished lines, as HTML and as plain text. */
  private html: string[] = [];
  private text: string[] = [];
  private cur: Cell[] = [];
  private col = 0;
  private style: Style = {};
  private pending = "";
  /** Local addresses printed by the task, like http://localhost:5173. */
  urls: string[] = [];

  constructor(private maxLines = 5000) {}

  write(data: string) {
    data = this.pending + data;
    this.pending = "";
    let i = 0;
    while (i < data.length) {
      const c = data[i];
      if (c === "\x1b") {
        const end = this.escape(data, i);
        if (end < 0) {
          this.pending = data.slice(i); // the rest of this code comes in the next chunk
          return;
        }
        i = end;
        continue;
      }
      if (c === "\n") this.newline();
      else if (c === "\r") this.col = 0;
      else if (c === "\b") this.col = Math.max(0, this.col - 1);
      else if (c === "\t") {
        const n = 8 - (this.col % 8);
        for (let k = 0; k < n; k++) this.put(" ");
      } else if (c >= " " || c > "\x7f") this.put(c);
      i++;
    }
  }

  private put(ch: string) {
    const cell = { ch, s: this.style };
    while (this.cur.length < this.col) this.cur.push({ ch: " ", s: {} });
    this.cur[this.col++] = cell;
  }

  private newline() {
    const plain = this.cur.map((c) => c.ch).join("");
    this.html.push(lineHtml(this.cur));
    this.text.push(plain);
    this.findUrls(plain);
    this.cur = [];
    this.col = 0;
    if (this.html.length > this.maxLines) {
      const drop = this.html.length - this.maxLines;
      this.html.splice(0, drop);
      this.text.splice(0, drop);
    }
  }

  private findUrls(line: string) {
    for (const m of line.matchAll(LOCAL_URL)) {
      const url = m[0].replace(/[.,;:]+$/, "").replace("0.0.0.0", "localhost").replace(/\[::1?\]/, "localhost");
      if (!this.urls.includes(url)) this.urls.push(url);
    }
  }

  /** Reads one escape code starting at `i`. Returns the index after it, or -1 if it is cut off. */
  private escape(d: string, i: number): number {
    const next = d[i + 1];
    if (next === undefined) return -1;
    if (next === "[") {
      let j = i + 2;
      while (j < d.length && !(d.charCodeAt(j) >= 0x40 && d.charCodeAt(j) <= 0x7e)) j++;
      if (j >= d.length) return -1;
      this.csi(d.slice(i + 2, j), d[j]);
      return j + 1;
    }
    if (next === "]") {
      // Titles and links: skip to BEL or ESC \
      for (let j = i + 2; j < d.length; j++) {
        if (d[j] === "\x07") return j + 1;
        if (d[j] === "\x1b" && d[j + 1] === "\\") return j + 2;
      }
      return -1;
    }
    return i + 2;
  }

  private csi(params: string, cmd: string) {
    const nums = params.replace(/^[?>=]/, "").split(";").map((x) => (x === "" ? NaN : Number(x)));
    const n = Number.isNaN(nums[0]) ? 1 : nums[0];
    switch (cmd) {
      case "m":
        this.sgr(params === "" ? [0] : nums.map((x) => (Number.isNaN(x) ? 0 : x)));
        break;
      case "K": {
        const mode = Number.isNaN(nums[0]) ? 0 : nums[0];
        if (mode === 0) this.cur.length = Math.min(this.cur.length, this.col);
        else if (mode === 1) for (let k = 0; k < this.col && k < this.cur.length; k++) this.cur[k] = { ch: " ", s: {} };
        else this.cur = [];
        break;
      }
      case "G":
        this.col = Math.max(0, n - 1);
        break;
      case "C":
        this.col += n;
        break;
      case "D":
        this.col = Math.max(0, this.col - n);
        break;
      // Cursor jumps between lines and screen clears are ignored on purpose: output is kept as history.
    }
  }

  private sgr(p: number[]) {
    const s: Style = { ...this.style };
    for (let k = 0; k < p.length; k++) {
      const x = p[k];
      if (x === 0) {
        for (const key of Object.keys(s)) delete s[key as keyof Style];
      } else if (x === 1) s.bold = true;
      else if (x === 2) s.dim = true;
      else if (x === 3) s.italic = true;
      else if (x === 4) s.underline = true;
      else if (x === 22) s.bold = s.dim = false;
      else if (x === 23) s.italic = false;
      else if (x === 24) s.underline = false;
      else if (x >= 30 && x <= 37) s.fg = `var(--t-${BASE[x - 30]})`;
      else if (x >= 90 && x <= 97) s.fg = `var(--t-bright-${BASE[x - 90]})`;
      else if (x >= 40 && x <= 47) s.bg = `var(--t-${BASE[x - 40]})`;
      else if (x >= 100 && x <= 107) s.bg = `var(--t-bright-${BASE[x - 100]})`;
      else if (x === 39) delete s.fg;
      else if (x === 49) delete s.bg;
      else if (x === 38 || x === 48) {
        let value: string | undefined;
        if (p[k + 1] === 5) {
          value = color256(p[k + 2]);
          k += 2;
        } else if (p[k + 1] === 2) {
          value = `rgb(${p[k + 2]},${p[k + 3]},${p[k + 4]})`;
          k += 4;
        }
        if (value) {
          if (x === 38) s.fg = value;
          else s.bg = value;
        }
      }
    }
    this.style = s;
  }

  toHtml(): string {
    const last = this.cur.length ? lineHtml(this.cur) : "";
    return this.html.join("\n") + (this.html.length ? "\n" : "") + last;
  }

  /** Plain text of the last `count` lines, for pasting to an agent. */
  toText(count = Infinity): string {
    const all = this.cur.length ? [...this.text, this.cur.map((c) => c.ch).join("")] : this.text;
    return all.slice(Math.max(0, all.length - count)).map((l) => l.trimEnd()).join("\n");
  }

  get lineCount() {
    return this.html.length + (this.cur.length ? 1 : 0);
  }
}
