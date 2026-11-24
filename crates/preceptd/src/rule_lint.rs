//! Project-bound, redacted repository-rule diagnostics for the Operator.

use crate::profiles::Profiles;
use precept_ipc::{Access, Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

const MAX_REPORT_BYTES: usize = 768 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum RootIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u32, index: u64 },
    #[cfg(not(any(unix, windows)))]
    Path(PathBuf),
}

fn root_identity(root: &std::path::Path) -> Result<(PathBuf, RootIdentity)> {
    let canonical = root.canonicalize().map_err(|_| Error::UnsafeRuntime)?;
    if canonical != root || !canonical.is_dir() {
        return Err(Error::UnsafeRuntime);
    }
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| Error::UnsafeRuntime)?;
    if !metadata.is_dir() {
        return Err(Error::UnsafeRuntime);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return Ok((
            canonical,
            RootIdentity::Unix {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        };
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            GetFileInformationByHandle,
        };
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(Error::UnsafeRuntime);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&canonical)
            .map_err(|_| Error::UnsafeRuntime)?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `file` owns the directory handle and `info` is writable for the call.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(Error::UnsafeRuntime);
        }
        Ok((
            canonical,
            RootIdentity::Windows {
                volume: info.dwVolumeSerialNumber,
                index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            },
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Ok((canonical, RootIdentity::Path(canonical)))
    }
}

fn bounded(value: Value) -> Result<Value> {
    if serde_json::to_vec(&value)
        .map_err(|_| Error::Unavailable)?
        .len()
        > MAX_REPORT_BYTES
    {
        return Err(Error::TooLarge);
    }
    Ok(value)
}

pub(super) fn report(access: Access, params: Value, profiles: &Profiles) -> Result<Value> {
    if access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Lint {
        project: String,
    }
    let p: Lint = serde_json::from_value(params).map_err(|_| Error::InvalidRequest)?;
    if !profiles.allowed(&p.project) {
        return Err(Error::NotFound);
    }
    let (_, roots) = profiles.read()?;
    let root = roots.get(&p.project).ok_or(Error::NotFound)?;
    let (pinned, before) = root_identity(root)?;
    let report = precept_core::contracts::Snapshot::read(&pinned).lint();
    let (_, after) = root_identity(root)?;
    if before != after {
        return Err(Error::UnsafeRuntime);
    }
    bounded(serde_json::to_value(report).map_err(|_| Error::Unavailable)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn report_reply_is_bounded_before_ipc_framing() {
        let value = json!({"padding":"x".repeat(MAX_REPORT_BYTES)});
        assert_eq!(bounded(value), Err(Error::TooLarge));
        assert_eq!(bounded(json!({"ok":true})), Ok(json!({"ok":true})));
    }

    #[test]
    fn root_identity_changes_when_the_directory_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let (_, before) = root_identity(&root).unwrap();
        let moved = temp.path().join("moved");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        let (_, after) = root_identity(&root).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn root_identity_refuses_a_link_substitution() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&elsewhere).unwrap();
        let root = root.canonicalize().unwrap();
        root_identity(&root).unwrap();
        #[cfg(unix)]
        {
            let moved = temp.path().join("moved");
            std::fs::rename(&root, &moved).unwrap();
            std::os::unix::fs::symlink(&moved, &root).unwrap();
            assert_eq!(root_identity(&root), Err(Error::UnsafeRuntime));
        }
        #[cfg(windows)]
        {
            let moved = temp.path().join("moved");
            std::fs::rename(&root, &moved).unwrap();
            let status = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "& { param([string]$link,[string]$target) New-Item -ItemType Junction -Path $link -Target $target | Out-Null }",
                    &root.to_string_lossy(),
                    &elsewhere.to_string_lossy(),
                ])
                .status()
                .unwrap();
            if !status.success() {
                panic!("Windows junction fixture failed");
            }
            assert_eq!(root_identity(&root), Err(Error::UnsafeRuntime));
        }
        #[cfg(not(any(unix, windows)))]
        let _ = elsewhere;
    }
}
