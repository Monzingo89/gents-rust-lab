//! Descriptor-relative filesystem adapter. Lexical validation is not authority.
//! No path component is followed through a symlink; writes replace one file
//! atomically, not an existing inode (including a hard-linked outside inode).
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;
pub trait Workspace {
    fn read(&self, path: &str, offset: usize, limit: usize) -> Result<Value>;
    /// Whole-file read with an allocation/I/O ceiling checked before reading.
    fn read_complete(&self, path: &str, limit: usize) -> Result<Value>;
    /// Complete confined binary input for owned document/media adapters. Never
    /// exposes bytes as an agent capability by itself; callers bound/parse them.
    fn read_bytes_complete(&self, path: &str, limit: usize) -> Result<Vec<u8>>;
    /// Metadata-only identity check for case/normalization aliases of frozen
    /// owner files. Confined descriptors, no content or implicit read grant.
    fn same_file(&self, left: &str, right: &str) -> Result<bool>;
    fn write(&self, path: &str, content: &str) -> Result<Value>;
    /// Create a new bounded binary artifact; never replace an existing name.
    fn create_bytes(&self, _path: &str, _content: &[u8]) -> Result<Value> {
        bail!("binary artifact creation unavailable on this workspace adapter")
    }
    /// Hash precondition plus atomic replacement. Registry serializes MCP
    /// writers; privileged, noncooperating host writes are outside that lock.
    fn write_if(&self, path: &str, content: &str, expected: Option<&str>) -> Result<Value> {
        if let Some(expected) = expected {
            anyhow::ensure!(
                expected.len() == 64
                    && expected
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "invalid expected_sha256"
            );
            // Paged reads fingerprint the complete bounded file; do not request
            // a 32 KiB page from the 8 KiB page API or expose its content here.
            let current = self.read(path, 0, 4)?;
            anyhow::ensure!(
                current["sha256"] == expected,
                "expected_sha256 does not match current file; no replacement performed"
            );
        }
        self.write(path, content)
    }
    fn list(&self, path: &str) -> Result<Value>;
}
pub fn open(path: &Path, create: bool) -> Result<Box<dyn Workspace>> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        Ok(Box::new(unix::Directory::open(path, create)?))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (path, create);
        bail!("native workspace adapter requires macOS or Linux");
    }
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix {
    use super::*;
    use crate::{digest, workspace_path::is_safe_relative_path};
    use serde_json::json;
    use std::{
        ffi::{CStr, CString},
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::{MetadataExt, OpenOptionsExt},
        },
        path::Component,
    };
    const MAX_FILE: usize = 16 * 1024 * 1024;
    fn errno() -> *mut libc::c_int {
        // SAFETY: these platform APIs return this thread's live errno slot.
        #[cfg(target_os = "macos")]
        {
            unsafe { libc::__error() }
        }
        #[cfg(target_os = "linux")]
        {
            unsafe { libc::__errno_location() }
        }
    }
    pub struct Directory {
        fd: File,
    }
    fn name(value: &str) -> Result<CString> {
        Ok(CString::new(value)?)
    }
    fn open_at(parent: &File, value: &CStr, flags: i32) -> Result<File> {
        // SAFETY: the borrowed descriptor is live, the CString is NUL-terminated,
        // and a successful new descriptor is immediately owned by File exactly once.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                value.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    fn descend(parent: &File, part: &str, create: bool) -> Result<File> {
        let part = name(part)?;
        if create {
            // SAFETY: live descriptor and valid NUL-terminated single component.
            if unsafe { libc::mkdirat(parent.as_raw_fd(), part.as_ptr(), 0o700) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error.into());
                }
            }
        }
        open_at(parent, &part, libc::O_RDONLY | libc::O_DIRECTORY)
    }
    fn parts(path: &str) -> Result<Vec<&str>> {
        if !is_safe_relative_path(path) {
            bail!("invalid workspace relative path");
        }
        Ok(path.split('/').collect())
    }
    impl Directory {
        pub fn open(path: &Path, create: bool) -> Result<Self> {
            if !path.is_absolute() {
                bail!("workspace root must be absolute");
            }
            let mut fd = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open("/")?;
            for component in path.components() {
                match component {
                    Component::RootDir => {}
                    Component::Normal(part) => {
                        fd = descend(
                            &fd,
                            part.to_str().context("workspace directory is not UTF-8")?,
                            create,
                        )?;
                    }
                    _ => bail!("invalid workspace root"),
                }
            }
            Ok(Self { fd })
        }
        fn directory(&self, parts: &[&str], create: bool) -> Result<File> {
            let mut fd = self.fd.try_clone()?;
            for part in parts {
                fd = descend(&fd, part, create)?;
            }
            Ok(fd)
        }
        fn parent(&self, path: &str, create: bool) -> Result<(File, CString)> {
            let parts = parts(path)?;
            Ok((
                self.directory(&parts[..parts.len() - 1], create)?,
                name(parts[parts.len() - 1])?,
            ))
        }
    }
    impl Directory {
        fn identity(&self, path: &str) -> Result<Option<(u64, u64)>> {
            let result = (|| {
                let (parent, base) = self.parent(path, false)?;
                let file = open_at(&parent, &base, libc::O_RDONLY | libc::O_NONBLOCK)?;
                let meta = file.metadata()?;
                anyhow::ensure!(
                    meta.is_file() && meta.nlink() == 1,
                    "file identity requires a single-link regular file"
                );
                Ok((meta.dev(), meta.ino()))
            })();
            match result {
                Ok(value) => Ok(Some(value)),
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                {
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }
        fn bytes_bounded(&self, path: &str, limit: usize) -> Result<Vec<u8>> {
            anyhow::ensure!(
                (1..=MAX_FILE).contains(&limit),
                "invalid complete-file byte limit"
            );
            let (parent, base) = self.parent(path, false)?;
            let file = open_at(&parent, &base, libc::O_RDONLY | libc::O_NONBLOCK)?;
            let before = file.metadata()?;
            anyhow::ensure!(
                before.is_file() && before.nlink() == 1 && before.len() <= limit as u64,
                "complete read requires a bounded single-link regular file up to {limit} bytes"
            );
            let mut bytes = Vec::new();
            (&file).take((limit + 1) as u64).read_to_end(&mut bytes)?;
            let after = file.metadata()?;
            anyhow::ensure!(
                bytes.len() <= limit
                    && bytes.len() as u64 == before.len()
                    && before.dev() == after.dev()
                    && before.ino() == after.ino()
                    && before.len() == after.len()
                    && before.mtime() == after.mtime()
                    && before.mtime_nsec() == after.mtime_nsec()
                    && before.ctime() == after.ctime()
                    && before.ctime_nsec() == after.ctime_nsec(),
                "file changed during complete read"
            );
            Ok(bytes)
        }
        fn read_bounded(
            &self,
            path: &str,
            offset: usize,
            limit: usize,
            file_limit: usize,
        ) -> Result<Value> {
            if !(1..=8192).contains(&limit) {
                bail!("read pages must be 1..8192 bytes");
            }
            let bytes = self.bytes_bounded(path, file_limit)?;
            let body = std::str::from_utf8(&bytes)
                .context("use a document adapter for non-UTF-8 files")?;
            crate::safety::require_persistable(body)?;
            if offset > body.len() || !body.is_char_boundary(offset) {
                bail!("offset must be a UTF-8 boundary inside this file");
            }
            let mut end = offset.saturating_add(limit).min(body.len());
            while !body.is_char_boundary(end) {
                end -= 1;
            }
            if end == offset && offset < body.len() {
                bail!("page limit is smaller than the next UTF-8 character");
            }
            Ok(
                json!({"path":path,"text":&body[offset..end],"bytes":end-offset,"total_bytes":body.len(),"sha256":digest(&bytes),"offset_bytes":offset,"truncated":end<body.len(),"next_offset_bytes":if end<body.len(){Some(end)}else{None}}),
            )
        }
    }
    impl Workspace for Directory {
        fn create_bytes(&self, path: &str, content: &[u8]) -> Result<Value> {
            anyhow::ensure!(
                !content.is_empty() && content.len() <= 8 * 1024 * 1024,
                "binary artifact size exceeds budget"
            );
            let (parent, base) = self.parent(path, true)?;
            let temporary = name(&format!(".mcp-{}.tmp", uuid::Uuid::new_v4()))?;
            let result = (|| -> Result<()> {
                let mut file = open_at(
                    &parent,
                    &temporary,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                )?;
                file.write_all(content)?;
                file.sync_all()?;
                // Atomic no-replace publication. A symlink/existing target fails;
                // no model-controlled path is ever passed to an image/audio codec.
                if unsafe {
                    libc::linkat(
                        parent.as_raw_fd(),
                        temporary.as_ptr(),
                        parent.as_raw_fd(),
                        base.as_ptr(),
                        0,
                    )
                } != 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                parent.sync_all()?;
                Ok(())
            })();
            // Remove only this call's random temporary directory entry.
            unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) };
            result?;
            Ok(
                json!({"path":path,"bytes":content.len(),"sha256":digest(content),"created_new":true,"retry_rule":"Read the retained path/hash before retrying an uncertain effect; no automatic overwrite or replay."}),
            )
        }
        fn read(&self, path: &str, offset: usize, limit: usize) -> Result<Value> {
            self.read_bounded(path, offset, limit, MAX_FILE)
        }
        fn read_complete(&self, path: &str, limit: usize) -> Result<Value> {
            self.read_bounded(path, 0, limit, limit)
        }
        fn read_bytes_complete(&self, path: &str, limit: usize) -> Result<Vec<u8>> {
            self.bytes_bounded(path, limit)
        }
        fn same_file(&self, left: &str, right: &str) -> Result<bool> {
            let Some(left) = self.identity(left)? else {
                return Ok(false);
            };
            Ok(self.identity(right)? == Some(left))
        }
        fn write(&self, path: &str, content: &str) -> Result<Value> {
            if content.len() > 32768 {
                bail!("native writes are bounded to 32768 UTF-8 bytes");
            }
            crate::safety::require_persistable(content)?;
            let (parent, base) = self.parent(path, true)?;
            // Check an existing target without following it. Atomic rename below never
            // writes through that target, even if its name changes after this check.
            match open_at(&parent, &base, libc::O_RDONLY | libc::O_NONBLOCK) {
                Ok(file) => {
                    let m = file.metadata()?;
                    if !m.is_file() || m.nlink() != 1 {
                        bail!("write target is not a single-link regular file");
                    }
                }
                Err(error) => {
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_none_or(|e| e.kind() != std::io::ErrorKind::NotFound)
                    {
                        return Err(error);
                    }
                }
            }
            let temporary = name(&format!(".mcp-{}.tmp", uuid::Uuid::new_v4()))?;
            let result = (|| -> Result<()> {
                let mut file = open_at(
                    &parent,
                    &temporary,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                )?;
                file.write_all(content.as_bytes())?;
                file.sync_all()?;
                // SAFETY: both names are single components and parent is live and owned.
                if unsafe {
                    libc::renameat(
                        parent.as_raw_fd(),
                        temporary.as_ptr(),
                        parent.as_raw_fd(),
                        base.as_ptr(),
                    )
                } != 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                parent.sync_all()?;
                Ok(())
            })();
            if result.is_err() {
                // SAFETY: remove only this call's exclusive random temporary filename.
                unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) };
            }
            result?;
            Ok(
                json!({"path":path,"bytes":content.len(),"sha256":digest(content.as_bytes()),"atomic_replace":true,"retry_rule":"If the caller loses the receipt, inspect this path and hash before retrying; file replacement and SQLite audit are not one distributed transaction."}),
            )
        }
        fn list(&self, path: &str) -> Result<Value> {
            let components = if path.is_empty() || path == "." {
                Vec::new()
            } else {
                parts(path)?
            };
            let directory = self.directory(&components, false)?;
            // fdopendir takes ownership only of a duplicated descriptor, never ours.
            let duplicate = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
            if duplicate < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let stream = unsafe { libc::fdopendir(duplicate) };
            if stream.is_null() {
                unsafe { libc::close(duplicate) };
                return Err(std::io::Error::last_os_error().into());
            }
            struct Stream(*mut libc::DIR);
            impl Drop for Stream {
                fn drop(&mut self) {
                    unsafe { libc::closedir(self.0) };
                }
            }
            let stream = Stream(stream);
            let mut entries = Vec::new();
            let mut scanned = 0;
            loop {
                // SAFETY: stream is live; copy the returned entry name before the next call.
                // Clear thread-local errno so NULL distinguishes EOF from I/O failure.
                unsafe {
                    *errno() = 0;
                }
                let entry = unsafe { libc::readdir(stream.0) };
                if entry.is_null() {
                    if unsafe { *errno() } != 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                    break;
                }
                scanned += 1;
                if scanned > 2048 {
                    bail!("directory scan exceeds bounded workspace inventory");
                }
                let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
                let Ok(child) = std::str::from_utf8(raw.to_bytes()) else {
                    continue;
                };
                if !is_safe_relative_path(child) {
                    continue;
                }
                let Ok(file) = open_at(&directory, raw, libc::O_RDONLY | libc::O_NONBLOCK) else {
                    continue;
                };
                let meta = file.metadata()?;
                if !meta.is_dir() && (!meta.is_file() || meta.nlink() != 1) {
                    continue;
                }
                entries.push(json!({"name":child,"path":if path.is_empty()||path=="."{child.to_string()}else{format!("{path}/{child}")},"type":if meta.is_dir(){"directory"}else{"file"},"bytes":meta.len()}));
            }
            entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
            let truncated = entries.len() > 128;
            entries.truncate(128);
            Ok(
                json!({"files":entries,"truncated":truncated,"limit":128,"filtered":"hidden names, symlinks, special files and multiple-link files are excluded"}),
            )
        }
    }
}
