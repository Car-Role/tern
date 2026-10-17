import { describe, expect, it } from "vitest";
import { looksLikeWrite, openQuery, quoteIdent, redisArg, schemaText, tableName, toCsv, toJson, toMarkdown, toTsv } from "./dbtext";
import type { DbResultSet, DbTable } from "./types";

const t = (schema: string, name: string, kind = "table"): DbTable => ({ schema, name, kind, columns: [] });

describe("database text", () => {
  it("quotes names only when needed, in each dialect", () => {
    expect(quoteIdent("postgres", "users")).toBe("users");
    expect(quoteIdent("postgres", "Users")).toBe('"Users"');
    expect(quoteIdent("sqlite", "order items")).toBe('"order items"');
    expect(quoteIdent("mssql", "weird]name")).toBe("[weird]]name]");
    expect(quoteIdent("sqlite", 'a"b')).toBe('"a""b"');
  });

  it("leaves out the default schema", () => {
    expect(tableName("postgres", t("public", "users"))).toBe("users");
    expect(tableName("postgres", t("billing", "invoices"))).toBe("billing.invoices");
    expect(tableName("mssql", t("dbo", "Orders"))).toBe("Orders");
    expect(tableName("mssql", t("sales", "Order Lines"))).toBe("sales.[Order Lines]");
    expect(tableName("sqlite", t("main", "x"))).toBe("x");
  });

  it("writes starter queries", () => {
    expect(openQuery("mssql", t("dbo", "Orders"))).toBe("SELECT TOP 100 * FROM Orders;");
    expect(openQuery("postgres", t("public", "users"))).toBe("SELECT * FROM users LIMIT 100;");
    expect(openQuery("redis", t("user", "user:1", "hash"))).toBe("HGETALL user:1");
    expect(openQuery("redis", t("", "my key", "zset"))).toBe('ZRANGE "my key" 0 999 WITHSCORES');
    expect(redisArg('say "hi"')).toBe('"say \\"hi\\""');
  });

  it("spots writes, ignoring strings and comments", () => {
    expect(looksLikeWrite("postgres", "select * from t")).toBe(false);
    expect(looksLikeWrite("postgres", "select 'delete' as word -- drop later")).toBe(false);
    expect(looksLikeWrite("postgres", "update t set a = 1")).toBe(true);
    expect(looksLikeWrite("mssql", "SELECT 1;\nDROP TABLE x")).toBe(true);
    expect(looksLikeWrite("redis", "GET a\nDEL b")).toBe(true);
    expect(looksLikeWrite("redis", "HGETALL h")).toBe(false);
  });

  const set: DbResultSet = { columns: ["id", "note"], rows: [[1, 'a, "b"'], [2, null], [3, "x|y\nz"]], more: 7, affected: null };

  it("copies results as CSV, TSV, JSON, and Markdown", () => {
    expect(toCsv(set).split("\n")[1]).toBe('1,"a, ""b"""');
    expect(toTsv(set).split("\n")[3]).toBe("3\tx|y z");
    expect(JSON.parse(toJson(set))[1]).toEqual({ id: 2, note: null });
    const md = toMarkdown(set, 2);
    expect(md).toContain("| id | note |");
    expect(md).toContain("(8 more rows not shown)");
    expect(toMarkdown(set)).toContain("x\\|y z");
  });

  it("writes the schema for an agent", () => {
    const s = { version: "PostgreSQL 16.4", truncated: false, tables: [{ schema: "public", name: "users", kind: "table", columns: [
      { name: "id", kind: "integer", nullable: false, key: true }, { name: "email", kind: "text", nullable: true, key: false }] }] };
    expect(schemaText("postgres", s)).toBe("PostgreSQL 16.4\nusers(id integer pk not null, email text)");
  });
});
