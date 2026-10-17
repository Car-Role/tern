//! SQLite: a file on disk. Opened read-only unless the query asks for writes; never created.

use super::{Column, Conn, QueryResult, ResultSet, Schema, Table};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::time::Duration;

fn open(conn: &Conn, write: bool) -> Result<Connection, String> {
    if conn.file.trim().is_empty() {
        return Err("Pick the database file first.".into());
    }
    if !std::path::Path::new(&conn.file).is_file() {
        return Err(format!("{} does not exist.", conn.file));
    }
    let mode = if write { OpenFlags::SQLITE_OPEN_READ_WRITE } else { OpenFlags::SQLITE_OPEN_READ_ONLY };
    let c = Connection::open_with_flags(&conn.file, mode | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI)
        .map_err(|e| format!("{}: {e}", conn.file))?;
    c.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    if !write {
        // Belt and braces: even a read-write file handle refuses writes with this on.
        c.pragma_update(None, "query_only", true).map_err(|e| e.to_string())?;
    }
    Ok(c)
}

pub fn version(conn: &Conn) -> Result<String, String> {
    let c = open(conn, false)?;
    let v: String = c.query_row("select sqlite_version()", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(format!("SQLite {v}"))
}

pub fn schema(conn: &Conn) -> Result<Schema, String> {
    let c = open(conn, false)?;
    let mut tables = Vec::new();
    let mut st = c
        .prepare("select name, type from sqlite_master where type in ('table','view') and name not like 'sqlite_%' order by type, name")
        .map_err(|e| e.to_string())?;
    let names: Vec<(String, String)> =
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
    for (name, kind) in names {
        let mut cols = Vec::new();
        if let Ok(mut p) = c.prepare("select name, type, \"notnull\", pk from pragma_table_info(?1)") {
            let rows = p.query_map([&name], |r| {
                Ok(Column { name: r.get(0)?, kind: r.get::<_, String>(1)?, nullable: r.get::<_, i64>(2)? == 0, key: r.get::<_, i64>(3)? > 0 })
            });
            if let Ok(rows) = rows {
                cols = rows.filter_map(Result::ok).collect();
            }
        }
        tables.push(Table { schema: "main".into(), name, kind, columns: cols });
    }
    Ok(Schema { version: version(conn)?, tables, truncated: false })
}

fn value(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) if f.is_finite() => json!(f),
        ValueRef::Real(f) => json!(f.to_string()),
        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => json!(format!("<{} bytes>", b.len())),
    }
}

pub fn query(conn: &Conn, sql: &str, write: bool) -> Result<QueryResult, String> {
    let c = open(conn, write)?;
    let mut sets = Vec::new();
    for stmt in split_statements(sql) {
        let mut st = c.prepare(&stmt).map_err(|e| err(&e, &stmt))?;
        let mut set = ResultSet::default();
        if st.column_count() == 0 {
            let n = st.execute([]).map_err(|e| err(&e, &stmt))?;
            set.affected = Some(n as u64);
        } else {
            set.columns = st.column_names().iter().map(|s| s.to_string()).collect();
            let width = set.columns.len();
            let mut rows = st.query([]).map_err(|e| err(&e, &stmt))?;
            while let Some(r) = rows.next().map_err(|e| err(&e, &stmt))? {
                set.push((0..width).map(|i| r.get_ref(i).map(value).unwrap_or(Value::Null)).collect());
            }
        }
        sets.push(set);
    }
    Ok(QueryResult { sets, ms: 0, note: None })
}

fn err(e: &rusqlite::Error, stmt: &str) -> String {
    let msg = e.to_string();
    if msg.contains("readonly") || msg.contains("query_only") {
        return "This connection is read-only, so the statement was refused.".into();
    }
    let short: String = stmt.trim().chars().take(80).collect();
    format!("{msg}\n  in: {short}")
}

/// Splits a script into statements at semicolons that are not inside quotes, comments, or a trigger body.
pub fn split_statements(sql: &str) -> Vec<String> {
    let b: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let mut depth = 0i32; // BEGIN ... END inside CREATE TRIGGER
    while i < b.len() {
        let c = b[i];
        match c {
            '\'' | '"' | '`' | '[' => {
                let close = if c == '[' { ']' } else { c };
                cur.push(c);
                i += 1;
                while i < b.len() {
                    cur.push(b[i]);
                    if b[i] == close {
                        // A doubled quote is an escaped quote.
                        if close != ']' && i + 1 < b.len() && b[i + 1] == close {
                            cur.push(b[i + 1]);
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
            }
            '-' if b.get(i + 1) == Some(&'-') => {
                while i < b.len() && b[i] != '\n' {
                    cur.push(b[i]);
                    i += 1;
                }
                continue;
            }
            '/' if b.get(i + 1) == Some(&'*') => {
                cur.push_str("/*");
                i += 2;
                while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                    cur.push(b[i]);
                    i += 1;
                }
                if i < b.len() {
                    cur.push_str("*/");
                    i += 1;
                }
            }
            ';' if depth == 0 => {
                if !is_blank(&cur) {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            c if c.is_alphabetic() => {
                let start = i;
                while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                    i += 1;
                }
                let word: String = b[start..i].iter().collect();
                cur.push_str(&word);
                let upper = word.to_uppercase();
                let in_trigger = cur.to_uppercase().contains("TRIGGER");
                if in_trigger && (upper == "BEGIN" || upper == "CASE") {
                    depth += 1;
                } else if in_trigger && upper == "END" && depth > 0 {
                    depth -= 1;
                }
                continue;
            }
            c => cur.push(c),
        }
        i += 1;
    }
    if !is_blank(&cur) {
        out.push(cur.trim().to_string());
    }
    out
}

/// Only spaces and comments.
fn is_blank(s: &str) -> bool {
    let mut rest = s.trim();
    loop {
        if rest.is_empty() {
            return true;
        }
        if let Some(r) = rest.strip_prefix("--") {
            rest = r.split_once('\n').map(|(_, t)| t).unwrap_or("").trim();
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.split_once("*/").map(|(_, t)| t).unwrap_or("").trim();
        } else {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Engine, MAX_ROWS};
    use super::*;

    fn db(name: &str) -> Conn {
        let p = std::env::temp_dir().join(format!("tern-sqlite-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let c = Connection::open(&p).unwrap();
        c.execute_batch(
            "create table people (id integer primary key, name text not null, score real, photo blob);
             insert into people (name, score, photo) values ('Ada', 9.5, x'0102'), ('Jo; Kim', null, null), ('O''Neil', 1e308 * 10, null);
             create view top as select name from people where score > 5;
             create trigger t after insert on people begin update people set score = 0 where id = new.id; end;",
        )
        .unwrap();
        Conn { engine: Engine::Sqlite, file: p.to_string_lossy().into_owned(), ..Default::default() }
    }

    #[test]
    fn splits_scripts() {
        assert_eq!(split_statements("select 1; select 2;"), vec!["select 1", "select 2"]);
        assert_eq!(split_statements("select ';' as a; -- c;\nselect \"x;y\""), vec!["select ';' as a", "-- c;\nselect \"x;y\""]);
        assert_eq!(split_statements("select 'it''s; ok'"), vec!["select 'it''s; ok'"]);
        assert_eq!(split_statements("/* a; b */ select 1"), vec!["/* a; b */ select 1"]);
        assert_eq!(split_statements("-- only a comment;\n"), Vec::<String>::new());
        let t = split_statements("create trigger t after insert on x begin update x set a = 1; delete from y; end; select 1");
        assert_eq!(t.len(), 2, "{t:?}");
    }

    #[test]
    fn reads_tables_and_refuses_writes() {
        let c = db("ro");
        let s = schema(&c).unwrap();
        assert!(s.version.starts_with("SQLite 3"));
        let people = s.tables.iter().find(|t| t.name == "people").unwrap();
        assert_eq!(people.kind, "table");
        assert!(people.columns.iter().any(|c| c.name == "id" && c.key));
        assert!(people.columns.iter().any(|c| c.name == "name" && !c.nullable));
        assert!(s.tables.iter().any(|t| t.name == "top" && t.kind == "view"));

        let r = query(&c, "select * from people order by id; select count(*) as n from people", false).unwrap();
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.sets[0].columns, vec!["id", "name", "score", "photo"]);
        assert_eq!(r.sets[0].rows[0][3], json!("<2 bytes>"));
        assert_eq!(r.sets[0].rows[1][1], json!("Jo; Kim"));
        assert_eq!(r.sets[0].rows[2][2], json!("inf"), "infinity is not valid JSON, so it becomes text");
        assert_eq!(r.sets[1].rows[0][0], json!(3));

        let e = query(&c, "delete from people", false).unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        // Turning query_only off does not help: the file itself is open read-only.
        assert!(query(&c, "pragma query_only = 0; delete from people", false).is_err());
        assert_eq!(query(&c, "select count(*) from people", false).unwrap().sets[0].rows[0][0], json!(3), "nothing was deleted");

        let w = query(&c, "insert into people (name) values ('New')", true).unwrap();
        assert_eq!(w.sets[0].affected, Some(1));
        let _ = std::fs::remove_file(&c.file);
    }

    #[test]
    fn caps_big_results_and_reports_errors() {
        let c = db("big");
        let r = query(&c, "with recursive n(i) as (select 1 union all select i + 1 from n where i < 5000) select i from n", false).unwrap();
        assert_eq!(r.sets[0].rows.len(), MAX_ROWS);
        assert_eq!(r.sets[0].more, 5000 - MAX_ROWS as u64);
        let e = query(&c, "select nope from people", false).unwrap_err();
        assert!(e.contains("no such column"), "{e}");
        assert!(query(&Conn { engine: Engine::Sqlite, file: "Z:/missing.db".into(), ..Default::default() }, "select 1", false).is_err());
        let _ = std::fs::remove_file(&c.file);
    }
}
