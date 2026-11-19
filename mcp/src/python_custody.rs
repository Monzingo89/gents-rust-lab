//! Current Python source custody from exact native arguments/results and audit
//! receipts. Byte coverage, not read counts, proves a complete paged readback.
use crate::{Phase, Scope};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;

fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS python_source_custody(run_id TEXT NOT NULL,actor TEXT NOT NULL,policy_revision INTEGER NOT NULL,path TEXT NOT NULL,source_sha256 TEXT NOT NULL,operation TEXT NOT NULL,offset_bytes INTEGER NOT NULL,arguments TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(run_id,actor,policy_revision,path,source_sha256,operation,offset_bytes));")?;
    Ok(())
}
pub(crate) fn capture(
    db: &Connection,
    scope: &Scope,
    name: &str,
    args: &Value,
    result: &Value,
    revision: u64,
) -> Result<()> {
    if name != "files" || scope.phase != Phase::Worker {
        return Ok(());
    }
    let Some(operation) = args["op"]
        .as_str()
        .filter(|op| matches!(*op, "read" | "write"))
    else {
        return Ok(());
    };
    let Some(path) = args["path"].as_str() else {
        return Ok(());
    };
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |row| row.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    if !crate::python_check::source_for_actor(&intake, &scope.actor, path)? {
        return Ok(());
    }
    let Some(hash) = result["sha256"].as_str() else {
        return Ok(());
    };
    let offset = if operation == "write" {
        0
    } else {
        let Some(offset) = result["offset_bytes"].as_u64() else {
            return Ok(());
        };
        offset
    };
    install(db)?;
    db.execute("INSERT INTO python_source_custody VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(run_id,actor,policy_revision,path,source_sha256,operation,offset_bytes) DO UPDATE SET arguments=excluded.arguments,result=excluded.result",
        params![scope.run.to_string(),scope.actor,revision as i64,path,hash,operation,offset as i64,args.to_string(),result.to_string()])?;
    Ok(())
}
pub(crate) fn check(
    db: &Connection,
    scope: &Scope,
    revision: u64,
    path: &str,
    content: &str,
) -> Result<Value> {
    install(db)?;
    let hash = crate::digest(content.as_bytes());
    let mut query=db.prepare("SELECT operation,arguments,result FROM python_source_custody WHERE run_id=?1 AND actor=?2 AND policy_revision=?3 AND path=?4 AND source_sha256=?5 ORDER BY offset_bytes LIMIT 256")?;
    let rows = query.query_map(
        params![
            scope.run.to_string(),
            scope.actor,
            revision as i64,
            path,
            hash
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )?;
    let mut written = false;
    let mut pages = vec![];
    for row in rows {
        let (operation, arguments, result) = row?;
        let (Ok(args), Ok(value)) = (
            serde_json::from_str::<Value>(&arguments),
            serde_json::from_str::<Value>(&result),
        ) else {
            continue;
        };
        if args["op"] != operation
            || args["path"] != path
            || value["path"] != path
            || value["sha256"] != hash
        {
            continue;
        }
        let receipts:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.tool')='files' AND json_extract(body,'$.phase')='worker' AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.policy_revision')=?3 AND json_extract(body,'$.arguments_sha256')=?4 AND json_extract(body,'$.result_sha256')=?5",
            params![scope.run.to_string(),scope.actor,revision as i64,crate::digest(arguments.as_bytes()),crate::digest(result.as_bytes())],|row|row.get(0))?;
        if receipts == 0 {
            continue;
        }
        if operation == "write" {
            written |= args["content"] == content && value["bytes"] == content.len();
            continue;
        }
        let (Some(start), Some(text), Some(truncated)) = (
            value["offset_bytes"].as_u64(),
            value["text"].as_str(),
            value["truncated"].as_bool(),
        ) else {
            continue;
        };
        let Some(end) = (start as usize).checked_add(text.len()) else {
            continue;
        };
        if args
            .get("offset_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            != start
            || value["total_bytes"] != content.len()
            || value["bytes"] != text.len()
            || content.get(start as usize..end) != Some(text)
            || (truncated && (end == start as usize || value["next_offset_bytes"] != end))
            || (!truncated && (end != content.len() || !value["next_offset_bytes"].is_null()))
        {
            continue;
        }
        pages.push((start as usize, end, truncated));
    }
    pages.sort_unstable();
    let mut offset = 0;
    let mut read_back = false;
    for (start, end, truncated) in pages {
        if start > offset {
            break;
        }
        if end < offset {
            continue;
        }
        if !truncated && end == content.len() {
            read_back = true;
            break;
        }
        offset = offset.max(end);
    }
    Ok(serde_json::json!({"written":written,"read_back":read_back,"source_sha256":hash}))
}
