//! Owner-frozen attempt ceilings. Crashes and failed preparation never refund
//! reservations; changing models or restarting cannot renew a run's allowance.
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use std::path::Path;
use uuid::Uuid;

/// A verified owner allowance was consumed. Unlike preparation/provider errors,
/// retrying cannot resolve this condition, and reservations are never refunded.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Exhausted {
    pub used: u64,
    pub maximum: u64,
}
impl std::fmt::Display for Exhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter,"Owner-frozen model-call budget exhausted ({}/{}); no provider was called. Preserve the run and inspect its evidence before any new task.",self.used,self.maximum)
    }
}
impl std::error::Error for Exhausted {}

fn limit(intake: &Value) -> Result<Option<u64>> {
    intake
        .get("max_model_calls")
        .map(|value| {
            value
                .as_u64()
                .filter(|n| (1..=1024).contains(n))
                .context("max_model_calls must be an owner-frozen integer in 1..1024")
        })
        .transpose()
}
pub fn validate(intake: &Value) -> Result<()> {
    limit(intake).map(|_| ())
}

pub fn reserve(
    root: &Path,
    company: &str,
    owner: &str,
    run: Uuid,
    call: Uuid,
    actor: (&str, &str),
) -> Result<Option<Value>> {
    let mut db = crate::company::connect(root, company, owner, false)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let frozen: Option<(String, String, Option<String>)> = tx
        .query_row(
            "SELECT intake,digest,approval FROM jobs WHERE run_id=?1",
            [run.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    // Pre-intake CEO conversations have no frozen job allowance. This function
    // cannot create one or turn its absence into task-execution permission.
    let Some((encoded, digest, approval)) = frozen else {
        return Ok(None);
    };
    let intake: Value = serde_json::from_str(&encoded)?;
    let Some(maximum) = limit(&intake)? else {
        return Ok(None);
    };
    let approved: Value =
        serde_json::from_str(&approval.context("Budgeted inference requires approved intake")?)?;
    ensure!(
        gents_local_mcp::digest(encoded.as_bytes()) == digest
            && approved["owner"] == owner
            && approved["sha256"] == digest
            && approved["type"] == "local_operator_approval",
        "Model-call allowance is not owner-approved"
    );
    tx.execute_batch("CREATE TABLE IF NOT EXISTS inference_attempt_reservations(run_id TEXT NOT NULL,call_id TEXT NOT NULL,actor TEXT NOT NULL,phase TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(run_id,call_id));")?;
    let prior: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM inference_attempt_reservations WHERE run_id=?1 AND call_id=?2",
            params![run.to_string(), call.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    ensure!(
        prior.is_none(),
        "Inference attempt was already reserved; inspect before retrying"
    );
    let used: i64 = tx.query_row(
        "SELECT COUNT(*) FROM inference_attempt_reservations WHERE run_id=?1",
        [run.to_string()],
        |row| row.get(0),
    )?;
    if used as u64 >= maximum {
        return Err(Exhausted {
            used: used as u64,
            maximum,
        }
        .into());
    }
    tx.execute(
        "INSERT INTO inference_attempt_reservations VALUES(?1,?2,?3,?4,?5)",
        params![
            run.to_string(),
            call.to_string(),
            actor.0,
            actor.1,
            crate::now_ms() as i64
        ],
    )?;
    tx.commit()?;
    Ok(Some(
        serde_json::json!({"reserved_attempt":used+1,"maximum":maximum,
        "call_id":call,"scope":"Owner-frozen run-wide reservation, including failed preparation and uncertain attempts. Not actual token usage, permission, provider execution or task acceptance."}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn malformed_or_model_created_limits_cannot_silently_become_unlimited() {
        assert!(validate(&json!({})).is_ok());
        for v in [
            json!(null),
            json!(true),
            json!(0),
            json!(-1),
            json!(1.5),
            json!("16"),
            json!(1025),
        ] {
            assert!(validate(&json!({"max_model_calls":v})).is_err());
        }
        assert_eq!(limit(&json!({"max_model_calls":16})).unwrap(), Some(16));
    }
    #[test]
    fn concurrent_attempts_and_restarts_share_the_frozen_ceiling() {
        let root = std::env::temp_dir().join(format!("gents-inference-budget-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let run = Uuid::new_v4();
        crate::company::operation(
            &root,
            "budget-test",
            "owner",
            &json!({"op":"register","name":"Budget test"}),
        )
        .unwrap();
        let frozen=crate::company::operation(&root,"budget-test","owner",&json!({"op":"intake","run_id":run,"intake":{"idea":"Synthetic reservation fixture","acceptance":"Two attempts maximum","exclusions":"External calls","timeline":"Test","company_signer":"Owner","provider_signer":"Local","budget_usd":0,"max_model_calls":2}})).unwrap();
        let digest = &frozen["jobs"][0]["digest"];
        assert!(reserve(
            &root,
            "budget-test",
            "owner",
            run,
            Uuid::new_v4(),
            ("Worker", "worker")
        )
        .is_err());
        crate::company::operation(
            &root,
            "budget-test",
            "owner",
            &json!({"op":"approve","run_id":run,"digest":digest}),
        )
        .unwrap();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || {
                    reserve(
                        &root,
                        "budget-test",
                        "owner",
                        run,
                        Uuid::new_v4(),
                        ("Worker", "worker"),
                    )
                    .is_ok()
                })
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .filter(|accepted| *accepted)
                .count(),
            2
        );
        let exhausted = reserve(
            &root,
            "budget-test",
            "owner",
            run,
            Uuid::new_v4(),
            ("Other model", "judge"),
        )
        .unwrap_err();
        let stop = exhausted.downcast_ref::<Exhausted>().unwrap();
        assert_eq!((stop.used, stop.maximum), (2, 2));
        assert!(reserve(
            &root,
            "budget-test",
            "other-owner",
            run,
            Uuid::new_v4(),
            ("Worker", "worker")
        )
        .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
