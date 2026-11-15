//! Offline OS observations and conservative bounds; no model choice or authority.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sysinfo::{MemoryRefreshKind, System};

#[derive(Clone, Debug)]
pub enum ProcessLimit {
    Observed {
        total_bytes: u64,
        available_bytes: u64,
    },
    Unavailable,
    NotObservedOnPlatform,
}
pub struct Reading {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub process_limit: ProcessLimit,
}
pub trait MemoryProbe {
    fn read(&self) -> Result<Reading>;
}
pub struct NativeMemoryProbe;
impl MemoryProbe for NativeMemoryProbe {
    fn read(&self) -> Result<Reading> {
        let mut system =
            System::new().map_err(|_| anyhow::anyhow!("OS memory probe unavailable"))?;
        system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        Ok(Reading {
            total_bytes: system.total_memory(),
            available_bytes: system.available_memory(),
            process_limit: process_limit(&mut system),
        })
    }
}
#[cfg(target_os = "linux")]
fn process_limit(system: &mut System) -> ProcessLimit {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
    let pid = Pid::from_u32(std::process::id());
    // Query only this process, without tasks, command line, environment or users.
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    match system.process(pid).and_then(|p| p.cgroup_limits()) {
        Some(limits) => ProcessLimit::Observed {
            total_bytes: limits.total_memory,
            available_bytes: limits.free_memory,
        },
        None => ProcessLimit::Unavailable,
    }
}
#[cfg(not(target_os = "linux"))]
fn process_limit(_: &mut System) -> ProcessLimit {
    ProcessLimit::NotObservedOnPlatform
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub observed_at_ms: u64,
    pub os: String,
    pub architecture: String,
    pub available_parallelism: Option<usize>,
    pub host_total_bytes: u64,
    pub host_available_bytes: u64,
    pub process_limit_status: String,
    pub process_limit_bytes: Option<u64>,
    pub process_available_bytes: Option<u64>,
    pub effective_limit_bytes: Option<u64>,
    pub effective_available_bytes: Option<u64>,
    pub gpu_memory_bytes: Option<u64>,
    pub probe: String,
}
impl Snapshot {
    pub fn capture(probe: &dyn MemoryProbe) -> Result<Self> {
        let reading = probe.read()?;
        ensure!(
            reading.total_bytes > 0 && reading.available_bytes <= reading.total_bytes,
            "OS memory observation is invalid"
        );
        let (status, limit, free, effective, available) = match reading.process_limit {
            ProcessLimit::Observed {
                total_bytes,
                available_bytes,
            } => {
                ensure!(
                    available_bytes <= total_bytes,
                    "process memory observation is invalid"
                );
                (
                    "observed",
                    Some(total_bytes),
                    Some(available_bytes),
                    Some(reading.total_bytes.min(total_bytes)),
                    Some(reading.available_bytes.min(available_bytes)),
                )
            }
            ProcessLimit::Unavailable => ("unavailable", None, None, None, None),
            ProcessLimit::NotObservedOnPlatform => (
                "not_observed_on_this_platform",
                None,
                None,
                Some(reading.total_bytes),
                Some(reading.available_bytes),
            ),
        };
        Ok(Self {
            observed_at_ms: crate::now_ms(), os: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            available_parallelism: std::thread::available_parallelism().ok().map(usize::from),
            host_total_bytes: reading.total_bytes, host_available_bytes: reading.available_bytes,
            process_limit_status: status.into(), process_limit_bytes: limit, process_available_bytes: free,
            effective_limit_bytes: effective, effective_available_bytes: available,
            gpu_memory_bytes: None,
            probe: "sysinfo 0.40.1: RAM and current-process Linux cgroup; available parallelism from Rust std. Estimates, not model fit or GPU capacity.".into(),
        })
    }
    pub fn effective_total(&self) -> Result<u64> {
        self.effective_limit_bytes
            .context("process memory limit unavailable; capacity admission denied")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GIB: u64 = 1024 * 1024 * 1024;
    struct Fixture {
        host: u64,
        free: u64,
        limit: ProcessLimit,
    }
    impl MemoryProbe for Fixture {
        fn read(&self) -> Result<Reading> {
            Ok(Reading {
                total_bytes: self.host,
                available_bytes: self.free,
                process_limit: self.limit.clone(),
            })
        }
    }
    #[test]
    fn process_limits_and_pressure_bound_host_ram_without_inventing_gpu_memory() {
        let snapshot = Snapshot::capture(&Fixture {
            host: 128 * GIB,
            free: 80 * GIB,
            limit: ProcessLimit::Observed {
                total_bytes: 8 * GIB,
                available_bytes: 2 * GIB,
            },
        })
        .unwrap();
        assert_eq!(snapshot.effective_total().unwrap(), 8 * GIB);
        assert_eq!(snapshot.effective_available_bytes, Some(2 * GIB));
        assert!(snapshot.gpu_memory_bytes.is_none());
        let snapshot = Snapshot::capture(&Fixture {
            host: 32 * GIB,
            free: GIB,
            limit: ProcessLimit::Observed {
                total_bytes: 8 * GIB,
                available_bytes: 7 * GIB,
            },
        })
        .unwrap();
        assert_eq!(snapshot.effective_available_bytes, Some(GIB));
        let unknown = Snapshot::capture(&Fixture {
            host: 32 * GIB,
            free: 20 * GIB,
            limit: ProcessLimit::Unavailable,
        })
        .unwrap();
        assert!(unknown.effective_total().is_err());
        let global = Snapshot::capture(&Fixture {
            host: 36 * GIB,
            free: 20 * GIB,
            limit: ProcessLimit::NotObservedOnPlatform,
        })
        .unwrap();
        assert_eq!(global.effective_total().unwrap(), 36 * GIB);
        assert_eq!(global.process_limit_status, "not_observed_on_this_platform");
        assert!(Snapshot::capture(&Fixture {
            host: 0,
            free: 0,
            limit: ProcessLimit::Unavailable
        })
        .is_err());
        assert!(Snapshot::capture(&Fixture {
            host: 1,
            free: 2,
            limit: ProcessLimit::Unavailable
        })
        .is_err());
        assert!(Snapshot::capture(&Fixture {
            host: 2,
            free: 1,
            limit: ProcessLimit::Observed {
                total_bytes: 1,
                available_bytes: 2
            }
        })
        .is_err());
    }
    #[test]
    fn actual_host_probe_returns_only_bounded_capacity_observations() {
        let snapshot = Snapshot::capture(&NativeMemoryProbe).unwrap();
        assert!(snapshot.host_total_bytes > 0);
        assert!(snapshot.host_available_bytes <= snapshot.host_total_bytes);
        let value = serde_json::to_value(&snapshot).unwrap();
        for key in ["host_name", "username", "environ", "cmd", "processes"] {
            assert!(value.get(key).is_none());
        }
        println!("{}", value);
    }
}
