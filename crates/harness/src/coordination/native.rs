//! Native local worker adapter. No vendor endpoints, model discovery/download,
//! shared company context, tool broker or implicit memory reads are available.
use super::*;
use crate::{provider::Model, runtime_pool::RuntimePool};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct NativeWorkers<'a> {
    root: PathBuf,
    runtime: &'a RuntimePool,
    models: BTreeMap<String, (Model, String)>,
    scope: String,
}
impl<'a> NativeWorkers<'a> {
    pub fn new(
        root: &Path,
        runtime: &'a RuntimePool,
        contract: &Contract,
        models: BTreeMap<String, Model>,
        scope: &str,
    ) -> Result<Self> {
        contract.validate()?;
        ensure!(
            !scope.is_empty() && scope.len() <= 200 && !scope.chars().any(char::is_control),
            "invalid native coordination scope"
        );
        ensure!(
            models.len() == contract.roles.len(),
            "native worker map must match frozen roster"
        );
        let mut bound = BTreeMap::new();
        for role in &contract.roles {
            let model = models.get(&role.actor).context("native worker missing")?;
            model.validate()?;
            ensure!(
                model.provider == "local"
                    && model.adapter.is_none()
                    && model
                        .path
                        .as_deref()
                        .is_some_and(crate::model_artifact::relative_path),
                "coordination requires owned native local GGUF workers"
            );
            bound.insert(
                role.actor.clone(),
                (model.clone(), role.model_sha256.clone()),
            );
        }
        Ok(Self {
            root: root.into(),
            runtime,
            models: bound,
            scope: scope.into(),
        })
    }
    fn session(&self, turn: &Turn, revision: u64) -> String {
        crate::context::session_key(&self.scope, "coordination", &turn.actor, revision)
    }
}
#[async_trait]
impl WorkerPort for NativeWorkers<'_> {
    async fn complete(
        &self,
        turn: &Turn,
        packet: &Value,
        revision: u64,
        cancel: &AtomicBool,
    ) -> Result<crate::provider::Reply> {
        let (model, pin) = self
            .models
            .get(&turn.actor)
            .context("native coordination actor missing")?;
        let root = self.root.clone();
        let path = model.path.clone().context("worker path missing")?;
        let pin = pin.clone();
        tokio::task::spawn_blocking(move || {
            crate::model_artifact::verify(&root, &path, &pin).map_err(anyhow::Error::msg)
        })
        .await??;
        ensure!(
            !cancel.load(Ordering::Relaxed),
            "coordination cancelled before native inference"
        );
        let instruction = if packet["contract_version"] == 2 && turn.purpose != Purpose::Route {
            format!("FACTORY_WORK_V2: {} Evidence must contain only exact IDs from evidence_references, or be empty. IDs refer to available input/messages, not verified facts. Never write narrative claims in evidence. Unknown or unsupported claims belong in unknowns.", instruction(turn.purpose).trim_start_matches("FACTORY_WORK: "))
        } else {
            instruction(turn.purpose).to_owned()
        };
        self.runtime
            .complete_scoped(
                model,
                &instruction,
                &packet.to_string(),
                1024,
                cancel,
                &self.session(turn, revision),
            )
            .await
    }
    async fn cancel(&self, turn: &Turn, revision: u64) {
        self.runtime
            .stop_session(&self.session(turn, revision))
            .await;
    }
}
