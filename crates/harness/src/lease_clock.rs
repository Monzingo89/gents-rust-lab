//! Active local work uses elapsed process time, never adjustable UTC time.
//! Persisted leases require a separately fenced controller epoch on restart.
use std::{sync::OnceLock, time::Instant};
pub fn now_ms() -> u64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
