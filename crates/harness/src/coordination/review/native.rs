//! One pinned local reviewer; isolated task session, no memory/tool adapter.
use super::*;
use crate::runtime_pool::RuntimePool;
use std::path::Path;
pub struct NativeReviewer<'a> {
    pub root: &'a Path,
    pub runtime: &'a RuntimePool,
    pub contract: &'a ReviewContract,
    pub scope: &'a str,
}
impl NativeReviewer<'_> {
    fn session(&self, revision: u64) -> String {
        crate::context::session_key(
            self.scope,
            "reasoning_acceptance",
            &self.contract.reviewer.sha256,
            revision,
        )
    }
    async fn preflight(&self) -> Result<()> {
        let root = self.root.to_path_buf();
        let candidate = self.contract.reviewer.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let memory = gents_local_mcp::host_memory::Snapshot::capture(
                &gents_local_mcp::host_memory::NativeMemoryProbe,
            )?;
            ensure!(
                memory.effective_total()? >= candidate.minimum_memory_mib * 1024 * 1024,
                "review model exceeds current host/process floor"
            );
            crate::model_artifact::verify(
                &root,
                candidate
                    .model
                    .path
                    .as_deref()
                    .context("review model path missing")?,
                &candidate.sha256,
            )
            .map_err(anyhow::Error::msg)?;
            Ok(())
        })
        .await??;
        Ok(())
    }
}
#[async_trait]
impl ReviewerPort for NativeReviewer<'_> {
    async fn verify(&self) -> Result<()> {
        self.preflight().await
    }
    async fn complete(&self, packet: &Value, revision: u64, cancel: &AtomicBool) -> Result<Reply> {
        self.verify().await?;
        ensure!(
            !cancel.load(Ordering::Relaxed),
            "review cancelled before native inference"
        );
        let reply = self
            .runtime
            .complete_scoped(
                &self.contract.reviewer.model,
                &instruction(),
                &packet.to_string(),
                1024,
                cancel,
                &self.session(revision),
            )
            .await?;
        Ok(reply)
    }
    async fn cancel(&self, revision: u64) {
        self.runtime.stop_session(&self.session(revision)).await;
    }
}
