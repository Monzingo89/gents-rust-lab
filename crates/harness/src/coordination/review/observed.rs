//! Native review telemetry. No inferred tokens, call replay or task acceptance.
use super::*;
use crate::App;
use std::sync::Mutex;
use uuid::Uuid;
pub struct ObservedReviewer<'a> {
    pub native: native::NativeReviewer<'a>,
    pub app: &'a App,
    pub run: Uuid,
    pub active: Mutex<Option<Uuid>>,
}
impl ObservedReviewer<'_> {
    async fn event(
        &self,
        call: Uuid,
        kind: &str,
        message: &str,
        reply: Option<Reply>,
    ) -> Result<()> {
        crate::coordination::company::inference_event(
            self.app,
            (self.run, call),
            ("Independent Reviewer", "reasoning_acceptance"),
            kind,
            message,
            Some(format!(
                "local / {}",
                self.native.contract.reviewer.model.model
            )),
            reply,
        )
        .await
    }
}
#[async_trait]
impl ReviewerPort for ObservedReviewer<'_> {
    async fn verify(&self) -> Result<()> {
        self.native.verify().await
    }
    async fn complete(&self, packet: &Value, revision: u64, cancel: &AtomicBool) -> Result<Reply> {
        let id = Uuid::new_v4();
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("review telemetry lock failed"))? = Some(id);
        self.event(id,"call_started",&json!({"packet_sha256":digest(packet)?,"policy_revision":revision,"tools_available":[]}).to_string(),None).await?;
        let result = self.native.complete(packet, revision, cancel).await;
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("review telemetry lock failed"))? = None;
        match result {
            Ok(reply) => {
                if crate::security::require_persistable(&reply.text).is_err() {
                    let safe = Reply {
                        text: "[Review content withheld by data policy]".into(),
                        ..reply
                    };
                    self.event(id,"call_failed","Review content rejected; native usage retained and pending review cannot replay",Some(safe)).await?;
                    anyhow::bail!("review content failed data policy");
                }
                self.event(id,"call_completed","Independent local reviewer replied; native and criterion admission still required",Some(reply.clone())).await?;
                Ok(reply)
            }
            Err(error) => {
                self.event(id,"call_failed","Review failed; unobserved usage remains unknown and pending review cannot replay",None).await?;
                Err(error)
            }
        }
    }
    async fn cancel(&self, revision: u64) {
        self.native.cancel(revision).await;
        let id = self.active.lock().ok().and_then(|mut v| v.take());
        if let Some(id) = id {
            let _ = self
                .event(
                    id,
                    "policy_epoch_cancelled",
                    "Owner policy changed; review stopped, pending receipt retained",
                    None,
                )
                .await;
        }
    }
}
