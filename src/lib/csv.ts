// A CSV/TSV reader that follows RFC 4180: quoted fields, "" for a quote, and line breaks inside quotes.
// It never throws: broken input still gives rows, and `problems` says what was odd.

export type CsvResult = {
  rows: string[][];
  delimiter: string;
  /** The widest row. Shorter rows are padded on screen. */
  columns: number;
  /** For example an unclosed quote. Empty when the file is clean. */
  problems: string[];
};

const CANDIDATES = [",", ";", "\t", "|"];

/** Picks the delimiter that splits the first lines most evenly (quotes are skipped). */
export function detectDelimiter(text: string): string {
  const sample = text.slice(0, 64 * 1024);
  const lines: string[] = [];
  let cur = "";
  let quoted = false;
  for (let i = 0; i < sample.length && lines.length < 20; i++) {
    const c = sample[i];
    if (c === '"') quoted = !quoted;
    if (!quoted && (c === "\n" || c === "\r")) {
      if (cur.trim()) lines.push(cur);
      cur = "";
      if (c === "\r" && sample[i + 1] === "\n") i++;
    } else if (!quoted) cur += c;
  }
  if (cur.trim()) lines.push(cur);
  let best = ",";
  let bestScore = -1;
  for (const d of CANDIDATES) {
    const counts = lines.map((l) => l.split(d).length - 1);
    if (!counts.length || counts[0] === 0) continue;
    const same = counts.filter((n) => n === counts[0]).length;
    // Most lines agree on the count, and more columns is a tie-breaker.
    const score = same * 1000 + counts[0];
    if (score > bestScore) {
      bestScore = score;
      best = d;
    }
  }
  return best;
}

export function parseCsv(input: string, delimiter?: string, maxRows = Infinity): CsvResult {
  const text = input.charCodeAt(0) === 0xfeff ? input.slice(1) : input; // byte order mark
  const d = delimiter ?? detectDelimiter(text);
  const rows: string[][] = [];
  const problems: string[] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let fieldStarted = false;
  let columns = 0;
  const pushRow = () => {
    row.push(field);
    field = "";
    fieldStarted = false;
    // A line with nothing on it is skipped, not a row with one empty cell.
    if (!(row.length === 1 && row[0] === "")) {
      rows.push(row);
      if (row.length > columns) columns = row.length;
    }
    row = [];
  };
  const n = text.length;
  for (let i = 0; i < n; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else quoted = false;
      } else field += c;
      continue;
    }
    if (c === '"' && !fieldStarted) {
      quoted = true;
      fieldStarted = true;
    } else if (c === d) {
      row.push(field);
      field = "";
      fieldStarted = false;
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      pushRow();
      if (rows.length >= maxRows) break;
    } else {
      field += c;
      fieldStarted = true;
    }
  }
  if (quoted) problems.push("A quoted field is never closed, so the last cell holds the rest of the file.");
  if (field !== "" || row.length) pushRow();
  return { rows, delimiter: d, columns, problems };
}
