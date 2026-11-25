use super::super::{Entry, NodeKind, OpenError, RootIdentity};
use std::{
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::Path,
};

pub(crate) fn open_root(path: &Path) -> Result<File, OpenError> {
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| OpenError::Unsafe)?;
    let fd = unsafe {
        libc::open(
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(crate) fn open_relative(
    parent: &File,
    name: &OsStr,
    kind: NodeKind,
) -> Result<File, OpenError> {
    let name = CString::new(name.as_bytes()).map_err(|_| OpenError::Unsafe)?;
    let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
    if kind == NodeKind::Directory {
        flags |= libc::O_DIRECTORY;
    }
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(io_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(crate) fn create_dir(parent: &File, name: &OsStr) -> Result<File, OpenError> {
    let name = CString::new(name.as_bytes()).map_err(|_| OpenError::Unsafe)?;
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o777) } < 0 {
        let error = unsafe { libc::__errno_location().read() };
        if error != libc::EEXIST {
            return Err(io_error());
        }
    }
    open_relative(
        parent,
        OsStr::from_bytes(name.as_bytes()),
        NodeKind::Directory,
    )
}

pub(crate) fn write_atomic(
    parent: &File,
    target: &OsStr,
    temporary: &str,
    bytes: &[u8],
    mode: Option<&std::fs::Permissions>,
    expected: Option<&[u8]>,
) -> Result<(), OpenError> {
    let temporary = CString::new(temporary).map_err(|_| OpenError::Unsafe)?;
    let target = CString::new(target.as_bytes()).map_err(|_| OpenError::Unsafe)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            temporary.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        return Err(
            if unsafe { libc::__errno_location().read() } == libc::EEXIST {
                OpenError::Collision
            } else {
                io_error()
            },
        );
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let result = (|| {
        file.write_all(bytes).map_err(|_| OpenError::Unavailable)?;
        file.sync_all().map_err(|_| OpenError::Unavailable)?;
        if let Some(mode) = mode {
            file.set_permissions(mode.clone())
                .map_err(|_| OpenError::Unavailable)?;
        }
        if let Some(expected) = expected {
            if !target_matches(parent, target.as_bytes(), expected)? {
                return Err(OpenError::Conflict);
            }
            if unsafe {
                libc::renameat(
                    parent.as_raw_fd(),
                    temporary.as_ptr(),
                    parent.as_raw_fd(),
                    target.as_ptr(),
                )
            } < 0
            {
                return Err(OpenError::Unavailable);
            }
        } else {
            if unsafe {
                libc::linkat(
                    parent.as_raw_fd(),
                    temporary.as_ptr(),
                    parent.as_raw_fd(),
                    target.as_ptr(),
                    0,
                )
            } < 0
            {
                return Err(
                    if unsafe { libc::__errno_location().read() } == libc::EEXIST {
                        OpenError::Conflict
                    } else {
                        io_error()
                    },
                );
            }
            if unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) } < 0 {
                return Err(OpenError::Unavailable);
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = unsafe { libc::unlinkat(parent.as_raw_fd(), temporary.as_ptr(), 0) };
    }
    result
}

fn target_matches(parent: &File, target: &[u8], expected: &[u8]) -> Result<bool, OpenError> {
    let file = open_relative(parent, OsStr::from_bytes(target), NodeKind::File)?;
    if !file
        .metadata()
        .map_err(|_| OpenError::Unavailable)?
        .is_file()
    {
        return Err(OpenError::Unsafe);
    }
    let mut actual = Vec::with_capacity(expected.len() + 1);
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut actual)
        .map_err(|_| OpenError::Unavailable)?;
    Ok(actual == expected)
}

pub(crate) fn remove_file(parent: &File, name: &OsStr) -> Result<(), OpenError> {
    let name = CString::new(name.as_bytes()).map_err(|_| OpenError::Unsafe)?;
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        return Err(io_error());
    }
    Ok(())
}

pub(crate) fn dup(file: &File) -> Result<File, OpenError> {
    let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if fd < 0 {
        return Err(OpenError::Unavailable);
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(crate) fn identity(file: &File) -> Result<RootIdentity, OpenError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata().map_err(|_| OpenError::Unavailable)?;
    Ok(RootIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

pub(crate) fn is_reparse(_file: &File) -> Result<bool, OpenError> {
    Ok(false)
}

pub(crate) fn entries(
    handle: &File,
    examined_limit: usize,
    kept_limit: usize,
    keep: &mut dyn FnMut(&OsStr) -> bool,
) -> Result<Vec<Entry>, OpenError> {
    let directory_file = dup(handle)?;
    let raw_fd = directory_file.as_raw_fd();
    // SAFETY: `raw_fd` is a fresh duplicate owned by the returned DIR.
    let directory = unsafe { libc::fdopendir(raw_fd) };
    if directory.is_null() {
        drop(directory_file);
        return Err(OpenError::Unavailable);
    }
    std::mem::forget(directory_file);
    unsafe { libc::rewinddir(directory) };
    let mut found = Vec::new();
    let mut examined = 0;
    let result = loop {
        // readdir returns null on both EOF and failure. Clear this thread's
        // errno so that only an error from this call is reported.
        unsafe { libc::__errno_location().write(0) };
        let entry = unsafe { libc::readdir(directory) };
        if entry.is_null() {
            let error = unsafe { libc::__errno_location().read() };
            break if error == 0 {
                Ok(found)
            } else {
                Err(OpenError::Unavailable)
            };
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (*entry).d_name.as_ptr().cast(),
                libc::strlen((*entry).d_name.as_ptr()),
            )
        };
        if bytes == b"." || bytes == b".." {
            continue;
        }
        if examined >= examined_limit {
            break Err(OpenError::TooMany);
        }
        examined += 1;
        let name = OsStr::from_bytes(bytes);
        if !keep(name) {
            continue;
        }
        if found.len() >= kept_limit {
            break Err(OpenError::TooMany);
        }
        found.push(Entry {
            name: OsString::from_vec(bytes.to_vec()),
        });
    };
    unsafe { libc::closedir(directory) };
    result
}

fn io_error() -> OpenError {
    match unsafe { libc::__errno_location().read() } {
        libc::ELOOP => OpenError::Unsafe,
        libc::ENOENT => OpenError::Missing,
        libc::ENOTDIR => OpenError::Unsafe,
        _ => OpenError::Unavailable,
    }
}
