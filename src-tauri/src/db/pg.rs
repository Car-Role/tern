//! Postgres, through tokio-postgres. Results come back as text, the way psql shows them.

use super::{Column, Conn, QueryResult, ResultSet, Schema, Table, Tls};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio_postgres::config::SslMode;
use tokio_postgres::{Client, SimpleQueryMessage};

pub async fn connect(conn: &Conn, password: &str, write: bool) -> Result<Client, String> {
    let mut cfg = tokio_postgres::Config::new();
    cfg.host(if conn.host.is_empty() { "localhost" } else { &conn.host })
        .port(if conn.port == 0 { 5432 } else { conn.port })
        .user(if conn.user.is_empty() { "postgres" } else { &conn.user })
        .dbname(if conn.database.is_empty() { "postgres" } else { &conn.database })
        .application_name("Tern")
        .connect_timeout(Duration::from_secs(15))
        .ssl_mode(match conn.tls {
            Tls::Off => SslMode::Disable,
            Tls::Prefer => SslMode::Prefer,
            Tls::Require | Tls::RequireNoVerify => SslMode::Require,
        });
    if !password.is_empty() {
        cfg.password(password);
    }
    // "Prefer" also accepts any certificate, like libpq does: it is about privacy on the wire, not identity.
    let loose = matches!(conn.tls, Tls::Prefer | Tls::RequireNoVerify);
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(loose)
        .danger_accept_invalid_hostnames(loose)
        .build()
        .map_err(|e| e.to_string())?;
    let (client, connection) = cfg.connect(postgres_native_tls::MakeTlsConnector::new(tls)).await.map_err(|e| describe(&e))?;
    tauri::async_runtime::spawn(async move {
        let _ = connection.await;
    });
    if !write {
        client.batch_execute("SET default_transaction_read_only = on").await.map_err(|e| describe(&e))?;
    }
    Ok(client)
}

/// The server's message with its detail, hint, and position, when it gives them.
fn describe(e: &tokio_postgres::Error) -> String {
    let Some(db) = e.as_db_error() else {
        let mut s = e.to_string();
        let mut src = std::error::Error::source(e);
        while let Some(inner) = src {
            s.push_str(&format!(": {inner}"));
            src = inner.source();
        }
        return s;
    };
    let mut s = db.message().to_string();
    if db.code().code() == "25006" {
        s = "This connection is read-only, so the statement was refused.".into();
    }
    if let Some(d) = db.detail() {
        s.push_str(&format!("\n{d}"));
    }
    if let Some(h) = db.hint() {
        s.push_str(&format!("\nHint: {h}"));
    }
    if let Some(tokio_postgres::error::ErrorPosition::Original(p)) = db.position() {
        s.push_str(&format!("\n(at character {p})"));
    }
    s
}

pub async fn version(c: &mut Client) -> Result<String, String> {
    let rows = c.simple_query("SHOW server_version").await.map_err(|e| describe(&e))?;
    for m in rows {
        if let SimpleQueryMessage::Row(r) = m {
            return Ok(format!("PostgreSQL {}", r.get(0).unwrap_or("?")));
        }
    }
    Ok("PostgreSQL".into())
}

fn rows(msgs: Vec<SimpleQueryMessage>) -> Vec<Vec<String>> {
    msgs.into_iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(r) => Some((0..r.len()).map(|i| r.get(i).unwrap_or("").to_string()).collect()),
            _ => None,
        })
        .collect()
}

pub async fn schema(c: &mut Client) -> Result<Schema, String> {
    let version = version(c).await?;
    let cols = rows(
        c.simple_query(
            "SELECT c.table_schema, c.table_name, t.table_type, c.column_name, c.data_type, c.is_nullable
             FROM information_schema.columns c
             JOIN information_schema.tables t ON t.table_schema = c.table_schema AND t.table_name = c.table_name
             WHERE c.table_schema NOT IN ('pg_catalog', 'information_schema') AND c.table_schema NOT LIKE 'pg_toast%'
             ORDER BY c.table_schema, c.table_name, c.ordinal_position
             LIMIT 50001",
        )
        .await
        .map_err(|e| describe(&e))?,
    );
    let keys: HashSet<(String, String, String)> = rows(
        c.simple_query(
            "SELECT k.table_schema, k.table_name, k.column_name
             FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage k ON k.constraint_name = tc.constraint_name AND k.table_schema = tc.table_schema
             WHERE tc.constraint_type = 'PRIMARY KEY'",
        )
        .await
        .map_err(|e| describe(&e))?,
    )
    .into_iter()
    .map(|r| (r[0].clone(), r[1].clone(), r[2].clone()))
    .collect();
    let truncated = cols.len() > 50_000;
    Ok(Schema { version, tables: group(cols.into_iter().take(50_000), &keys), truncated })
}

/// "BASE TABLE", "VIEW", "FOREIGN" (a table that reads a file or another server) to a short kind.
fn kind(table_type: &str) -> &'static str {
    match table_type {
        t if t.contains("VIEW") => "view",
        "FOREIGN" => "foreign",
        _ => "table",
    }
}

/// Rows of (schema, table, table type, column, type, nullable) to tables, in order.
pub fn group(cols: impl Iterator<Item = Vec<String>>, keys: &HashSet<(String, String, String)>) -> Vec<Table> {
    let mut order = Vec::new();
    let mut by: HashMap<(String, String), Table> = HashMap::new();
    for r in cols {
        let k = (r[0].clone(), r[1].clone());
        let t = by.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            Table { schema: r[0].clone(), name: r[1].clone(), kind: kind(&r[2]).into(), columns: vec![] }
        });
        t.columns.push(Column {
            key: keys.contains(&(r[0].clone(), r[1].clone(), r[3].clone())),
            name: r[3].clone(),
            kind: r[4].clone(),
            nullable: r[5] == "YES",
        });
    }
    order.into_iter().filter_map(|k| by.remove(&k)).collect()
}

pub async fn query(c: &mut Client, sql: &str) -> Result<QueryResult, String> {
    let msgs = c.simple_query(sql).await.map_err(|e| describe(&e))?;
    let mut sets = Vec::new();
    let mut cur: Option<ResultSet> = None;
    for m in msgs {
        match m {
            SimpleQueryMessage::RowDescription(cols) => {
                cur = Some(ResultSet { columns: cols.iter().map(|c| c.name().to_string()).collect(), ..Default::default() });
            }
            SimpleQueryMessage::Row(r) => {
                let set = cur.get_or_insert_with(|| ResultSet { columns: r.columns().iter().map(|c| c.name().to_string()).collect(), ..Default::default() });
                set.push((0..r.len()).map(|i| r.get(i).map(|s| Value::String(s.to_string())).unwrap_or(Value::Null)).collect());
            }
            SimpleQueryMessage::CommandComplete(n) => match cur.take() {
                Some(set) => sets.push(set),
                None => sets.push(ResultSet { affected: Some(n), ..Default::default() }),
            },
            _ => {}
        }
    }
    if let Some(set) = cur {
        sets.push(set);
    }
    Ok(QueryResult { sets, ms: 0, note: None })
}
