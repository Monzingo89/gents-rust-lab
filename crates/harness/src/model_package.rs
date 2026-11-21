//! Immutable identity for local MLX base/adapter packages. No weight loading,
//! conversion, downloads or execution of package-supplied code.
use crate::model_artifact::metadata_identity;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
};
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Model,
    Adapter,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FilePin {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub kind: Kind,
    pub files: BTreeMap<String, FilePin>,
    pub sha256: String,
    pub weight_bytes: u64,
    pub total_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub fingerprint: Value,
    pub weight_bytes: u64,
    pub total_bytes: u64,
    files: BTreeMap<String, u64>,
}
pub fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1000
        && !path.contains('\\')
        && Path::new(path).components().count() <= 16
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn secure_metadata(path: &Path, owner: &fs::Metadata) -> Result<fs::Metadata> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(!meta.is_symlink(), "package symlink rejected");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            meta.uid() == owner.uid() && meta.mode() & 0o022 == 0,
            "package owner or writable permissions rejected"
        );
        if meta.is_file() {
            ensure!(meta.nlink() == 1, "package hard link rejected");
        }
    }
    ensure!(
        meta.is_file() || meta.is_dir(),
        "package special file rejected"
    );
    Ok(meta)
}
fn documentation(name: &str) -> bool {
    name.ends_with(".md")
        || matches!(
            name,
            "LICENSE" | "LICENSE.txt" | "NOTICE" | ".gitattributes"
        )
}
pub fn snapshot(root: &Path, relative: &str, kind: Kind) -> Result<Snapshot> {
    ensure!(safe_relative(relative), "confined package path required");
    let owner = fs::symlink_metadata(root)?;
    ensure!(
        owner.is_dir() && !owner.is_symlink(),
        "package root rejected"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(owner.mode() & 0o022 == 0, "writable package root rejected");
    }
    let mut path = root.to_path_buf();
    let mut identities = serde_json::Map::new();
    for component in Path::new(relative).components() {
        path.push(component);
        let meta = secure_metadata(&path, &owner)?;
        ensure!(meta.is_dir(), "model package path is not a directory");
        identities.insert(
            path.to_string_lossy().into_owned(),
            metadata_identity(&meta),
        );
    }
    let mut files = BTreeMap::new();
    let mut pending = vec![(path.clone(), String::new(), 0)];
    let mut scanned = 0;
    while let Some((directory, prefix, depth)) = pending.pop() {
        ensure!(depth <= 8, "package depth bound exceeded");
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            scanned += 1;
            ensure!(scanned <= 1024, "package inventory bound exceeded");
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("package filename must be UTF-8"))?;
            // These are not inference inputs to the installed local loader.
            if matches!(name.as_str(), ".cache" | ".git") || documentation(&name) {
                continue;
            }
            ensure!(
                !name.starts_with('.')
                    && !name.ends_with(".py")
                    && !name.ends_with(".pyc")
                    && !name.ends_with(".pkl")
                    && !name.ends_with(".pickle")
                    && !name.ends_with(".bin"),
                "executable/hidden/pickle package input unsupported"
            );
            let relative_name = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            ensure!(safe_relative(&relative_name), "invalid package input name");
            let meta = secure_metadata(&entry.path(), &owner)?;
            identities.insert(relative_name.clone(), metadata_identity(&meta));
            if meta.is_dir() {
                pending.push((entry.path(), relative_name, depth + 1));
            } else {
                ensure!(
                    meta.len() > 0 && meta.len() <= 128 * 1024 * 1024 * 1024,
                    "empty or oversized package input"
                );
                files.insert(relative_name, meta.len());
                ensure!(files.len() <= 512, "package file bound exceeded");
            }
        }
    }
    let required = match kind {
        Kind::Model => "config.json",
        Kind::Adapter => "adapter_config.json",
    };
    ensure!(
        files.contains_key(required),
        "required package configuration missing"
    );
    ensure!(
        files.keys().any(|p| p.ends_with(".safetensors")),
        "safetensors weights missing"
    );
    if kind == Kind::Model {
        ensure!(
            files.contains_key("tokenizer.json")
                || files.contains_key("tokenizer.model")
                || files.contains_key("tiktoken.model"),
            "local tokenizer missing"
        );
    }
    if kind == Kind::Model {
        for name in ["config.json", "tokenizer_config.json"] {
            if files.contains_key(name) {
                ensure!(files[name] <= 1024 * 1024, "configuration exceeds bound");
                let value: Value =
                    serde_json::from_slice(&fs::read(root.join(relative).join(name))?)?;
                ensure!(
                    value
                        .get("auto_map")
                        .is_none_or(|v| v.is_null() || v.as_object().is_some_and(|o| o.is_empty())),
                    "custom remote-code package mapping unsupported"
                );
            }
        }
    }
    let total_bytes = files.values().try_fold(0u64, |sum, n| {
        sum.checked_add(*n).context("package byte count overflow")
    })?;
    ensure!(
        total_bytes <= 128 * 1024 * 1024 * 1024,
        "package total size bound exceeded"
    );
    let weight_bytes = files
        .iter()
        .filter(|(p, _)| p.ends_with(".safetensors"))
        .map(|(_, n)| n)
        .sum();
    Ok(Snapshot {
        fingerprint: json!({"version":1,"kind":kind,"root":root,"package":relative,"identities":identities}),
        files,
        weight_bytes,
        total_bytes,
    })
}
pub fn inspect(root: &Path, relative: &str, kind: Kind) -> Result<Manifest> {
    let before = snapshot(root, relative, kind)?;
    let mut files = BTreeMap::new();
    for (name, length) in &before.files {
        let path = root.join(relative).join(name);
        let mut file = fs::File::open(&path)?;
        let descriptor_before = metadata_identity(&file.metadata()?);
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut bytes = 0u64;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            ensure!(bytes <= *length, "package input changed during hashing");
            hash.update(&buffer[..n]);
        }
        ensure!(
            bytes == *length
                && metadata_identity(&file.metadata()?) == descriptor_before
                && metadata_identity(&fs::symlink_metadata(&path)?) == descriptor_before,
            "package input replaced during hashing"
        );
        files.insert(
            name.clone(),
            FilePin {
                bytes,
                sha256: format!("{:x}", hash.finalize()),
            },
        );
    }
    ensure!(
        snapshot(root, relative, kind)?.fingerprint == before.fingerprint,
        "package inventory changed during hashing"
    );
    // This is a versioned inference-input digest. Documentation/download cache
    // is excluded explicitly, not silently presented as an all-directory hash.
    let sha256 = gents_local_mcp::digest(&serde_json::to_vec(
        &json!({"schema_version":1,"kind":kind,"files":files}),
    )?);
    Ok(Manifest {
        schema_version: 1,
        kind,
        files,
        sha256,
        weight_bytes: before.weight_bytes,
        total_bytes: before.total_bytes,
    })
}
pub fn verify(root: &Path, relative: &str, kind: Kind, expected: &str) -> Result<Snapshot> {
    ensure!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "lowercase package SHA-256 required"
    );
    let before = snapshot(root, relative, kind)?;
    ensure!(
        inspect(root, relative, kind)?.sha256 == expected,
        "model package pin mismatch"
    );
    let after = snapshot(root, relative, kind)?;
    ensure!(
        after.fingerprint == before.fingerprint,
        "package changed while verifying"
    );
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Root(std::path::PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn root() -> Root {
        let p = std::env::temp_dir().join(format!("gents-package-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Root(p.canonicalize().unwrap())
    }
    fn base(root: &Path) -> String {
        let relative = "models/base";
        let p = root.join(relative);
        fs::create_dir_all(&p).unwrap();
        fs::write(
            p.join("config.json"),
            r#"{"model_type":"fixture","max_position_embeddings":4096}"#,
        )
        .unwrap();
        fs::write(p.join("tokenizer.json"), "{}").unwrap();
        fs::write(
            p.join("model.safetensors"),
            b"synthetic tensor identity, not serveable weights",
        )
        .unwrap();
        relative.into()
    }
    #[test]
    fn package_digest_covers_weights_config_tokenizer_and_exact_file_set() {
        let root = root();
        let p = base(&root.0);
        let first = inspect(&root.0, &p, Kind::Model).unwrap();
        verify(&root.0, &p, Kind::Model, &first.sha256).unwrap();
        assert_eq!(first.files.len(), 3);
        for name in ["config.json", "tokenizer.json", "model.safetensors"] {
            let path = root.0.join(&p).join(name);
            let original = fs::read(&path).unwrap();
            fs::write(
                &path,
                if name.ends_with("json") {
                    b"{\"changed\":true}".as_slice()
                } else {
                    b"other tensor"
                },
            )
            .unwrap();
            assert!(verify(&root.0, &p, Kind::Model, &first.sha256).is_err());
            fs::write(path, original).unwrap();
        }
        fs::write(root.0.join(&p).join("new.json"), "{}").unwrap();
        assert!(verify(&root.0, &p, Kind::Model, &first.sha256).is_err());
    }
    #[test]
    fn docs_and_download_cache_are_explicitly_outside_inference_digest() {
        let root = root();
        let p = base(&root.0);
        let first = inspect(&root.0, &p, Kind::Model).unwrap();
        fs::write(root.0.join(&p).join("README.md"), "changed documentation").unwrap();
        fs::create_dir_all(root.0.join(&p).join(".cache/download")).unwrap();
        fs::write(
            root.0.join(&p).join(".cache/download/meta"),
            "cache metadata",
        )
        .unwrap();
        assert_eq!(
            inspect(&root.0, &p, Kind::Model).unwrap().sha256,
            first.sha256
        );
    }
    #[test]
    fn custom_code_mappings_and_executable_package_inputs_are_unavailable() {
        let root = root();
        let p = base(&root.0);
        fs::write(
            root.0.join(&p).join("config.json"),
            r#"{"auto_map":{"AutoModel":"remote.Code"}}"#,
        )
        .unwrap();
        assert!(snapshot(&root.0, &p, Kind::Model).is_err());
        fs::write(root.0.join(&p).join("config.json"), "{}").unwrap();
        fs::write(root.0.join(&p).join("model.py"), "unexecuted code").unwrap();
        assert!(inspect(&root.0, &p, Kind::Model).is_err());
        assert!(snapshot(&root.0, "models/../../outside", Kind::Model).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_hardlink_and_writable_inputs_are_rejected() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let root = root();
        let p = base(&root.0);
        let target = root.0.join(&p).join("model.safetensors");
        let link = root.0.join(&p).join("alias.safetensors");
        symlink(&target, &link).unwrap();
        assert!(snapshot(&root.0, &p, Kind::Model).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&target, &link).unwrap();
        assert!(snapshot(&root.0, &p, Kind::Model).is_err());
        fs::remove_file(&link).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(snapshot(&root.0, &p, Kind::Model).is_err());
    }
    #[tokio::test]
    async fn adapter_pins_are_company_bound_and_changes_invalidate_cached_pin() {
        let root = root();
        let p = base(&root.0);
        let adapter = "data/companies/one/training/runs/frozen/adapters";
        fs::create_dir_all(root.0.join(adapter)).unwrap();
        fs::write(root.0.join(adapter).join("adapter_config.json"), "{}").unwrap();
        fs::write(
            root.0.join(adapter).join("adapters.safetensors"),
            b"unexecuted synthetic adapter",
        )
        .unwrap();
        let sha = inspect(&root.0, &p, Kind::Model).unwrap().sha256;
        let adapter_sha = inspect(&root.0, adapter, Kind::Adapter).unwrap().sha256;
        let model: crate::provider::Model = serde_json::from_value(
            json!({"provider":"mlx","model":"synthetic","path":p,"adapter":adapter}),
        )
        .unwrap();
        model.validate().unwrap();
        let service = crate::model_pins::ModelPins::default();
        service
            .verify(
                &root.0,
                &model,
                Some(sha.as_str()),
                Some(adapter_sha.as_str()),
                "one",
            )
            .await
            .unwrap();
        assert!(service
            .verify(
                &root.0,
                &model,
                Some(sha.as_str()),
                Some(adapter_sha.as_str()),
                "two"
            )
            .await
            .is_err());
        fs::write(
            root.0.join(adapter).join("adapters.safetensors"),
            b"changed synthetic adapter",
        )
        .unwrap();
        assert!(service
            .verify(
                &root.0,
                &model,
                Some(sha.as_str()),
                Some(adapter_sha.as_str()),
                "one"
            )
            .await
            .is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn warm_gguf_pin_never_bypasses_changed_root_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = root();
        fs::create_dir_all(root.0.join("models/fixture")).unwrap();
        let bytes = b"Synthetic GGUF identity bytes; not a loaded model";
        fs::write(root.0.join("models/fixture/model.gguf"), bytes).unwrap();
        let model: crate::provider::Model = serde_json::from_value(
            json!({"provider":"local","model":"synthetic","path":"models/fixture/model.gguf"}),
        )
        .unwrap();
        let sha = gents_local_mcp::digest(bytes);
        let service = crate::model_pins::ModelPins::default();
        service
            .verify(&root.0, &model, Some(sha.as_str()), None, "one")
            .await
            .unwrap();
        let original = fs::metadata(&root.0).unwrap().permissions();
        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o777)).unwrap();
        let denied = service
            .verify(&root.0, &model, Some(sha.as_str()), None, "one")
            .await
            .is_err();
        fs::set_permissions(&root.0, original).unwrap();
        assert!(
            denied,
            "a warm pin must recheck its root authorization boundary"
        );
    }
}
