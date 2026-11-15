//! Read-only identity checks for owner-managed local model artifacts. No model selection or execution.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};

pub fn relative_path(path: &str) -> bool {
    path.starts_with("models/")
        && path.ends_with(".gguf")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !path.contains('\\')
}

// Descriptor/hash checks reduce accidental replacement; a privileged same-user
// process is outside this boundary and may still replace bytes before loading.
pub fn verify(
    root: &Path,
    relative: &str,
    expected_sha256: &str,
) -> std::result::Result<u64, &'static str> {
    if !relative_path(relative) {
        return Err("model_path_rejected");
    }
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("model_pin_rejected");
    }
    let root_meta = fs::symlink_metadata(root).map_err(|_| "model_root_unavailable")?;
    if !root_meta.is_dir() || root_meta.is_symlink() {
        return Err("model_root_rejected");
    }
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        path.push(part);
        let meta = fs::symlink_metadata(&path).map_err(|_| "model_not_installed_or_unreadable")?;
        if meta.is_symlink() {
            return Err("model_symlink_rejected");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != root_meta.uid()
                || meta.mode() & 0o022 != 0
                || root_meta.mode() & 0o022 != 0
            {
                return Err("model_owner_or_directory_permissions_rejected");
            }
        }
    }
    let mut file = fs::File::open(&path).map_err(|_| "model_unreadable")?;
    let before = file.metadata().map_err(|_| "model_metadata_unavailable")?;
    if !before.is_file() || before.len() == 0 || before.len() > 128 * 1024 * 1024 * 1024 {
        return Err("model_not_bounded_regular_file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.nlink() != 1 || before.mode() & 0o022 != 0 {
            return Err("model_links_or_permissions_rejected");
        }
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|_| "model_read_failed")?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > before.len() {
            return Err("model_changed_during_inspection");
        }
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(|_| "model_metadata_unavailable")?;
    let named = fs::symlink_metadata(&path).map_err(|_| "model_changed_during_inspection")?;
    if read != before.len()
        || after.len() != before.len()
        || named.is_symlink()
        || after.modified().ok() != before.modified().ok()
    {
        return Err("model_changed_during_inspection");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if named.dev() != before.dev()
            || named.ino() != before.ino()
            || after.ctime() != before.ctime()
            || after.ctime_nsec() != before.ctime_nsec()
        {
            return Err("model_changed_during_inspection");
        }
    }
    if format!("{:x}", hash.finalize()) != expected_sha256 {
        return Err("model_hash_pin_mismatch");
    }
    Ok(read)
}
