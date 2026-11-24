use super::super::{Entry, NodeKind, OpenError, RootIdentity};
use std::{ffi::OsStr, fs::File, path::PathBuf};

pub(crate) fn open_root(_path: &std::path::Path) -> Result<File, OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn open_relative(
    _parent: &File,
    _name: &OsStr,
    _kind: NodeKind,
) -> Result<File, OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn create_dir(_parent: &File, _name: &OsStr) -> Result<File, OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn write_atomic(
    _parent: &File,
    _target: &OsStr,
    _temporary: &str,
    _bytes: &[u8],
    _mode: Option<&std::fs::Permissions>,
    _expected: Option<&[u8]>,
) -> Result<(), OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn remove_file(_parent: &File, _name: &OsStr) -> Result<(), OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn dup(_file: &File) -> Result<File, OpenError> {
    Err(OpenError::Unavailable)
}

pub(crate) fn identity(_file: &File) -> Result<RootIdentity, OpenError> {
    Ok(RootIdentity::Path {
        path: PathBuf::from("/"),
    })
}

pub(crate) fn is_reparse(_file: &File) -> Result<bool, OpenError> {
    Ok(false)
}

pub(crate) fn entries(
    _handle: &File,
    _examined_limit: usize,
    _kept_limit: usize,
    _keep: &mut dyn FnMut(&OsStr) -> bool,
) -> Result<Vec<Entry>, OpenError> {
    Err(OpenError::Unavailable)
}
