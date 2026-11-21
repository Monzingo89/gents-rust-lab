#[cfg(target_os = "macos")]
use super::Mode;
use super::{api, Prepared};
pub(super) type BuildOutput = (Vec<PhaseResult>, Option<String>, Option<api::Report>);
use anyhow::Result;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhaseResult {
    pub phase: String,
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub limit: Option<String>,
    pub output_truncated: bool,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: u64,
    pub sampled_process_group_rss_bytes: Option<u64>,
    pub sampled_scratch_bytes: Option<u64>,
    pub sampled_scratch_entries: Option<u64>,
    pub resource_samples: u64,
}
#[cfg(not(target_os = "macos"))]
pub(super) fn build(_: &Prepared) -> Result<BuildOutput> {
    anyhow::bail!(".NET check requires macOS sandbox-exec; no unsandboxed fallback")
}
#[cfg(target_os = "macos")]
pub(crate) mod mac {
    use super::*;
    use anyhow::{ensure, Context};
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
    const OUTPUT: usize = 4096;
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn write(path: &Path, bytes: &[u8]) -> Result<()> {
        use std::io::Write;
        fs::create_dir_all(path.parent().context("scratch parent required")?)?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }
    fn literal(path: &Path) -> String {
        serde_json::to_string(path.to_str().unwrap()).unwrap()
    }
    fn profile(scratch: &Path, executable: &Path, sdk: &str, inputs: &[PathBuf]) -> Result<String> {
        let root = executable
            .parent()
            .context("dotnet installation root required")?;
        let csc = root
            .join("sdk")
            .join(sdk)
            .join("Roslyn/bincore/csc")
            .canonicalize()
            .context("exact frozen SDK compiler unavailable; no install attempted")?;
        ensure!(
            csc.starts_with(root),
            "SDK compiler escapes installed dotnet root"
        );
        let mut result=format!("(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow file-read* (literal \"/\") (subpath \"/System/Library\") (subpath \"/usr/lib\") (subpath \"/usr/share/icu\") (subpath \"/private/var/db/dyld\") (subpath {}) (subpath {}) (literal \"/dev/null\") (literal \"/dev/random\") (literal \"/dev/urandom\"))(allow file-write* (subpath {}) (literal \"/dev/null\"))(allow process-fork)(allow process-exec (literal {}) (literal {}))(allow signal (target same-sandbox))",literal(root),literal(scratch),literal(scratch),literal(executable),literal(&csc));
        for path in inputs {
            result += &format!("(deny file-write* (literal {}))", literal(path));
        }
        Ok(result)
    }
    // Keep only a bounded prefix but drain to EOF, avoiding a candidate-induced
    // blocked pipe or SIGPIPE being misreported as a normal compiler failure.
    fn reader(mut stream: impl Read + Send + 'static) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
        std::thread::spawn(move || {
            let mut saved = Vec::new();
            let mut buffer = [0u8; 4096];
            let mut truncated = false;
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let keep = n.min(OUTPUT.saturating_sub(saved.len()));
                        saved.extend_from_slice(&buffer[..keep]);
                        truncated |= keep < n;
                    }
                }
            }
            (saved, truncated)
        })
    }
    // A separate trusted watchdog owns only a pipe and the child PGID. EOF on
    // server termination/cancellation kills that group even when Rust Drop
    // cannot run. The child has already entered its new session before fork.
    pub(crate) struct Watchdog {
        write: i32,
        pid: i32,
    }
    impl Watchdog {
        pub(crate) fn new(group: i32) -> Result<Self> {
            let mut fds = [-1; 2];
            // SAFETY: pipe/fcntl/fork; the child branch uses only async-signal-safe libc.
            unsafe {
                ensure!(libc::pipe(fds.as_mut_ptr()) == 0, "watchdog pipe failed");
                for fd in fds {
                    if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) != 0 {
                        libc::close(fds[0]);
                        libc::close(fds[1]);
                        anyhow::bail!("watchdog close-on-exec failed");
                    }
                }
                let descriptor_ceiling = libc::getdtablesize();
                let pid = libc::fork();
                if pid < 0 {
                    libc::close(fds[0]);
                    libc::close(fds[1]);
                    anyhow::bail!("watchdog fork failed");
                }
                if pid == 0 {
                    libc::close(fds[1]);
                    // Close inherited DB, transport and output descriptors. Retain
                    // only the pipe read endpoint; never keep the MCP session alive.
                    for fd in 0..descriptor_ceiling {
                        if fd != fds[0] {
                            libc::close(fd);
                        }
                    }
                    let mut byte = 0u8;
                    loop {
                        let n = libc::read(fds[0], (&mut byte as *mut u8).cast(), 1);
                        if n >= 0 {
                            break;
                        }
                        if *libc::__error() != libc::EINTR {
                            break;
                        }
                    }
                    libc::kill(-group, libc::SIGKILL);
                    libc::_exit(0);
                }
                libc::close(fds[0]);
                Ok(Self { write: fds[1], pid })
            }
        }
    }
    impl Drop for Watchdog {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.write);
                libc::waitpid(self.pid, std::ptr::null_mut(), 0);
            }
        }
    }
    pub(crate) struct Process(pub(crate) std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
    fn usage(scratch: &Path, pgid: u32) -> Result<(u64, u64, u64)> {
        let output = Command::new("/bin/ps")
            .args(["-axo", "pgid=,rss="])
            .env_clear()
            .output()?;
        ensure!(
            output.status.success(),
            "process-group RSS observation unavailable"
        );
        let mut rss = 0;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() == 2 && parts[0].parse::<u32>().ok() == Some(pgid) {
                rss += parts[1].parse::<u64>()?.saturating_mul(1024);
            }
        }
        let mut bytes = 0u64;
        let mut entries = 0u64;
        let mut dirs = vec![scratch.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let metadata = entry.path().symlink_metadata()?;
                entries += 1;
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "unexpected scratch symlink"
                );
                if metadata.is_dir() {
                    dirs.push(entry.path());
                } else {
                    bytes = bytes.saturating_add(metadata.len());
                }
                if entries > 4096 || bytes > 256 * 1024 * 1024 {
                    return Ok((rss, bytes, entries));
                }
            }
        }
        Ok((rss, bytes, entries))
    }
    fn command(executable: &Path, args: &[String], scratch: &Path, profile: &str) -> Command {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args(["-p", profile])
            .arg(executable)
            .args(args)
            .current_dir(scratch)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("TMPDIR", scratch)
            .env("DOTNET_CLI_HOME", scratch.join(".cli"))
            .env("XDG_DATA_HOME", scratch.join(".data"))
            .env("NUGET_PACKAGES", scratch.join("packages"))
            .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
            .env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1")
            .env("DOTNET_NOLOGO", "1")
            .env("DOTNET_CLI_WORKLOAD_UPDATE_NOTIFY_DISABLE", "true")
            .env("MSBUILDDISABLENODEREUSE", "1")
            .env("DOTNET_CLI_DO_NOT_USE_MSBUILD_SERVER", "1")
            .env("DOTNET_EnableDiagnostics", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: child-only, async-signal-safe libc calls, no allocation.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for (resource, value) in [
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_CPU, 25),
                    (libc::RLIMIT_FSIZE, 32 * 1024 * 1024),
                    (libc::RLIMIT_NOFILE, 256),
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
        command
    }
    fn run(
        executable: &Path,
        args: &[String],
        scratch: &Path,
        profile: &str,
        phase: &str,
    ) -> Result<PhaseResult> {
        let mut command = command(executable, args, scratch, profile);
        let start = Instant::now();
        let mut child = Process(
            command
                .spawn()
                .context("isolated dotnet process could not start")?,
        );
        let watchdog = Watchdog::new(child.0.id() as i32)?;
        let stdout = reader(child.0.stdout.take().unwrap());
        let stderr = reader(child.0.stderr.take().unwrap());
        let mut limit = None;
        let mut last = Instant::now();
        let mut peaks = (0, 0, 0);
        let mut samples = 0;
        let status = loop {
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(30) {
                limit = Some("wall_time_limit".into());
            }
            if last.elapsed() > Duration::from_millis(100) {
                match usage(scratch, child.0.id()) {
                    Ok((rss, bytes, entries)) => {
                        samples += 1;
                        peaks = (peaks.0.max(rss), peaks.1.max(bytes), peaks.2.max(entries));
                        if rss > 2 * 1024 * 1024 * 1024 {
                            limit = Some("sampled_process_group_rss_limit".into());
                        }
                        if bytes > 256 * 1024 * 1024 || entries > 4096 {
                            limit = Some("sampled_scratch_limit".into());
                        }
                    }
                    Err(_) => limit = Some("resource_observation_unavailable".into()),
                }
                last = Instant::now();
            }
            if limit.is_some() {
                unsafe {
                    libc::kill(-(child.0.id() as i32), libc::SIGKILL);
                }
                break child.0.wait()?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(watchdog); // Kill any compiler descendants before joining output readers.
        let (out, out_truncated) = stdout
            .join()
            .map_err(|_| anyhow::anyhow!("dotnet stdout reader failed"))?;
        let (err, err_truncated) = stderr
            .join()
            .map_err(|_| anyhow::anyhow!("dotnet stderr reader failed"))?;
        Ok(PhaseResult {
            phase: phase.into(),
            passed: status.success() && limit.is_none() && !out_truncated && !err_truncated,
            exit_code: status.code(),
            limit,
            output_truncated: out_truncated || err_truncated,
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).into_owned(),
            elapsed_ms: start.elapsed().as_millis() as u64,
            sampled_process_group_rss_bytes: (peaks.0 > 0).then_some(peaks.0),
            sampled_scratch_bytes: (samples > 0).then_some(peaks.1),
            sampled_scratch_entries: (samples > 0).then_some(peaks.2),
            resource_samples: samples,
        })
    }
    fn run_api(prepared: &Prepared, scratch: &Path) -> Result<api::Report> {
        use std::{net::TcpListener, os::fd::AsRawFd};
        let contract = prepared
            .spec
            .api
            .as_ref()
            .context("frozen API contract missing")?;
        contract.validate()?;
        // Retain the socket until the oracle finishes: no rebind race, public
        // interface selection, or connection to an unrelated host service.
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let source_fd = listener.as_raw_fd();
        let root = prepared
            .executable
            .parent()
            .context("dotnet installation root missing")?;
        // Keep process inspection explicitly self-only as well as restricting
        // sysctls. A deny-default profile alone did not stop a real numeric
        // kern.procargs2 read in the runtime boundary test on this host.
        // hw.pagesize_compat is needed by the installed ARM64 runtime.
        let mut profile=format!("(version 1)(deny default)(allow sysctl-read (sysctl-name \"hw.ncpu\") (sysctl-name \"hw.activecpu\") (sysctl-name \"hw.logicalcpu\") (sysctl-name \"hw.logicalcpu_max\") (sysctl-name \"hw.physicalcpu\") (sysctl-name \"hw.physicalcpu_max\") (sysctl-name \"hw.memsize\") (sysctl-name \"hw.pagesize\") (sysctl-name \"hw.pagesize_compat\") (sysctl-name \"hw.cputype\") (sysctl-name \"hw.cpusubtype\") (sysctl-name \"hw.cachelinesize\") (sysctl-name-prefix \"hw.optional.\") (sysctl-name-prefix \"hw.perflevel\") (sysctl-name \"kern.osrelease\") (sysctl-name \"kern.osversion\") (sysctl-name \"kern.osproductversion\") (sysctl-name \"kern.ostype\") (sysctl-name \"kern.version\") (sysctl-name \"kern.hostname\") (sysctl-name \"kern.argmax\") (sysctl-name \"kern.ngroups\") (sysctl-name \"sysctl.proc_cputype\"))(allow file-read* (literal \"/\") (subpath \"/System/Library\") (subpath \"/usr/lib\") (subpath \"/usr/share/icu\") (subpath \"/private/var/db/dyld\") (subpath {}) (subpath {}) (subpath {}) (literal {}) (literal \"/dev/null\") (literal \"/dev/random\") (literal \"/dev/urandom\"))(allow file-write* (literal \"/dev/null\"))(allow process-exec (literal {}))(allow signal (target same-sandbox))(allow network-inbound (local tcp \"localhost:{port}\"))(deny network-bind)(deny process-info*)(allow process-info* (target self))(deny sysctl-read (sysctl-name-prefix \"kern.procargs\"))",literal(&root.join("host")),literal(&root.join("shared")),literal(scratch),literal(&prepared.executable),literal(&prepared.executable));
        let mut parents = std::collections::BTreeSet::new();
        for path in [root, scratch, prepared.executable.as_path()] {
            for parent in path.ancestors() {
                parents.insert(parent.to_owned());
            }
        }
        for parent in parents {
            profile += &format!("(allow file-read-metadata (literal {}))", literal(&parent));
        }

        let mut command = command(
            &prepared.executable,
            &[scratch
                .join(&prepared.artifact)
                .to_str()
                .context("invalid artifact path")?
                .into()],
            scratch,
            &profile,
        );
        command
            .env("GENTS_API_LISTEN_FD", "200")
            .env("DOTNET_USE_POLLING_FILE_WATCHER", "1")
            .env("ASPNETCORE_PREVENTHOSTINGSTARTUP", "true")
            .env("Logging__LogLevel__Default", "Warning");
        // SAFETY: child-only, async-signal-safe descriptor duplication/flags.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(source_fd, 200) < 0 || libc::fcntl(200, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let start = Instant::now();
        let mut child = Process(
            command
                .spawn()
                .context("isolated API process could not start")?,
        );
        let watchdog = Watchdog::new(child.0.id() as i32)?;
        let stdout = reader(child.0.stdout.take().unwrap());
        let stderr = reader(child.0.stderr.take().unwrap());
        let frozen = contract.clone();
        let (send, receive) = std::sync::mpsc::channel();
        // Blocking HTTP owns its runtime on this thread, outside MCP's Tokio
        // runtime. The destination is fixed by the host-held listener.
        let oracle = std::thread::spawn(move || {
            let result = api::requests(&frozen, port);
            let _ = send.send(result);
        });
        let mut result = None;
        let mut limit = None;
        let mut peaks = (0, 0, 0);
        let mut samples = 0;
        let mut last = Instant::now();
        loop {
            if let Ok(value) = receive.try_recv() {
                result = Some(value);
                break;
            }
            if child.0.try_wait()?.is_some() {
                limit = Some("application_exited_before_checks_completed".into());
                break;
            }
            if start.elapsed() > Duration::from_millis(contract.runtime_timeout_ms) {
                limit = Some("api_runtime_wall_limit".into());
                break;
            }
            if last.elapsed() > Duration::from_millis(100) {
                match usage(scratch, child.0.id()) {
                    Ok((rss, bytes, entries)) => {
                        samples += 1;
                        peaks = (peaks.0.max(rss), peaks.1.max(bytes), peaks.2.max(entries));
                        if rss > 2 * 1024 * 1024 * 1024 {
                            limit = Some("sampled_process_group_rss_limit".into());
                        }
                        if bytes > 256 * 1024 * 1024 || entries > 4096 {
                            limit = Some("sampled_scratch_limit".into());
                        }
                    }
                    Err(_) => limit = Some("resource_observation_unavailable".into()),
                }
                last = Instant::now();
            }
            if limit.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(watchdog);
        let status = child.0.wait()?;
        drop(listener);
        oracle
            .join()
            .map_err(|_| anyhow::anyhow!("API oracle thread failed"))?;
        if result.is_none() {
            result = receive.try_recv().ok();
        }
        let (cases, http_requests) = result.unwrap_or_else(|| {
            (
                api::pending(
                    contract,
                    "API runtime stopped before host requests completed",
                ),
                0,
            )
        });
        let (out, out_truncated) = stdout
            .join()
            .map_err(|_| anyhow::anyhow!("API stdout reader failed"))?;
        let (err, err_truncated) = stderr
            .join()
            .map_err(|_| anyhow::anyhow!("API stderr reader failed"))?;
        let passed = limit.is_none()
            && !out_truncated
            && !err_truncated
            && cases.len() == contract.cases.len()
            && cases.iter().all(|c| c.status == "passed");
        let runtime = PhaseResult {
            phase: "api_host_requests".into(),
            passed,
            exit_code: status.code(),
            limit,
            output_truncated: out_truncated || err_truncated,
            stdout: String::from_utf8_lossy(&out).into_owned(),
            stderr: String::from_utf8_lossy(&err).into_owned(),
            elapsed_ms: start.elapsed().as_millis() as u64,
            sampled_process_group_rss_bytes: (peaks.0 > 0).then_some(peaks.0),
            sampled_scratch_bytes: (samples > 0).then_some(peaks.1),
            sampled_scratch_entries: (samples > 0).then_some(peaks.2),
            resource_samples: samples,
        };
        Ok(api::Report{observed_at_unix_seconds:crate::now_ms()/1000,contract_sha256:crate::digest(&serde_json::to_vec(contract)?),passed,http_requests,cases,runtime,scope:"Frozen finite HTTP status/JSON oracle only, not general API correctness. Expected data stays in the host. Candidate runtime uses a host-held inherited IPv4 loopback socket; new binds, outbound connections, filesystem writes, and subprocesses are denied. No redirects, proxy, arbitrary destination, download, model call, qualification or promotion.".into()})
    }
    pub(crate) fn build(prepared: &Prepared) -> Result<BuildOutput> {
        ensure!(
            Path::new("/usr/bin/sandbox-exec").is_file(),
            "sandbox-exec unavailable; no fallback"
        );
        let scratch = Scratch(
            std::env::temp_dir()
                .canonicalize()?
                .join(format!("gents-dotnet-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&scratch.0)?;
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700))?;
        let base = Path::new(&prepared.spec.project_path).parent().unwrap();
        let mut inputs = vec![];
        for (path, text) in &prepared.files {
            let path = scratch.0.join(Path::new(path).strip_prefix(base)?);
            write(&path, text.as_bytes())?;
            inputs.push(path);
        }
        for (name,text) in [
            ("global.json",serde_json::json!({"sdk":{"version":prepared.spec.sdk_version,"rollForward":"disable","allowPrerelease":false}}).to_string()),
            ("NuGet.Config","<configuration><packageSources><clear /></packageSources></configuration>".into()),
            (".data/NuGet/Migrations/1",String::new()),
        ] {let path=scratch.0.join(name);write(&path,text.as_bytes())?;inputs.push(path);}
        let profile = profile(
            &scratch.0,
            &prepared.executable,
            &prepared.spec.sdk_version,
            &inputs,
        )?;
        let project = Path::new(&prepared.spec.project_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let mut phases = vec![];
        for (name, args) in [
            ("sdk_version", vec!["--version".into()]),
            (
                "restore",
                vec![
                    "restore".into(),
                    project.into(),
                    "--configfile".into(),
                    scratch.0.join("NuGet.Config").to_str().unwrap().into(),
                    "--disable-build-servers".into(),
                ],
            ),
            (
                "build",
                vec![
                    "build".into(),
                    project.into(),
                    "--no-restore".into(),
                    "--disable-build-servers".into(),
                    "-p:UseSharedCompilation=false".into(),
                ],
            ),
        ] {
            let mut phase = run(&prepared.executable, &args, &scratch.0, &profile, name)?;
            if name == "sdk_version" && phase.stdout.trim() != prepared.spec.sdk_version {
                phase.passed = false;
                phase.limit = Some("sdk_selection_mismatch".into());
            }
            let passed = phase.passed;
            phases.push(phase);
            if !passed {
                break;
            }
        }
        let mut artifact = None;
        if phases.len() == 3 && phases.iter().all(|p| p.passed) {
            let bytes = fs::read(scratch.0.join(&prepared.artifact))
                .context("successful build did not produce expected managed DLL")?;
            ensure!(
                !bytes.is_empty() && bytes.len() <= 32 * 1024 * 1024,
                "artifact byte limit exceeded"
            );
            artifact = Some(crate::digest(&bytes));
        }
        for (name, text) in &prepared.files {
            ensure!(
                fs::read(scratch.0.join(Path::new(name).strip_prefix(base)?))? == text.as_bytes(),
                "copied source changed during .NET build"
            );
        }
        let api = if artifact.is_some() && prepared.mode == Mode::Api {
            Some(run_api(prepared, &scratch.0)?)
        } else {
            None
        };
        if let Some(expected) = &artifact {
            ensure!(
                crate::digest(&fs::read(scratch.0.join(&prepared.artifact))?) == *expected,
                "compiled artifact changed during API execution"
            );
        }
        Ok((phases, artifact, api))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[allow(clippy::zombie_processes)] // Intentional abrupt parent death; watchdog kills the orphaned group.
        fn watchdog_kills_owned_process_group_when_server_exits_without_drop() {
            const CHILD: &str = "GENTS_DOTNET_WATCHDOG_TEST_CHILD";
            if let Some(path) = std::env::var_os(CHILD) {
                let mut command = Command::new("/bin/sleep");
                command
                    .arg("30")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                // SAFETY: child-only, async-signal-safe setsid.
                unsafe {
                    command.pre_exec(|| {
                        if libc::setsid() < 0 {
                            Err(std::io::Error::last_os_error())
                        } else {
                            Ok(())
                        }
                    });
                }
                let child = command.spawn().unwrap();
                let _watchdog = Watchdog::new(child.id() as i32).unwrap();
                fs::write(path, child.id().to_string()).unwrap();
                // Exercise actual parent termination, bypassing all Rust destructors.
                unsafe {
                    libc::_exit(0);
                }
            }
            let path =
                std::env::temp_dir().join(format!("gents-watchdog-test-{}", uuid::Uuid::new_v4()));
            let output=Command::new(std::env::current_exe().unwrap()).args(["--exact","dotnet_check::sandbox::mac::tests::watchdog_kills_owned_process_group_when_server_exits_without_drop","--nocapture"]).env(CHILD,&path).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let pid = fs::read_to_string(&path).unwrap();
            fs::remove_file(path).unwrap();
            let start = Instant::now();
            loop {
                let status = Command::new("/bin/ps")
                    .args(["-o", "stat=", "-p", &pid])
                    .env_clear()
                    .output()
                    .unwrap();
                let text = String::from_utf8_lossy(&status.stdout);
                if text.trim().is_empty() || text.trim().starts_with('Z') {
                    break;
                }
                if start.elapsed() > Duration::from_secs(2) {
                    unsafe {
                        libc::kill(-pid.parse::<i32>().unwrap(), libc::SIGKILL);
                    }
                    panic!("owned process group survived server death");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
#[cfg(target_os = "macos")]
pub(super) use mac::build;
