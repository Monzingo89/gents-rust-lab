//! Capacity-limited adapter around the owned local inference backend. Existing
//! production configuration stays serial; an operator can opt into two small
//! model lanes. Training, vision helper operations and large models stay exclusive.
use crate::{
    local_runtime::LocalRuntime,
    provider::{Model, Reply},
};
use anyhow::{bail, Context, Result};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::sync::{Mutex, RwLock, Semaphore};

const GIB: u64 = 1024 * 1024 * 1024;
const SMALL_MODEL_MAX: u64 = 3_000_000_000;
const LANE_OVERHEAD: u64 = 2 * GIB;

struct Lane {
    runtime: LocalRuntime,
    busy: Mutex<()>,
}

pub struct RuntimePool {
    serial: LocalRuntime,
    lanes: Vec<Lane>,
    available: Semaphore,
    gate: RwLock<()>,
    // Serializes eviction transitions only, not inference. The fixed lane
    // count and conservative startup budget cap concurrent model residency.
    admission: Mutex<()>,
}

async fn cancellable<T>(future: impl Future<Output = T>, cancel: &AtomicBool) -> Result<T> {
    tokio::pin!(future);
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled while queued for local capacity");
        }
        tokio::select! {
            value = &mut future => return Ok(value),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

impl RuntimePool {
    #[cfg(test)]
    pub(crate) fn serial_at_port(root: PathBuf, port: u16) -> Result<Self> {
        Self::configured(root, port, 0)
    }
    pub fn new(root: PathBuf) -> Result<Self> {
        let lanes = std::env::var("GENTS_LOCAL_SMALL_LANES")
            .unwrap_or_else(|_| "0".into())
            .parse::<usize>()
            .context("GENTS_LOCAL_SMALL_LANES must be 0 or 2")?;
        let port = std::env::var("LOCAL_MODEL_PORT")
            .unwrap_or_else(|_| "8791".into())
            .parse::<u16>()?;
        Self::configured(root, port, lanes)
    }

    fn configured(root: PathBuf, port: u16, count: usize) -> Result<Self> {
        if !matches!(count, 0 | 2) || port > u16::MAX - count as u16 {
            bail!("invalid local lane count or port range");
        }
        if count > 0 {
            let observation = gents_local_mcp::host_memory::Snapshot::capture(
                &gents_local_mcp::host_memory::NativeMemoryProbe,
            )?;
            let bytes = observation.effective_total()?;
            if !fits_budget(bytes, count, SMALL_MODEL_MAX)
                || !fits_available(
                    observation.effective_available_bytes,
                    count,
                    SMALL_MODEL_MAX,
                )
            {
                bail!("small-model lanes exceed the observed host/process memory budget");
            }
        }
        let serial = LocalRuntime::at_port(root.clone(), port)?;
        let lanes = (0..count)
            .map(|index| {
                Ok(Lane {
                    runtime: LocalRuntime::at_port(root.clone(), port + index as u16 + 1)?,
                    busy: Mutex::new(()),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            serial,
            lanes,
            available: Semaphore::new(count),
            gate: RwLock::new(()),
            admission: Mutex::new(()),
        })
    }

    pub async fn complete(
        &self,
        model: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
    ) -> Result<Reply> {
        self.complete_scoped(model, system, input, max_tokens, cancel, "legacy")
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn complete_scoped(
        &self,
        model: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
        session: &str,
    ) -> Result<Reply> {
        model.validate()?;
        let path = self.serial.resolve_model(
            model.path.as_deref().context("local model path required")?,
            &model.provider,
        )?;
        let small = !self.lanes.is_empty()
            && model.provider == "local"
            && path.metadata()?.len() <= SMALL_MODEL_MAX;
        if !small {
            let _exclusive = cancellable(self.gate.write(), cancel).await?;
            self.stop_lanes().await;
            return self
                .serial
                .complete_scoped(model, system, input, max_tokens, cancel, session)
                .await;
        }
        let _shared = cancellable(self.gate.read(), cancel).await?;
        let _capacity = cancellable(self.available.acquire(), cancel).await??;
        // A semaphore permit guarantees an idle lane; no await occurs between
        // selecting and holding its mutex. Other callers cannot steal it.
        let (lane, _busy) = self
            .lanes
            .iter()
            .find_map(|lane| lane.busy.try_lock().ok().map(|busy| (lane, busy)))
            .context("local lane admission invariant failed")?;
        {
            let _admission = cancellable(self.admission.lock(), cancel).await?;
            // A previously used large model must not remain resident alongside
            // the small pool. The shared gate excludes active large inference.
            self.serial.stop_owned().await;
        }
        lane.runtime
            .complete_scoped(model, system, input, max_tokens, cancel, session)
            .await
    }

    async fn stop_lanes(&self) {
        for lane in &self.lanes {
            lane.runtime.stop_owned().await;
        }
    }

    pub async fn stop_owned(&self) {
        let _exclusive = self.gate.write().await;
        self.stop_lanes().await;
        self.serial.stop_owned().await;
    }

    pub async fn stop_session(&self, prefix: &str) {
        // Underlying stop_session uses try_lock: finishing one task must not
        // wait for or terminate another agent's in-flight call or training.
        self.serial.stop_session(prefix).await;
        for lane in &self.lanes {
            lane.runtime.stop_session(prefix).await;
        }
    }

    pub async fn operation<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let _exclusive = self.gate.write().await;
        self.stop_lanes().await;
        self.serial.operation(work).await
    }

    pub async fn train(&self, manifest: &Path, cancel: &AtomicBool) -> Result<()> {
        let _exclusive = cancellable(self.gate.write(), cancel).await?;
        self.stop_lanes().await;
        self.serial.train(manifest, cancel).await
    }
}

fn fits_available(available: Option<u64>, lanes: usize, model_bytes: u64) -> bool {
    let required = model_bytes
        .saturating_add(LANE_OVERHEAD)
        .saturating_mul(lanes as u64);
    available.is_some_and(|bytes| bytes.saturating_sub(4 * GIB) >= required)
}

fn fits_budget(memory: u64, lanes: usize, model_bytes: u64) -> bool {
    // Budget both models plus a conservative runtime/KV allowance per lane,
    // and leave at least 12 GiB for the host. This is an admission estimate,
    // not a guarantee against unrelated host memory use; profile actual RSS.
    let required = model_bytes
        .saturating_add(LANE_OVERHEAD)
        .saturating_mul(lanes as u64);
    memory.saturating_sub(12 * GIB) >= required && memory >= 24 * GIB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_estimate_reserves_host_memory_and_rejects_overcommit() {
        assert!(fits_budget(36 * GIB, 2, SMALL_MODEL_MAX));
        assert!(!fits_budget(16 * GIB, 2, SMALL_MODEL_MAX));
        assert!(!fits_budget(36 * GIB, 2, 18 * GIB));
        assert!(!fits_budget(36 * GIB, usize::MAX, u64::MAX));
        assert!(!fits_available(None, 2, SMALL_MODEL_MAX));
        assert!(!fits_available(Some(4 * GIB), 2, SMALL_MODEL_MAX));
        assert!(fits_available(Some(24 * GIB), 2, SMALL_MODEL_MAX));
    }

    #[tokio::test]
    async fn queued_admission_is_cancellable_and_does_not_take_capacity() {
        let lock = Mutex::new(());
        let _occupied = lock.lock().await;
        let cancel = AtomicBool::new(true);
        assert!(cancellable(lock.lock(), &cancel).await.is_err());
    }

    #[tokio::test]
    async fn exclusive_training_gate_waits_for_small_calls() {
        let gate = RwLock::new(());
        let left = gate.read().await;
        let right = gate.read().await;
        assert!(gate.try_write().is_err());
        drop(left);
        assert!(gate.try_write().is_err());
        drop(right);
        assert!(gate.try_write().is_ok());
    }

    #[tokio::test]
    async fn pool_exclusive_operations_wait_and_session_cleanup_does_not() {
        let pool = RuntimePool::configured(std::env::temp_dir(), 27910, 0).unwrap();
        let small = pool.gate.read().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(20), pool.operation(async { Ok(7) }))
                .await
                .is_err()
        );
        let cancelled = AtomicBool::new(true);
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            pool.train(Path::new("unused"), &cancelled)
        )
        .await
        .unwrap()
        .is_err());
        drop(small);
        assert_eq!(pool.operation(async { Ok(7) }).await.unwrap(), 7);
        let _training = pool.gate.write().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), pool.stop_session("unrelated"))
                .await
                .is_ok()
        );
    }
}
