//! Trusted launching-host selection only. Model arguments never select code.
//! Checks precede path-based spawn; this is not atomic executable attestation
//! against a privileged same-user process that can rewrite read-only files.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};
const ROLES: [&str; 4] = [
    "gents-harness",
    "gents-local-mcp",
    "gents-sqlite-bridge",
    "gents-rust-acceptance-check",
];
const MAX_BINARY: u64 = 512 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Executable {
    file: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    id: String,
    executables: BTreeMap<String, Executable>,
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[cfg(unix)]
fn owned(meta: &Metadata, directory: bool, sealed: bool) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    ensure!(
        if directory {
            meta.is_dir()
        } else {
            meta.is_file()
        },
        "runtime bundle requires ordinary paths"
    );
    // SAFETY: geteuid has no memory or environment access and returns this process identity.
    ensure!(
        meta.uid() == unsafe { libc::geteuid() }
            && meta.mode() & 0o022 == 0
            && (!sealed || (meta.mode() & 0o222 == 0 && (directory || meta.nlink() == 1))),
        "runtime bundle path ownership/permissions changed"
    );
    Ok(())
}
#[cfg(not(unix))]
fn owned(_: &Metadata, _: bool, _: bool) -> Result<()> {
    anyhow::bail!("runtime bundle owner/ACL adapter unavailable on this platform")
}
fn ordinary(path: &Path, directory: bool, sealed: bool) -> Result<Metadata> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        !meta.file_type().is_symlink(),
        "runtime bundle symlinks forbidden"
    );
    owned(&meta, directory, sealed)?;
    Ok(meta)
}
fn unchanged(before: &Metadata, after: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.len() == after.len()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        before.len() == after.len() && before.modified().ok() == after.modified().ok()
    }
}
fn open(path: &Path, max: u64) -> Result<(File, Metadata)> {
    let observed = ordinary(path, false, true)?;
    ensure!(
        observed.len() > 0 && observed.len() <= max,
        "runtime bundle file outside size bound"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let before = file.metadata()?;
    ensure!(
        unchanged(&observed, &before),
        "runtime bundle file changed before reading"
    );
    owned(&before, false, true)?;
    Ok((file, before))
}
fn filename(role: &str) -> String {
    format!("{role}{}", std::env::consts::EXE_SUFFIX)
}
pub fn mcp_binary(root: &Path) -> Result<PathBuf> {
    let path = std::env::var_os("GENTS_RUNTIME_MANIFEST");
    let digest = std::env::var_os("GENTS_RUNTIME_MANIFEST_SHA256");
    select(
        root,
        path.as_ref()
            .map(|p| p.to_str().context("runtime manifest path must be UTF-8"))
            .transpose()?,
        digest
            .as_ref()
            .map(|p| p.to_str().context("runtime manifest hash must be UTF-8"))
            .transpose()?,
    )
}
fn select(root: &Path, path: Option<&str>, digest: Option<&str>) -> Result<PathBuf> {
    let (path,digest)=match(path,digest){(None,None)=>return Ok(root.join("target/debug").join(filename("gents-local-mcp"))),(Some(p),Some(d))=>(p,d),_=>anyhow::bail!("runtime manifest and expected digest must both be present; no build-directory fallback")};
    let parts = path.split('/').collect::<Vec<_>>();
    ensure!(
        parts.len() == 4
            && parts[0] == "data"
            && parts[1] == "mcp-runtime-bundles"
            && hash(parts[2])
            && parts[3] == "manifest.json"
            && hash(digest),
        "invalid trusted runtime manifest identity"
    );
    let root = root.canonicalize()?;
    ordinary(&root, true, false)?;
    let mut folder = root.clone();
    for (i, part) in parts[..3].iter().enumerate() {
        folder.push(part);
        ordinary(&folder, true, i == 2)?;
    }
    let manifest_path = folder.join("manifest.json");
    let (mut file, before) = open(&manifest_path, 16384)?;
    let mut bytes = Vec::new();
    (&mut file).take(16385).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == before.len()
            && unchanged(&before, &file.metadata()?)
            && unchanged(&before, &ordinary(&manifest_path, false, true)?)
            && crate::digest(&bytes) == digest,
        "runtime manifest changed or its digest differs from launch identity"
    );
    let manifest: Manifest =
        serde_json::from_slice(&bytes).context("invalid private runtime manifest")?;
    ensure!(
        manifest.schema_version == 1
            && manifest.id == parts[2]
            && manifest.executables.len() == ROLES.len(),
        "unsupported native runtime bundle"
    );
    for role in ROLES {
        let entry = manifest
            .executables
            .get(role)
            .context("native runtime role missing")?;
        ensure!(
            entry.file == filename(role)
                && entry.bytes > 0
                && entry.bytes <= MAX_BINARY
                && hash(&entry.sha256),
            "invalid native runtime executable identity"
        );
    }
    let entry = &manifest.executables["gents-local-mcp"];
    let binary = folder.join(&entry.file);
    let (mut file, before) = open(&binary, MAX_BINARY)?;
    ensure!(
        before.len() == entry.bytes,
        "MCP runtime size differs from frozen manifest"
    );
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        ensure!(size <= entry.bytes, "MCP runtime grew while hashing");
        hash.update(&buffer[..n]);
    }
    ensure!(
        size == entry.bytes
            && format!("{:x}", hash.finalize()) == entry.sha256
            && unchanged(&before, &file.metadata()?)
            && unchanged(&before, &ordinary(&binary, false, true)?),
        "MCP runtime bytes changed; refuse launch without fallback"
    );
    Ok(binary)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    struct Fixture {
        root: PathBuf,
        path: String,
        sha: String,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("gents-bundle-{}", uuid::Uuid::new_v4()));
            let id = "a".repeat(64);
            let path = format!("data/mcp-runtime-bundles/{id}/manifest.json");
            let folder = root.join(&path).parent().unwrap().to_path_buf();
            fs::create_dir_all(&folder).unwrap();
            let mut entries = serde_json::Map::new();
            for role in ROLES {
                let file = filename(role);
                fs::write(folder.join(&file), b"owned executable fixture").unwrap();
                fs::set_permissions(folder.join(&file), fs::Permissions::from_mode(0o500)).unwrap();
                entries.insert(role.into(),json!({"file":file,"bytes":24,"sha256":crate::digest(b"owned executable fixture")}));
            }
            let bytes = json!({"schema_version":1,"id":id,"executables":entries}).to_string();
            fs::write(root.join(&path), &bytes).unwrap();
            fs::set_permissions(root.join(&path), fs::Permissions::from_mode(0o400)).unwrap();
            fs::set_permissions(folder, fs::Permissions::from_mode(0o500)).unwrap();
            Self {
                root,
                path,
                sha: crate::digest(bytes.as_bytes()),
            }
        }
        fn folder(&self) -> PathBuf {
            self.root.join(&self.path).parent().unwrap().into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::set_permissions(self.folder(), fs::Permissions::from_mode(0o700)).unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn selected_copy_survives_build_directory_replacement() {
        let f = Fixture::new();
        let selected = select(&f.root, Some(&f.path), Some(&f.sha)).unwrap();
        fs::create_dir_all(f.root.join("target/debug")).unwrap();
        fs::write(f.root.join("target/debug/gents-local-mcp"), b"new build").unwrap();
        assert_eq!(fs::read(&selected).unwrap(), b"owned executable fixture");
        assert_eq!(
            select(&f.root, Some(&f.path), Some(&f.sha)).unwrap(),
            selected
        );
    }
    #[test]
    fn tampered_bytes_manifest_and_partial_configuration_never_fall_back() {
        let f = Fixture::new();
        assert!(select(&f.root, Some(&f.path), None).is_err());
        assert!(select(&f.root, None, Some(&f.sha)).is_err());
        assert!(select(&f.root, Some(&f.path), Some(&"b".repeat(64))).is_err());
        let binary = f.folder().join("gents-local-mcp");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&binary, b"changed executable bytes").unwrap();
        fs::set_permissions(binary, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(select(&f.root, Some(&f.path), Some(&f.sha)).is_err());
    }
    #[test]
    fn symlink_and_writable_copies_are_rejected() {
        let f = Fixture::new();
        let binary = f.folder().join("gents-local-mcp");
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(select(&f.root, Some(&f.path), Some(&f.sha)).is_err());
        fs::set_permissions(f.folder(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(&binary).unwrap();
        std::os::unix::fs::symlink(f.folder().join("gents-harness"), &binary).unwrap();
        fs::set_permissions(f.folder(), fs::Permissions::from_mode(0o500)).unwrap();
        assert!(select(&f.root, Some(&f.path), Some(&f.sha)).is_err());
    }
    #[test]
    fn invalid_namespace_and_legacy_selection_stay_distinct() {
        let f = Fixture::new();
        for path in [
            "/tmp/manifest.json",
            "data/mcp-runtime-bundles/../manifest.json",
            "data/other/manifest.json",
        ] {
            assert!(select(&f.root, Some(path), Some(&f.sha)).is_err());
        }
        assert_eq!(
            select(&f.root, None, None).unwrap(),
            f.root.join("target/debug/gents-local-mcp")
        );
    }
}
