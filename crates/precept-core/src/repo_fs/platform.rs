#[cfg(unix)]
#[path = "unix.rs"]
mod implementation;
#[cfg(windows)]
#[path = "windows.rs"]
mod implementation;
#[cfg(not(any(unix, windows)))]
#[path = "unsupported.rs"]
mod implementation;

pub(crate) use implementation::*;
