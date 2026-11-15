//! Owned media boundary: fixed offline engines, bounded process, confined publication.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::path::Path;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub fn result_schema(kind: &str) -> Value {
    let hash = json!({"type":"string","pattern":"^[a-f0-9]{64}$"});
    let mut schema = json!({"type":"object","additionalProperties":false,"required":["artifact","mime_type","engine","request_sha256","model_manifest_sha256","provider_cost_usd","semantic_acceptance"],"properties":{
        "artifact":{"type":"object","additionalProperties":false,"required":["path","bytes","sha256","created_new","retry_rule"],"properties":{"path":{"type":"string"},"bytes":{"type":"integer","minimum":1,"maximum":MAX_BYTES},"sha256":hash,"created_new":{"const":true},"retry_rule":{"type":"string"}}},
        "mime_type":{"const":if kind=="image"{"image/png"}else{"audio/wav"}},"engine":{"const":if kind=="image"{"mflux_flux2_klein_4bit"}else{"piper_ljspeech_cpu"}},"request_sha256":hash,"model_manifest_sha256":hash,"provider_cost_usd":{"const":0},"semantic_acceptance":{"const":false}
    }});
    let fields = if kind == "image" {
        vec![
            ("width", json!({"type":"integer","enum":[256,384,512]})),
            ("height", json!({"type":"integer","enum":[256,384,512]})),
        ]
    } else {
        vec![("audio", json!({"type":"object"}))]
    };
    for (field, value) in fields {
        schema["required"]
            .as_array_mut()
            .unwrap()
            .push(json!(field));
        schema["properties"][field] = value;
    }
    schema
}
/// Configuration discovery only; hashes and current capacity are checked on call.
pub fn capabilities() -> Value {
    let root = std::env::current_dir().unwrap_or_default();
    let config: Value =
        serde_json::from_slice(include_bytes!("../../config/media.local.json")).unwrap();
    json!([("image","image_generate"),("audio","audio_generate")].map(|(kind,tool)|{
        let profile=&config["profiles"][kind];
        json!({"modality":kind,"tool":tool,"configured":cfg!(target_os="macos") && root.join(config["python"].as_str().unwrap_or("")).is_file() && root.join(profile["root"].as_str().unwrap_or("")).is_dir(),"provider_cost_usd":0,"model_verified_on_call":true,"execution_authorized":false})
    }))
}
pub trait Backend {
    fn generate(&self, request: &Value) -> Result<Vec<u8>>;
}
pub struct LocalBackend;
impl Backend for LocalBackend {
    fn generate(&self, request: &Value) -> Result<Vec<u8>> {
        isolated::generate(request)
    }
}
pub fn publish(
    directory: &dyn crate::workspace::Workspace,
    request: &Value,
    backend: &dyn Backend,
) -> Result<Value> {
    let path = request["path"]
        .as_str()
        .context("media destination missing")?;
    ensure!(
        crate::workspace_path::is_safe_relative_path(path),
        "unsafe media destination"
    );
    let kind = request["kind"].as_str().context("media kind missing")?;
    ensure!(
        matches!(
            (kind, Path::new(path).extension().and_then(|x| x.to_str())),
            ("image", Some("png")) | ("audio", Some("wav"))
        ),
        "media destination type mismatch"
    );
    let bytes = backend.generate(request)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_BYTES,
        "media output exceeds byte budget"
    );
    let mut properties = match kind {
        "image" => {
            ensure!(
                bytes.len() >= 24
                    && &bytes[..8] == b"\x89PNG\r\n\x1a\n"
                    && &bytes[12..16] == b"IHDR",
                "media backend did not return PNG"
            );
            let width = u32::from_be_bytes(bytes[16..20].try_into()?);
            let height = u32::from_be_bytes(bytes[20..24].try_into()?);
            ensure!(
                Some(width as u64) == request["width"].as_u64()
                    && Some(height as u64) == request["height"].as_u64(),
                "image dimensions differ from approved request"
            );
            json!({"mime_type":"image/png","width":width,"height":height,"engine":"mflux_flux2_klein_4bit","semantic_acceptance":false})
        }
        "audio" => {
            let inspected = crate::audio::inspect_pcm16(&bytes)?;
            json!({"mime_type":"audio/wav","engine":"piper_ljspeech_cpu","audio":inspected,"semantic_acceptance":false})
        }
        _ => anyhow::bail!("unsupported media kind"),
    };
    properties["artifact"] = directory.create_bytes(path, &bytes)?;
    properties["request_sha256"] = json!(crate::digest(serde_json::to_string(request)?.as_bytes()));
    properties["provider_cost_usd"] = json!(0);
    properties["model_manifest_sha256"] = json!(crate::digest(include_bytes!(
        "../../config/media.local.json"
    )));
    Ok(properties)
}

#[cfg(not(target_os = "macos"))]
mod isolated {
    use super::*;
    pub fn generate(_: &Value) -> Result<Vec<u8>> {
        anyhow::bail!(
            "media process isolation currently requires macOS; no unsandboxed or paid fallback"
        )
    }
}
#[cfg(target_os = "macos")]
mod isolated {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        os::unix::{fs::PermissionsExt, process::CommandExt},
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct Child(std::process::Child, Option<std::process::Child>);
    impl Drop for Child {
        fn drop(&mut self) {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
            if let Some(watchdog) = self.1.as_mut() {
                let _ = watchdog.kill();
                let _ = watchdog.wait();
            }
        }
    }
    fn literal(path: &Path) -> String {
        serde_json::to_string(path.to_str().unwrap()).unwrap()
    }
    // Fixed stdlib-only host watchdog, not an inference process. EOF on its
    // private pipe means the MCP host died, even while a renderer holds its GIL.
    // No site packages, prompts, credentials, arbitrary targets or network.
    fn watchdog(python: &Path, group: u32, seconds: u32) -> Result<std::process::Child> {
        const PROGRAM: &str = r#"import os,select,signal,sys
sys.stdout.buffer.write(b'R');sys.stdout.buffer.flush()
ready,_,_=select.select([0],[],[],int(sys.argv[2]))
if not ready or os.read(0,1)!=b'D':
 try: os.killpg(int(sys.argv[1]),signal.SIGKILL)
 except ProcessLookupError: pass
"#;
        let mut child = Command::new(python)
            .args([
                "-I",
                "-S",
                "-c",
                PROGRAM,
                &group.to_string(),
                &seconds.to_string(),
            ])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdout = child
            .stdout
            .take()
            .context("media watchdog handshake missing")?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut byte = [0u8];
            let result = stdout.read_exact(&mut byte).map(|_| byte[0]);
            let _ = sender.send(result);
        });
        if !matches!(receiver.recv_timeout(Duration::from_secs(2)), Ok(Ok(b'R'))) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("media watchdog could not start; generation denied");
        }
        Ok(child)
    }
    fn local_path(root: &Path, relative: &str) -> Result<PathBuf> {
        ensure!(
            !Path::new(relative).is_absolute()
                && Path::new(relative)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "invalid media configuration path"
        );
        let mut path = root.to_path_buf();
        for part in Path::new(relative).components() {
            path.push(part);
            ensure!(!path.is_symlink(), "media configuration symlinks forbidden");
        }
        Ok(path)
    }
    pub fn generate(request: &Value) -> Result<Vec<u8>> {
        run_program(request, include_str!("../media/worker.py"))
    }
    fn run_program(request: &Value, program: &str) -> Result<Vec<u8>> {
        let root = std::env::current_dir()?.canonicalize()?;
        use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
        fs::create_dir_all(root.join("data"))?;
        let lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("data/media-inference.lock"))?;
        ensure!(
            unsafe { libc::flock(lease.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "local media engine is busy; no automatic retry"
        );
        let config: Value =
            serde_json::from_slice(include_bytes!("../../config/media.local.json"))?;
        let kind = request["kind"].as_str().context("media kind missing")?;
        ensure!(matches!(kind, "image" | "audio"), "unknown media kind");
        let python = root.join(
            config["python"]
                .as_str()
                .context("media interpreter missing")?,
        );
        let executable = python
            .canonicalize()
            .context("install the pinned isolated media environment first")?;
        let environment = python
            .parent()
            .and_then(Path::parent)
            .context("invalid media environment")?;
        ensure!(
            environment == root.join(".venv-media"),
            "unsupported media environment path"
        );
        let base = executable
            .parent()
            .and_then(Path::parent)
            .context("invalid Python base environment")?;
        let mut model = config["profiles"][kind].clone();
        let model_root = local_path(
            &root,
            model["root"].as_str().context("model directory missing")?,
        )?;
        ensure!(
            model_root.starts_with(root.join("models")) && model_root.is_dir(),
            "local media model missing"
        );
        model["root"] = json!(model_root);
        let memory = crate::host_memory::Snapshot::capture(&crate::host_memory::NativeMemoryProbe)?;
        ensure!(
            memory
                .effective_available_bytes
                .is_some_and(|available| available
                    >= if kind == "image" {
                        8 * 1024 * 1024 * 1024
                    } else {
                        512 * 1024 * 1024
                    }),
            "insufficient observed free memory for media; no fallback or guessed fit"
        );
        let scratch = Scratch(
            std::env::temp_dir()
                .canonicalize()?
                .join(format!("gents-media-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir(&scratch.0)?;
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700))?;
        let profile=format!("(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow mach-lookup)(allow iokit-open)(allow file-read* (literal \"/\") (subpath \"/System\") (subpath \"/usr/lib\") (subpath \"/private/var/db/dyld\") (literal \"/dev/null\") (literal \"/dev/random\") (literal \"/dev/urandom\") (subpath {}) (subpath {}) (subpath {}) (subpath {}))(allow file-write* (subpath {}) (literal \"/dev/null\"))(allow process-exec (literal {}) (literal {}))(allow signal (target same-sandbox))",literal(environment),literal(base),literal(&model_root),literal(&scratch.0),literal(&scratch.0),literal(&python),literal(&executable));
        // Apple's Metal compiler uses its OS-owned module cache even when
        // TMPDIR/HOME point to scratch. Permit only that compiler cache subtree.
        let profile = if kind == "image" {
            let cache = Command::new("/usr/bin/getconf")
                .arg("DARWIN_USER_CACHE_DIR")
                .env_clear()
                .output()?;
            ensure!(
                cache.status.success() && cache.stdout.len() <= 4096,
                "Metal cache location unavailable"
            );
            let cache_alias =
                PathBuf::from(std::str::from_utf8(&cache.stdout)?.trim()).join("com.apple.metalfe");
            let cache = PathBuf::from(std::str::from_utf8(&cache.stdout)?.trim())
                .canonicalize()?
                .join("com.apple.metalfe");
            ensure!(
                cache.starts_with("/private/var/folders"),
                "unexpected Metal compiler cache location"
            );
            format!(
                "{profile}(allow file-read* file-write* (subpath {}) (subpath {}))(allow file-issue-extension (require-all (extension-class \"com.apple.app-sandbox.read-write\") (require-any (subpath {}) (subpath {}))))",
                literal(&cache),
                literal(&cache_alias),
                literal(&cache),
                literal(&cache_alias)
            )
        } else {
            profile
        };
        let packet = serde_json::to_vec(&json!({"request":request,"profile":model}))?;
        ensure!(packet.len() <= 16384, "media input exceeds bounded packet");
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args(["-p", &profile])
            .arg(&python)
            .args(["-I", "-c", program])
            .current_dir(&scratch.0)
            .env_clear()
            .env("HOME", &scratch.0)
            .env("TMPDIR", &scratch.0)
            .env("HF_HOME", &scratch.0)
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_IMPLICIT_TOKEN", "1")
            .env("TOKENIZERS_PARALLELISM", "false")
            .env("OMP_NUM_THREADS", "2")
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                for (resource, value) in [
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_CPU, 150),
                    (libc::RLIMIT_FSIZE, 16 * 1024 * 1024),
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
        let mut child = Child(
            command
                .spawn()
                .context("isolated media process failed to start")?,
            None,
        );
        child.1 = Some(watchdog(&executable, child.0.id(), 160)?);
        child
            .0
            .stdin
            .take()
            .context("media stdin unavailable")?
            .write_all(&packet)?;
        let stdout = child.0.stdout.take().context("media stdout unavailable")?;
        let stderr = child
            .0
            .stderr
            .take()
            .context("media diagnostics unavailable")?;
        let diagnostics = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.take(16384).read_to_end(&mut bytes).map(|_| bytes)
        });
        let output = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take((MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(160),
                "local media deadline exceeded; inspect receipts before retrying"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        let bytes = output
            .join()
            .map_err(|_| anyhow::anyhow!("media output reader failed"))??;
        let diagnostic = diagnostics
            .join()
            .map_err(|_| anyhow::anyhow!("media diagnostic reader failed"))??;
        if !status.success() {
            let folder = root.join("data/media-diagnostics");
            fs::create_dir_all(&folder)?;
            let relative = format!("data/media-diagnostics/{}.log", uuid::Uuid::new_v4());
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(root.join(&relative))?;
            file.write_all(&diagnostic)?;
            anyhow::bail!("local media engine failed ({}); owner-only bounded diagnostic {}; no automatic retry",status,relative);
        }
        ensure!(
            status.success() && !bytes.is_empty() && bytes.len() <= MAX_BYTES,
            "local media engine rejected generation or exceeded output budget; no automatic retry"
        );
        Ok(bytes)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        #[ignore = "opt-in native watchdog probe; requires the installed media interpreter"]
        fn media_watchdog_eof_stops_the_owned_renderer() {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let python = root.join(".venv-media/bin/python").canonicalize().unwrap();
            let mut command = Command::new(&python);
            command
                .args(["-I", "-S", "-c", "import time;time.sleep(30)"])
                .env_clear()
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut renderer = Child(command.spawn().unwrap(), None);
            let mut guard = watchdog(&python, renderer.0.id(), 10).unwrap();
            // Closing this descriptor models an abruptly killed MCP parent.
            drop(guard.stdin.take());
            let start = Instant::now();
            loop {
                if let Some(status) = renderer.0.try_wait().unwrap() {
                    assert!(!status.success());
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(guard.wait().unwrap().success());
        }
        #[test]
        #[ignore = "opt-in native sandbox probe; requires the installed media interpreter"]
        fn media_sandbox_denies_private_files_network_and_shell() {
            std::env::set_current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
                .unwrap();
            let private = std::env::current_dir()
                .unwrap()
                .join(format!("data/media-boundary-{}.txt", uuid::Uuid::new_v4()));
            fs::write(&private, b"SYNTHETIC private boundary fixture").unwrap();
            let program = format!(
                r#"import json,os,socket,subprocess
from pathlib import Path
p=Path({})
result={{}}
for name,op in [('private_read',lambda:p.read_bytes()),('private_write',lambda:p.write_text('changed')),('network',lambda:socket.socket().connect(('127.0.0.1',9))),('shell',lambda:subprocess.run(['/bin/echo','forbidden'],check=True))]:
 try:
  op();result[name]=False
 except PermissionError:
  result[name]=True
result['credentials_absent']=all(k not in os.environ for k in ['OPENAI_API_KEY','RESEND_API_KEY','HF_TOKEN','LLAMA_API_KEY'])
print(json.dumps(result))
"#,
                serde_json::to_string(private.to_str().unwrap()).unwrap()
            );
            let bytes = run_program(&json!({"kind":"audio"}), &program).unwrap();
            let result: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                fs::read(&private).unwrap(),
                b"SYNTHETIC private boundary fixture"
            );
            fs::remove_file(private).unwrap();
            for key in [
                "private_read",
                "private_write",
                "network",
                "shell",
                "credentials_absent",
            ] {
                assert_eq!(result[key], true, "{key}");
            }
        }
    }
}
