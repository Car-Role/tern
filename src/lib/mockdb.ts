// Fake database back end for the browser preview.

import type { DbConn, DbQueryResult, DbSchema } from "./types";

let conns: DbConn[] = [
  { id: "pg", name: "billing (local)", engine: "postgres", file: "", host: "localhost", port: 5432, database: "billing", user: "postgres", tls: "prefer", hasPassword: true, allowWrites: true, color: "#5fbf8b", entra: false },
  { id: "ms", name: "Orders prod replica", engine: "mssql", file: "", host: "sql-prod.example.com", port: 1433, database: "Orders", user: "reader", tls: "require", hasPassword: true, allowWrites: false, color: "#e5786d", entra: false },
  { id: "rd", name: "cache", engine: "redis", file: "", host: "localhost", port: 6379, database: "0", user: "", tls: "off", hasPassword: false, allowWrites: false, color: "", entra: false },
  { id: "lite", name: "app.db", engine: "sqlite", file: "C:\\Users\\you\\Code\\notes-app\\app.db", host: "", port: 0, database: "", user: "", tls: "off", hasPassword: false, allowWrites: false, color: "#6fa8ff", entra: false },
];

const col = (name: string, kind: string, key = false, nullable = !key) => ({ name, kind, key, nullable });

function schema(c: DbConn): DbSchema {
  if (c.engine === "redis") {
    const keys = [["session:a1", "string"], ["session:b2", "string"], ["user:1", "hash"], ["user:2", "hash"], ["queue:email", "list"], ["leaders", "zset"], ["tags", "set"], ["events", "stream"]];
    return { version: "Redis 7.4.1", truncated: false, tables: keys.map(([k, t]) => ({ schema: k.includes(":") ? k.split(":")[0] : "", name: k, kind: t, columns: [] })) };
  }
  const s = c.engine === "mssql" ? "dbo" : c.engine === "postgres" ? "public" : "main";
  const tables = [
    { schema: s, name: "customers", kind: "table", columns: [col("id", "integer", true), col("name", "text", false, false), col("email", "text"), col("created_at", "timestamp")] },
    { schema: s, name: "invoices", kind: "table", columns: [col("id", "integer", true), col("customer_id", "integer", false, false), col("total", "numeric(10,2)"), col("status", "text"), col("due", "date")] },
    { schema: s, name: "big_events", kind: "table", columns: [col("id", "bigint", true), col("kind", "text"), col("payload", "jsonb")] },
    { schema: s, name: "open_invoices", kind: "view", columns: [col("id", "integer"), col("total", "numeric(10,2)")] },
  ];
  if (c.engine === "postgres") tables.push({ schema: "audit", name: "log", kind: "table", columns: [col("at", "timestamptz"), col("who", "text"), col("what", "text")] });
  const v = c.engine === "postgres" ? "PostgreSQL 16.4" : c.engine === "mssql" ? "Microsoft SQL Server 2022 (RTM-CU14) - 16.0.4135.4 (X64)" : "SQLite 3.46.0";
  return { version: v, truncated: false, tables };
}

const names = ["Ada Park", "Jo Kim", "Sam Lee", "Zoë Müller", "Ola Nordmann", "李雷"];

function query(c: DbConn, sql: string, write: boolean): DbQueryResult {
  const q = sql.trim();
  if (/\bnope\b|syntax/i.test(q)) throw new Error(c.engine === "postgres" ? 'column "nope" does not exist\n(at character 8)' : "Invalid column name 'nope'.\n(line 1)");
  if (c.engine === "redis") {
    const sets = q.split("\n").filter(Boolean).map((line) => {
      const [cmd, key] = line.trim().split(/\s+/);
      const u = cmd.toUpperCase();
      if (!write && /^(SET|DEL|HSET|FLUSHALL)$/.test(u)) throw new Error(`${u} changes data, and this connection is read-only.`);
      if (u === "HGETALL") return { columns: ["field", "value"], rows: [["name", "Ada"], ["email", "ada@example.com"], ["plan", "pro"]], more: 0, affected: null };
      if (u === "ZRANGE") return { columns: ["member", "score"], rows: names.map((n, i) => [n, String(100 - i * 7)]), more: 0, affected: null };
      if (u === "LRANGE" || u === "SMEMBERS" || u === "XRANGE") return { columns: ["#", "value"], rows: names.map((n, i) => [i + 1, n]), more: 0, affected: null };
      if (u === "TTL") return { columns: ["expires"], rows: [["in 1 h 0 min (3600 s)"]], more: 0, affected: null };
      return { columns: ["value"], rows: [[key ? `{"user":1,"key":"${key}"}` : "PONG"]], more: 0, affected: null };
    });
    return { sets, ms: 2, note: null };
  }
  if (!write && /^\s*(insert|update|delete|drop)/i.test(q)) {
    if (c.engine === "mssql") return { sets: [], ms: 12, note: "Read-only: the batch ran in a transaction that was rolled back." };
    throw new Error('This connection is read-only, so the statement was refused.');
  }
  if (/^\s*(insert|update|delete)/i.test(q)) return { sets: [{ columns: [], rows: [], more: 0, affected: 3 }], ms: 9, note: null };
  const sets = q.split(";").map((s) => s.trim()).filter(Boolean).map((s) => {
    if (/count\(\*\)/i.test(s)) return { columns: ["rows"], rows: [[/big/.test(s) ? 250000 : 42]], more: 0, affected: null };
    if (/big_events/i.test(s)) return { columns: ["id", "kind", "payload"], rows: Array.from({ length: 1000 }, (_, i) => [i + 1, ["click", "view", "buy"][i % 3], JSON.stringify({ page: `/p/${i}`, ms: i % 400 })]), more: 249000, affected: null };
    if (/invoices/i.test(s)) return { columns: ["id", "customer_id", "total", "status", "due"], rows: Array.from({ length: 24 }, (_, i) => [i + 1, (i % 6) + 1, ((i * 37.5) % 900).toFixed(2), i % 4 ? "paid" : "open", i % 5 ? `2026-1${i % 3}-0${(i % 9) + 1}` : null]), more: 0, affected: null };
    if (/where false|1\s*=\s*0/i.test(s)) return { columns: ["id", "name"], rows: [], more: 0, affected: null };
    return { columns: ["id", "name", "email", "created_at"], rows: names.map((n, i) => [i + 1, n, i === 2 ? null : `${n.split(" ")[0].toLowerCase()}@example.com`, `2026-0${i + 1}-1${i} 09:3${i}:00`]), more: 0, affected: null };
  });
  return { sets, ms: 14, note: c.engine === "mssql" && !write ? "Read-only: the batch ran in a transaction that was rolled back." : null };
}

export async function mockDb(cmd: string, args: Record<string, any>): Promise<unknown> {
  switch (cmd) {
    case "db_list": return conns;
    case "db_save": {
      const c: DbConn = { ...args.conn, id: args.conn.id || "db" + Date.now() };
      if (args.password !== null) c.hasPassword = args.password !== "";
      conns = conns.some((x) => x.id === c.id) ? conns.map((x) => (x.id === c.id ? c : x)) : [...conns, c];
      return c;
    }
    case "db_remove": conns = conns.filter((c) => c.id !== args.id); return null;
    case "db_disconnect": return null;
    case "db_test":
      await new Promise((r) => setTimeout(r, 500));
      if (args.conn.host === "bad") throw new Error("bad:5432: No such host is known. (os error 11001)");
      return schema(args.conn).version;
    case "db_schema":
      await new Promise((r) => setTimeout(r, 300));
      if (args.conn.host === "bad") throw new Error("Could not reach bad:5432 within 20 seconds.");
      return schema(args.conn);
    case "db_query":
      await new Promise((r) => setTimeout(r, /big/.test(args.sql) ? 900 : 150));
      return query(args.conn, args.sql, args.write);
  }
  return undefined;
}
