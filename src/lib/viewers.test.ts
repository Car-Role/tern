// Stress tests for the file viewers' parsers: normal files, big files with time limits, and broken input.
import { describe, expect, it } from "vitest";
import { detectDelimiter, parseCsv } from "./csv";
import { childCount, children, parseData, pathString, preview, typeOf } from "./datatree";
import { LineBuffer, levelOf, makeFilter, stripAnsi } from "./logs";
import { hasPage, isBinaryKind, richKind } from "./filekind";

const time = (fn: () => void) => {
  const t = performance.now();
  fn();
  return performance.now() - t;
};

describe("file kinds", () => {
  it("picks the view from the name", () => {
    expect(richKind("docs/a.PNG")).toBe("image");
    expect(richKind("logo.svg")).toBe("svg");
    expect(richKind("spec.pdf")).toBe("pdf");
    expect(richKind("tsconfig.json")).toBe("json");
    expect(richKind(".github/workflows/build.yml")).toBe("yaml");
    expect(richKind("Cargo.lock")).toBe("toml");
    expect(richKind("data/export.tsv")).toBe("tsv");
    expect(richKind("logs/app.log.1")).toBe("log");
    expect(richKind("logs/app.log.2026-10-17")).toBe("log");
    expect(richKind("README")).toBeNull();
    expect(richKind("src/app.ts")).toBeNull();
    expect(isBinaryKind("image") && isBinaryKind("pdf") && !isBinaryKind("svg")).toBe(true);
    expect(hasPage("csv") && !hasPage("image") && !hasPage(null)).toBe(true);
  });
});

describe("CSV", () => {
  it("reads quotes, doubled quotes, and line breaks inside quotes", () => {
    const r = parseCsv('name,note\n"Smith, Ann","said ""hi""\nthen left"\nBob,ok\n');
    expect(r.rows).toEqual([["name", "note"], ["Smith, Ann", 'said "hi"\nthen left'], ["Bob", "ok"]]);
    expect(r.problems).toEqual([]);
  });

  it("handles a byte order mark, CRLF, blank lines, and no final line break", () => {
    const r = parseCsv("﻿a,b\r\n1,2\r\n\r\n3,4");
    expect(r.rows).toEqual([["a", "b"], ["1", "2"], ["3", "4"]]);
  });

  it("finds semicolons, tabs, and pipes", () => {
    expect(detectDelimiter("a;b;c\n1;2;3\n")).toBe(";");
    expect(detectDelimiter("a\tb\n1\t2\n")).toBe("\t");
    expect(detectDelimiter("a|b|c\n1|2|3\n")).toBe("|");
    // A comma inside quotes must not win over the real delimiter.
    expect(detectDelimiter('a;b\n"1,5";2\n"3,5";4\n')).toBe(";");
  });

  it("keeps ragged rows and reports the widest", () => {
    const r = parseCsv("a,b,c\n1\n1,2,3,4,5\n");
    expect(r.columns).toBe(5);
    expect(r.rows[1]).toEqual(["1"]);
  });

  it("never throws on an unclosed quote", () => {
    const r = parseCsv('a,b\n"open,1\n2,3\n');
    expect(r.problems.length).toBe(1);
    expect(r.rows.length).toBe(2);
  });

  it("parses 200,000 rows fast", () => {
    const lines = ["id,name,email,amount,note"];
    for (let i = 0; i < 200_000; i++) lines.push(`${i},"Person ${i}",p${i}@example.com,${(i * 1.17).toFixed(2)},"line, with comma"`);
    const text = lines.join("\n");
    let rows = 0;
    const ms = time(() => (rows = parseCsv(text).rows.length));
    expect(rows).toBe(200_001);
    expect(ms).toBeLessThan(3000);
  });

  it("stops early when asked for a limited number of rows", () => {
    const text = Array.from({ length: 1000 }, (_, i) => `${i},x`).join("\n");
    expect(parseCsv(text, ",", 10).rows.length).toBe(10);
  });
});

describe("JSON, YAML, TOML", () => {
  it("parses JSON, and JSON with comments and trailing commas", async () => {
    const a = await parseData('{"a": [1, 2, {"b": null}]}', "json");
    expect(a.ok && typeOf(a.value) === "object").toBe(true);
    const b = await parseData('{\n  // a comment\n  "a": 1, /* block */\n  "b": [1,2,],\n}', "json");
    expect(b.ok && (b.value as { b: number[] }).b).toEqual([1, 2]);
  });

  it("points at the line of a JSON error", async () => {
    const r = await parseData('{\n  "a": 1,\n  "b": ,\n}', "json");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.line).toBe(3);
  });

  it("reads YAML with anchors and several documents", async () => {
    const r = await parseData("base: &b\n  x: 1\nchild:\n  <<: *b\n  y: 2\n---\nsecond: true\n", "yaml");
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.docs).toBe(2);
      expect((r.value as Record<string, unknown>[])[0]).toEqual({ base: { x: 1 }, child: { x: 1, y: 2 } });
    }
  });

  it("refuses a YAML alias bomb instead of hanging", async () => {
    const bomb = 'a: &a ["x","x","x","x","x","x","x","x","x"]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\nc: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\ne: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\nf: [*e,*e,*e,*e,*e,*e,*e,*e,*e]\n';
    const t = performance.now();
    const r = await parseData(bomb, "yaml");
    expect(performance.now() - t).toBeLessThan(2000);
    expect(r.ok).toBe(false);
  });

  it("reports YAML and TOML errors with a line", async () => {
    const y = await parseData("a: 1\n  b: 2\n c: [\n", "yaml");
    expect(y.ok).toBe(false);
    const t = await parseData('[a]\nx = 1\nx = 2\n', "toml");
    expect(t.ok).toBe(false);
  });

  it("reads TOML dates and tables", async () => {
    const r = await parseData('title = "x"\n[owner]\ndob = 1979-05-27T07:32:00Z\n[[items]]\nn = 1\n[[items]]\nn = 2\n', "toml");
    expect(r.ok).toBe(true);
    if (r.ok) {
      const v = r.value as { owner: { dob: unknown }; items: unknown[] };
      expect(v.items.length).toBe(2);
      expect(["date", "string"]).toContain(typeOf(v.owner.dob));
    }
  });

  it("handles a JSON file with 100,000 items and 2,000 levels of nesting", async () => {
    const big = JSON.stringify({ items: Array.from({ length: 100_000 }, (_, i) => ({ id: i, tags: ["a", "b"] })) });
    const t = performance.now();
    const r = await parseData(big, "json");
    expect(performance.now() - t).toBeLessThan(3000);
    expect(r.ok && childCount((r.value as { items: unknown[] }).items)).toBe(100_000);
    if (r.ok) expect(children((r.value as { items: unknown[] }).items, 0, 200).length).toBe(200);
    const deep = "[".repeat(2000) + "]".repeat(2000);
    expect((await parseData(deep, "json")).ok).toBe(true);
  });

  it("previews and paths", () => {
    expect(preview("x".repeat(200)).length).toBeLessThanOrEqual(81);
    expect(preview([1, 2, 3])).toBe("[3]");
    expect(pathString([{ key: "jobs", inArray: false }, { key: "build", inArray: false }, { key: "2", inArray: true }, { key: "run name", inArray: false }])).toBe('jobs.build[2]["run name"]');
  });
});

describe("logs", () => {
  it("holds back a line until its line break arrives", () => {
    const b = new LineBuffer();
    expect(b.push("first\nsec")).toBe(1);
    expect(b.pending).toBe("sec");
    expect(b.push("ond\r\nthird\n")).toBe(2);
    expect(b.lines).toEqual(["first", "second", "third"]);
  });

  it("strips terminal colors", () => {
    expect(stripAnsi("\x1b[31mred\x1b[0m and \x1b]0;title\x07plain")).toBe("red and plain");
  });

  it("tells errors and warnings apart, without false alarms", () => {
    expect(levelOf("2026-10-17 ERROR Something broke")).toBe("error");
    expect(levelOf("npm ERR! code 1")).toBe("error");
    expect(levelOf("Unhandled exception in worker")).toBe("error");
    expect(levelOf("WARN deprecated option")).toBe("warn");
    expect(levelOf("Build finished with 0 errors")).toBe(null);
    expect(levelOf("[debug] cache hit")).toBe("debug");
    expect(levelOf("terrorist")).toBe(null);
  });

  it("filters by text or /regex/", () => {
    expect(makeFilter("Timeout")("request TIMEOUT after 30s")).toBe(true);
    expect(makeFilter("/HTTP [45]\\d\\d/")("got HTTP 503 from upstream")).toBe(true);
    expect(makeFilter("/[unclosed/")("has [unclosed in it")).toBe(true);
  });

  it("keeps at most the newest lines of a huge log, fast", () => {
    const b = new LineBuffer(50_000);
    const chunk = Array.from({ length: 10_000 }, (_, i) => `2026-10-17 INFO request ${i} done`).join("\n") + "\n";
    const ms = time(() => {
      for (let i = 0; i < 30; i++) b.push(chunk);
    });
    expect(b.lines.length).toBe(50_000);
    expect(b.dropped).toBe(250_000);
    expect(ms).toBeLessThan(3000);
  });
});

import { migrateStorage } from "./migrate";

describe("saved settings migration", () => {
  it("moves scout.* to tern.* once, keeping newer values", () => {
    const m = new Map<string, string>();
    const store = {
      get length() { return m.size; },
      key: (i: number) => [...m.keys()][i] ?? null,
      getItem: (k: string) => m.get(k) ?? null,
      setItem: (k: string, v: string) => void m.set(k, v),
      removeItem: (k: string) => void m.delete(k),
      clear: () => m.clear(),
    } as Storage;
    store.setItem("scout.tab", "history");
    store.setItem("scout.railW", "300");
    store.setItem("tern.railW", "280");
    store.setItem("theme", "ink");
    expect(migrateStorage(store)).toBe(2);
    expect(store.getItem("tern.tab")).toBe("history");
    expect(store.getItem("tern.railW")).toBe("280");
    expect(store.getItem("scout.tab")).toBeNull();
    expect(store.getItem("theme")).toBe("ink");
    expect(migrateStorage(store)).toBe(0);
  });
});
