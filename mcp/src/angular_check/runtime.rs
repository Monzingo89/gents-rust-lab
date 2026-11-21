//! Bounded SDK executor shared by trusted in-process callers and the governed
//! native tool. Explicit frozen snapshots only; no install or fallback.
#[cfg(target_os = "macos")]
use super::BrowserAction;
use super::{Contract, SdkPin};
use anyhow::{ensure, Context, Result};
use schemars::JsonSchema;
#[cfg(target_os = "macos")]
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSdk {
    pub node: PathBuf,
    pub angular: PathBuf,
    pub driver: PathBuf,
    pub browser: PathBuf,
}
fn file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(
            total <= 512 * 1024 * 1024,
            "SDK file exceeds identity bound"
        );
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
// A seal is never a substitute for initial content verification. Native
// receipts bind this exact metadata snapshot only after evaluate has verified
// all content pins before/after execution. Status can then detect SDK changes
// without holding a SQLite writer while rereading hundreds of MiB. Include
// ctime, so restoring mtime after an in-place edit cannot preserve the seal.
fn metadata_tree(root: &Path) -> Result<String> {
    use std::os::unix::fs::MetadataExt;
    let root = root.canonicalize()?;
    let mut pending = vec![root.clone()];
    let mut entries = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let m = fs::symlink_metadata(&path)?;
        ensure!(
            m.is_file() || m.is_dir() || m.file_type().is_symlink(),
            "Special SDK file unsupported"
        );
        let relative = path
            .strip_prefix(&root)?
            .to_str()
            .context("UTF-8 SDK paths required")?
            .to_string();
        let link = if m.file_type().is_symlink() {
            ensure!(
                path.canonicalize()?.starts_with(&root),
                "SDK symlink leaves frozen tree"
            );
            Some(
                fs::read_link(&path)?
                    .to_str()
                    .context("UTF-8 SDK link required")?
                    .to_string(),
            )
        } else {
            None
        };
        entries.insert(relative,json!({"dev":m.dev(),"ino":m.ino(),"nlink":m.nlink(),"size":m.len(),"mode":m.mode(),"mtime":m.mtime(),"mtime_nsec":m.mtime_nsec(),"ctime":m.ctime(),"ctime_nsec":m.ctime_nsec(),"link":link}));
        ensure!(entries.len() <= 100000, "SDK metadata tree bound exceeded");
        if m.is_dir() {
            for child in fs::read_dir(path)? {
                pending.push(child?.path());
            }
        }
    }
    Ok(crate::digest(&serde_json::to_vec(&entries)?))
}
/// Hash sorted relative names, file contents, executable modes and internal
/// symlink targets. External symlinks/special files and oversized trees fail.
pub fn tree_hash(root: &Path) -> Result<String> {
    use std::os::unix::fs::PermissionsExt;
    let root = root.canonicalize()?;
    ensure!(root.is_dir(), "SDK tree root must be a directory");
    let mut pending = vec![root.clone()];
    let mut entries = BTreeMap::new();
    let mut bytes = 0u64;
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            let name = path
                .strip_prefix(&root)?
                .to_str()
                .context("UTF-8 SDK paths required")?
                .to_string();
            let value = if metadata.file_type().is_symlink() {
                ensure!(
                    path.canonicalize()?.starts_with(&root),
                    "SDK symlink leaves its frozen tree"
                );
                json!({"symlink":fs::read_link(&path)?.to_str().context("UTF-8 symlink required")?})
            } else if metadata.is_dir() {
                pending.push(path.clone());
                json!({"directory":true})
            } else {
                ensure!(metadata.is_file(), "Special SDK file unsupported");
                bytes = bytes
                    .checked_add(metadata.len())
                    .context("SDK byte overflow")?;
                ensure!(
                    bytes <= 2 * 1024 * 1024 * 1024,
                    "SDK tree exceeds 2 GiB identity bound"
                );
                json!({"sha256":file_hash(&path)?,"executable":metadata.permissions().mode() & 0o111})
            };
            entries.insert(name, value);
            ensure!(entries.len() <= 100000, "SDK tree exceeds 100000 entries");
        }
    }
    Ok(crate::digest(&serde_json::to_vec(&entries)?))
}
impl HostSdk {
    pub(crate) fn seal(&self) -> Result<String> {
        let sdk = self.canonical()?;
        let roots = [
            sdk.node.clone(),
            sdk.angular.clone(),
            sdk.angular.parent().unwrap().join("package.json"),
            sdk.angular.parent().unwrap().join("package-lock.json"),
            sdk.driver.clone(),
            sdk.browser.parent().unwrap().to_path_buf(),
        ];
        let seals = roots
            .into_iter()
            .map(|p| {
                Ok((
                    p.to_str().context("UTF-8 SDK paths required")?.to_string(),
                    metadata_tree(&p)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(crate::digest(&serde_json::to_vec(&seals)?))
    }
    pub fn configured() -> Result<Self> {
        let get = |name| {
            std::env::var_os(name).map(PathBuf::from).with_context(|| {
                format!(
                    "Angular needs owner-configured absolute {name}; no discovery/install fallback"
                )
            })
        };
        Self {
            node: get("GENTS_NODE")?,
            angular: get("GENTS_ANGULAR_MODULES")?,
            driver: get("GENTS_PLAYWRIGHT_DRIVER")?,
            browser: get("GENTS_ANGULAR_BROWSER")?,
        }
        .canonical()
    }
    pub fn canonical(&self) -> Result<Self> {
        for path in [&self.node, &self.angular, &self.driver, &self.browser] {
            ensure!(path.is_absolute(), "Host SDK paths must be absolute");
        }
        let sdk = Self {
            node: self.node.canonicalize()?,
            angular: self.angular.canonicalize()?,
            driver: self.driver.canonicalize()?,
            browser: self.browser.canonicalize()?,
        };
        ensure!(
            sdk.node.is_file()
                && sdk.node.file_name().is_some_and(|n| n == "node")
                && sdk.angular.join("@angular/cli/bin/ng.js").is_file()
                && sdk.driver.join("index.mjs").is_file()
                && sdk
                    .browser
                    .file_name()
                    .is_some_and(|n| n == "chrome-headless-shell")
                && sdk.browser.is_file(),
            "Exact installed Node, Angular, Playwright and headless-shell SDK paths required"
        );
        ensure!(
            !sdk.angular.starts_with(&sdk.driver) && !sdk.driver.starts_with(&sdk.angular),
            "SDK trees must have independent identities"
        );
        Ok(sdk)
    }
    pub fn pins(&self, node_version: &str) -> Result<SdkPin> {
        let sdk = self.canonical()?;
        let metadata: Value =
            serde_json::from_slice(&fs::read(sdk.angular.join("@angular/cli/package.json"))?)?;
        Ok(SdkPin {
            node_version: node_version.into(),
            node_sha256: file_hash(&sdk.node)?,
            angular_cli_version: metadata["version"]
                .as_str()
                .context("CLI version unavailable")?
                .into(),
            sdk_tree_sha256: crate::digest(&serde_json::to_vec(
                &json!({"node_modules":tree_hash(&sdk.angular)?,"package":file_hash(&sdk.angular.parent().unwrap().join("package.json"))?,"lock":file_hash(&sdk.angular.parent().unwrap().join("package-lock.json"))?}),
            )?),
            driver_tree_sha256: tree_hash(&sdk.driver)?,
            browser_tree_sha256: tree_hash(sdk.browser.parent().unwrap())?,
        })
    }
    pub(crate) fn verify(&self, pin: &SdkPin) -> Result<()> {
        ensure!(
            serde_json::to_value(self.pins(&pin.node_version)?)? == serde_json::to_value(pin)?,
            "Installed Angular SDK/browser/driver differs from frozen owner pins"
        );
        Ok(())
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct Snapshot {
    contract: Contract,
    sdk: HostSdk,
    files: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
    identity: String,
}
impl Snapshot {
    pub fn prepare(
        contract: Contract,
        sdk: HostSdk,
        files: BTreeMap<String, String>,
    ) -> Result<Self> {
        contract.validate()?;
        sdk.verify(&contract.sdk)?;
        Self::prepare_deferred(contract, sdk, files)
    }
    // Only the governed native adapter defers the heavy SDK check until after
    // releasing its dispatch transaction. evaluate verifies it before execution
    // and again afterward; this constructor never claims an SDK observation.
    pub(crate) fn prepare_deferred(
        contract: Contract,
        sdk: HostSdk,
        files: BTreeMap<String, String>,
    ) -> Result<Self> {
        contract.validate()?;
        let sdk = sdk.canonical()?;
        let paths = std::iter::once(&contract.project_path)
            .chain(&contract.source_paths)
            .collect::<Vec<_>>();
        ensure!(
            files.len() == paths.len() && paths.iter().all(|p| files.contains_key(*p)),
            "Snapshot must contain exactly the explicit Angular inputs"
        );
        ensure!(
            files.values().map(String::len).sum::<usize>() <= 2 * 1024 * 1024,
            "Angular source snapshot exceeds 2 MiB"
        );
        let mut hashes = BTreeMap::new();
        for (path, text) in &files {
            crate::safety::require_persistable(text)?;
            hashes.insert(path.clone(), crate::digest(text.as_bytes()));
        }
        ensure!(
            contract
                .frozen_inputs
                .iter()
                .all(|(p, h)| hashes.get(p) == Some(h)),
            "Frozen Angular configuration hash mismatch"
        );
        project(&contract, &files)?;
        let identity = crate::digest(&serde_json::to_vec(
            &json!({"contract":contract,"inputs":hashes,"implementation":crate::digest(concat!(include_str!("../angular_check.rs"),include_str!("runtime.rs"),include_str!("browser.mjs"),
                    include_str!("browser_profile.mjs"),include_str!("../dotnet_check/sandbox.rs"),include_str!("../strict_json.rs"),include_str!("../safety.rs")).as_bytes())}),
        )?);
        Ok(Self {
            contract,
            sdk,
            files,
            hashes,
            identity,
        })
    }
    pub fn source_identity(&self) -> &str {
        &self.identity
    }
}

fn project(contract: &Contract, files: &BTreeMap<String, String>) -> Result<()> {
    // Only the checked standard browser application builder. Custom Node
    // builders, SSR/prerender, lifecycle scripts and downloads are not run.
    let base = Path::new(&contract.project_path).parent().unwrap();
    let document = crate::strict_json::parse(&files[&contract.project_path])?;
    let selected = &document["projects"][&contract.project_name];
    let build = &selected["architect"]["build"];
    ensure!(
        document["version"] == 1
            && selected["projectType"] == "application"
            && selected["root"] == ""
            && build["builder"] == "@angular/build:application",
        "Only a root standard Angular application builder is supported"
    );
    ensure!(
        document["cli"]["analytics"] == false && document["cli"]["cache"]["enabled"] == false,
        "Owner configuration must disable Angular analytics and disk cache"
    );
    let options = build["options"]
        .as_object()
        .context("Angular build options required")?;
    ensure!(
        options.keys().all(|k| [
            "browser",
            "index",
            "outputPath",
            "tsConfig",
            "assets",
            "styles",
            "polyfills"
        ]
        .contains(&k.as_str()))
            && options.get("outputPath") == Some(&json!("dist")),
        "Unsupported build options/output; no SSR, server, prerender or executable plugins"
    );
    for name in ["browser", "index", "tsConfig"] {
        let path = options
            .get(name)
            .and_then(Value::as_str)
            .context("Explicit browser/index/tsconfig paths required")?;
        ensure!(
            crate::workspace_path::is_safe_relative_path(path)
                && base
                    .join(path)
                    .to_str()
                    .is_some_and(|p| files.contains_key(p)),
            "Angular entrypoint outside explicit snapshot"
        );
    }
    for name in ["assets", "styles", "polyfills"] {
        ensure!(options.get(name)==Some(&json!([])), "External assets/styles/polyfills unsupported in the initial bounded application executor");
    }
    ensure!(
        build["configurations"]["development"]
            == json!({"optimization":false,"sourceMap":false,"extractLicenses":false}),
        "Exact development build controls required"
    );
    for (path, text) in files.iter().filter(|(p, _)| p.ends_with(".json")) {
        let value = crate::strict_json::parse(text)?;
        if Path::new(path)
            .file_name()
            .is_some_and(|p| p == "package.json")
        {
            ensure!(
                value.get("scripts").is_none() && value.get("workspaces").is_none(),
                "No package scripts or workspaces supported"
            );
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub limit: Option<String>,
    pub elapsed_ms: u64,
    pub sampled_process_group_rss_bytes: Option<u64>,
    pub sampled_scratch_bytes: Option<u64>,
    pub resource_samples: u64,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserStep {
    pub id: String,
    pub status: String,
    pub actual_sha256: Option<String>,
    pub expected_sha256: Option<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserReport {
    pub passed: bool,
    pub started: bool,
    pub held_loopback_families: Vec<String>,
    pub reason: String,
    pub steps: Vec<BrowserStep>,
    pub blocked_requests: u64,
    pub outer_seatbelt: bool,
    pub chromium_inner_sandbox: bool,
    pub phase: Phase,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub status: String,
    pub contract_id: String,
    pub verification_identity: String,
    pub inputs_sha256: BTreeMap<String, String>,
    pub sdk: SdkPin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk_metadata_seal: Option<String>,
    pub compile: Phase,
    pub compile_stdout: String,
    pub compile_stderr: String,
    pub artifact_sha256: Option<String>,
    pub browser: Option<BrowserReport>,
    pub snapshot_unchanged: bool,
    pub elapsed_ms: u64,
    pub model_calls: u64,
    pub provider_cost_usd: u64,
    pub qualified: bool,
    pub promoted: bool,
    pub authority_granted: bool,
    pub scope: String,
}
#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    id: String,
    action: String,
    completed: bool,
    #[serde(default)]
    actual: Option<String>,
}
#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBrowser {
    schema_version: u32,
    started: bool,
    owned_process_group: bool,
    held_loopback_families: Vec<String>,
    steps: Vec<RawStep>,
    reason: String,
    blocked_requests: u64,
    outer_seatbelt: bool,
    chromium_inner_sandbox: bool,
}

#[cfg(not(target_os = "macos"))]
pub fn evaluate(_: &Snapshot) -> Result<Evaluation> {
    anyhow::bail!("Angular executor needs macOS Seatbelt; no unconfined fallback")
}
#[cfg(target_os = "macos")]
pub fn evaluate(snapshot: &Snapshot) -> Result<Evaluation> {
    mac::evaluate(snapshot)
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use crate::dotnet_check::sandbox::mac::{Process, Watchdog};
    use std::{
        io::Write,
        os::unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn write(path: &Path, bytes: &[u8]) -> Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        Ok(())
    }
    fn reader(
        mut stream: impl Read + Send + 'static,
        cap: usize,
    ) -> std::thread::JoinHandle<(Vec<u8>, bool)> {
        std::thread::spawn(move || {
            let mut saved = vec![];
            let mut truncated = false;
            let mut buffer = [0; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let keep = n.min(cap.saturating_sub(saved.len()));
                        saved.extend_from_slice(&buffer[..keep]);
                        truncated |= keep < n;
                    }
                }
            }
            (saved, truncated)
        })
    }
    fn usage(scratch: &Path, group: u32) -> Result<(u64, u64)> {
        let output = Command::new("/bin/ps")
            .args(["-axo", "pgid=,rss="])
            .env_clear()
            .output()?;
        ensure!(output.status.success(), "RSS observation unavailable");
        let mut rss = 0u64;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let p = line.split_whitespace().collect::<Vec<_>>();
            if p.len() == 2 && p[0].parse::<u32>().ok() == Some(group) {
                rss = rss.saturating_add(p[1].parse::<u64>()?.saturating_mul(1024));
            }
        }
        let mut dirs = vec![scratch.to_path_buf()];
        let mut bytes = 0u64;
        let mut entries = 0;
        while let Some(dir) = dirs.pop() {
            let children = match fs::read_dir(dir) {
                Ok(children) => children,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            for entry in children {
                let path = entry?.path();
                // Compiler/browser caches rename and remove temporary files.
                // A disappeared entry contributes zero to this sample; other
                // observation errors still stop the execution.
                let m = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(error.into()),
                };
                entries += 1;
                if m.is_dir() {
                    dirs.push(path);
                } else {
                    bytes = bytes.saturating_add(m.len());
                }
                ensure!(
                    entries <= 16384 && bytes <= 256 * 1024 * 1024,
                    "Scratch resource bound exceeded"
                );
            }
        }
        Ok((rss, bytes))
    }
    fn run(
        mut command: Command,
        scratch: &Path,
        seconds: u64,
        cap: usize,
    ) -> Result<(Phase, Vec<u8>, Vec<u8>)> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: child-only, async-signal-safe session/resource operations.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for (resource, value) in [
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_CPU, 45),
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
        let start = Instant::now();
        let mut child = Process(command.spawn()?);
        let watchdog = Watchdog::new(child.0.id() as i32)?;
        let stdout = reader(child.0.stdout.take().unwrap(), cap);
        let stderr = reader(child.0.stderr.take().unwrap(), 16384);
        let mut limit = None;
        let mut peaks = (0, 0);
        let mut samples = 0;
        let mut last = Instant::now();
        let status = loop {
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(seconds) {
                limit = Some("wall_time_limit".into());
            }
            if last.elapsed() > Duration::from_millis(100) {
                match usage(scratch, child.0.id()) {
                    Ok((rss, bytes)) => {
                        samples += 1;
                        peaks = (peaks.0.max(rss), peaks.1.max(bytes));
                        if rss > 3 * 1024 * 1024 * 1024 {
                            limit = Some("sampled_process_group_rss_limit".into());
                        }
                    }
                    Err(_) => {
                        limit = Some("resource_observation_unavailable_or_scratch_limit".into())
                    }
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
        drop(watchdog);
        let (out, ot) = stdout
            .join()
            .map_err(|_| anyhow::anyhow!("SDK stdout reader failed"))?;
        let (err, et) = stderr
            .join()
            .map_err(|_| anyhow::anyhow!("SDK stderr reader failed"))?;
        if ot || et {
            limit = Some("bounded_output_exceeded".into());
        }
        Ok((
            Phase {
                passed: status.success() && limit.is_none(),
                exit_code: status.code(),
                limit,
                elapsed_ms: start.elapsed().as_millis() as u64,
                sampled_process_group_rss_bytes: (samples > 0).then_some(peaks.0),
                sampled_scratch_bytes: (samples > 0).then_some(peaks.1),
                resource_samples: samples,
            },
            out,
            err,
        ))
    }
    fn node(snapshot: &Snapshot, scratch: &Path) -> Command {
        let mut command = Command::new(&snapshot.sdk.node);
        command
            .current_dir(scratch)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("OPENSSL_CONF", "/dev/null")
            .env("TMPDIR", scratch)
            .env("NODE_OPTIONS", "--max-old-space-size=1536");
        // Forward the real existing home unchanged, solely for OS user lookup.
        // The compiler/browser profiles deny owner-home data reads.
        if let Some(home) = std::env::var_os("HOME") {
            command.env("HOME", home);
        }
        command
    }
    fn literal(path: &Path) -> String {
        serde_json::to_string(path.to_str().unwrap()).unwrap()
    }
    fn build_profile(sdk: &HostSdk, app: &Path, inputs: &[PathBuf]) -> Result<String> {
        let esbuild = sdk
            .angular
            .join("@esbuild/darwin-arm64/bin/esbuild")
            .canonicalize()?;
        ensure!(
            esbuild.starts_with(&sdk.angular),
            "esbuild escapes frozen SDK"
        );
        let mut profile=format!("(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow file-read* (literal \"/\")(subpath \"/System/Library\")(subpath \"/usr/lib\")(subpath \"/opt/homebrew/Cellar\")(subpath \"/private/var/db/dyld\")(subpath {})(subpath {})(literal \"/dev/null\")(literal \"/dev/random\")(literal \"/dev/urandom\"))(allow file-write* (subpath {})(literal \"/dev/null\"))(allow process-fork)(allow process-exec (literal {})(literal {}))(allow signal (target same-sandbox))(deny process-info*)(allow process-info* (target self))(deny sysctl-read (sysctl-name-prefix \"kern.procargs\"))",literal(&sdk.angular),literal(app),literal(app),literal(&sdk.node),literal(&esbuild));
        let sdk_parent = sdk
            .angular
            .parent()
            .context("SDK installation parent missing")?;
        profile += &format!(
            "(allow file-read* (literal {})(literal {})(literal {}))",
            literal(sdk_parent),
            literal(&sdk_parent.join("package.json")),
            literal(&sdk_parent.join("package-lock.json"))
        );
        for path in inputs {
            profile += &format!("(deny file-write* (literal {}))", literal(path));
        }
        Ok(profile)
    }
    fn assets(root: &Path) -> Result<(Vec<Value>, String)> {
        let mut dirs = vec![root.to_path_buf()];
        let mut found = BTreeMap::new();
        let mut total = 0;
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                let m = fs::symlink_metadata(&path)?;
                ensure!(
                    !m.file_type().is_symlink(),
                    "Compiled asset symlink unsupported"
                );
                if m.is_dir() {
                    dirs.push(path);
                    continue;
                }
                ensure!(m.is_file(), "Special compiled asset unsupported");
                total += m.len();
                ensure!(
                    total <= 16 * 1024 * 1024 && found.len() < 64,
                    "Compiled asset count/byte bound exceeded"
                );
                let text = fs::read_to_string(&path)
                    .context("Only bounded UTF-8 browser assets supported")?;
                let relative = path
                    .strip_prefix(root)?
                    .to_str()
                    .context("Asset path must be UTF-8")?
                    .to_string();
                ensure!(
                    crate::workspace_path::is_safe_relative_path(&relative),
                    "Unsafe asset name"
                );
                let media = match path.extension().and_then(|e| e.to_str()) {
                    Some("js") => "text/javascript",
                    Some("css") => "text/css",
                    Some("html") => "text/html",
                    Some("svg") => "image/svg+xml",
                    _ => anyhow::bail!("Unsupported compiled asset type"),
                };
                found.insert(
                    format!("/{relative}"),
                    json!({"text":text,"media_type":media}),
                );
            }
        }
        ensure!(
            found.contains_key("/index.html") && found.len() >= 2,
            "Compiled index and application assets required"
        );
        let digest = crate::digest(&serde_json::to_vec(
            &found
                .iter()
                .map(|(p, v)| (p, crate::digest(v["text"].as_str().unwrap().as_bytes())))
                .collect::<BTreeMap<_, _>>(),
        )?);
        Ok((
            found
                .into_iter()
                .map(|(path, v)| json!({"path":path,"text":v["text"],"media_type":v["media_type"]}))
                .collect(),
            digest,
        ))
    }
    pub(super) fn evaluate(snapshot: &Snapshot) -> Result<Evaluation> {
        let start = Instant::now();
        ensure!(
            Path::new("/usr/bin/sandbox-exec").is_file(),
            "Seatbelt unavailable; no fallback"
        );
        snapshot.sdk.verify(&snapshot.contract.sdk)?;
        let path = std::env::temp_dir().join(format!("gents-angular-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        let scratch = Scratch(path.canonicalize()?);
        let app = scratch.0.join("app");
        fs::create_dir(&app)?;
        let base = Path::new(&snapshot.contract.project_path).parent().unwrap();
        for (path, text) in &snapshot.files {
            write(
                &app.join(Path::new(path).strip_prefix(base)?),
                text.as_bytes(),
            )?;
        }
        write(
            &app.join(".config/angular/config.json"),
            br#"{"version":1,"cli":{"analytics":false}}"#,
        )?;
        std::os::unix::fs::symlink(&snapshot.sdk.angular, app.join("node_modules"))?;
        let mut version = node(snapshot, &app);
        version.arg("--version");
        let (v, out, _) = run(version, &scratch.0, 3, 64)?;
        ensure!(
            v.passed && String::from_utf8(out)?.trim() == snapshot.contract.sdk.node_version,
            "Installed Node runtime version differs from frozen pin"
        );
        let inputs = snapshot
            .files
            .keys()
            .map(|p| Ok(app.join(Path::new(p).strip_prefix(base)?)))
            .collect::<Result<Vec<_>>>()?;
        let profile = build_profile(&snapshot.sdk, &app, &inputs)?;
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args(["-p", &profile])
            .arg(&snapshot.sdk.node)
            .arg(snapshot.sdk.angular.join("@angular/cli/bin/ng.js"))
            .args([
                "build",
                &snapshot.contract.project_name,
                "--configuration=development",
                "--progress=false",
            ])
            .current_dir(&app)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("TMPDIR", &app)
            .env("XDG_CONFIG_HOME", app.join(".config"))
            .env("NODE_COMPILE_CACHE", app.join(".node-cache"))
            .env("NODE_OPTIONS", "--max-old-space-size=1536")
            .env("CI", "1")
            .env("NG_CLI_ANALYTICS", "false")
            .env("NG_BUILD_MAX_WORKERS", "1")
            .env("OPENSSL_CONF", "/dev/null");
        if let Some(home) = std::env::var_os("HOME") {
            command.env("HOME", home);
        }
        let (compile, out, err) = run(command, &scratch.0, 45, 16384)?;
        let stdout = String::from_utf8_lossy(&out).into_owned();
        let stderr = String::from_utf8_lossy(&err).into_owned();
        crate::safety::require_persistable(&stdout)?;
        crate::safety::require_persistable(&stderr)?;
        let mut browser = None;
        let mut artifact = None;
        if compile.passed {
            let (assets, hash) = assets(&app.join("dist/browser"))?;
            artifact = Some(hash);
            let profile = scratch.0.join("browser-profile");
            fs::create_dir(&profile)?;
            write(&scratch.0.join("oracle.mjs"), include_bytes!("browser.mjs"))?;
            write(
                &scratch.0.join("browser_profile.mjs"),
                include_bytes!("browser_profile.mjs"),
            )?;
            write(
                &scratch.0.join("oracle.json"),
                &serde_json::to_vec(
                    &json!({"driver":snapshot.sdk.driver,"browser":snapshot.sdk.browser,"profile":profile,"assets":assets,"contract":snapshot.contract.browser}),
                )?,
            )?;
            let mut command = node(snapshot, &scratch.0);
            command
                .arg(scratch.0.join("oracle.mjs"))
                .arg(scratch.0.join("oracle.json"));
            let (phase, out, _) = run(command, &scratch.0, 45, 160 * 1024)?;
            if phase.passed {
                let text = std::str::from_utf8(&out)?;
                crate::safety::require_persistable(text)?;
                let raw: RawBrowser = serde_json::from_value(crate::strict_json::parse(text)?)?;
                ensure!(
                    raw.schema_version == 1
                        && raw.steps.len() <= snapshot.contract.browser.cases.len()
                        && raw.held_loopback_families == ["ipv4", "ipv6"]
                        && raw.outer_seatbelt
                        && !raw.chromium_inner_sandbox,
                    "Invalid host browser report"
                );
                let mut steps = vec![];
                for (index, expected) in snapshot.contract.browser.cases.iter().enumerate() {
                    let actual = raw.steps.get(index);
                    let (mut passed, mut observed) = (false, None);
                    if let Some(actual) = actual {
                        let action = match expected.action {
                            BrowserAction::ExpectText => "expect_text",
                            BrowserAction::Click => "click",
                            BrowserAction::Fill => "fill",
                        };
                        ensure!(
                            actual.id == expected.id && actual.action == action,
                            "Browser step identity mismatch"
                        );
                        if matches!(expected.action, BrowserAction::ExpectText) {
                            ensure!(
                                actual.actual.as_ref().is_none_or(|s| s.len() <= 8192),
                                "DOM observation exceeds bound"
                            );
                            observed = actual.actual.as_ref().map(|s| crate::digest(s.as_bytes()));
                            passed = actual.completed && actual.actual == expected.value;
                        } else {
                            ensure!(actual.actual.is_none(), "Unexpected interaction text");
                            passed = actual.completed;
                        }
                    }
                    steps.push(BrowserStep {
                        id: expected.id.clone(),
                        status: if actual.is_none() {
                            "not_run"
                        } else if passed {
                            "passed"
                        } else {
                            "failed"
                        }
                        .into(),
                        actual_sha256: observed,
                        expected_sha256: expected
                            .value
                            .as_ref()
                            .filter(|_| matches!(expected.action, BrowserAction::ExpectText))
                            .map(|s| crate::digest(s.as_bytes())),
                    });
                }
                let passed = raw.started
                    && raw.owned_process_group
                    && raw.reason == "completed"
                    && steps.iter().all(|s| s.status == "passed");
                ensure!(
                    [
                        "completed",
                        "steps_incomplete",
                        "step_failed",
                        "browser_or_step_failed",
                        "startup_failed"
                    ]
                    .contains(&raw.reason.as_str()),
                    "Unknown browser reason"
                );
                browser = Some(BrowserReport {
                    passed,
                    started: raw.started,
                    held_loopback_families: raw.held_loopback_families,
                    reason: raw.reason,
                    steps,
                    blocked_requests: raw.blocked_requests,
                    outer_seatbelt: true,
                    chromium_inner_sandbox: false,
                    phase,
                });
            } else {
                browser = Some(BrowserReport {
                    passed: false,
                    started: false,
                    held_loopback_families: vec![],
                    reason: "host_process_failed_or_limited".into(),
                    steps: vec![],
                    blocked_requests: 0,
                    outer_seatbelt: true,
                    chromium_inner_sandbox: false,
                    phase,
                });
            }
        }
        let unchanged = snapshot.sdk.verify(&snapshot.contract.sdk).is_ok()
            && snapshot.files.iter().all(|(p, s)| {
                fs::read(app.join(Path::new(p).strip_prefix(base).unwrap()))
                    .is_ok_and(|bytes| bytes == s.as_bytes())
            });
        let passed = unchanged
            && compile.passed
            && artifact.is_some()
            && browser.as_ref().is_some_and(|b| b.passed);
        let result=Evaluation{status:if passed{"browser_passed"}else if !unchanged||compile.limit.is_some(){"indeterminate"}else{"check_failed"}.into(),contract_id:snapshot.contract.id.clone(),verification_identity:snapshot.identity.clone(),inputs_sha256:snapshot.hashes.clone(),sdk:snapshot.contract.sdk.clone(),sdk_metadata_seal:None,compile,compile_stdout:stdout,compile_stderr:stderr,artifact_sha256:artifact,browser,snapshot_unchanged:unchanged,elapsed_ms:start.elapsed().as_millis()as u64,model_calls:0,provider_cost_usd:0,qualified:false,promoted:false,authority_granted:false,scope:"Copied bounded standard Angular application build and finite owner-frozen DOM checks. Outer deny-default Seatbelt replaces Chromium's non-nestable inner sandbox; browser reads only system/browser SDK and its disposable profile, connects only to the host-held asset port, cannot bind TCP, and shares the watchdog-owned group. Compiler cannot access owner files/network. SDK trees and Node executable are hashed; system/Homebrew dynamic-library closure is not fully pinned. RSS/scratch measurements are sampled. No general correctness, expert qualification, company completion or promotion.".into()};
        crate::safety::require_persistable(&serde_json::to_string(&result)?)?;
        Ok(result)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[ignore = "explicit installed GENTS_NODE and GENTS_ANGULAR_MODULES; actual macOS isolation"]
        fn actual_compiler_profile_denies_owner_io_network_exec_and_frozen_writes() {
            let get = |key| {
                PathBuf::from(std::env::var_os(key).expect("explicit installed SDK required"))
                    .canonicalize()
                    .unwrap()
            };
            let sdk = HostSdk {
                node: get("GENTS_NODE"),
                angular: get("GENTS_ANGULAR_MODULES"),
                driver: PathBuf::new(),
                browser: PathBuf::new(),
            };
            let path = std::env::temp_dir().join(format!(
                "gents-angular-compiler-boundary-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).unwrap();
            let scratch = Scratch(path.canonicalize().unwrap());
            let app = scratch.0.join("app");
            fs::create_dir(&app).unwrap();
            let protected = app.join("frozen-input.ts");
            fs::write(&protected, "owned").unwrap();
            let outside = scratch.0.join("private-sentinel");
            fs::write(&outside, "de-identified").unwrap();
            let source = r#"const fs=require('node:fs'),net=require('node:net'),cp=require('node:child_process');
const [outside,frozen,allowed,sdk,port]=process.argv.slice(2);
function denied(path,flags){try{const fd=fs.openSync(path,flags);fs.closeSync(fd);return false;}catch(error){return ['EPERM','EACCES'].includes(error.code);}}
const result={outside_read_denied:denied(outside,'r'),outside_write_denied:denied(outside,'r+'),frozen_write_denied:denied(frozen,'r+'),sdk_write_denied:denied(sdk,'r+')};
fs.writeFileSync(allowed,'allowed');result.scratch_write_allowed=fs.readFileSync(allowed,'utf8')==='allowed';
const network=new Promise(resolve=>{const socket=net.connect(Number(port),'127.0.0.1');socket.setTimeout(500);socket.once('connect',()=>{socket.destroy();resolve(false);});socket.once('error',error=>resolve(['EPERM','EACCES'].includes(error.code)));socket.once('timeout',()=>{socket.destroy();resolve(false);});});
const shell=new Promise(resolve=>{try{const child=cp.spawn('/bin/sh',['-c','exit 0']);child.once('error',error=>resolve(['EPERM','EACCES'].includes(error.code)));child.once('exit',()=>resolve(false));}catch(error){resolve(['EPERM','EACCES'].includes(error.code));}});
Promise.all([network,shell]).then(([network,shell])=>{result.loopback_connect_denied=network;result.shell_exec_denied=shell;process.stdout.write(JSON.stringify(result));});"#;
            let script = app.join("probe.cjs");
            fs::write(&script, source).unwrap();
            let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let profile = build_profile(&sdk, &app, std::slice::from_ref(&protected)).unwrap();
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command
                .args(["-p", &profile])
                .arg(&sdk.node)
                .arg(script)
                .arg(outside)
                .arg(protected)
                .arg(app.join("allowed-file"))
                .arg(sdk.angular.join("@angular/cli/package.json"))
                .arg(listener.local_addr().unwrap().port().to_string())
                .current_dir(&app)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("OPENSSL_CONF", "/dev/null");
            let (phase, stdout, stderr) = run(command, &scratch.0, 5, 4096).unwrap();
            assert!(phase.passed, "{}", String::from_utf8_lossy(&stderr));
            let checks: Value = serde_json::from_slice(&stdout).unwrap();
            assert_eq!(checks.as_object().unwrap().len(), 7);
            assert!(
                checks.as_object().unwrap().values().all(|v| v == true),
                "{checks}"
            );
            if let Some(path) = std::env::var_os("GENTS_ANGULAR_COMPILER_REPORT") {
                fs::write(path,serde_json::to_vec_pretty(&json!({"scope":"actual compiler profile, installed Node, finite host-held loopback and de-identified fixtures","checks":checks,"phase":phase,"provider_cost_usd":0,"model_calls":0})).unwrap()).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_metadata_seal_detects_equal_size_edits_with_restored_mtime() {
        let root = std::env::temp_dir().join(format!("gents-sdk-seal-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("sdk.js");
        fs::write(&path, b"one").unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let before = metadata_tree(&root).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(&path, b"two").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_ne!(metadata_tree(&root).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn tree_identity_checks_contents_permissions_and_symlink_confinement() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = std::env::temp_dir().join(format!("gents-sdk-hash-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("input");
        fs::write(&path, b"one").unwrap();
        let first = tree_hash(&root).unwrap();
        fs::write(&path, b"two").unwrap();
        assert_ne!(first, tree_hash(&root).unwrap());
        let second = tree_hash(&root).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(second, tree_hash(&root).unwrap());
        symlink("input", root.join("internal")).unwrap();
        assert!(tree_hash(&root).is_ok());
        symlink("/dev/null", root.join("external")).unwrap();
        assert!(tree_hash(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
