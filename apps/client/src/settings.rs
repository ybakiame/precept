//! The connection kept between launches, in a private per-user directory:
//! `%LOCALAPPDATA%\ACP-state\connection.json` on Windows (outside the portable
//! product directory), `$XDG_CONFIG_HOME/acp/connection.json` elsewhere.
//!
//! Only the target and its non-secret coordinates are stored (`Saved`); never a
//! credential, Profile path or preference value. The directory and file must be
//! private (acp-ipc's owner-only checks); anything else refuses rather than
//! loosening permissions or following a link.
use acp_ipc::transport::{new_private, open_private, private_dir};
use acp_operator_core::connection::{Connection, Saved};
use std::io::{Read, Write};
use std::path::PathBuf;

pub const FILE: &str = "connection.json";
pub const NOT_PRIVATE: &str = "Settings directory is not private";
pub const UNAVAILABLE: &str = "Settings directory unavailable";

/// The private directory holding the saved connection.
pub fn directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|p| p.join("ACP-state"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|h| h.join(".config"))
            })
            .map(|p| p.join("acp"))
    }
}

/// The saved connection, if one exists, is private, parses and is still valid.
/// Absence and any refusal both yield `None`: the app then starts unconnected.
pub fn load() -> Option<Connection> {
    load_from(&directory()?)
}
pub fn load_from(dir: &std::path::Path) -> Option<Connection> {
    private_dir(dir, false).ok()?;
    let mut file = open_private(&dir.join(FILE), false).ok()?;
    let mut text = String::new();
    Read::by_ref(&mut file)
        .take(16 * 1024)
        .read_to_string(&mut text)
        .ok()?;
    serde_json::from_str::<Saved>(&text).ok()?.connection()
}

/// Saves `connection` (creating the private directory if absent). Written to a
/// new private file, then renamed over the old one.
pub fn save(connection: &Connection) -> Result<(), &'static str> {
    save_to(&directory().ok_or(UNAVAILABLE)?, connection)
}
pub fn save_to(dir: &std::path::Path, connection: &Connection) -> Result<(), &'static str> {
    if !connection.validate() {
        return Err("Invalid connection inputs");
    }
    if let Some(parent) = dir.parent()
        && !parent.is_dir()
    {
        return Err(UNAVAILABLE);
    }
    private_dir(dir, true).map_err(|_| NOT_PRIVATE)?;
    let body = serde_json::to_vec_pretty(&Saved::new(connection)).map_err(|_| UNAVAILABLE)?;
    let temporary = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    let written = new_private(&temporary)
        .map_err(|_| NOT_PRIVATE)
        .and_then(|mut f| {
            f.write_all(&body)
                .and_then(|_| f.sync_all())
                .map_err(|_| UNAVAILABLE)
        });
    if let Err(e) =
        written.and_then(|_| std::fs::rename(&temporary, dir.join(FILE)).map_err(|_| UNAVAILABLE))
    {
        let _ = std::fs::remove_file(&temporary);
        return Err(e);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use acp_operator_core::connection::Target;
    use std::os::unix::fs::PermissionsExt;
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn wsl() -> Connection {
        Connection {
            target: Target::Wsl,
            distribution: "archlinux".into(),
            user: "operator".into(),
            helper: "/usr/bin/acp-wsl-bridge".into(),
            wsl_runtime: "/home/operator/.local/state/acp/runtime".into(),
            wsl_endpoint: "/home/operator/.local/state/acp/runtime/control.sock".into(),
            ..Default::default()
        }
    }
    #[test]
    fn saves_privately_reloads_and_replaces() {
        let root = scratch("save");
        let dir = root.join("acp");
        save_to(&dir, &wsl()).unwrap();
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join(FILE)), 0o600);
        assert_eq!(load_from(&dir), Some(wsl()));
        let mut other = wsl();
        other.distribution = "Ubuntu-24.04".into();
        save_to(&dir, &other).unwrap();
        assert_eq!(load_from(&dir), Some(other));
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temporary left"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn refuses_shared_or_invalid_files_instead_of_loosening() {
        let root = scratch("refuse");
        let dir = root.join("acp");
        save_to(&dir, &wsl()).unwrap();
        std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(load_from(&dir), None, "group/world readable file refused");
        std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(load_from(&dir), None, "shared directory refused");
        assert_eq!(save_to(&dir, &wsl()), Err(NOT_PRIVATE));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(
            dir.join(FILE),
            r#"{"format":1,"connection":{"target":"wsl"}}"#,
        )
        .unwrap();
        std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(load_from(&dir), None, "invalid saved connection refused");
        let mut bad = wsl();
        bad.wsl_runtime = "relative".into();
        assert!(save_to(&dir, &bad).is_err());
        assert_eq!(save_to(&root.join("missing/acp"), &wsl()), Err(UNAVAILABLE));
        let _ = std::fs::remove_dir_all(&root);
    }
}
