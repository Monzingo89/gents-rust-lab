//! Private bookkeeping IPC, deliberately absent from the MCP tool registry.
//! No credentials, network, code loading, reconnects or automatic effect replay.
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection, OpenFlags,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, Read, Write},
    time::{Duration, Instant},
};
const MAX_WIRE: usize = 8 * 1024 * 1024;
const MAX_SQL: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    request: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Open {
        path: String,
        uri: bool,
        timeout_ms: u64,
        autocommit: bool,
    },
    Execute {
        sql: String,
        params: Vec<Value>,
    },
    Script {
        sql: String,
    },
    ValidateDml {
        sql: String,
    },
    Commit,
    Rollback,
    Close,
}
struct State {
    db: Option<Connection>,
    autocommit: bool,
}
fn metadata() -> Value {
    json!({"protocol_version":1,"sqlite_version":rusqlite::version()})
}
fn sql_checked(sql: &str) -> Result<()> {
    ensure!(
        !sql.trim().is_empty() && sql.len() <= MAX_SQL,
        "invalid SQL length"
    );
    Ok(())
}
fn dml(mut sql: &str) -> bool {
    loop {
        sql = sql.trim_start();
        if let Some(rest) = sql.strip_prefix("--") {
            sql = rest.split_once('\n').map_or("", |(_, s)| s);
        } else if let Some(rest) = sql.strip_prefix("/*") {
            sql = rest.split_once("*/").map_or("", |(_, s)| s);
        } else {
            break;
        }
    }
    matches!(
        sql.split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str(),
        "INSERT" | "UPDATE" | "DELETE" | "REPLACE"
    )
}
fn parameter(value: Value) -> Result<SqlValue> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::Bool(v) => SqlValue::Integer(i64::from(v)),
        Value::Number(n) => {
            if let Some(n) = n.as_i64() {
                SqlValue::Integer(n)
            } else {
                ensure!(!n.is_u64(), "integer out of SQLite range");
                SqlValue::Real(
                    n.as_f64()
                        .filter(|f| f.is_finite())
                        .context("invalid number")?,
                )
            }
        }
        Value::String(s) => {
            ensure!(s.len() <= MAX_SQL, "parameter too large");
            SqlValue::Text(s)
        }
        Value::Object(map) if map.len() == 1 && map.contains_key("__blob_hex") => {
            let text = map["__blob_hex"].as_str().context("invalid blob")?;
            ensure!(
                text.len() <= 2 * MAX_SQL && text.len() % 2 == 0 && text.is_ascii(),
                "invalid blob size"
            );
            let data = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            SqlValue::Blob(data)
        }
        _ => bail!("unsupported SQLite parameter"),
    })
}
fn cell(value: ValueRef<'_>) -> Result<Value> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => json!(n),
        ValueRef::Real(n) => {
            ensure!(n.is_finite(), "non-finite SQLite result");
            json!(n)
        }
        ValueRef::Text(s) => {
            ensure!(s.len() <= MAX_SQL, "cell too large");
            json!(std::str::from_utf8(s)?)
        }
        ValueRef::Blob(b) => {
            ensure!(b.len() <= MAX_SQL, "blob too large");
            json!({"__blob_hex":b.iter().map(|v|format!("{v:02x}")).collect::<String>()})
        }
    })
}
impl State {
    fn apply(&mut self, op: Operation) -> Result<Value> {
        if let Operation::Open {
            path,
            uri,
            timeout_ms,
            autocommit,
        } = op
        {
            ensure!(
                self.db.is_none() && path.len() <= 8192 && timeout_ms <= 120_000,
                "invalid connection"
            );
            let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW;
            if uri {
                flags |= OpenFlags::SQLITE_OPEN_URI;
            }
            self.db = Some(Connection::open_with_flags(path, flags)?);
            self.db
                .as_ref()
                .unwrap()
                .busy_timeout(Duration::from_millis(timeout_ms))?;
            self.autocommit = autocommit;
            return Ok(metadata());
        }
        let db = self.db.as_mut().context("connection is not open")?;
        let started = Instant::now();
        db.progress_handler(
            1000,
            Some(move || started.elapsed() > Duration::from_secs(60)),
        )?;
        match op {
            Operation::Execute { sql, params } => {
                sql_checked(&sql)?;
                ensure!(params.len() <= 16_384, "parameter count exceeded");
                let params = params
                    .into_iter()
                    .map(parameter)
                    .collect::<Result<Vec<_>>>()?;
                // Match legacy Python transaction control for ordinary DML. The
                // client rejects alternate isolation modes rather than guessing.
                if !self.autocommit && db.is_autocommit() && dml(&sql) {
                    db.execute_batch("BEGIN DEFERRED")?;
                }
                let mut statement = db.prepare(&sql)?;
                ensure!(
                    statement.parameter_count() == params.len(),
                    "parameter count mismatch"
                );
                let columns = statement
                    .column_names()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                let mut rows = Vec::new();
                let mut bytes = 0usize;
                if columns.is_empty() {
                    statement.execute(params_from_iter(params))?;
                } else {
                    let mut cursor = statement.query(params_from_iter(params))?;
                    while let Some(row) = cursor.next()? {
                        ensure!(rows.len() < 100_000, "result row bound exceeded");
                        let values = (0..columns.len())
                            .map(|i| cell(row.get_ref(i)?))
                            .collect::<Result<Vec<_>>>()?;
                        bytes = bytes.saturating_add(serde_json::to_vec(&values)?.len());
                        ensure!(bytes < MAX_WIRE - 65_536, "result byte bound exceeded");
                        rows.push(values);
                    }
                }
                Ok(
                    json!({"columns":columns,"rows":rows,"rowcount":if dml(&sql){i64::try_from(db.changes())?}else{-1},"lastrowid":db.last_insert_rowid(),"in_transaction":!db.is_autocommit()}),
                )
            }
            Operation::ValidateDml { sql } => {
                sql_checked(&sql)?;
                ensure!(dml(&sql), "executemany requires DML");
                let statement = db.prepare(&sql)?;
                ensure!(
                    statement.column_count() == 0,
                    "executemany returning rows is unsupported"
                );
                Ok(json!({"parameters":statement.parameter_count()}))
            }
            Operation::Script { sql } => {
                sql_checked(&sql)?;
                if !db.is_autocommit() {
                    db.execute_batch("COMMIT")?;
                }
                db.execute_batch(&sql)?;
                Ok(json!({"in_transaction":!db.is_autocommit()}))
            }
            Operation::Commit | Operation::Rollback => {
                if !db.is_autocommit() {
                    db.execute_batch(if matches!(op, Operation::Commit) {
                        "COMMIT"
                    } else {
                        "ROLLBACK"
                    })?;
                }
                Ok(json!({"in_transaction":!db.is_autocommit()}))
            }
            Operation::Close => {
                self.db = None;
                Ok(json!({"closed":true}))
            }
            Operation::Open { .. } => unreachable!(),
        }
    }
}
fn safe_error(error: &anyhow::Error) -> Value {
    let (kind, code) = match error.downcast_ref::<rusqlite::Error>() {
        Some(rusqlite::Error::SqliteFailure(e, _)) => (
            if e.code == rusqlite::ErrorCode::ConstraintViolation {
                "IntegrityError"
            } else {
                "OperationalError"
            },
            Some(e.extended_code),
        ),
        Some(_) => ("DatabaseError", None),
        None => ("ProgrammingError", None),
    };
    json!({"kind":kind,"sqlite_errorcode":code,"message":"Owned SQLite operation failed; inspect the caller and code. SQL, parameters and database payload were not logged.","automatic_replay":false})
}
fn run() -> Result<()> {
    ensure!(
        rusqlite::version_number() >= 3_054_000,
        "patched SQLite runtime required"
    );
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("{}", metadata());
        return Ok(());
    }
    ensure!(
        std::env::args().len() == 1,
        "no database paths accepted as command arguments"
    );
    #[cfg(unix)]
    unsafe {
        libc::umask(0o077);
    }
    let mut state = State {
        db: None,
        autocommit: false,
    };
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut wire = Vec::new();
        let count = input
            .by_ref()
            .take((MAX_WIRE + 1) as u64)
            .read_until(b'\n', &mut wire)?;
        if count == 0 {
            break;
        }
        ensure!(
            count <= MAX_WIRE && wire.last() == Some(&b'\n'),
            "bounded JSON line required"
        );
        let request: Request = serde_json::from_slice(&wire)?;
        let closing = matches!(request.request, Operation::Close);
        let response = match state.apply(request.request) {
            Ok(value) => json!({"id":request.id,"ok":true,"value":value}),
            Err(error) => {
                json!({"id":request.id,"ok":false,"error":safe_error(&error),"in_transaction":state.db.as_ref().is_some_and(|db|!db.is_autocommit())})
            }
        };
        let wire = serde_json::to_vec(&response)?;
        ensure!(wire.len() < MAX_WIRE, "bounded response required");
        output.write_all(&wire)?;
        output.write_all(b"\n")?;
        output.flush()?;
        if closing {
            break;
        }
    }
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!("Owned SQLite IPC failed; no request or database data logged; do not replay uncertain writes.");
        std::process::exit(1);
    }
}
