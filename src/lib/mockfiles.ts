// Test files for the browser preview: one or more of each kind of rich view, with big and broken ones.

export const qaFiles = [
  "samples/logo.png", "samples/screenshot.png", "samples/broken.png", "samples/huge.png", "samples/logo.svg",
  "samples/manual.pdf", "samples/fake.pdf",
  "samples/config.json", "samples/big.json", "samples/deep.json", "samples/bad.json", "samples/settings.jsonc",
  "samples/app.yaml", "samples/multi.yaml", "samples/bad.yaml", "samples/bomb.yaml", "samples/Cargo.toml", "samples/bad.toml",
  "samples/people.csv", "samples/big.csv", "samples/huge.csv", "samples/ragged.csv", "samples/semi.csv", "samples/data.tsv", "samples/empty.csv",
  "samples/server.log", "samples/rotate.log", "samples/quiet.log", "samples/app.db",
];

function picture(w: number, h: number, hue: number, label: string) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const g = c.getContext("2d")!;
  const grad = g.createLinearGradient(0, 0, w, h);
  grad.addColorStop(0, `hsl(${hue} 70% 55%)`);
  grad.addColorStop(1, `hsl(${hue + 60} 70% 35% / 0.4)`);
  g.fillStyle = grad;
  g.fillRect(0, 0, w, h);
  g.fillStyle = "#fff";
  g.font = `${Math.round(h / 8)}px sans-serif`;
  g.textAlign = "center";
  g.fillText(label, w / 2, h / 2);
  return c.toDataURL("image/png");
}

/** A small PDF with one page of text, written by hand. */
function pdf() {
  const objs = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    "",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  ];
  const stream = "BT /F1 28 Tf 72 700 Td (Tern test manual) Tj ET";
  objs[3] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  let out = "%PDF-1.4\n";
  const at: number[] = [];
  objs.forEach((o, i) => {
    at.push(out.length);
    out += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n${at.map((n) => String(n).padStart(10, "0") + " 00000 n \n").join("")}`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`;
  return "data:application/pdf;base64," + btoa(out);
}

const bytes = (dataUrl: string | null, mime: string, size?: number, tooLarge = false) => ({
  dataUrl, mime, size: size ?? (dataUrl ? Math.round((dataUrl.length - dataUrl.indexOf(",")) * 0.75) : 0), tooLarge,
});

/** `file_bytes`: pictures and PDFs. `rev` set means an old version (for the before/after view). */
export function qaBytes(rel: string, rev: string | null) {
  switch (rel) {
    case "samples/logo.png": return bytes(picture(320, 200, 200, "logo"), "image/png");
    case "samples/screenshot.png": return rev ? bytes(picture(640, 400, 20, "before"), "image/png") : bytes(picture(800, 400, 140, "after"), "image/png");
    case "samples/broken.png": return bytes("data:image/png;base64,bm90IGFuIGltYWdl", "image/png");
    case "samples/huge.png": return bytes(null, "image/png", 52_428_800, true);
    case "samples/logo.svg": return bytes("data:image/svg+xml;base64," + btoa(svgText), "image/svg+xml");
    case "samples/manual.pdf": return bytes(pdf(), "application/pdf");
    case "samples/fake.pdf": return bytes("data:text/plain;base64," + btoa("hello"), "text/plain");
    case "samples/new.png": return rev ? bytes(null, "image/png") : bytes(picture(300, 300, 280, "new"), "image/png");
  }
  return bytes(null, "application/octet-stream");
}

const svgText = '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200"><circle cx="100" cy="100" r="80" fill="#f0b54a"/><path d="M60 110 L100 70 L140 110" stroke="#1d2129" stroke-width="12" fill="none"/></svg>';

function bigCsv(n: number) {
  const out = ["id,name,email,amount,note"];
  for (let i = 1; i <= n; i++) out.push(`${i},Person ${i},p${i}@example.com,${(i * 3.17).toFixed(2)},"note, with comma ${i}"`);
  return out.join("\n");
}

function bigJson() {
  return JSON.stringify({ items: Array.from({ length: 100_000 }, (_, i) => ({ id: i, name: `item ${i}`, tags: ["a", "b"], ok: i % 2 === 0, price: i * 1.5, parent: null })) });
}

function deepJson() {
  let s = '"bottom"';
  for (let i = 0; i < 2000; i++) s = `{"level${i}":${s}}`;
  return s;
}

const texts: Record<string, () => string> = {
  "samples/logo.svg": () => svgText,
  "samples/config.json": () => JSON.stringify({
    name: "tern", version: "0.3.1", private: true,
    scripts: { dev: "vite", build: "vite build", test: "vitest run" },
    window: { width: 1280, height: 800, decorations: false, title: "Tern" },
    features: ["tickets", "ci", "terminal", "hunks"], empty: {}, none: [],
    long: "x".repeat(400), unicode: "café ☕ 日本語", nested: { a: { b: { c: { d: [1, 2, { e: "deep" }] } } } },
  }, null, 2),
  "samples/big.json": bigJson,
  "samples/deep.json": deepJson,
  "samples/bad.json": () => '{\n  "name": "tern",\n  "version": "0.3.1"\n  "missing": "a comma"\n}',
  "samples/settings.jsonc": () => '{\n  // editor settings\n  "tabSize": 2,\n  "theme": "dark", /* trailing comma next */\n  "rulers": [80, 120],\n}',
  "samples/app.yaml": () => "name: tern\nversion: 0.3.1\nbase: &base\n  retries: 3\n  timeout: 30\nprod:\n  <<: *base\n  timeout: 60\nhosts:\n  - a.example.com\n  - b.example.com\nwhen: 2026-10-17\nempty:\nmultiline: |\n  line one\n  line two\n",
  "samples/multi.yaml": () => "kind: Service\nname: api\n---\nkind: Deployment\nname: api\nreplicas: 3\n---\nkind: ConfigMap\ndata:\n  LOG: debug\n",
  "samples/bad.yaml": () => "name: tern\n  version: 0.3.1\n- list: wrong\n",
  "samples/bomb.yaml": () => 'a: &a ["x","x","x","x","x","x","x","x","x"]\nb: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\nc: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\nd: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\ne: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\nf: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]\ng: [*f,*f,*f,*f,*f,*f,*f,*f,*f]\n',
  "samples/Cargo.toml": () => '[package]\nname = "tern"\nversion = "0.3.1"\nedition = "2021"\n\n[dependencies]\nserde = { version = "1", features = ["derive"] }\ntauri = "2"\n\n[[bin]]\nname = "tern"\npath = "src/main.rs"\n\n[profile.release]\nlto = true\nreleased = 2026-10-17T10:00:00Z\n',
  "samples/bad.toml": () => '[package]\nname = "tern\nversion = 1\n',
  "samples/people.csv": () => 'name,role,"bio, short",start\nAda Park,Lead,"Writes ""quoted"" text",2024-01-02\nJo Kim,Dev,"Two\nlines",2025-03-04\nSam Lee,QA,,2026-05-06\n',
  "samples/big.csv": () => bigCsv(200_000),
  "samples/huge.csv": () => bigCsv(120_000),
  "samples/ragged.csv": () => 'a,b,c\n1,2\n3,4,5,6,7\n"unclosed,8,9\n',
  "samples/semi.csv": () => "Name;Price;Qty\nApple;1,20;3\nPear;0,95;10\n",
  "samples/data.tsv": () => "id\tcity\tpop\n1\tOslo\t709000\n2\tLima\t10000000\n3\tPerth, WA\t2100000\n",
  "samples/empty.csv": () => "",
};

/** `read_file`: text files go to the code view. Big ones come back "too large" so the rich view reads them itself. */
export function qaReadFile(rel: string) {
  if (rel === "samples/huge.csv") return { text: null, binary: false, tooLarge: true, size: 9_800_000 };
  if (rel.endsWith(".log")) return { text: logLines(0, 40).join("\n"), binary: false, tooLarge: false, size: 4000 };
  const t = texts[rel]?.();
  if (t === undefined) return null;
  return { text: t, binary: false, tooLarge: false, size: t.length };
}

export function qaReadText(rel: string) {
  const t = texts[rel]?.() ?? "";
  return { text: t, size: t.length, tooLarge: false };
}

const levels = ["INFO", "INFO", "DEBUG", "WARN", "INFO", "ERROR", "INFO"];
function logLines(from: number, n: number) {
  return Array.from({ length: n }, (_, i) => {
    const k = from + i;
    const lv = levels[k % levels.length];
    return `2026-10-17T11:${String(Math.floor(k / 60) % 60).padStart(2, "0")}:${String(k % 60).padStart(2, "0")} ${lv} \x1b[2mworker\x1b[0m request ${k} ${lv === "ERROR" ? "failed: connection reset" : "ok in " + (k % 97) + "ms"}`;
  });
}

const tails: Record<string, { lines: number; reads: number }> = {};

/** `read_tail`: server.log grows by a few lines each read; rotate.log starts over every 8 reads; quiet.log never changes. */
export function qaReadTail(rel: string, offset: number) {
  const s = (tails[rel] ??= { lines: rel === "samples/server.log" ? 1200 : 30, reads: 0 });
  s.reads++;
  if (rel === "samples/quiet.log" && offset > 0) return { text: "", offset, size: offset, reset: false, cut: false };
  let reset = offset === 0;
  if (rel === "samples/rotate.log" && s.reads % 8 === 0) {
    s.lines = 3;
    reset = true;
  }
  const before = reset ? 0 : s.lines;
  if (!reset) s.lines += 1 + (s.reads % 4);
  const text = logLines(before, s.lines - before).join("\n") + "\n";
  const size = s.lines * 80;
  return { text, offset: size, size, reset, cut: false };
}
