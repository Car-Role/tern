//! Tests against real servers. Each runs only when its variable is set, for example in CI with service containers:
//!   TERN_TEST_PG=localhost:5432:postgres:secret:postgres
//!   TERN_TEST_MSSQL=localhost:1433:sa:Secret_123:master
//!   TERN_TEST_REDIS=localhost:6379
//! Each test makes its own table or keys, named after the process, and removes them at the end.

use super::*;
use serde_json::{json, Value};

fn conn(var: &str, engine: Engine) -> Option<Conn> {
    let v = std::env::var(var).ok()?;
    let p: Vec<&str> = v.split(':').collect();
    Some(Conn {
        id: format!("test-{var}"),
        engine,
        host: p[0].into(),
        port: p.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
        user: p.get(2).unwrap_or(&"").to_string(),
        database: p.get(4).unwrap_or(&"").to_string(),
        tls: Tls::Off,
        allow_writes: true,
        ..Default::default()
    })
}

fn pass(var: &str) -> String {
    std::env::var(var).ok().and_then(|v| v.split(':').nth(3).map(String::from)).unwrap_or_default()
}

fn run<T>(f: impl std::future::Future<Output = T>) -> T {
    tauri::async_runtime::block_on(f)
}

/// The pool reads passwords from the keychain; tests pass them through `test`, which keeps no connection.
async fn q(pool: &Pool, c: &Conn, pw: &str, sql: &str, write: bool) -> Result<QueryResult, String> {
    let l = pool.get(c, write, Some(pw)).await?;
    drop(l);
    query(pool, c, sql, write).await
}

#[test]
fn postgres_live() {
    let Some(c) = conn("TERN_TEST_PG", Engine::Postgres) else { return };
    let pw = pass("TERN_TEST_PG");
    let pool = Pool::default();
    let t = format!("tern_test_{}", std::process::id());
    run(async {
        assert!(test(&pool, &c, Some(pw.clone())).await.unwrap().starts_with("PostgreSQL"));
        let w = q(&pool, &c, &pw, &format!(
            "DROP TABLE IF EXISTS {t}; CREATE TABLE {t} (id serial PRIMARY KEY, name text NOT NULL, n numeric(10,2), at timestamptz, j jsonb);
             INSERT INTO {t} (name, n, at, j) VALUES ('Ada', 1.50, '2026-10-17 10:00+00', '{{\"a\":1}}'), ('Zoë ☕', NULL, NULL, NULL)"), true).await.unwrap();
        assert_eq!(w.sets.last().unwrap().affected, Some(2));

        let s = schema(&pool, &c).await.unwrap();
        let tab = s.tables.iter().find(|x| x.name == t).expect("table in schema");
        assert!(tab.columns.iter().any(|c| c.name == "id" && c.key));
        assert!(tab.columns.iter().any(|c| c.name == "name" && !c.nullable));

        let r = q(&pool, &c, &pw, &format!("SELECT name, n, j FROM {t} ORDER BY id; SELECT 1 AS one"), false).await.unwrap();
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.sets[0].columns, vec!["name", "n", "j"]);
        assert_eq!(r.sets[0].rows[1][0], json!("Zoë ☕"));
        assert_eq!(r.sets[0].rows[1][1], Value::Null);
        assert_eq!(r.sets[0].rows[0][1], json!("1.50"));

        let e = q(&pool, &c, &pw, &format!("INSERT INTO {t} (name) VALUES ('nope')"), false).await.unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        let e = q(&pool, &c, &pw, "SELECT nope FROM nowhere", false).await.unwrap_err();
        assert!(e.contains("nowhere") && e.contains("character"), "{e}");

        let big = q(&pool, &c, &pw, "SELECT g FROM generate_series(1, 5000) g", false).await.unwrap();
        assert_eq!((big.sets[0].rows.len(), big.sets[0].more), (MAX_ROWS, 5000 - MAX_ROWS as u64));
        let empty = q(&pool, &c, &pw, &format!("SELECT * FROM {t} WHERE false"), false).await.unwrap();
        assert_eq!(empty.sets[0].columns.len(), 5, "an empty result still has its columns");

        q(&pool, &c, &pw, &format!("DROP TABLE {t}"), true).await.unwrap();
    });
}

#[test]
fn mssql_live() {
    let Some(c) = conn("TERN_TEST_MSSQL", Engine::Mssql) else { return };
    let pw = pass("TERN_TEST_MSSQL");
    let pool = Pool::default();
    let t = format!("tern_test_{}", std::process::id());
    run(async {
        let v = test(&pool, &c, Some(pw.clone())).await.unwrap();
        assert!(v.contains("SQL Server"), "{v}");
        q(&pool, &c, &pw, &format!(
            "IF OBJECT_ID('{t}') IS NOT NULL DROP TABLE {t};
             CREATE TABLE {t} (id int IDENTITY PRIMARY KEY, name nvarchar(50) NOT NULL, n decimal(10,2), at datetime2, g uniqueidentifier, ok bit);
             GO
             INSERT INTO {t} (name, n, at, g, ok) VALUES (N'Ada', 1.50, '2026-10-17T10:00:00', '6F9619FF-8B86-D011-B42D-00C04FC964FF', 1), (N'Zoë ☕', NULL, NULL, NULL, 0)"), true).await.unwrap();

        let s = schema(&pool, &c).await.unwrap();
        let tab = s.tables.iter().find(|x| x.name == t).expect("table in schema");
        assert!(tab.columns.iter().any(|c| c.name == "id" && c.key));

        let r = q(&pool, &c, &pw, &format!("SELECT name, n, at, g, ok FROM {t} ORDER BY id; SELECT COUNT(*) AS c FROM {t}"), false).await.unwrap();
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.sets[0].rows[1][0], json!("Zoë ☕"));
        assert_eq!(r.sets[0].rows[0][1], json!("1.50"));
        assert_eq!(r.sets[0].rows[0][2], json!("2026-10-17 10:00:00"));
        assert_eq!(r.sets[0].rows[0][3].as_str().map(str::to_lowercase), Some("6f9619ff-8b86-d011-b42d-00c04fc964ff".into()));
        assert_eq!(r.sets[0].rows[0][4], json!(true));
        assert!(r.note.is_some());

        // Read-only: the insert runs, then is rolled back.
        let ro = q(&pool, &c, &pw, &format!("INSERT INTO {t} (name) VALUES (N'gone'); SELECT COUNT(*) FROM {t}"), false).await.unwrap();
        assert_eq!(ro.sets[0].rows[0][0], json!(3), "inside the transaction");
        let after = q(&pool, &c, &pw, &format!("SELECT COUNT(*) FROM {t}"), false).await.unwrap();
        assert_eq!(after.sets[0].rows[0][0], json!(2), "rolled back");
        assert!(q(&pool, &c, &pw, &format!("INSERT INTO {t} (name) VALUES (N'x'); COMMIT"), false).await.unwrap_err().contains("COMMIT"));

        let e = q(&pool, &c, &pw, "SELECT nope FROM nowhere", false).await.unwrap_err();
        assert!(e.contains("nowhere"), "{e}");
        // The connection still works after an error.
        assert_eq!(q(&pool, &c, &pw, "SELECT 1", false).await.unwrap().sets[0].rows[0][0], json!(1));

        let big = q(&pool, &c, &pw, "SELECT TOP 5000 ROW_NUMBER() OVER (ORDER BY (SELECT 1)) AS n FROM sys.all_objects a CROSS JOIN sys.all_objects b", false).await.unwrap();
        assert_eq!((big.sets[0].rows.len(), big.sets[0].more), (MAX_ROWS, 5000 - MAX_ROWS as u64));

        q(&pool, &c, &pw, &format!("DROP TABLE {t}"), true).await.unwrap();
    });
}

#[test]
fn redis_live() {
    let Some(c) = conn("TERN_TEST_REDIS", Engine::Redis) else { return };
    let pool = Pool::default();
    let k = format!("tern:test:{}", std::process::id());
    run(async {
        assert!(test(&pool, &c, Some(String::new())).await.unwrap().starts_with("Redis"));
        q(&pool, &c, "", &format!("SET {k}:s \"hello world\"\nHSET {k}:h a 1 b 2\nZADD {k}:z 1.5 m1 2 m2\nRPUSH {k}:l x y z"), true).await.unwrap();

        let r = q(&pool, &c, "", &format!("GET {k}:s\nHGETALL {k}:h\nZRANGE {k}:z 0 -1 WITHSCORES\nLRANGE {k}:l 0 -1"), false).await.unwrap();
        assert_eq!(r.sets[0].rows[0][0], json!("hello world"));
        assert_eq!(r.sets[1].columns, vec!["field", "value"]);
        assert_eq!(r.sets[1].rows.len(), 2);
        assert_eq!(r.sets[2].columns, vec!["member", "score"]);
        assert_eq!(r.sets[3].rows.len(), 3);

        let e = q(&pool, &c, "", &format!("DEL {k}:s"), false).await.unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        assert_eq!(q(&pool, &c, "", &format!("EXISTS {k}:s"), false).await.unwrap().sets[0].rows[0][0], json!(1));

        let s = schema(&pool, &c).await.unwrap();
        assert!(s.tables.iter().any(|t| t.name == format!("{k}:h") && t.kind == "hash" && t.schema == "tern"));

        q(&pool, &c, "", &format!("DEL {k}:s {k}:h {k}:z {k}:l"), true).await.unwrap();
    });
}
