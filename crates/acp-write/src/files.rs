//! Fresh-file reservation and no-replace publication; no profile discovery.
use crate::{Error, Result};
use std::{
    fs::{self, File},
    path::{Component, Path, PathBuf},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity(pub u64, pub u64);
fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::IoFailure),
    }
}
pub(crate) fn sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut n = path.as_os_str().to_owned();
        n.push(suffix);
        if exists(Path::new(&n))? {
            return Err(Error::Collision);
        }
    }
    Ok(())
}
pub(crate) fn live_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut n = path.as_os_str().to_owned();
        n.push(suffix);
        if exists(Path::new(&n))? {
            check(Path::new(&n), false, false)?;
        }
    }
    Ok(())
}
fn no_links(path: &Path) -> Result<()> {
    let mut p = PathBuf::new();
    for c in path.components() {
        if matches!(c, Component::ParentDir | Component::CurDir) {
            return Err(Error::UnsafeWorkspace);
        }
        p.push(c);
        // A Windows prefix alone (especially \\?\C:) is not an absolute
        // filesystem object. Inspect it only after the root component is added.
        if matches!(c, Component::Prefix(_)) {
            continue;
        }
        let m = fs::symlink_metadata(&p).map_err(|_| Error::UnsafeWorkspace)?;
        if m.file_type().is_symlink() {
            return Err(Error::UnsafeWorkspace);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err(Error::UnsafeWorkspace);
            }
        }
    }
    Ok(())
}
pub(crate) fn check_parent(path: &Path) -> Result<Identity> {
    no_links(path)?;
    check(path, true, true)
}
pub(crate) fn check_file(path: &Path) -> Result<Identity> {
    no_links(path)?;
    check(path, false, true)
}
pub(crate) fn destination(path: &Path) -> Result<(PathBuf, Identity)> {
    if !path.is_absolute() {
        return Err(Error::UnsafeWorkspace);
    }
    let name = path.file_name().ok_or(Error::UnsafeWorkspace)?;
    let name_str = name.to_str().ok_or(Error::UnsafeWorkspace)?;
    if [
        "daemon.lock",
        "daemon.sock",
        "operator.cap",
        "operator-write.cap",
    ]
    .contains(&name_str)
        || name_str.starts_with(".acp-create-")
        || name_str.ends_with("-wal")
        || name_str.ends_with("-shm")
        || name_str.ends_with("-journal")
    {
        return Err(Error::Collision);
    }
    #[cfg(windows)]
    if name_str.contains(':') || name_str.ends_with(['.', ' ']) {
        return Err(Error::UnsafeWorkspace);
    }
    let parent = path.parent().ok_or(Error::UnsafeWorkspace)?;
    let identity = check_parent(parent)?;
    for control in [
        "daemon.lock",
        "daemon.sock",
        "operator.cap",
        "operator-write.cap",
    ] {
        if exists(&parent.join(control))? {
            return Err(Error::Collision);
        }
    }
    if exists(path)? {
        return Err(Error::Collision);
    }
    sidecars(path)?;
    let parent = parent.canonicalize().map_err(|_| Error::UnsafeWorkspace)?;
    Ok((parent.join(name), identity))
}
fn check(path: &Path, directory: bool, _protected: bool) -> Result<Identity> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::UnsafeWorkspace)?;
    if m.file_type().is_symlink() || (if directory { !m.is_dir() } else { !m.is_file() }) {
        return Err(Error::UnsafeWorkspace);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
            || (!directory && m.nlink() != 1)
        {
            return Err(Error::UnsafeWorkspace);
        }
        Ok(Identity(m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        windows::check(path, directory, _protected)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::UnsafeWorkspace)
    }
}
pub(crate) fn create_private(path: &Path) -> Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::Collision
                } else {
                    Error::IoFailure
                }
            })
    }
    #[cfg(windows)]
    {
        windows::create(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::UnsafeWorkspace)
    }
}
pub(crate) fn publish(source: &Path, target: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let a = std::ffi::CString::new(source.as_os_str().as_bytes())
            .map_err(|_| Error::UnsafeWorkspace)?;
        let b = std::ffi::CString::new(target.as_os_str().as_bytes())
            .map_err(|_| Error::UnsafeWorkspace)?;
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                a.as_ptr(),
                libc::AT_FDCWD,
                b.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::AlreadyExists {
                    Error::Collision
                } else {
                    Error::IoFailure
                },
            );
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        windows::publish(source, target)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (source, target);
        Err(Error::UnsafeWorkspace)
    }
}
pub(crate) fn flush_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::IoFailure)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
#[cfg(test)]
pub(crate) fn private_test_parent(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    {
        windows::secure_directory(path).unwrap();
    }
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        os::windows::{
            ffi::OsStrExt,
            fs::OpenOptionsExt,
            io::{AsRawHandle, FromRawHandle},
        },
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::Threading::*,
    };
    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    struct Local(*mut std::ffi::c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn sid() -> Result<(Vec<u64>, PSID)> {
        unsafe {
            let mut token = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(Error::UnsafeWorkspace);
            }
            let mut n = 0;
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut n);
            let mut buf = vec![0u64; (n as usize).div_ceil(8)];
            let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), n, &mut n);
            CloseHandle(token);
            if ok == 0 {
                return Err(Error::UnsafeWorkspace);
            }
            let sid = (*(buf.as_ptr().cast::<TOKEN_USER>())).User.Sid;
            Ok((buf, sid))
        }
    }
    fn descriptor(directory: bool) -> Result<Local> {
        unsafe {
            let (_buf, sid) = sid()?;
            let mut s = ptr::null_mut();
            if ConvertSidToStringSidW(sid, &mut s) == 0 {
                return Err(Error::UnsafeWorkspace);
            }
            let _text = Local(s.cast());
            let mut n = 0;
            while *s.add(n) != 0 {
                n += 1
            }
            let sid = String::from_utf16(std::slice::from_raw_parts(s, n))
                .map_err(|_| Error::UnsafeWorkspace)?;
            let sddl = if directory {
                format!("O:{sid}D:P(A;OICI;FA;;;{sid})")
            } else {
                format!("O:{sid}D:P(A;;FA;;;{sid})")
            };
            let s: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut sd = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                s.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                ptr::null_mut(),
            ) == 0
            {
                return Err(Error::UnsafeWorkspace);
            }
            Ok(Local(sd))
        }
    }
    pub(super) fn create(path: &Path) -> Result<File> {
        unsafe {
            let sd = descriptor(false)?;
            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.0,
                bInheritHandle: 0,
            };
            let h = CreateFileW(
                wide(path).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                &sa,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            );
            if h == INVALID_HANDLE_VALUE {
                return Err(
                    if GetLastError() == ERROR_FILE_EXISTS || GetLastError() == ERROR_ALREADY_EXISTS
                    {
                        Error::Collision
                    } else {
                        Error::IoFailure
                    },
                );
            }
            Ok(File::from_raw_handle(h))
        }
    }
    pub(super) fn check(path: &Path, directory: bool, protected: bool) -> Result<Identity> {
        unsafe {
            use std::os::windows::fs::MetadataExt;
            let m = fs::symlink_metadata(path).map_err(|_| Error::UnsafeWorkspace)?;
            if m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(Error::UnsafeWorkspace);
            }
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(
                    FILE_FLAG_OPEN_REPARSE_POINT
                        | if directory {
                            FILE_FLAG_BACKUP_SEMANTICS
                        } else {
                            0
                        },
                )
                .open(path)
                .map_err(|_| Error::UnsafeWorkspace)?;
            let mut info = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();
            if GetFileInformationByHandle(file.as_raw_handle(), &mut info) == 0
                || (!directory && info.nNumberOfLinks != 1)
            {
                return Err(Error::UnsafeWorkspace);
            }
            let (_buf, user) = sid()?;
            let mut owner = ptr::null_mut();
            let mut dacl = ptr::null_mut();
            let mut sd = ptr::null_mut();
            if GetNamedSecurityInfoW(
                wide(path).as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut sd,
            ) != ERROR_SUCCESS
            {
                return Err(Error::UnsafeWorkspace);
            }
            let _sd = Local(sd);
            if owner.is_null()
                || EqualSid(owner, user) == 0
                || dacl.is_null()
                || (*dacl).AceCount != 1
            {
                return Err(Error::UnsafeWorkspace);
            }
            let mut control = 0u16;
            let mut revision = 0;
            if GetSecurityDescriptorControl(sd, &mut control, &mut revision) == 0
                || (protected && control & SE_DACL_PROTECTED == 0)
            {
                return Err(Error::UnsafeWorkspace);
            }
            let mut ace = ptr::null_mut();
            if GetAce(dacl, 0, &mut ace) == 0 {
                return Err(Error::UnsafeWorkspace);
            }
            let ace = &*(ace.cast::<ACCESS_ALLOWED_ACE>());
            if ace.Header.AceType != 0
                || ace.Mask & FILE_ALL_ACCESS != FILE_ALL_ACCESS
                || EqualSid((&ace.SidStart as *const u32).cast_mut().cast(), user) == 0
            {
                return Err(Error::UnsafeWorkspace);
            }
            Ok(Identity(
                u64::from(info.dwVolumeSerialNumber),
                (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            ))
        }
    }
    pub(super) fn publish(source: &Path, target: &Path) -> Result<()> {
        unsafe {
            if MoveFileExW(wide(source).as_ptr(), wide(target).as_ptr(), 0) == 0 {
                return Err(
                    if [ERROR_FILE_EXISTS, ERROR_ALREADY_EXISTS].contains(&GetLastError()) {
                        Error::Collision
                    } else {
                        Error::IoFailure
                    },
                );
            }
            Ok(())
        }
    }
    #[cfg(test)]
    pub(super) fn secure_directory(path: &Path) -> Result<()> {
        unsafe {
            let sd = descriptor(true)?;
            let mut present = 0;
            let mut defaulted = 0;
            let mut dacl = ptr::null_mut();
            if GetSecurityDescriptorDacl(sd.0, &mut present, &mut dacl, &mut defaulted) == 0 {
                return Err(Error::UnsafeWorkspace);
            }
            if SetNamedSecurityInfoW(
                wide(path).as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl,
                ptr::null_mut(),
            ) != ERROR_SUCCESS
            {
                return Err(Error::UnsafeWorkspace);
            }
            Ok(())
        }
    }
}
