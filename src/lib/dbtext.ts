// Text for the database tool: quoting names, starter queries, and copies of schemas and results.

import type { DbEngine, DbResultSet, DbSchema, DbTable } from "./types";

export const engineLabel: Record<DbEngine, string> = { sqlite: "SQLite", postgres: "Postgres", mssql: "SQL Server", redis: "Redis" };
export const defaultPort: Record<DbEngine, number> = { sqlite: 0, postgres: 5432, mssql: 1433, redis: 6379 };

/** A name, quoted only when it needs it. */
export function quoteIdent(engine: DbEngine, name: string): string {
  if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && !(engine === "postgres" && /[A-Z]/.test(name))) return name;
  if (engine === "mssql") return `[${name.replace(/]/g, "]]")}]`;
  return `"${name.replace(/"/g, '""')}"`;
}

export function tableName(engine: DbEngine, t: Pick<DbTable, "schema" | "name">): string {
  const skip = !t.schema || (engine === "sqlite" && t.schema === "main") || (engine === "postgres" && t.schema === "public") || (engine === "mssql" && t.schema === "dbo");
  return skip ? quoteIdent(engine, t.name) : `${quoteIdent(engine, t.schema)}.${quoteIdent(engine, t.name)}`;
}

/** Quotes a Redis argument when it has spaces or quotes. */
export function redisArg(s: string): string {
  return /^[^\s"']+$/.test(s) ? s : `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/** The query that opens a table (first 100 rows), or the command that shows a Redis key. */
export function openQuery(engine: DbEngine, t: DbTable): string {
  if (engine === "redis") {
    const k = redisArg(t.name);
    switch (t.kind) {
      case "hash": return `HGETALL ${k}`;
      case "list": return `LRANGE ${k} 0 999`;
      case "set": return `SMEMBERS ${k}`;
      case "zset": return `ZRANGE ${k} 0 999 WITHSCORES`;
      case "stream": return `XRANGE ${k} - + COUNT 1000`;
      case "ReJSON-RL": return `JSON.GET ${k}`;
      default: return `GET ${k}\nTTL ${k}`;
    }
  }
  const name = tableName(engine, t);
  return engine === "mssql" ? `SELECT TOP 100 * FROM ${name};` : `SELECT * FROM ${name} LIMIT 100;`;
}

/** True when SQL looks like it changes something. Used to warn, not to protect: the server does that. */
export function looksLikeWrite(engine: DbEngine, text: string): boolean {
  const clean = text.replace(/--[^\n]*|\/\*[\s\S]*?\*\/|'(?:[^']|'')*'|"(?:[^"]|"")*"/g, " ");
  if (engine === "redis") {
    return clean.split("\n").some((l) => /^\s*(SET|DEL|UNLINK|HSET|HDEL|LPUSH|RPUSH|LPOP|RPOP|SADD|SREM|ZADD|ZREM|EXPIRE|PERSIST|RENAME|FLUSHDB|FLUSHALL|INCR|DECR|INCRBY|APPEND|MSET|XADD|XDEL|COPY|MOVE|EVAL|CONFIG\s+SET)\b/i.test(l));
  }
  return /\b(insert|update|delete|merge|upsert|create|alter|drop|truncate|grant|revoke|replace|vacuum|attach|reindex|exec|execute|call)\b/i.test(clean);
}

function cell(v: unknown): string {
  if (v === null || v === undefined) return "";
  return typeof v === "string" ? v : JSON.stringify(v);
}

export function toTsv(set: DbResultSet): string {
  const clean = (s: string) => s.replace(/[\t\r\n]+/g, " ");
  return [set.columns.map(clean).join("\t"), ...set.rows.map((r) => r.map((v) => clean(cell(v))).join("\t"))].join("\n");
}

export function toCsv(set: DbResultSet): string {
  const q = (s: string) => (/[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  return [set.columns.map(q).join(","), ...set.rows.map((r) => r.map((v) => q(cell(v))).join(","))].join("\n");
}

export function toJson(set: DbResultSet): string {
  return JSON.stringify(set.rows.map((r) => Object.fromEntries(set.columns.map((c, i) => [c, r[i] ?? null]))), null, 2);
}

/** A Markdown table of the first `max` rows, for pasting to an agent. */
export function toMarkdown(set: DbResultSet, max = 50): string {
  const esc = (s: string) => s.replace(/\|/g, "\\|").replace(/[\r\n]+/g, " ");
  const head = `| ${set.columns.map(esc).join(" | ")} |\n|${set.columns.map(() => " --- ").join("|")}|`;
  const body = set.rows.slice(0, max).map((r) => `| ${set.columns.map((_, i) => esc(cell(r[i]))).join(" | ")} |`);
  const left = set.rows.length - Math.min(max, set.rows.length) + set.more;
  return [head, ...body, ...(left > 0 ? [`\n(${left.toLocaleString()} more rows not shown)`] : [])].join("\n");
}

/** The schema as short lines an agent can read: `table(col type pk, col type null, ...)`. */
export function schemaText(engine: DbEngine, s: DbSchema, only?: DbTable[]): string {
  const tables = only ?? s.tables;
  if (engine === "redis") {
    return [`${s.version}. Keys${s.truncated ? " (first 2,000)" : ""}:`, ...tables.map((t) => `${t.name} (${t.kind})`)].join("\n");
  }
  const lines = tables.map((t) => {
    const cols = t.columns.map((c) => `${c.name} ${c.kind}${c.key ? " pk" : ""}${c.nullable ? "" : " not null"}`).join(", ");
    return `${t.kind === "view" ? "view " : ""}${tableName(engine, t)}(${cols})`;
  });
  return [`${s.version}${s.truncated ? " (schema cut short: very many columns)" : ""}`, ...lines].join("\n");
}

/** Table and column names for the editor's completion, by table. */
export function completionSchema(engine: DbEngine, s: DbSchema): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const t of s.tables) out[tableName(engine, t)] = t.columns.map((c) => c.name);
  return out;
}
