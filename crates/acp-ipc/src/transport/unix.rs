use crate::{Error, Location, Result, Stream};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::net::{UnixListener, UnixStream};
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
pub fn private_dir(path: &Path, create: bool) -> Result<()> {
    if create && !path.exists() {
        DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| Error::UnsafeRuntime)?;
    }
    let m = fs::symlink_metadata(path).map_err(|_| Error::UnsafeRuntime)?;
    if !m.is_dir() || m.file_type().is_symlink() || m.uid() != uid() || m.mode() & 0o777 != 0o700 {
        return Err(Error::UnsafeRuntime);
    }
    Ok(())
}
pub fn private_file(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::UnsafeRuntime)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != uid()
        || m.mode() & 0o777 != 0o600
        || m.nlink() != 1
    {
        return Err(Error::UnsafeRuntime);
    }
    Ok(())
}
pub fn new_private(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| Error::UnsafeRuntime)
}
pub fn open_private(path: &Path, write: bool) -> Result<File> {
    private_file(path)?;
    OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| Error::UnsafeRuntime)
}
fn address(location: &Location) -> Result<PathBuf> {
    let root = location.root()?;
    let endpoint = Path::new(&location.endpoint);
    if !endpoint.is_absolute()
        || endpoint
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .as_ref()
            != Some(&root)
    {
        return Err(Error::UnsafeRuntime);
    }
    let path = root.join(endpoint.file_name().ok_or(Error::UnsafeRuntime)?);
    if path.as_os_str().len() > 100 {
        return Err(Error::UnsafeRuntime);
    }
    Ok(path)
}
fn socket(path: &Path) -> Result<fs::Metadata> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::UnsafeRuntime)?;
    if !m.file_type().is_socket() || m.uid() != uid() || m.mode() & 0o777 != 0o600 {
        return Err(Error::UnsafeRuntime);
    }
    Ok(m)
}
pub struct Listener {
    inner: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
}
impl Listener {
    /// Caller must hold RuntimeLease before stale probing or binding.
    pub async fn bind(location: &Location) -> Result<Self> {
        let path = address(location)?;
        if fs::symlink_metadata(&path).is_ok() {
            socket(&path)?;
            match tokio::time::timeout(Duration::from_millis(300), UnixStream::connect(&path)).await
            {
                Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    fs::remove_file(&path).map_err(|_| Error::EndpointCollision)?
                }
                _ => return Err(Error::EndpointCollision),
            }
        }
        let inner = UnixListener::bind(&path).map_err(|_| Error::EndpointCollision)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|_| Error::UnsafeRuntime)?;
        let m = socket(&path)?;
        Ok(Self {
            inner,
            path,
            identity: (m.dev(), m.ino()),
        })
    }
    pub async fn accept(&mut self) -> Result<Stream> {
        let (stream, _) = self.inner.accept().await.map_err(|_| Error::Unavailable)?;
        if stream.peer_cred().map_err(|_| Error::Unauthorized)?.uid() != uid() {
            return Err(Error::Unauthorized);
        }
        Ok(Box::new(stream))
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub async fn connect(location: &Location) -> Result<Stream> {
    let path = address(location)?;
    socket(&path)?;
    let stream = UnixStream::connect(path)
        .await
        .map_err(|_| Error::Unavailable)?;
    if stream.peer_cred().map_err(|_| Error::Unauthorized)?.uid() != uid() {
        return Err(Error::Unauthorized);
    }
    Ok(Box::new(stream))
}
