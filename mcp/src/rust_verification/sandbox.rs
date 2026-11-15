//! Installed-toolchain adapter. No Cargo, shell, package resolution or workspace execution.
#[cfg(not(target_os = "macos"))]
use anyhow::Result;
#[cfg(not(target_os = "macos"))]
use serde_json::Value;
#[cfg(not(target_os = "macos"))]
pub fn verify(_: &str, _: &str, _: usize) -> Result<Value> {
    anyhow::bail!(
        "isolated Rust verifier currently requires macOS sandbox-exec; no unsandboxed fallback"
    )
}
#[cfg(not(target_os = "macos"))]
pub fn invoke(_: &str, _: &str) -> Result<Value> {
    anyhow::bail!("isolated component invocation currently requires macOS sandbox-exec; no unsandboxed fallback")
}
#[cfg(target_os = "macos")]
mod mac {
    use anyhow::{ensure, Context, Result};
    use serde_json::{json, Value};
    use std::{
        fs,
        io::Read,
        os::unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    const OUTPUT: u64 = 16384;
    struct Process(std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Result<Self> {
            let path = std::env::temp_dir()
                .canonicalize()?
                .join(format!("gents-rust-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
            Ok(Self(path))
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn write(path: &Path, text: &str) -> Result<()> {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        Ok(())
    }
    fn literal(p: &Path) -> String {
        serde_json::to_string(p.to_str().unwrap()).unwrap()
    }
    fn profile(scratch: &Path, toolchain: &Path, compiler: bool) -> String {
        let mut profile=format!("(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow file-read* (literal \"/\") (subpath \"/System/Library\") (subpath \"/usr/lib\") (subpath \"/private/var/db/dyld\") (subpath {}) (literal \"/dev/null\") (literal \"/dev/random\") (literal \"/dev/urandom\"))(allow file-write* (subpath {}) (literal \"/dev/null\"))(allow signal (target same-sandbox))",literal(scratch),literal(scratch));
        if compiler {
            profile+=&format!("(allow file-read* (subpath {}) (subpath \"/Library/Developer/CommandLineTools\"))(allow process-fork)(allow process-exec (subpath {}) (subpath \"/Library/Developer/CommandLineTools\") (literal \"/usr/bin/cc\") (literal \"/usr/bin/ld\") (literal \"/usr/bin/xcrun\"))",literal(toolchain),literal(toolchain));
        } else {
            profile += &format!(
                "(allow process-exec (literal {}))",
                literal(&scratch.join("suite"))
            );
        }
        profile
    }
    fn run(
        binary: &Path,
        args: &[String],
        scratch: &Path,
        toolchain: &Path,
        compiler: bool,
    ) -> Result<Value> {
        ensure!(
            Path::new("/usr/bin/sandbox-exec").is_file(),
            "sandbox-exec unavailable; no unsandboxed fallback"
        );
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args(["-p", &profile(scratch, toolchain, compiler)])
            .arg(binary)
            .args(args)
            .current_dir(scratch)
            .env_clear()
            .env(
                "PATH",
                "/usr/bin:/bin:/Library/Developer/CommandLineTools/usr/bin",
            )
            .env("HOME", scratch)
            .env("TMPDIR", scratch)
            .env("LC_ALL", "C")
            .env("DEVELOPER_DIR", "/Library/Developer/CommandLineTools")
            .env(
                "SDKROOT",
                "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: child-only pre-exec uses async-signal-safe libc operations, no allocation.
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for (resource, value) in [
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_CPU, if compiler { 20 } else { 2 }),
                    (libc::RLIMIT_FSIZE, 32 * 1024 * 1024),
                    (libc::RLIMIT_NOFILE, 128),
                ] {
                    let limit = libc::rlimit {
                        rlim_cur: value,
                        rlim_max: value,
                    };
                    if libc::setrlimit(resource, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = Process(
            command
                .spawn()
                .context("isolated Rust process could not start")?,
        );
        let id = child.0.id();
        let stdout = child.0.stdout.take().unwrap();
        let stderr = child.0.stderr.take().unwrap();
        let read = |stream: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = stream.take(OUTPUT + 1).read_to_end(&mut bytes);
                bytes
            })
        };
        let out = read(Box::new(stdout));
        let err = read(Box::new(stderr));
        let start = Instant::now();
        let mut last_memory = Instant::now();
        let mut reason = None;
        let status = loop {
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(if compiler { 30 } else { 5 }) {
                reason = Some("wall_time_limit");
            }
            if last_memory.elapsed() > Duration::from_millis(100) {
                let usage = Command::new("/bin/ps")
                    .args(["-o", "rss=", "-p", &id.to_string()])
                    .env_clear()
                    .output()?;
                if let Ok(rss) = String::from_utf8_lossy(&usage.stdout).trim().parse::<u64>() {
                    if rss > if compiler { 1024 * 1024 } else { 256 * 1024 } {
                        reason = Some("resident_memory_limit");
                    }
                } else if child.0.try_wait()?.is_none() {
                    reason = Some("memory_observation_unavailable");
                }
                last_memory = Instant::now();
            }
            if reason.is_some() {
                // SAFETY: this child owns the new session/process group; no host group is targeted.
                unsafe {
                    libc::kill(-(id as i32), libc::SIGKILL);
                }
                break child.0.wait()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        // Reap any compiler children still holding inherited output descriptors.
        unsafe {
            libc::kill(-(id as i32), libc::SIGKILL);
        }
        let out = out
            .join()
            .map_err(|_| anyhow::anyhow!("stdout reader failed"))?;
        let err = err
            .join()
            .map_err(|_| anyhow::anyhow!("stderr reader failed"))?;
        ensure!(
            out.len() <= OUTPUT as usize && err.len() <= OUTPUT as usize,
            "isolated process exceeded output limit"
        );
        Ok(
            json!({"passed":status.success()&&reason.is_none(),"exit_code":status.code(),"signal":std::os::unix::process::ExitStatusExt::signal(&status),"limit":reason,"stdout":String::from_utf8_lossy(&out),"stderr":String::from_utf8_lossy(&err)}),
        )
    }
    pub fn verify(source: &str, tests: &str, count: usize) -> Result<Value> {
        execute(source, tests, Some(count))
    }
    pub fn invoke(source: &str, driver: &str) -> Result<Value> {
        execute(source, driver, None)
    }
    fn execute(source: &str, tests: &str, count: Option<usize>) -> Result<Value> {
        let compiler = crate::rust_verification::compiler_path()?
            .canonicalize()
            .context("installed rustc unavailable; no download attempted")?;
        let toolchain = compiler
            .parent()
            .and_then(Path::parent)
            .context("invalid Rust toolchain path")?;
        ensure!(
            toolchain.join("lib/rustlib").is_dir(),
            "rustc must be an installed native toolchain executable"
        );
        let scratch = Scratch::new()?;
        write(&scratch.0.join("candidate.rs"), source)?;
        write(&scratch.0.join("acceptance.rs"), tests)?;
        let version = run(
            &compiler,
            &["--version".into()],
            &scratch.0,
            toolchain,
            true,
        )?;
        ensure!(
            version["passed"] == true,
            "isolated compiler preflight failed: {}",
            version
        );
        let library = run(
            &compiler,
            &[
                "candidate.rs",
                "--edition=2021",
                "--crate-name=candidate",
                "--crate-type=lib",
                "-Funsafe_code",
                "-Dwarnings",
                "-o",
                "libcandidate.rlib",
            ]
            .map(str::to_string),
            &scratch.0,
            toolchain,
            true,
        )?;
        if library["passed"] != true {
            return Ok(
                json!({"passed":false,"stage":"compile_candidate","compiler":version["stdout"],"diagnostics":library}),
            );
        }
        let mut arguments = [
            "acceptance.rs",
            "--edition=2021",
            "--extern",
            "candidate=libcandidate.rlib",
            "-Funsafe_code",
            "-Dwarnings",
            "-Clinker=/Library/Developer/CommandLineTools/usr/bin/cc",
            "-o",
            "suite",
        ]
        .map(str::to_string)
        .to_vec();
        if count.is_some() {
            arguments.push("--test".into());
        }
        let suite = run(&compiler, &arguments, &scratch.0, toolchain, true)?;
        if suite["passed"] != true {
            return Ok(
                json!({"passed":false,"stage":if count.is_some(){"compile_acceptance"}else{"compile_driver"},"compiler":version["stdout"],"diagnostics":suite}),
            );
        }
        let execution = run(
            &scratch.0.join("suite"),
            &if count.is_some() {
                vec!["--test-threads=1".into()]
            } else {
                vec![]
            },
            &scratch.0,
            toolchain,
            false,
        )?;
        let passed = execution["passed"] == true
            && count.is_none_or(|count| {
                execution["stdout"].as_str().is_some_and(|s| {
                    s.contains(&format!(
                        "test result: ok. {count} passed; 0 failed; 0 ignored;"
                    ))
                })
            });
        if count.is_none() {
            let output = if passed {
                Some(
                    serde_json::from_str::<Value>(
                        execution["stdout"]
                            .as_str()
                            .context("component stdout missing")?,
                    )
                    .context("component returned invalid JSON")?,
                )
            } else {
                None
            };
            return Ok(
                json!({"passed":passed,"stage":"invocation","compiler":version["stdout"],"diagnostics":execution,
                "output":output,"isolation":"macOS Seatbelt deny-default; separate compiler/runtime profiles; no network, home data, external writes or runtime subprocesses"}),
            );
        }
        Ok(
            json!({"passed":passed,"stage":"acceptance","compiler":version["stdout"],"tests_count":count,"diagnostics":execution,"isolation":"macOS Seatbelt deny-default; separate compiler/runtime profiles; no network, home data, external writes or runtime subprocesses","limits":{"source_bytes":8192,"output_bytes_per_stream":OUTPUT,"compile_wall_seconds":30,"test_wall_seconds":5,"test_cpu_seconds":2,"test_rss_kib_observation_limit":262144}}),
        )
    }
}
#[cfg(target_os = "macos")]
pub use mac::{invoke, verify};

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn sandbox_independently_denies_host_io_network_and_subprocesses() {
        let file = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("gents-denial-{}", uuid::Uuid::new_v4()));
        std::fs::write(&file, "untouched").unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let source=format!("pub fn denied()->bool{{ std::fs::read_to_string({file:?}).is_err() && std::fs::write({file:?},b\"changed\").is_err() && std::net::TcpStream::connect({address:?}).is_err() && std::process::Command::new(\"/bin/echo\").arg(\"forbidden\").output().is_err() }}");
        let result = verify(
            &source,
            "#[test] fn boundary(){std::assert!(candidate::denied());}",
            1,
        )
        .unwrap();
        assert_eq!(result["passed"], true, "{result}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "untouched");
        assert!(listener.accept().is_err());
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn nonterminating_code_is_stopped_and_cannot_pass() {
        let result = verify(
            "pub fn spin()->!{loop{}}",
            "#[test] fn bounded(){candidate::spin();}",
            1,
        )
        .unwrap();
        assert_eq!(result["passed"], false, "{result}");
        assert_eq!(result["stage"], "acceptance");
    }
}
