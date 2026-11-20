//! Windows kernel named pipes with explicit current-user-only DACLs, not defaults.
use crate::{Error, Location, Result, Stream};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
    ptr,
};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{Pipes::*, SystemServices::ACCESS_ALLOWED_ACE_TYPE, Threading::*},
};
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Local(*mut c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn sid_string(sid: PSID) -> Result<String> {
    if sid.is_null() {
        return Err(Error::Unauthorized);
    }
    let mut text = ptr::null_mut();
    // Windows allocates and validates this SID string; Local owns the allocation.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(Error::Unauthorized);
    }
    let _owned = Local(text.cast());
    let mut n = 0;
    unsafe {
        while *text.add(n) != 0 {
            n += 1;
        }
        String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(|_| Error::Unauthorized)
    }
}
fn sid_for_handle(process: HANDLE) -> Result<String> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(Error::Unauthorized);
    }
    let token = OwnedHandle(token);
    let mut bytes = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut bytes);
    }
    if bytes == 0 || bytes > 65536 {
        return Err(Error::Unauthorized);
    }
    let mut storage = vec![0u64; (bytes as usize).div_ceil(8)];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(Error::Unauthorized);
    }
    sid_string(unsafe { (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid })
}
fn current_sid() -> Result<String> {
    sid_for_handle(unsafe { GetCurrentProcess() })
}
fn peer(pid: u32) -> Result<()> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return Err(Error::Unauthorized);
    }
    let handle = OwnedHandle(handle);
    if sid_for_handle(handle.0)? != current_sid()? {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
fn descriptor(directory: bool) -> Result<Local> {
    let sid = current_sid()?;
    let sddl = if directory {
        format!("O:{sid}D:P(A;OICI;FA;;;{sid})")
    } else {
        format!("O:{sid}D:P(A;;GA;;;{sid})")
    };
    let text = wide(std::ffi::OsStr::new(&sddl));
    let mut sd = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(Error::UnsafeRuntime);
    }
    Ok(Local(sd))
}
fn attributes(sd: &Local) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    }
}
fn inspect(path: &Path, directory: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|_| Error::UnsafeRuntime)?;
    if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || meta.is_dir() != directory
        || (!directory && !meta.is_file())
    {
        return Err(Error::UnsafeRuntime);
    }
    let path = wide(path.as_os_str());
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut sd = ptr::null_mut();
    let code = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut sd,
        )
    };
    if code != ERROR_SUCCESS {
        return Err(Error::UnsafeRuntime);
    }
    let _sd = Local(sd);
    let expected = current_sid()?;
    if sid_string(owner)? != expected || acl.is_null() {
        return Err(Error::UnsafeRuntime);
    }
    // Exactly one allow ACE for this user's SID. No SYSTEM/Everyone/inherited broad grants.
    if unsafe { (*acl).AceCount } != 1 {
        return Err(Error::UnsafeRuntime);
    }
    let mut ace = ptr::null_mut();
    if unsafe { GetAce(acl, 0, &mut ace) } == 0 || ace.is_null() {
        return Err(Error::UnsafeRuntime);
    }
    let allowed = ace.cast::<ACCESS_ALLOWED_ACE>();
    if unsafe { (*allowed).Header.AceType } != ACCESS_ALLOWED_ACE_TYPE as u8 {
        return Err(Error::UnsafeRuntime);
    }
    let sid = unsafe { ptr::addr_of_mut!((*allowed).SidStart).cast() };
    if sid_string(sid)? != expected {
        return Err(Error::UnsafeRuntime);
    }
    Ok(())
}
pub fn private_dir(path: &Path, create: bool) -> Result<()> {
    if create && !path.exists() {
        let sd = descriptor(true)?;
        let sa = attributes(&sd);
        let path = wide(path.as_os_str());
        if unsafe { CreateDirectoryW(path.as_ptr(), &sa) } == 0 {
            return Err(Error::UnsafeRuntime);
        }
    }
    inspect(path, true)
}
pub fn private_file(path: &Path) -> Result<()> {
    inspect(path, false)
}
pub fn new_private(path: &Path) -> Result<File> {
    let sd = descriptor(false)?;
    let sa = attributes(&sd);
    let path = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &sa,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::UnsafeRuntime);
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}
pub fn open_private(path: &Path, write: bool) -> Result<File> {
    private_file(path)?;
    OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| Error::UnsafeRuntime)
}
fn name(location: &Location) -> Result<String> {
    location.root()?;
    if location.endpoint.is_empty()
        || location.endpoint.len() > 48
        || !location
            .endpoint
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(Error::UnsafeRuntime);
    }
    let hash = format!("{:x}", Sha256::digest(current_sid()?.as_bytes()));
    Ok(format!(
        r"\\.\pipe\precept-{}-{}",
        &hash[..16],
        location.endpoint
    ))
}
fn server(name: &str, first: bool) -> Result<NamedPipeServer> {
    let sd = descriptor(false)?;
    let mut sa = attributes(&sd);
    // CreateNamedPipe copies the descriptor synchronously; sd/sa remain alive until return.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .max_instances(crate::MAX_PEERS + 2)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(name, (&mut sa as *mut SECURITY_ATTRIBUTES).cast())
    }
    .map_err(|_| Error::EndpointCollision)
}
pub struct Listener {
    pending: Option<NamedPipeServer>,
    name: String,
}
impl Listener {
    pub async fn bind(location: &Location) -> Result<Self> {
        let name = name(location)?;
        Ok(Self {
            pending: Some(server(&name, true)?),
            name,
        })
    }
    pub async fn accept(&mut self) -> Result<Stream> {
        self.pending
            .as_ref()
            .ok_or(Error::Unavailable)?
            .connect()
            .await
            .map_err(|_| Error::Unavailable)?;
        let connected = self.pending.take().ok_or(Error::Unavailable)?;
        self.pending = Some(server(&self.name, false)?);
        let mut pid = 0;
        if unsafe { GetNamedPipeClientProcessId(connected.as_raw_handle(), &mut pid) } == 0 {
            return Err(Error::Unauthorized);
        }
        peer(pid)?;
        Ok(Box::new(connected))
    }
}
pub async fn connect(location: &Location) -> Result<Stream> {
    let name = name(location)?;
    let start = tokio::time::Instant::now();
    let pipe = loop {
        match ClientOptions::new()
            .security_qos_flags(SECURITY_IDENTIFICATION | SECURITY_SQOS_PRESENT)
            .open(&name)
        {
            Ok(p) => break p,
            Err(e)
                if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                    && start.elapsed() < crate::FRAME_TIMEOUT =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await
            }
            Err(_) => return Err(Error::Unavailable),
        }
    };
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(Error::Unauthorized);
    }
    peer(pid)?;
    Ok(Box::new(pipe))
}
