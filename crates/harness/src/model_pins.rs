//! Shared read-only pin admission for local file and MLX package models.
//! No workflow settings, model selection, inference, downloads or grants.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct ModelPins {
    pins: Mutex<HashMap<(PathBuf, String, String), Value>>,
}
impl ModelPins {
    pub async fn verify(
        &self,
        root: &Path,
        model: &crate::provider::Model,
        model_sha256: Option<&str>,
        adapter_sha256: Option<&str>,
        company: &str,
    ) -> Result<()> {
        ensure!(
            matches!(model.provider.as_str(), "local" | "mlx" | "mock"),
            "pin verification requires owned local inference"
        );
        model.validate()?;
        ensure!(
            model.adapter.is_some() == adapter_sha256.is_some(),
            "adapter path and SHA-256 must be supplied together"
        );
        if model.provider == "mock" {
            return Ok(());
        }
        if model.provider == "mlx" {
            let relative = model.path.as_deref().context("MLX path missing")?;
            ensure!(
                relative.starts_with("models/") && crate::model_package::safe_relative(relative),
                "confined MLX package required"
            );
            self.verify_package(
                root,
                relative,
                crate::model_package::Kind::Model,
                model_sha256.context("MLX model pin missing")?,
            )
            .await?;
            if let Some(adapter) = &model.adapter {
                crate::company::identifier(company)?;
                ensure!(
                    crate::model_package::safe_relative(adapter)
                        && adapter.starts_with(&format!("data/companies/{company}/training/runs/")),
                    "adapter outside this company's training root"
                );
                self.verify_package(
                    root,
                    adapter,
                    crate::model_package::Kind::Adapter,
                    adapter_sha256.context("adapter pin missing")?,
                )
                .await?;
            }
            return Ok(());
        }
        let relative = model.path.as_deref().context("native model path absent")?;
        ensure!(
            crate::model_artifact::relative_path(relative),
            "local model requires a confined models/ GGUF path"
        );
        let fingerprint = || fingerprint(root, relative);
        let key = (
            root.to_path_buf(),
            format!("gguf/{relative}"),
            model_sha256.context("native model pin missing")?.to_owned(),
        );
        let mut pins = self.pins.lock().await;
        let observed = fingerprint()?;
        if pins.get(&key) == Some(&observed) {
            return Ok(());
        }
        let (root, path, hash) = (
            root.to_path_buf(),
            relative.to_owned(),
            model_sha256.context("native model pin missing")?.to_owned(),
        );
        tokio::task::spawn_blocking(move || {
            crate::model_artifact::verify(&root, &path, &hash).map_err(anyhow::Error::msg)
        })
        .await??;
        let after = fingerprint()?;
        ensure!(after == observed, "model path changed during pin admission");
        pins.insert(key, after);
        Ok(())
    }
    async fn verify_package(
        &self,
        root: &Path,
        relative: &str,
        kind: crate::model_package::Kind,
        expected: &str,
    ) -> Result<()> {
        let before = crate::model_package::snapshot(root, relative, kind)?;
        let key = (
            root.to_path_buf(),
            format!("package/v1/{kind:?}/{relative}"),
            expected.to_owned(),
        );
        let mut pins = self.pins.lock().await;
        if pins.get(&key) == Some(&before.fingerprint) {
            return Ok(());
        }
        let (root, relative, expected) =
            (root.to_path_buf(), relative.to_owned(), expected.to_owned());
        let verified = tokio::task::spawn_blocking(move || {
            crate::model_package::verify(&root, &relative, kind, &expected)
        })
        .await??;
        ensure!(
            verified.fingerprint == before.fingerprint,
            "package changed during pin admission"
        );
        pins.insert(key, verified.fingerprint);
        Ok(())
    }
}

// Recheck security metadata even on a warm cache hit. In particular the root
// itself must not disappear behind a symlink or become group/world writable.
fn fingerprint(root: &Path, relative: &str) -> Result<Value> {
    let owner = std::fs::symlink_metadata(root)?;
    ensure!(owner.is_dir() && !owner.is_symlink(), "model root rejected");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(owner.mode() & 0o022 == 0, "writable model root rejected");
    }
    let mut path = root.to_path_buf();
    let mut parts = vec![];
    let target = root.join(relative);
    for part in Path::new(relative).components() {
        path.push(part);
        let info = std::fs::symlink_metadata(&path)?;
        ensure!(!info.is_symlink(), "model symlink rejected");
        ensure!(
            if path == target {
                info.is_file() && info.len() > 0 && info.len() <= 128 * 1024 * 1024 * 1024
            } else {
                info.is_dir()
            },
            "model requires bounded ordinary file and directory parents"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(
                info.uid() == owner.uid() && info.mode() & 0o022 == 0,
                "model owner or writable permissions rejected"
            );
        }
        parts.push(crate::model_artifact::metadata_identity(&info));
    }
    Ok(json!({"root":crate::model_artifact::metadata_identity(&owner),"parts":parts}))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Root, crate::provider::Model, String) {
        let root = Root(std::env::temp_dir().join(format!("gents-pins-{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(root.0.join("models/fixture")).unwrap();
        let bytes = b"unloaded synthetic GGUF identity";
        std::fs::write(root.0.join("models/fixture/model.gguf"), bytes).unwrap();
        let model=serde_json::from_value(json!({"provider":"local","model":"identity-only fixture","path":"models/fixture/model.gguf"})).unwrap();
        (root, model, gents_local_mcp::digest(bytes))
    }
    #[tokio::test]
    async fn cached_pin_rechecks_changed_bytes_and_expected_identity_without_inference() {
        let (root, model, sha) = fixture();
        let pins = ModelPins::default();
        pins.verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .unwrap();
        pins.verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .unwrap();
        assert!(pins
            .verify(&root.0, &model, None, None, "one")
            .await
            .is_err());
        assert!(pins
            .verify(&root.0, &model, Some(&"0".repeat(64)), None, "one")
            .await
            .is_err());
        std::fs::write(
            root.0.join("models/fixture/model.gguf"),
            b"changed unloaded GGUF identity",
        )
        .unwrap();
        assert!(pins
            .verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .is_err());
        let replacement = gents_local_mcp::digest(b"changed unloaded GGUF identity");
        pins.verify(&root.0, &model, Some(&replacement), None, "one")
            .await
            .unwrap();
        let paid:crate::provider::Model=serde_json::from_value(json!({"provider":"openrouter","model":"uninvoked","url":"https://openrouter.ai/api/v1/chat/completions","key_env":"UNREAD_CREDENTIAL"})).unwrap();
        assert!(pins
            .verify(&root.0, &paid, Some(&sha), None, "one")
            .await
            .unwrap_err()
            .to_string()
            .contains("owned local"));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn warm_root_symlink_file_links_and_special_files_are_unavailable() {
        use std::os::unix::fs::symlink;
        let (root, model, sha) = fixture();
        let pins = ModelPins::default();
        pins.verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .unwrap();
        let alias = root.0.with_extension("alias");
        symlink(&root.0, &alias).unwrap();
        let denied = pins
            .verify(&alias, &model, Some(&sha), None, "one")
            .await
            .is_err();
        std::fs::remove_file(alias).unwrap();
        assert!(denied);
        let (path, link) = (
            root.0.join("models/fixture/model.gguf"),
            root.0.join("models/fixture/linked.gguf"),
        );
        std::fs::hard_link(&path, &link).unwrap();
        assert!(pins
            .verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        // Test the pre-open guard first: a missing guard must fail the test,
        // rather than leave a test thread blocked opening an unserved FIFO.
        assert!(fingerprint(&root.0, "models/fixture/model.gguf").is_err());
        assert!(pins
            .verify(&root.0, &model, Some(&sha), None, "one")
            .await
            .is_err());
        assert!(crate::model_artifact::verify(&root.0, "models/fixture/model.gguf", &sha).is_err());
    }
}
