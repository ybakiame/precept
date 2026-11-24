//! Descriptor-relative, no-follow repository reads with persistent root identity.

mod platform;
#[cfg(test)]
mod tests;

use platform::open_relative;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::Read as _,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RootIdentity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u32, index: u64 },
    #[cfg(not(any(unix, windows)))]
    Path { path: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    Missing,
    Unsafe,
    TooMany,
    Collision,
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeKind {
    Directory,
    File,
    Any,
}

#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
}

pub struct Repository {
    path: PathBuf,
    root: File,
    identity: RootIdentity,
}

#[derive(Debug)]
pub struct Directory {
    file: File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRead {
    Missing,
    Unsafe,
    TooLarge,
    BudgetExceeded,
    Unavailable,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let root = platform::open_root(path)?;
        if !root
            .metadata()
            .map_err(|_| OpenError::Unavailable)?
            .is_dir()
            || platform::is_reparse(&root).map_err(|_| OpenError::Unavailable)?
        {
            return Err(OpenError::Unsafe);
        }
        let identity = platform::identity(&root)?;
        Ok(Self {
            path: path.canonicalize().map_err(|_| OpenError::Unavailable)?,
            root,
            identity,
        })
    }

    pub fn open_expected(path: &Path, expected: &RootIdentity) -> Result<Self, OpenError> {
        let repository = Self::open(path)?;
        if &repository.identity != expected {
            return Err(OpenError::Unsafe);
        }
        Ok(repository)
    }

    pub fn identity(path: &Path) -> Result<RootIdentity, OpenError> {
        Ok(Self::open(path)?.identity)
    }

    pub fn current_identity(&self) -> Result<RootIdentity, OpenError> {
        platform::identity(&self.root)
    }

    pub fn is_current(&self) -> bool {
        self.current_identity()
            .is_ok_and(|identity| identity == self.identity)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn root_identity(&self) -> &RootIdentity {
        &self.identity
    }
    pub fn metadata(&self) -> Result<fs::Metadata, OpenError> {
        self.root.metadata().map_err(|_| OpenError::Unavailable)
    }

    pub fn open_dir(&self, relative: &Path) -> Result<Directory, OpenError> {
        let components = normal_components(relative)?;
        if components.is_empty() {
            let handle = platform::dup(&self.root)?;
            return Ok(Directory { file: handle });
        }
        let mut parent = platform::dup(&self.root)?;
        for name in components {
            let next = open_relative(&parent, name, NodeKind::Directory)?;
            validate_node(&next, NodeKind::Directory)?;
            parent = next;
        }
        Ok(Directory { file: parent })
    }

    pub fn open_dir_create(&self, relative: &Path) -> Result<Directory, OpenError> {
        let components = normal_components(relative)?;
        let mut parent = platform::dup(&self.root)?;
        for name in components {
            parent = match open_relative(&parent, name, NodeKind::Directory) {
                Ok(directory) => directory,
                Err(OpenError::Missing) => platform::create_dir(&parent, name)?,
                Err(error) => return Err(error),
            };
            validate_node(&parent, NodeKind::Directory)?;
        }
        Ok(Directory { file: parent })
    }

    pub fn write_atomic(
        &self,
        relative: &Path,
        bytes: &[u8],
        mode: Option<fs::Permissions>,
        expected: Option<&[u8]>,
    ) -> Result<(), OpenError> {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let components = normal_components(relative)?;
        let (Some(name), Some(parent)) =
            (components.last(), components.split_last().map(|(_, p)| p))
        else {
            return Err(OpenError::Unsafe);
        };
        if name.to_str().is_none() {
            return Err(OpenError::Unsafe);
        }
        let directory = self.open_dir_create(&component_path(parent))?;
        for _ in 0..128 {
            let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
            let temporary = format!(".precept-write-{}-{nonce}", std::process::id());
            match platform::write_atomic(
                &directory.file,
                name,
                &temporary,
                bytes,
                mode.as_ref(),
                expected,
            ) {
                Err(OpenError::Collision) => continue,
                result => return result,
            }
        }
        Err(OpenError::Unavailable)
    }

    pub fn remove_file(&self, relative: &Path) -> Result<(), OpenError> {
        let components = normal_components(relative)?;
        let (Some(name), Some(parent)) =
            (components.last(), components.split_last().map(|(_, p)| p))
        else {
            return Err(OpenError::Unsafe);
        };
        let directory = self.open_dir(&component_path(parent))?;
        platform::remove_file(&directory.file, name)
    }

    pub fn open_file(&self, relative: &Path) -> Result<File, OpenError> {
        let components = normal_components(relative)?;
        if components.is_empty() {
            return Err(OpenError::Unsafe);
        }
        let mut parent = platform::dup(&self.root)?;
        for (index, name) in components.iter().enumerate() {
            let last = index + 1 == components.len();
            let next = open_relative(
                &parent,
                name,
                if last {
                    NodeKind::File
                } else {
                    NodeKind::Directory
                },
            )?;
            validate_node(
                &next,
                if last {
                    NodeKind::File
                } else {
                    NodeKind::Directory
                },
            )?;
            parent = next;
        }
        Ok(parent)
    }

    pub fn entries(
        &self,
        relative: &Path,
        examined_limit: usize,
        kept_limit: usize,
        keep: impl FnMut(&std::ffi::OsStr) -> bool,
    ) -> Result<Vec<Entry>, OpenError> {
        let directory = self.open_dir(relative)?;
        directory.entries(examined_limit, kept_limit, keep)
    }

    pub fn read_file(
        &self,
        relative: &Path,
        max: usize,
        remaining: usize,
    ) -> Result<Vec<u8>, FileRead> {
        let mut file = self.open_file(relative).map_err(map_open)?;
        let size = file.metadata().map_err(|_| FileRead::Unavailable)?.len();
        if size > max as u64 {
            return Err(FileRead::TooLarge);
        }
        if size > remaining as u64 {
            return Err(FileRead::BudgetExceeded);
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.by_ref()
            .take(remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FileRead::Unavailable)?;
        if bytes.len() > max {
            return Err(FileRead::TooLarge);
        }
        if bytes.len() > remaining {
            return Err(FileRead::BudgetExceeded);
        }
        Ok(bytes)
    }

    /// `Some(true)` only when every observed no-follow component proves absence.
    pub fn selector_missing(&self, relative: &str) -> Option<bool> {
        let directory = relative.ends_with('/');
        let body = relative.strip_suffix('/').unwrap_or(relative);
        if body.is_empty() {
            return None;
        }
        let components: Vec<_> = body.split('/').collect();
        let mut parent = platform::dup(&self.root).ok()?;
        for (index, name) in components.iter().enumerate() {
            if name.is_empty() || *name == "." || *name == ".." {
                return None;
            }
            let next = match open_relative(&parent, std::ffi::OsStr::new(name), NodeKind::Any) {
                Ok(file) => file,
                Err(OpenError::Missing) => return Some(true),
                Err(_) => return None,
            };
            if platform::is_reparse(&next).ok()? {
                return None;
            }
            let metadata = next.metadata().ok()?;
            if index + 1 == components.len() {
                return Some(directory && !metadata.is_dir());
            }
            if !metadata.is_dir() {
                return Some(true);
            }
            parent = next;
        }
        Some(false)
    }
}

impl Directory {
    pub fn entries(
        &self,
        examined_limit: usize,
        kept_limit: usize,
        mut keep: impl FnMut(&std::ffi::OsStr) -> bool,
    ) -> Result<Vec<Entry>, OpenError> {
        platform::entries(&self.file, examined_limit, kept_limit, &mut keep)
    }
}

fn component_path(components: &[&std::ffi::OsStr]) -> PathBuf {
    let mut path = PathBuf::new();
    for component in components {
        path.push(component);
    }
    path
}

fn normal_components(relative: &Path) -> Result<Vec<&std::ffi::OsStr>, OpenError> {
    let mut components = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => components.push(name),
            Component::CurDir if components.is_empty() && relative.as_os_str() == "." => {}
            _ => return Err(OpenError::Unsafe),
        }
    }
    Ok(components)
}

fn validate_node(file: &File, expected: NodeKind) -> Result<(), OpenError> {
    if platform::is_reparse(file).map_err(|_| OpenError::Unavailable)? {
        return Err(OpenError::Unsafe);
    }
    let metadata = file.metadata().map_err(|_| OpenError::Unavailable)?;
    let valid = match expected {
        NodeKind::Directory => metadata.is_dir(),
        NodeKind::File => metadata.is_file(),
        NodeKind::Any => true,
    };
    if valid {
        Ok(())
    } else {
        Err(OpenError::Unsafe)
    }
}

fn map_open(error: OpenError) -> FileRead {
    match error {
        OpenError::Missing => FileRead::Missing,
        OpenError::Unsafe => FileRead::Unsafe,
        OpenError::Unavailable => FileRead::Unavailable,
        OpenError::TooMany | OpenError::Collision | OpenError::Conflict => FileRead::Unavailable,
    }
}
