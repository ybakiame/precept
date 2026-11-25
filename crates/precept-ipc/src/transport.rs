#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// Read an operator-selected ciphertext file without imposing private-state permissions.
/// Links and special files are refused; plaintext and credentials still use `open_private`.
pub fn open_ciphertext(path: &std::path::Path) -> crate::Result<std::fs::File> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            crate::Error::NotFound
        } else {
            crate::Error::Unavailable
        }
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(crate::Error::InvalidRequest);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(crate::Error::InvalidRequest);
        }
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(|_| crate::Error::Unavailable)?;
    if !file
        .metadata()
        .map_err(|_| crate::Error::Unavailable)?
        .is_file()
    {
        return Err(crate::Error::InvalidRequest);
    }
    Ok(file)
}
