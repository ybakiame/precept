use super::super::{Entry, NodeKind, OpenError, RootIdentity};
use std::{
    ffi::{OsStr, OsString},
    fs::{File, Permissions},
    io::{Read, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_CREATE, FILE_DIRECTORY_FILE, FILE_DIRECTORY_INFORMATION, FILE_NON_DIRECTORY_FILE,
            FILE_OPEN_REPARSE_POINT, FILE_RENAME_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT,
            FileDirectoryInformation, FileRenameInformation, NtCreateFile, NtOpenFile,
            NtQueryDirectoryFile, NtSetInformationFile,
        },
    },
    Win32::{
        Foundation::{
            CloseHandle, DUPLICATE_SAME_ACCESS, HANDLE, LocalFree, OBJ_CASE_INSENSITIVE,
            OBJ_DONT_REPARSE, STATUS_NO_MORE_FILES, UNICODE_STRING,
        },
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
            },
            DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetTokenInformation,
            PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
            SECURITY_DESCRIPTOR, SetKernelObjectSecurity, TOKEN_QUERY, TOKEN_USER, TokenUser,
            UNPROTECTED_DACL_SECURITY_INFORMATION,
        },
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, DELETE, FILE_DISPOSITION_FLAG_DELETE,
            FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO_EX,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
            FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, FileDispositionInfoEx,
            GetFileInformationByHandle, READ_CONTROL, SetFileInformationByHandle, WRITE_DAC,
        },
        System::{
            IO::IO_STATUS_BLOCK,
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    },
};

pub(crate) fn open_root(path: &Path) -> Result<File, OpenError> {
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| OpenError::Unavailable)
}

pub(crate) fn open_relative(
    parent: &File,
    name: &OsStr,
    kind: NodeKind,
) -> Result<File, OpenError> {
    let mut wide: Vec<u16> = name.encode_wide().collect();
    if wide.is_empty() || wide.len() > 32767 || wide.contains(&0) {
        return Err(OpenError::Unsafe);
    }
    wide.push(0);
    let object_name = UNICODE_STRING {
        Length: ((wide.len() - 1) * 2) as u16,
        MaximumLength: (wide.len() * 2) as u16,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let (access, options) = match kind {
        NodeKind::Directory => (
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | FILE_READ_DATA,
            FILE_DIRECTORY_FILE,
        ),
        NodeKind::File => (
            FILE_READ_ATTRIBUTES | FILE_READ_DATA | READ_CONTROL,
            FILE_NON_DIRECTORY_FILE,
        ),
        NodeKind::Any => (FILE_READ_ATTRIBUTES | FILE_READ_DATA, 0),
    };
    let mut handle = std::ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: the parent outlives the call and the UTF-16 name/attributes are valid.
    let result = unsafe {
        NtOpenFile(
            &mut handle,
            access | 0x0010_0000,
            &attributes,
            &mut status,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
        )
    };
    if result < 0 {
        return Err(match result as u32 {
            0xC000_0034 | 0xC000_003A | 0xC000_000F => OpenError::Missing,
            0xC000_00BA | 0xC000_0103 => OpenError::Unsafe,
            _ => OpenError::Unavailable,
        });
    }
    if handle.is_null() {
        return Err(OpenError::Unavailable);
    }
    // SAFETY: NtOpenFile transferred one owned handle to this process.
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn wide_name(name: &OsStr) -> Result<Vec<u16>, OpenError> {
    let mut wide: Vec<u16> = name.encode_wide().collect();
    if wide.is_empty() || wide.len() > 32767 || wide.contains(&0) {
        return Err(OpenError::Unsafe);
    }
    wide.push(0);
    Ok(wide)
}

fn object_name(wide: &mut [u16]) -> UNICODE_STRING {
    UNICODE_STRING {
        Length: ((wide.len() - 1) * 2) as u16,
        MaximumLength: (wide.len() * 2) as u16,
        Buffer: wide.as_mut_ptr(),
    }
}

fn attributes(parent: &File, name: &UNICODE_STRING) -> OBJECT_ATTRIBUTES {
    OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    }
}

pub(crate) fn create_dir(parent: &File, name: &OsStr) -> Result<File, OpenError> {
    let mut wide = wide_name(name)?;
    let object_name = object_name(&mut wide);
    let attributes = attributes(parent, &object_name);
    let mut handle = std::ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: the parent and UTF-16 object name outlive this synchronous call.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | 0x0010_0000,
            &attributes,
            &mut status,
            std::ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if result >= 0 && !handle.is_null() {
        // SAFETY: NtCreateFile transferred one owned handle.
        return Ok(unsafe { File::from_raw_handle(handle) });
    }
    if result as u32 == 0xC000_0035 {
        return open_relative(parent, name, NodeKind::Directory);
    }
    Err(OpenError::Unavailable)
}

fn private_security_descriptor() -> Result<PSECURITY_DESCRIPTOR, OpenError> {
    let mut token = std::ptr::null_mut();
    // SAFETY: GetCurrentProcess returns the process pseudo-handle and `token` is writable.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(OpenError::Unavailable);
    }
    let result = (|| {
        let mut size = 0u32;
        // SAFETY: the token is owned by this process and the size query writes one integer.
        let _ =
            unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size) };
        if size < std::mem::size_of::<TOKEN_USER>() as u32 {
            return Err(OpenError::Unavailable);
        }
        let words =
            (size as usize + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>();
        let mut buffer = vec![0usize; words];
        // SAFETY: the aligned buffer is `size` bytes and the token outlives the query.
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
        {
            return Err(OpenError::Unavailable);
        }
        // SAFETY: GetTokenInformation filled one TOKEN_USER within the aligned buffer.
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let mut sid = std::ptr::null_mut();
        // SAFETY: `user.User.Sid` is a valid token SID and `sid` receives one allocation.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
            return Err(OpenError::Unavailable);
        }
        let mut length = 0usize;
        // SAFETY: ConvertSidToStringSidW returned a NUL-terminated allocation.
        while length < 256 && unsafe { *sid.add(length) } != 0 {
            length += 1;
        }
        if length == 256 {
            unsafe { LocalFree(sid.cast()) };
            return Err(OpenError::Unavailable);
        }
        // SAFETY: the checked prefix is one initialized UTF-16 string.
        let sid_text = match String::from_utf16(unsafe { std::slice::from_raw_parts(sid, length) })
        {
            Ok(text) => text,
            Err(_) => {
                unsafe { LocalFree(sid.cast()) };
                return Err(OpenError::Unavailable);
            }
        };
        unsafe { LocalFree(sid.cast()) };
        let sddl = format!("D:P(A;;FA;;;{sid_text})");
        let wide: Vec<u16> = OsStr::new(&sddl).encode_wide().chain(Some(0)).collect();
        let mut descriptor = std::ptr::null_mut();
        let mut descriptor_size = 0u32;
        // SAFETY: `wide` is NUL-terminated and the output receives one LocalAlloc descriptor.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                &mut descriptor_size,
            )
        } == 0
        {
            return Err(OpenError::Unavailable);
        }
        Ok(descriptor)
    })();
    // SAFETY: OpenProcessToken returned one owned token handle.
    unsafe { CloseHandle(token) };
    result
}

fn copy_dacl(source: &File, target: &File) -> Result<(), OpenError> {
    let mut descriptor = std::ptr::null_mut();
    let mut owner = std::ptr::null_mut();
    let mut group = std::ptr::null_mut();
    let mut dacl = std::ptr::null_mut();
    let mut sacl = std::ptr::null_mut();
    // SAFETY: the source handle outlives the query and the output pointers are writable.
    let result = unsafe {
        GetSecurityInfo(
            source.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            &mut owner,
            &mut group,
            &mut dacl,
            &mut sacl,
            &mut descriptor,
        )
    };
    if result != 0 {
        return Err(OpenError::Unavailable);
    }
    let result = (|| {
        let mut control = 0u16;
        let mut revision = 0u32;
        // SAFETY: GetSecurityInfo returned one initialized security descriptor.
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
            return Err(OpenError::Unavailable);
        }
        let information = DACL_SECURITY_INFORMATION
            | if control & SE_DACL_PROTECTED != 0 {
                PROTECTED_DACL_SECURITY_INFORMATION
            } else {
                UNPROTECTED_DACL_SECURITY_INFORMATION
            };
        // SAFETY: the complete descriptor is initialized and the target handle is writable.
        let result =
            unsafe { SetKernelObjectSecurity(target.as_raw_handle(), information, descriptor) };
        if result != 0 {
            Ok(())
        } else {
            Err(OpenError::Unavailable)
        }
    })();
    // SAFETY: GetSecurityInfo returned one LocalAlloc descriptor.
    unsafe { LocalFree(descriptor.cast()) };
    result
}

fn target_matches(
    parent: &File,
    target: &OsStr,
    expected: &[u8],
) -> Result<(bool, File), OpenError> {
    let file = open_relative(parent, target, NodeKind::File)?;
    if is_reparse(&file)?
        || !file
            .metadata()
            .map_err(|_| OpenError::Unavailable)?
            .is_file()
    {
        return Err(OpenError::Unsafe);
    }
    let mut actual = Vec::with_capacity(expected.len() + 1);
    (&file)
        .take(expected.len() as u64 + 1)
        .read_to_end(&mut actual)
        .map_err(|_| OpenError::Unavailable)?;
    Ok((actual == expected, file))
}

pub(crate) fn write_atomic(
    parent: &File,
    target: &OsStr,
    temporary: &str,
    bytes: &[u8],
    mode: Option<&Permissions>,
    expected: Option<&[u8]>,
) -> Result<(), OpenError> {
    let mut temporary_wide = wide_name(OsStr::new(temporary))?;
    let target_wide = wide_name(target)?;
    let object_name = object_name(&mut temporary_wide);
    let mut attributes = attributes(parent, &object_name);
    let security = private_security_descriptor()?;
    attributes.SecurityDescriptor = security.cast::<SECURITY_DESCRIPTOR>().cast_const();
    let mut handle = std::ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: the parent and UTF-16 object name outlive this synchronous call.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_WRITE_DATA
                | FILE_WRITE_ATTRIBUTES
                | FILE_READ_ATTRIBUTES
                | READ_CONTROL
                | DELETE
                | WRITE_DAC
                | 0x0010_0000,
            &attributes,
            &mut status,
            std::ptr::null(),
            0x80,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW returned one LocalAlloc block.
    unsafe { LocalFree(security) };
    if result < 0 || handle.is_null() {
        return Err(if result as u32 == 0xC000_0035 {
            OpenError::Collision
        } else {
            OpenError::Unavailable
        });
    }
    // SAFETY: NtCreateFile transferred one owned handle.
    let file = unsafe { File::from_raw_handle(handle) };
    let result = (|| {
        let mut file = file;
        file.write_all(bytes).map_err(|_| OpenError::Unavailable)?;
        file.sync_all().map_err(|_| OpenError::Unavailable)?;
        if let Some(mode) = mode {
            file.set_permissions(mode.clone())
                .map_err(|_| OpenError::Unavailable)?;
        }
        if let Some(expected) = expected {
            let (matches, target) = target_matches(parent, target, expected)?;
            if !matches {
                return Err(OpenError::Conflict);
            }
            copy_dacl(&target, &file)?;
        }
        let start = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
        let name_bytes = (target_wide.len() - 1) * 2;
        let size = start + name_bytes;
        let words = (size + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>();
        let mut buffer = vec![0usize; words];
        let information = unsafe { &mut *buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>() };
        information.Anonymous.ReplaceIfExists = expected.is_some();
        information.RootDirectory = parent.as_raw_handle();
        information.FileNameLength = name_bytes as u32;
        unsafe {
            std::ptr::copy_nonoverlapping(
                target_wide.as_ptr(),
                information.FileName.as_mut_ptr(),
                target_wide.len() - 1,
            )
        };
        // SAFETY: the aligned buffer contains one complete FILE_RENAME_INFORMATION.
        let mut status = IO_STATUS_BLOCK::default();
        let result = unsafe {
            NtSetInformationFile(
                file.as_raw_handle(),
                &mut status,
                buffer.as_ptr().cast(),
                size as u32,
                FileRenameInformation,
            )
        };
        if result < 0 {
            return Err(if result as u32 == 0xC000_0035 {
                OpenError::Conflict
            } else {
                OpenError::Unavailable
            });
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_file(parent, OsStr::new(temporary));
    }
    result
}

pub(crate) fn remove_file(parent: &File, name: &OsStr) -> Result<(), OpenError> {
    let mut wide = wide_name(name)?;
    let object_name = object_name(&mut wide);
    let attributes = attributes(parent, &object_name);
    let mut handle = std::ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: the parent and UTF-16 object name outlive this synchronous call.
    let result = unsafe {
        NtOpenFile(
            &mut handle,
            DELETE | FILE_READ_ATTRIBUTES | 0x0010_0000,
            &attributes,
            &mut status,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
        )
    };
    if result < 0 || handle.is_null() {
        return Err(match result as u32 {
            0xC000_0034 | 0xC000_003A | 0xC000_000F => OpenError::Missing,
            _ => OpenError::Unavailable,
        });
    }
    // SAFETY: NtOpenFile transferred one owned handle.
    let file = unsafe { File::from_raw_handle(handle) };
    let disposition = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
    };
    // SAFETY: `disposition` is valid for FileDispositionInfoEx.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfoEx,
            (&disposition as *const FILE_DISPOSITION_INFO_EX).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    } == 0
    {
        return Err(OpenError::Unavailable);
    }
    Ok(())
}

pub(crate) fn dup(file: &File) -> Result<File, OpenError> {
    let process = usize::MAX as HANDLE;
    let mut handle = std::ptr::null_mut();
    // SAFETY: both handles are process-local and the duplicate is immediately owned.
    if unsafe {
        windows_sys::Win32::Foundation::DuplicateHandle(
            process,
            file.as_raw_handle(),
            process,
            &mut handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
        || handle.is_null()
    {
        return Err(OpenError::Unavailable);
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

pub(crate) fn identity(file: &File) -> Result<RootIdentity, OpenError> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns the directory handle and `info` is writable for the call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(OpenError::Unavailable);
    }
    Ok(RootIdentity::Windows {
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

pub(crate) fn is_reparse(file: &File) -> Result<bool, OpenError> {
    use std::os::windows::fs::MetadataExt;
    Ok(file
        .metadata()
        .map_err(|_| OpenError::Unavailable)?
        .file_attributes()
        & 0x400
        != 0)
}

pub(crate) fn entries(
    handle: &File,
    examined_limit: usize,
    kept_limit: usize,
    keep: &mut dyn FnMut(&OsStr) -> bool,
) -> Result<Vec<Entry>, OpenError> {
    let mut found = Vec::new();
    let mut examined = 0;
    let mut buffer = vec![0usize; 8192];
    let buffer_ptr = buffer.as_mut_ptr().cast::<u8>();
    let buffer_len = buffer.len() * std::mem::size_of::<usize>();
    let mut restart = true;
    loop {
        let mut status = IO_STATUS_BLOCK::default();
        // SAFETY: the directory handle is synchronous and the output buffer is writable.
        let result = unsafe {
            NtQueryDirectoryFile(
                handle.as_raw_handle(),
                std::ptr::null_mut(),
                None,
                std::ptr::null(),
                &mut status,
                buffer_ptr.cast(),
                buffer_len as u32,
                FileDirectoryInformation,
                true,
                std::ptr::null(),
                restart,
            )
        };
        restart = false;
        if result == STATUS_NO_MORE_FILES {
            break;
        }
        if result < 0 || status.Information == 0 || status.Information > buffer_len {
            return Err(OpenError::Unavailable);
        }
        let information = unsafe { &*(buffer_ptr.cast::<FILE_DIRECTORY_INFORMATION>()) };
        let start = std::mem::offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
        let end = start
            .checked_add(information.FileNameLength as usize)
            .filter(|end| *end <= status.Information)
            .ok_or(OpenError::Unavailable)?;
        if information.FileNameLength % 2 != 0 {
            return Err(OpenError::Unavailable);
        }
        let name = OsString::from_wide(unsafe {
            std::slice::from_raw_parts(buffer_ptr.add(start).cast::<u16>(), (end - start) / 2)
        });
        if name == "." || name == ".." {
            continue;
        }
        if examined >= examined_limit {
            return Err(OpenError::TooMany);
        }
        examined += 1;
        if keep(name.as_os_str()) {
            if found.len() >= kept_limit {
                return Err(OpenError::TooMany);
            }
            found.push(Entry { name });
        }
    }
    Ok(found)
}
