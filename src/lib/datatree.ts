// JSON, YAML, and TOML as one tree. Children are listed only when a node opens, so huge files stay fast.

export type DataKind = "json" | "yaml" | "toml";
export type Parsed = { ok: true; value: unknown; docs: number } | { ok: false; message: string; line: number | null; col: number | null };

/** Comments and trailing commas, as in tsconfig.json and .jsonc files. */
function stripJsonComments(text: string): string {
  let out = "";
  let inStr = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inStr) {
      out += c;
      if (c === "\\") out += text[++i] ?? "";
      else if (c === '"') inStr = false;
    } else if (c === '"') {
      inStr = true;
      out += c;
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      const chunk = text.slice(i, end < 0 ? text.length : end + 2);
      out += chunk.replace(/[^\n]/g, " "); // keep line numbers
      i = end < 0 ? text.length : end + 1;
    } else out += c;
  }
  return out.replace(/,(\s*[}\]])/g, "$1");
}

/** The position of the first JSON syntax error, or -1. Some engines do not say where JSON.parse failed. */
export function jsonErrorAt(text: string): number {
  let i = 0;
  const ws = () => {
    while (i < text.length && " \t\r\n".includes(text[i])) i++;
  };
  const fail = () => {
    throw i;
  };
  const value = (): void => {
    ws();
    const c = text[i];
    if (c === "{") {
      i++;
      ws();
      if (text[i] === "}") return void i++;
      for (;;) {
        ws();
        if (text[i] !== '"') fail();
        str();
        ws();
        if (text[i] !== ":") fail();
        i++;
        value();
        ws();
        if (text[i] === ",") i++;
        else if (text[i] === "}") return void i++;
        else fail();
      }
    } else if (c === "[") {
      i++;
      ws();
      if (text[i] === "]") return void i++;
      for (;;) {
        value();
        ws();
        if (text[i] === ",") i++;
        else if (text[i] === "]") return void i++;
        else fail();
      }
    } else if (c === '"') str();
    else if (/[-0-9]/.test(c ?? "")) {
      const m = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?/.exec(text.slice(i, i + 400));
      if (!m) fail();
      i += m![0].length;
    } else {
      for (const lit of ["true", "false", "null"]) if (text.startsWith(lit, i)) return void (i += lit.length);
      fail();
    }
  };
  const str = () => {
    i++;
    while (i < text.length && text[i] !== '"') {
      if (text[i] === "\\") i++;
      else if (text.charCodeAt(i) < 0x20) fail();
      i++;
    }
    if (i >= text.length) fail();
    i++;
  };
  try {
    value();
    ws();
    return i < text.length ? i : -1;
  } catch (pos) {
    return typeof pos === "number" ? pos : -1;
  }
}

function lineCol(text: string, pos: number) {
  const before = text.slice(0, pos);
  const line = before.split("\n").length;
  return { line, col: pos - before.lastIndexOf("\n") };
}

export async function parseData(text: string, kind: DataKind): Promise<Parsed> {
  const src = text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
  try {
    if (kind === "json") {
      try {
        return { ok: true, value: JSON.parse(src), docs: 1 };
      } catch {
        return { ok: true, value: JSON.parse(stripJsonComments(src)), docs: 1 };
      }
    }
    if (kind === "yaml") {
      const { parseAllDocuments } = await import("yaml");
      // merge: true reads "<<: *anchor" keys, which Docker Compose and CI files use.
      const docs = parseAllDocuments(src, { prettyErrors: false, merge: true });
      const list = Array.isArray(docs) ? docs : [docs];
      for (const d of list) {
        const err = d.errors?.[0];
        if (err) {
          const pos = err.linePos?.[0];
          return { ok: false, message: err.message.split("\n")[0], line: pos?.line ?? null, col: pos?.col ?? null };
        }
      }
      // maxAliasCount stops "billion laughs" files from expanding forever.
      const values = list.map((d) => d.toJS({ maxAliasCount: 1000 }));
      return { ok: true, value: values.length === 1 ? values[0] : values, docs: values.length };
    }
    const { parse } = await import("smol-toml");
    return { ok: true, value: parse(src), docs: 1 };
  } catch (e) {
    const err = e as Error & { line?: number; column?: number };
    let line: number | null = err.line ?? null;
    let col: number | null = err.column ?? null;
    // JSON.parse says "at position N" (Chromium) or "line L column C" (others).
    const m = /position (\d+)/.exec(err.message);
    if (line == null && m && kind === "json") ({ line, col } = lineCol(src, Number(m[1])));
    const lc = /line (\d+) column (\d+)/.exec(err.message);
    if (line == null && lc) [line, col] = [Number(lc[1]), Number(lc[2])];
    if (line == null && kind === "json") {
      const at = jsonErrorAt(stripJsonComments(src));
      if (at >= 0) ({ line, col } = lineCol(src, at));
    }
    return { ok: false, message: (err.message || String(e)).split("\n")[0], line, col };
  }
}

export type NodeType = "object" | "array" | "string" | "number" | "boolean" | "null" | "date";

export function typeOf(v: unknown): NodeType {
  if (v === null || v === undefined) return "null";
  if (Array.isArray(v)) return "array";
  if (v instanceof Date) return "date";
  const t = typeof v;
  if (t === "object") return "object";
  if (t === "number" || t === "bigint") return "number";
  if (t === "boolean") return "boolean";
  return "string";
}

export const isBranch = (v: unknown) => typeOf(v) === "object" || typeOf(v) === "array";

export function childCount(v: unknown): number {
  if (Array.isArray(v)) return v.length;
  if (typeOf(v) === "object") return Object.keys(v as object).length;
  return 0;
}

/** The children of a node, from `start`, at most `limit`. */
export function children(v: unknown, start = 0, limit = 200): { key: string; value: unknown }[] {
  if (Array.isArray(v)) return v.slice(start, start + limit).map((value, i) => ({ key: String(start + i), value }));
  if (typeOf(v) === "object") return Object.keys(v as object).slice(start, start + limit).map((key) => ({ key, value: (v as Record<string, unknown>)[key] }));
  return [];
}

/** A short text for a value on one line. */
export function preview(v: unknown, max = 80): string {
  const t = typeOf(v);
  if (t === "string") {
    const s = JSON.stringify(v);
    return s.length > max ? s.slice(0, max - 1) + "…\"" : s;
  }
  if (t === "date") return (v as Date).toISOString();
  if (t === "array") return `[${(v as unknown[]).length}]`;
  if (t === "object") return `{${Object.keys(v as object).length}}`;
  return String(v);
}

/** A path an agent or jq can use: jobs.build.steps[2]["run name"] */
export function pathString(keys: { key: string; inArray: boolean }[]): string {
  let out = "";
  for (const k of keys) {
    if (k.inArray) out += `[${k.key}]`;
    else if (/^[A-Za-z_$][\w$-]*$/.test(k.key)) out += out ? `.${k.key}` : k.key;
    else out += `[${JSON.stringify(k.key)}]`;
  }
  return out;
}
