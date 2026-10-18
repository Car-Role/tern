//! The database tool: saved connections, schemas, and queries for SQLite, Postgres, SQL Server, and Redis.
//!
//! Every connection is read-only unless the query asks for writes. Each engine enforces that in its own way:
//! SQLite opens the file read-only, Postgres sets `default_transaction_read_only`, SQL Server runs the
//! batch in a transaction that is always rolled back, and Redis allows only commands that read.

#[cfg(test)]
mod live_tests;
mod entra;
mod mssql;
mod pg;
mod redis_db;
mod sqlite;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Most rows a query sends to the window. The rest are counted, not sent.
pub const MAX_ROWS: usize = 1000;
/// A query that runs longer than this is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(120);
const KEYRING_SERVICE: &str = "tern-db";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    #[default]
    Sqlite,
    Postgres,
    Mssql,
    Redis,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Tls {
    /// No encryption.
    Off,
    /// Encrypt when the server offers it (Postgres "prefer"). SQL Server and Redis treat this as Off.
    #[default]
    Prefer,
    /// Encrypt and check the certificate.
    Require,
    /// Encrypt, but accept any certificate (self-signed dev servers).
    RequireNoVerify,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Conn {
    pub id: String,
    pub name: String,
    pub engine: Engine,
    /// SQLite: the database file.
    pub file: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub tls: Tls,
    /// The password is in the system keychain, not in this file.
    pub has_password: bool,
    /// Lets queries write. Off by default; the window also asks before a write runs.
    pub allow_writes: bool,
    /// A color dot, to tell production from development at a glance.
    pub color: String,
    /// SQL Server: sign in with Microsoft Entra, using a token from the Azure CLI (`az login`).
    pub entra: bool,
}

impl Conn {
    pub fn label(&self) -> String {
        match self.engine {
            Engine::Sqlite => self.file.clone(),
            _ => format!("{}:{}", self.host, self.port),
        }
    }
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    pub kind: String,
    pub nullable: bool,
    pub key: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub schema: String,
    pub name: String,
    /// "table", "view", or for Redis the key type ("string", "hash", ...).
    pub kind: String,
    pub columns: Vec<Column>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub version: String,
    pub tables: Vec<Table>,
    /// True when there were more tables (or keys) than were read.
    pub truncated: bool,
}

/// One result set. A batch with several statements returns several.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Rows past MAX_ROWS that were not sent.
    pub more: u64,
    /// For statements that change rows.
    pub affected: Option<u64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub sets: Vec<ResultSet>,
    pub ms: u64,
    /// Something the window should say, such as "rolled back: read-only".
    pub note: Option<String>,
}

impl ResultSet {
    pub fn push(&mut self, row: Vec<serde_json::Value>) {
        if self.rows.len() < MAX_ROWS {
            self.rows.push(row);
        } else {
            self.more += 1;
        }
    }
}

// ---- Saved connections ----

fn store_path(config_dir: &Path) -> PathBuf {
    config_dir.join("databases.json")
}

pub fn list(config_dir: &Path) -> Vec<Conn> {
    fs::read_to_string(store_path(config_dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_all(config_dir: &Path, conns: &[Conn]) -> Result<(), String> {
    fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(conns).map_err(|e| e.to_string())?;
    fs::write(store_path(config_dir), text).map_err(|e| e.to_string())
}

/// Adds or replaces a connection. `password`: None keeps the saved one, Some("") removes it.
pub fn save(config_dir: &Path, mut conn: Conn, password: Option<String>) -> Result<Conn, String> {
    if conn.id.is_empty() {
        conn.id = format!("db{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    }
    if let Some(pw) = password {
        set_password(&conn.id, &pw)?;
        conn.has_password = !pw.is_empty();
    }
    let mut all = list(config_dir);
    match all.iter_mut().find(|c| c.id == conn.id) {
        Some(c) => *c = conn.clone(),
        None => all.push(conn.clone()),
    }
    write_all(config_dir, &all)?;
    Ok(conn)
}

pub fn remove(config_dir: &Path, id: &str) -> Result<(), String> {
    let all: Vec<Conn> = list(config_dir).into_iter().filter(|c| c.id != id).collect();
    let _ = set_password(id, "");
    write_all(config_dir, &all)
}

fn set_password(id: &str, pw: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, id).map_err(|e| format!("keychain: {e}"))?;
    if pw.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keychain: {e}")),
        }
    } else {
        entry.set_password(pw).map_err(|e| format!("keychain: {e}"))
    }
}

fn password(conn: &Conn, given: Option<&str>) -> Result<String, String> {
    if let Some(p) = given {
        return Ok(p.to_string());
    }
    if !conn.has_password {
        return Ok(String::new());
    }
    let entry = keyring::Entry::new(KEYRING_SERVICE, &conn.id).map_err(|e| format!("keychain: {e}"))?;
    match entry.get_password() {
        Ok(p) => Ok(p),
        Err(keyring::Error::NoEntry) => Err("The password is not in the keychain any more. Edit the connection and enter it again.".into()),
        Err(e) => Err(format!("keychain: {e}")),
    }
}

// ---- Open connections, kept for reuse ----

enum Live {
    Pg(tokio_postgres::Client),
    Ms(mssql::Client),
    Redis(::redis::aio::MultiplexedConnection),
}

#[derive(Default)]
pub struct Pool {
    open: Mutex<HashMap<String, Arc<Mutex<Live>>>>,
}

/// The key changes when the settings change, so an edited connection reconnects.
fn pool_key(conn: &Conn, write: bool) -> String {
    format!("{}|{write}|{}", conn.id, serde_json::to_string(conn).unwrap_or_default())
}

impl Pool {
    async fn get(&self, conn: &Conn, write: bool, pw: Option<&str>) -> Result<Arc<Mutex<Live>>, String> {
        let key = pool_key(conn, write);
        if let Some(l) = self.open.lock().await.get(&key) {
            return Ok(l.clone());
        }
        let pass = password(conn, pw)?;
        let live = tokio::time::timeout(Duration::from_secs(20), async {
            Ok::<_, String>(match conn.engine {
                Engine::Postgres => Live::Pg(pg::connect(conn, &pass, write).await?),
                Engine::Mssql => Live::Ms(mssql::connect(conn, &pass).await?),
                Engine::Redis => Live::Redis(redis_db::connect(conn, &pass).await?),
                Engine::Sqlite => return Err("SQLite does not use the pool".into()),
            })
        })
        .await
        .map_err(|_| format!("Could not reach {} within 20 seconds.", conn.label()))??;
        let l = Arc::new(Mutex::new(live));
        self.open.lock().await.insert(key, l.clone());
        Ok(l)
    }

    async fn drop_conn(&self, conn: &Conn) {
        self.open.lock().await.retain(|k, _| !k.starts_with(&format!("{}|", conn.id)));
    }

    /// Closes every open connection of this saved connection (after an edit, a delete, or a "Disconnect").
    pub async fn forget(&self, id: &str) {
        self.open.lock().await.retain(|k, _| !k.starts_with(&format!("{id}|")));
    }
}

/// A connection error that means "open a new connection and try again".
fn is_broken(e: &str) -> bool {
    let e = e.to_lowercase();
    ["closed", "broken pipe", "reset", "eof", "not connected", "connection refused", "forcibly"].iter().any(|w| e.contains(w))
}

pub async fn test(pool: &Pool, conn: &Conn, pw: Option<String>) -> Result<String, String> {
    if conn.engine == Engine::Sqlite {
        let c = conn.clone();
        return tauri::async_runtime::spawn_blocking(move || sqlite::version(&c)).await.map_err(|e| e.to_string())?;
    }
    pool.drop_conn(conn).await;
    let live = pool.get(conn, false, pw.as_deref()).await?;
    let mut l = live.lock().await;
    let v = match &mut *l {
        Live::Pg(c) => pg::version(c).await,
        Live::Ms(c) => mssql::version(c).await,
        Live::Redis(c) => redis_db::version(c).await,
    };
    drop(l);
    // A test with a typed password must not leave that connection behind.
    if pw.is_some() {
        pool.drop_conn(conn).await;
    }
    v
}

pub async fn schema(pool: &Pool, conn: &Conn) -> Result<Schema, String> {
    if conn.engine == Engine::Sqlite {
        let c = conn.clone();
        return tauri::async_runtime::spawn_blocking(move || sqlite::schema(&c)).await.map_err(|e| e.to_string())?;
    }
    for attempt in 0..2 {
        let live = pool.get(conn, false, None).await?;
        let mut l = live.lock().await;
        let r = tokio::time::timeout(TIMEOUT, async {
            match &mut *l {
                Live::Pg(c) => pg::schema(c).await,
                Live::Ms(c) => mssql::schema(c).await,
                Live::Redis(c) => redis_db::schema(c).await,
            }
        })
        .await
        .unwrap_or_else(|_| Err("Reading the schema took over 2 minutes.".into()));
        drop(l);
        match r {
            Err(e) if attempt == 0 && is_broken(&e) => pool.drop_conn(conn).await,
            other => return other,
        }
    }
    unreachable!()
}

pub async fn query(pool: &Pool, conn: &Conn, sql: &str, write: bool) -> Result<QueryResult, String> {
    if write && !conn.allow_writes {
        return Err("This connection is read-only. Edit it and turn on \"Allow writes\" first.".into());
    }
    let start = Instant::now();
    let mut out = if conn.engine == Engine::Sqlite {
        let (c, q) = (conn.clone(), sql.to_string());
        tauri::async_runtime::spawn_blocking(move || sqlite::query(&c, &q, write)).await.map_err(|e| e.to_string())??
    } else {
        let mut res = None;
        for attempt in 0..2 {
            let live = pool.get(conn, write, None).await?;
            let mut l = live.lock().await;
            let r = tokio::time::timeout(TIMEOUT, async {
                match &mut *l {
                    Live::Pg(c) => pg::query(c, sql).await,
                    Live::Ms(c) => mssql::query(c, sql, write).await,
                    Live::Redis(c) => redis_db::query(c, sql, write).await,
                }
            })
            .await;
            drop(l);
            let r = match r {
                Ok(r) => r,
                Err(_) => {
                    // The server may still be working; this connection is in an unknown state.
                    pool.drop_conn(conn).await;
                    Err("The query took over 2 minutes and was stopped.".into())
                }
            };
            match r {
                Err(e) if attempt == 0 && is_broken(&e) => pool.drop_conn(conn).await,
                other => {
                    res = Some(other);
                    break;
                }
            }
        }
        res.unwrap()?
    };
    out.ms = start.elapsed().as_millis() as u64;
    Ok(out)
}

/// Splits arguments like a shell: spaces separate, quotes group, backslash escapes inside double quotes.
pub fn split_args(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some('"'), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(match n {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                }
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            (None, c) => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("A quote is not closed.".into());
    }
    if in_word {
        out.push(cur);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_command_lines() {
        assert_eq!(split_args(r#"GET "a key" 'b c' d"#).unwrap(), vec!["GET", "a key", "b c", "d"]);
        assert_eq!(split_args(r#"SET k "line\nbreak \"q\"""#).unwrap(), vec!["SET", "k", "line\nbreak \"q\""]);
        assert_eq!(split_args(r#"GET """#).unwrap(), vec!["GET", ""]);
        assert!(split_args("GET \"open").is_err());
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn caps_rows() {
        let mut s = ResultSet::default();
        for i in 0..(MAX_ROWS + 5) {
            s.push(vec![serde_json::json!(i)]);
        }
        assert_eq!((s.rows.len(), s.more), (MAX_ROWS, 5));
    }

    #[test]
    fn saves_connections_without_passwords_in_the_file() {
        let d = std::env::temp_dir().join(format!("tern-db-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let c = save(&d, Conn { name: "local".into(), engine: Engine::Sqlite, file: "x.db".into(), ..Default::default() }, None).unwrap();
        assert!(!c.id.is_empty());
        let mut c2 = c.clone();
        c2.name = "renamed".into();
        save(&d, c2, None).unwrap();
        let all = list(&d);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "renamed");
        assert!(!fs::read_to_string(store_path(&d)).unwrap().contains("password\":\""));
        remove(&d, &c.id).unwrap();
        assert!(list(&d).is_empty());
        let _ = fs::remove_dir_all(&d);
    }
}
