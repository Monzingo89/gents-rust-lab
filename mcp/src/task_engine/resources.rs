//! Task-wide reservations, shared across workers, retries and review stages.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEstimate {
    pub weights_bytes: u64,
    pub adapters_bytes: u64,
    pub kv_bytes: u64,
    pub context_bytes: u64,
    pub runtime_bytes: u64,
    pub headroom_bytes: u64,
    pub provenance: String,
}
impl MemoryEstimate {
    pub fn total(&self) -> Result<u64> {
        [
            self.weights_bytes,
            self.adapters_bytes,
            self.kv_bytes,
            self.context_bytes,
            self.runtime_bytes,
            self.headroom_bytes,
        ]
        .into_iter()
        .try_fold(0u64, |sum, n| {
            sum.checked_add(n)
                .ok_or_else(|| anyhow::anyhow!("memory estimate overflow"))
        })
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Consumption {
    pub reserved_calls: u32,
    pub reserved_generation_tokens: u32,
    pub tool_calls: u32,
}
pub struct Budget {
    pub deadline_ms: u64,
    max_calls: u32,
    max_tokens: u32,
    max_tools: u32,
    consumed: Mutex<Consumption>,
    pub cancel: Arc<AtomicBool>,
}
impl Budget {
    pub fn new(
        policy: &super::contracts::ExecutionPolicy,
        max_tools: u32,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            deadline_ms: policy.deadline_ms,
            max_calls: policy.max_model_calls,
            max_tokens: policy.max_tokens,
            max_tools,
            consumed: Mutex::new(Consumption::default()),
            cancel,
        }
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!self.cancel.load(Ordering::Relaxed), "task cancelled");
        ensure!(
            crate::now_ms() < self.deadline_ms,
            "global deadline exhausted"
        );
        Ok(())
    }
    pub fn reserve_call(&self, tokens: u32) -> Result<()> {
        self.check()?;
        let mut used = self
            .consumed
            .lock()
            .map_err(|_| anyhow::anyhow!("budget unavailable"))?;
        ensure!(
            tokens > 0
                && used.reserved_calls < self.max_calls
                && used
                    .reserved_generation_tokens
                    .checked_add(tokens)
                    .is_some_and(|n| n <= self.max_tokens),
            "global call/token budget exhausted"
        );
        used.reserved_calls += 1;
        used.reserved_generation_tokens += tokens;
        Ok(())
    }
    pub fn reserve_tool(&self) -> Result<()> {
        self.check()?;
        let mut used = self
            .consumed
            .lock()
            .map_err(|_| anyhow::anyhow!("budget unavailable"))?;
        ensure!(
            used.tool_calls < self.max_tools,
            "global tool budget exhausted"
        );
        used.tool_calls += 1;
        Ok(())
    }
    pub fn consumption(&self) -> Result<Consumption> {
        Ok(self
            .consumed
            .lock()
            .map_err(|_| anyhow::anyhow!("budget unavailable"))?
            .clone())
    }
    pub fn can_fit(&self, calls: u32, tokens: u32) -> Result<bool> {
        self.check()?;
        let used = self.consumption()?;
        Ok(used
            .reserved_calls
            .checked_add(calls)
            .is_some_and(|n| n <= self.max_calls)
            && used
                .reserved_generation_tokens
                .checked_add(tokens)
                .is_some_and(|n| n <= self.max_tokens))
    }
}
/// A process-wide gate: idle profiles reserve nothing. Callers sharing the
/// existing inference pool share this scheduler too. RAII returns reservations.
pub struct Scheduler {
    slots: Arc<Semaphore>,
    memory: Arc<Semaphore>,
    memory_mib: u32,
}
pub struct Reservation {
    _slot: OwnedSemaphorePermit,
    _memory: OwnedSemaphorePermit,
}
impl Scheduler {
    pub fn new(concurrency: usize, memory_bytes: u64) -> Result<Self> {
        let mib = (memory_bytes / (1024 * 1024)).min(u32::MAX as u64) as u32;
        ensure!(
            (1..=32).contains(&concurrency) && mib > 0,
            "invalid scheduler capacity"
        );
        Ok(Self {
            slots: Arc::new(Semaphore::new(concurrency)),
            memory: Arc::new(Semaphore::new(mib as usize)),
            memory_mib: mib,
        })
    }
    pub async fn reserve(&self, estimate: &MemoryEstimate, budget: &Budget) -> Result<Reservation> {
        let bytes = estimate.total()?;
        let mib = bytes.div_ceil(1024 * 1024).max(1);
        if mib > u64::from(self.memory_mib) {
            return Err(super::workflows::ExecutionFailure{kind:super::workflows::StopReason::Capacity,message:"estimated model/context memory exceeds capacity; choose a host-configured compatible fallback".into()}.into());
        }
        let acquire = async {
            Ok::<_, anyhow::Error>(Reservation {
                _slot: self.slots.clone().acquire_owned().await?,
                _memory: self.memory.clone().acquire_many_owned(mib as u32).await?,
            })
        };
        tokio::pin!(acquire);
        loop {
            budget.check()?;
            tokio::select! {r=&mut acquire=>return r,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{}}
        }
    }
}
