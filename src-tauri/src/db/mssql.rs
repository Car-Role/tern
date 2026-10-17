//! SQL Server and Azure SQL, through tiberius. Read-only batches run in a transaction that is always rolled back.

use super::{Conn, QueryResult, ResultSet, Schema, Tls};
use futures_util::TryStreamExt;
use serde_json::{json, Value};
use std::collections::HashSet;
use tiberius::{AuthMethod, ColumnData, Config, EncryptionLevel, FromSql, QueryItem, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

pub type Client = tiberius::Client<Compat<TcpStream>>;

fn config(conn: &Conn, password: &str, host: &str, port: u16) -> Config {
    let mut cfg = Config::new();
    // "server\instance" names an instance; the SQL Server Browser service gives its port.
    let (h, instance) = match host.split_once('\\') {
        Some((h, i)) => (h, Some(i)),
        None => (host, None),
    };
    cfg.host(if h.is_empty() || h == "." { "localhost" } else { h });
    if let Some(i) = instance {
        cfg.instance_name(i);
    } else {
        cfg.port(if port == 0 { 1433 } else { port });
    }
    if !conn.database.is_empty() {
        cfg.database(&conn.database);
    }
    cfg.application_name("Tern");
    if conn.user.is_empty() {
        #[cfg(windows)]
        cfg.authentication(AuthMethod::Integrated);
        #[cfg(not(windows))]
        cfg.authentication(AuthMethod::sql_server("", password));
    } else {
        cfg.authentication(AuthMethod::sql_server(&conn.user, password));
    }
    match conn.tls {
        Tls::Off => cfg.encryption(EncryptionLevel::NotSupported),
        // Only the login is encrypted, and local servers use self-signed certificates: do not check it.
        Tls::Prefer => {
            cfg.encryption(EncryptionLevel::Off);
            cfg.trust_cert();
        }
        Tls::Require => cfg.encryption(EncryptionLevel::Required),
        Tls::RequireNoVerify => {
            cfg.encryption(EncryptionLevel::Required);
            cfg.trust_cert();
        }
    }
    cfg
}

pub async fn connect(conn: &Conn, password: &str) -> Result<Client, String> {
    let mut host = conn.host.clone();
    let mut port = conn.port;
    // Azure SQL may send the client to another server once (a "routing" answer).
    for _ in 0..2 {
        let cfg = config(conn, password, &host, port);
        let tcp = if host.contains('\\') {
            TcpStream::connect_named(&cfg).await.map_err(|e| format!("{}: {e}", conn.host))?
        } else {
            TcpStream::connect(cfg.get_addr()).await.map_err(|e| format!("{}: {e}", conn.label()))?
        };
        tcp.set_nodelay(true).ok();
        match tiberius::Client::connect(cfg, tcp.compat_write()).await {
            Ok(c) => return Ok(c),
            Err(tiberius::error::Error::Routing { host: h, port: p }) => {
                host = h;
                port = p;
            }
            Err(e) => return Err(describe(e)),
        }
    }
    Err("SQL Server sent Tern to another server twice. Check the host name.".into())
}

fn describe(e: tiberius::error::Error) -> String {
    match e {
        tiberius::error::Error::Server(t) => {
            let mut s = t.message().to_string();
            if t.line() > 0 {
                s.push_str(&format!("\n(line {})", t.line()));
            }
            s
        }
        other => other.to_string(),
    }
}

pub async fn version(c: &mut Client) -> Result<String, String> {
    let row = c.simple_query("SELECT @@VERSION").await.map_err(describe)?.into_row().await.map_err(describe)?;
    let v: Option<&str> = row.as_ref().and_then(|r| r.get(0));
    Ok(v.unwrap_or("SQL Server").lines().next().unwrap_or("SQL Server").trim().to_string())
}

async fn text_rows(c: &mut Client, sql: &str) -> Result<Vec<Vec<String>>, String> {
    let sets = c.simple_query(sql).await.map_err(describe)?.into_first_result().await.map_err(describe)?;
    Ok(sets.into_iter().map(|r| r.into_iter().map(|d| text(&d)).collect()).collect())
}

pub async fn schema(c: &mut Client) -> Result<Schema, String> {
    let version = version(c).await?;
    let cols = text_rows(
        c,
        "SELECT TOP 50001 c.TABLE_SCHEMA, c.TABLE_NAME, t.TABLE_TYPE, c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE
         FROM INFORMATION_SCHEMA.COLUMNS c
         JOIN INFORMATION_SCHEMA.TABLES t ON t.TABLE_SCHEMA = c.TABLE_SCHEMA AND t.TABLE_NAME = c.TABLE_NAME
         ORDER BY c.TABLE_SCHEMA, c.TABLE_NAME, c.ORDINAL_POSITION",
    )
    .await?;
    let keys: HashSet<(String, String, String)> = text_rows(
        c,
        "SELECT k.TABLE_SCHEMA, k.TABLE_NAME, k.COLUMN_NAME
         FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc
         JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE k ON k.CONSTRAINT_NAME = tc.CONSTRAINT_NAME AND k.TABLE_SCHEMA = tc.TABLE_SCHEMA
         WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY'",
    )
    .await?
    .into_iter()
    .map(|r| (r[0].clone(), r[1].clone(), r[2].clone()))
    .collect();
    let truncated = cols.len() > 50_000;
    let tables = super::pg::group(cols.into_iter().take(50_000), &keys);
    Ok(Schema { version, tables, truncated })
}

/// Splits a script at "GO" lines, the way SQL Server Management Studio does.
pub fn batches(sql: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in sql.lines() {
        let t = line.trim();
        if t.eq_ignore_ascii_case("go") || (t.len() > 3 && t[..3].eq_ignore_ascii_case("go ") && t[3..].trim().parse::<u32>().is_ok()) {
            out.push(String::new());
        } else {
            let cur = out.last_mut().unwrap();
            cur.push_str(line);
            cur.push('\n');
        }
    }
    out.into_iter().filter(|b| !b.trim().is_empty()).collect()
}

/// True when the text has a COMMIT outside quotes and comments. Read-only mode refuses those, since a
/// COMMIT would end the transaction that is rolled back.
pub fn has_commit(sql: &str) -> bool {
    let words = super::sqlite::split_statements(sql).join(" ");
    let mut clean = String::new();
    let mut quote = false;
    let mut chars = words.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            quote = !quote;
            clean.push(' ');
        } else if c == '-' && chars.peek() == Some(&'-') && !quote {
            for n in chars.by_ref() {
                if n == '\n' {
                    break;
                }
            }
        } else if !quote {
            clean.push(c);
        }
    }
    clean.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w.eq_ignore_ascii_case("commit"))
}

pub async fn query(c: &mut Client, sql: &str, write: bool) -> Result<QueryResult, String> {
    if !write && has_commit(sql) {
        return Err("Read-only mode runs every batch in a transaction that is rolled back, so COMMIT is not allowed.".into());
    }
    if !write {
        c.simple_query("SET XACT_ABORT ON; BEGIN TRANSACTION").await.map_err(describe)?.into_results().await.map_err(describe)?;
    }
    let mut sets = Vec::new();
    let mut failed = None;
    'outer: for batch in batches(sql) {
        let mut stream = match c.simple_query(&batch).await {
            Ok(s) => s,
            Err(e) => {
                failed = Some(describe(e));
                break;
            }
        };
        let mut cur: Option<ResultSet> = None;
        loop {
            match stream.try_next().await {
                Ok(Some(QueryItem::Metadata(m))) => {
                    if let Some(set) = cur.take() {
                        sets.push(set);
                    }
                    cur = Some(ResultSet { columns: m.columns().iter().map(|c| c.name().to_string()).collect(), ..Default::default() });
                }
                Ok(Some(QueryItem::Row(r))) => {
                    let set = cur.get_or_insert_with(ResultSet::default);
                    set.push(r.into_iter().map(|d| value(&d)).collect());
                }
                Ok(None) => break,
                Err(e) => {
                    failed = Some(describe(e));
                    break 'outer;
                }
            }
        }
        if let Some(set) = cur {
            sets.push(set);
        }
    }
    let mut note = None;
    if !write {
        // Always undo, whatever the batch did. The connection may be broken after an error; then it is dropped.
        let _ = c.simple_query("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION").await.map(|s| s.into_results());
        note = Some("Read-only: the batch ran in a transaction that was rolled back.".to_string());
    }
    if let Some(e) = failed {
        return Err(e);
    }
    Ok(QueryResult { sets, ms: 0, note })
}

fn text(d: &ColumnData<'static>) -> String {
    match value(d) {
        Value::String(s) => s,
        Value::Null => String::new(),
        v => v.to_string(),
    }
}

fn value(d: &ColumnData<'static>) -> Value {
    fn opt<T: Into<Value>>(v: Option<T>) -> Value {
        v.map(Into::into).unwrap_or(Value::Null)
    }
    match d {
        ColumnData::U8(v) => opt(*v),
        ColumnData::I16(v) => opt(*v),
        ColumnData::I32(v) => opt(*v),
        ColumnData::I64(v) => opt(*v),
        ColumnData::F32(v) => v.map(|f| if f.is_finite() { json!(f) } else { json!(f.to_string()) }).unwrap_or(Value::Null),
        ColumnData::F64(v) => v.map(|f| if f.is_finite() { json!(f) } else { json!(f.to_string()) }).unwrap_or(Value::Null),
        ColumnData::Bit(v) => opt(*v),
        ColumnData::String(v) => opt(v.as_ref().map(|s| s.to_string())),
        ColumnData::Guid(v) => opt(v.map(|g| g.to_string())),
        ColumnData::Binary(v) => opt(v.as_ref().map(|b| format!("<{} bytes>", b.len()))),
        ColumnData::Numeric(v) => opt(v.map(|n| n.to_string())),
        ColumnData::Xml(v) => opt(v.as_ref().map(|x| x.to_string())),
        ColumnData::DateTime(_) | ColumnData::SmallDateTime(_) | ColumnData::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(d).ok().flatten().map(|t| json!(t.to_string())).unwrap_or(Value::Null)
        }
        ColumnData::Date(_) => chrono::NaiveDate::from_sql(d).ok().flatten().map(|t| json!(t.to_string())).unwrap_or(Value::Null),
        ColumnData::Time(_) => chrono::NaiveTime::from_sql(d).ok().flatten().map(|t| json!(t.to_string())).unwrap_or(Value::Null),
        ColumnData::DateTimeOffset(_) => {
            chrono::DateTime::<chrono::FixedOffset>::from_sql(d).ok().flatten().map(|t| json!(t.to_rfc3339())).unwrap_or(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_go_batches() {
        assert_eq!(batches("select 1\nGO\nselect 2\n  go  \n\nGO 2\nselect 3"), vec!["select 1\n", "select 2\n", "select 3\n"]);
        assert_eq!(batches("select 'GO' as go_away"), vec!["select 'GO' as go_away\n"]);
    }

    #[test]
    fn finds_commit_outside_strings() {
        assert!(has_commit("update t set a = 1; COMMIT"));
        assert!(has_commit("commit transaction"));
        assert!(!has_commit("select 'commit' as word"));
        assert!(!has_commit("select committed_at from t -- commit later"));
    }
}
