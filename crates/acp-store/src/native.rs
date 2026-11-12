//! Read-only connection/file binding for the daemon's independently validated guard.
use crate::{Error, Result};
use rusqlite::{Connection, ffi};
use std::fs::File;

pub fn identity(file: &File) -> Result<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata().map_err(|_| Error::OpenFailed)?;
        Ok((m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
            return Err(Error::OpenFailed);
        }
        let info = unsafe { info.assume_init() };
        Ok((
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(Error::OpenFailed)
    }
}

/// No raw connection is exposed by SqliteStore. This shared check only observes
/// a connection owned by its caller; it never changes SQLite or the filesystem.
pub fn check(db: &Connection, guard: &File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mut moved: i32 = 1;
        let rc = unsafe {
            ffi::sqlite3_file_control(
                db.handle(),
                c"main".as_ptr(),
                ffi::SQLITE_FCNTL_HAS_MOVED,
                (&mut moved as *mut i32).cast(),
            )
        };
        if rc != ffi::SQLITE_OK || moved != 0 {
            return Err(Error::OpenFailed);
        }
        let m = std::fs::metadata(db.path().ok_or(Error::OpenFailed)?)
            .map_err(|_| Error::OpenFailed)?;
        if (m.dev(), m.ino()) != identity(guard)? {
            return Err(Error::OpenFailed);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        let mut handle: windows_sys::Win32::Foundation::HANDLE = std::ptr::null_mut();
        let rc = unsafe {
            ffi::sqlite3_file_control(
                db.handle(),
                c"main".as_ptr(),
                ffi::SQLITE_FCNTL_WIN32_GET_HANDLE,
                (&mut handle as *mut windows_sys::Win32::Foundation::HANDLE).cast(),
            )
        };
        if rc != ffi::SQLITE_OK || handle.is_null() {
            return Err(Error::OpenFailed);
        }
        // Borrow the VFS-owned handle; this wrapper must never close it.
        let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_handle(handle) });
        if identity(&file)? != identity(guard)? {
            return Err(Error::OpenFailed);
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (db, guard);
        return Err(Error::OpenFailed);
    }
    Ok(())
}
