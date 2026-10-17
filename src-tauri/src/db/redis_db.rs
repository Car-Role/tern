//! Redis. A query is one command per line, like redis-cli. Read-only mode allows only commands that read.

use super::{split_args, Conn, QueryResult, ResultSet, Schema, Table, Tls};
use redis::aio::MultiplexedConnection;
use redis::Value as RV;
use serde_json::{json, Value};

/// Commands that never change data. Two-word commands are checked by both words.
const READ: &[&str] = &[
    "GET", "MGET", "STRLEN", "GETRANGE", "SUBSTR", "EXISTS", "TYPE", "TTL", "PTTL", "EXPIRETIME", "PEXPIRETIME", "KEYS", "SCAN",
    "RANDOMKEY", "DBSIZE", "HGET", "HGETALL", "HKEYS", "HVALS", "HLEN", "HMGET", "HSCAN", "HEXISTS", "HSTRLEN", "HRANDFIELD",
    "LRANGE", "LLEN", "LINDEX", "LPOS", "SMEMBERS", "SCARD", "SISMEMBER", "SMISMEMBER", "SSCAN", "SRANDMEMBER", "SINTER", "SUNION",
    "SDIFF", "SINTERCARD", "ZRANGE", "ZRANGEBYSCORE", "ZRANGEBYLEX", "ZREVRANGE", "ZREVRANGEBYSCORE", "ZREVRANGEBYLEX", "ZCARD",
    "ZSCORE", "ZMSCORE", "ZRANK", "ZREVRANK", "ZSCAN", "ZCOUNT", "ZLEXCOUNT", "ZRANDMEMBER", "XRANGE", "XREVRANGE", "XLEN", "XREAD",
    "XINFO", "XPENDING", "GETBIT", "BITCOUNT", "BITPOS", "PFCOUNT", "GEOPOS", "GEODIST", "GEOHASH", "GEOSEARCH", "INFO", "PING",
    "ECHO", "TIME", "LASTSAVE", "ROLE", "LOLWUT", "DUMP", "TOUCH", "OBJECT", "MEMORY", "SLOWLOG GET", "SLOWLOG LEN", "CONFIG GET",
    "CLIENT LIST", "CLIENT INFO", "CLIENT GETNAME", "CLIENT ID", "COMMAND", "JSON.GET", "JSON.MGET", "JSON.TYPE", "JSON.STRLEN",
    "JSON.OBJKEYS", "JSON.OBJLEN", "JSON.ARRLEN", "FT.SEARCH", "FT.INFO", "FT._LIST", "FT.AGGREGATE", "TS.GET", "TS.RANGE",
    "TS.MRANGE", "TS.INFO", "SELECT", "CLUSTER INFO", "CLUSTER NODES", "CLUSTER SLOTS", "LATENCY LATEST", "MODULE LIST",
];

pub fn is_read(args: &[String]) -> bool {
    let first = args.first().map(|s| s.to_uppercase()).unwrap_or_default();
    let two = format!("{first} {}", args.get(1).map(|s| s.to_uppercase()).unwrap_or_default());
    // CONFIG, CLIENT, SLOWLOG, and the like are listed only with the word that reads.
    READ.contains(&two.as_str()) || (READ.contains(&first.as_str()) && !READ.iter().any(|r| r.starts_with(&format!("{first} "))))
}

fn url(conn: &Conn, password: &str) -> String {
    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    let scheme = if matches!(conn.tls, Tls::Require | Tls::RequireNoVerify) { "rediss" } else { "redis" };
    let auth = match (conn.user.is_empty(), password.is_empty()) {
        (_, true) if conn.user.is_empty() => String::new(),
        (true, false) => format!(":{}@", enc(password)),
        (false, _) => format!("{}:{}@", enc(&conn.user), enc(password)),
        _ => String::new(),
    };
    let host = if conn.host.is_empty() { "localhost" } else { &conn.host };
    let port = if conn.port == 0 { 6379 } else { conn.port };
    let db: u32 = conn.database.trim().parse().unwrap_or(0);
    let insecure = if conn.tls == Tls::RequireNoVerify { "#insecure" } else { "" };
    format!("{scheme}://{auth}{host}:{port}/{db}{insecure}")
}

pub async fn connect(conn: &Conn, password: &str) -> Result<MultiplexedConnection, String> {
    let client = redis::Client::open(url(conn, password)).map_err(|e| e.to_string())?;
    client.get_multiplexed_async_connection().await.map_err(|e| e.to_string())
}

pub async fn version(c: &mut MultiplexedConnection) -> Result<String, String> {
    let info: String = redis::cmd("INFO").arg("server").query_async(c).await.map_err(|e| e.to_string())?;
    let v = info.lines().find_map(|l| l.strip_prefix("redis_version:")).unwrap_or("?").trim().to_string();
    Ok(format!("Redis {v}"))
}

/// Up to 2,000 keys, with their types, grouped by the part before the first ":".
pub async fn schema(c: &mut MultiplexedConnection) -> Result<Schema, String> {
    let version = version(c).await?;
    let mut keys: Vec<String> = Vec::new();
    let mut cursor: u64 = 0;
    loop {
        let (next, batch): (u64, Vec<String>) =
            redis::cmd("SCAN").arg(cursor).arg("COUNT").arg(500).query_async(c).await.map_err(|e| e.to_string())?;
        keys.extend(batch);
        cursor = next;
        if cursor == 0 || keys.len() >= 2000 {
            break;
        }
    }
    let truncated = cursor != 0;
    keys.truncate(2000);
    keys.sort();
    let mut pipe = redis::pipe();
    for k in &keys {
        pipe.cmd("TYPE").arg(k);
    }
    let types: Vec<String> = if keys.is_empty() { vec![] } else { pipe.query_async(c).await.map_err(|e| e.to_string())? };
    let tables = keys
        .into_iter()
        .zip(types)
        .map(|(k, t)| Table { schema: k.split_once(':').map(|(p, _)| p.to_string()).unwrap_or_default(), name: k, kind: t, columns: vec![] })
        .collect();
    Ok(Schema { version, tables, truncated })
}

fn text(b: &[u8]) -> Value {
    match std::str::from_utf8(b) {
        Ok(s) => json!(s),
        Err(_) => json!(format!("<{} bytes>", b.len())),
    }
}

/// One Redis reply as a JSON value (nested replies stay nested).
pub fn to_json(v: &RV) -> Value {
    match v {
        RV::Nil => Value::Null,
        RV::Int(i) => json!(i),
        RV::BulkString(b) => text(b),
        RV::SimpleString(s) => json!(s),
        RV::Okay => json!("OK"),
        RV::Double(f) if f.is_finite() => json!(f),
        RV::Boolean(b) => json!(b),
        RV::Array(a) | RV::Set(a) => Value::Array(a.iter().map(to_json).collect()),
        RV::Map(m) => Value::Array(m.iter().map(|(k, v)| json!([to_json(k), to_json(v)])).collect()),
        RV::VerbatimString { text, .. } => json!(text),
        other => json!(format!("{other:?}")),
    }
}

/// A reply as a table: pairs for HGETALL, WITHSCORES, and maps; one row per item for lists; one cell otherwise.
pub fn to_set(args: &[String], v: &RV) -> ResultSet {
    let cmd = args.first().map(|s| s.to_uppercase()).unwrap_or_default();
    let pairs = matches!(cmd.as_str(), "HGETALL" | "CONFIG") || args.iter().any(|a| a.eq_ignore_ascii_case("WITHSCORES"));
    let mut set = ResultSet::default();
    match v {
        RV::Map(m) => {
            set.columns = vec!["field".into(), "value".into()];
            for (k, v) in m {
                set.push(vec![to_json(k), to_json(v)]);
            }
        }
        RV::Array(a) | RV::Set(a) if pairs => {
            set.columns = if cmd.starts_with('Z') { vec!["member".into(), "score".into()] } else { vec!["field".into(), "value".into()] };
            for p in a.chunks(2) {
                set.push(vec![to_json(&p[0]), p.get(1).map(to_json).unwrap_or(Value::Null)]);
            }
        }
        RV::Array(a) | RV::Set(a) => {
            set.columns = vec!["#".into(), "value".into()];
            for (i, x) in a.iter().enumerate() {
                let j = to_json(x);
                set.push(vec![json!(i + 1), if j.is_array() { json!(j.to_string()) } else { j }]);
            }
        }
        RV::BulkString(b) if cmd == "INFO" => {
            set.columns = vec!["field".into(), "value".into()];
            for line in String::from_utf8_lossy(b).lines() {
                if let Some((k, val)) = line.split_once(':') {
                    set.push(vec![json!(k), json!(val.trim())]);
                }
            }
        }
        other => {
            set.columns = vec!["value".into()];
            set.push(vec![to_json(other)]);
        }
    }
    set
}

pub async fn query(c: &mut MultiplexedConnection, text: &str, write: bool) -> Result<QueryResult, String> {
    let mut sets = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//")) {
        let args = split_args(line)?;
        if args.is_empty() {
            continue;
        }
        if !write && !is_read(&args) {
            return Err(format!("{} changes data, and this connection is read-only.", args[0].to_uppercase()));
        }
        if args[0].eq_ignore_ascii_case("SELECT") {
            return Err("Pick the database number in the connection settings instead of SELECT.".into());
        }
        let mut cmd = redis::cmd(&args[0]);
        for a in &args[1..] {
            cmd.arg(a);
        }
        let v: RV = cmd.query_async(c).await.map_err(|e| format!("{line}\n{e}"))?;
        sets.push(to_set(&args, &v));
    }
    Ok(QueryResult { sets, ms: 0, note: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        split_args(s).unwrap()
    }

    #[test]
    fn allows_only_reads() {
        for ok in ["GET k", "hgetall h", "CONFIG GET maxmemory", "client list", "XINFO STREAM s", "scan 0 match a:*"] {
            assert!(is_read(&a(ok)), "{ok}");
        }
        for bad in ["SET k v", "DEL k", "FLUSHALL", "CONFIG SET maxmemory 1", "CLIENT KILL x", "EVAL x 0", "HSET h f v", "SLOWLOG RESET"] {
            assert!(!is_read(&a(bad)), "{bad}");
        }
    }

    #[test]
    fn builds_urls() {
        let mut c = Conn { host: "cache.example.com".into(), port: 6380, database: "2".into(), ..Default::default() };
        assert_eq!(url(&c, ""), "redis://cache.example.com:6380/2");
        assert_eq!(url(&c, "p@ss:w/rd"), "redis://:p%40ss%3Aw%2Frd@cache.example.com:6380/2");
        c.user = "app".into();
        c.tls = Tls::RequireNoVerify;
        assert_eq!(url(&c, "x"), "rediss://app:x@cache.example.com:6380/2#insecure");
    }

    #[test]
    fn shapes_replies_as_tables() {
        let h = to_set(&a("HGETALL h"), &RV::Array(vec![RV::BulkString(b"f".to_vec()), RV::BulkString(b"v".to_vec())]));
        assert_eq!(h.columns, vec!["field", "value"]);
        assert_eq!(h.rows, vec![vec![json!("f"), json!("v")]]);
        let z = to_set(&a("ZRANGE z 0 -1 WITHSCORES"), &RV::Array(vec![RV::BulkString(b"m".to_vec()), RV::BulkString(b"1.5".to_vec())]));
        assert_eq!(z.columns, vec!["member", "score"]);
        let l = to_set(&a("LRANGE l 0 -1"), &RV::Array(vec![RV::Int(1), RV::Nil, RV::Array(vec![RV::Int(2)])]));
        assert_eq!(l.rows[2][1], json!("[2]"));
        let s = to_set(&a("GET k"), &RV::BulkString(vec![0xff, 0xfe]));
        assert_eq!(s.rows[0][0], json!("<2 bytes>"));
    }
}
