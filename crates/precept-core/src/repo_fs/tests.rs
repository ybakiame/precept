use super::*;
use std::fs;

#[test]
fn a_pinned_root_survives_replacement_but_expected_open_refuses_it() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("value"), "original").unwrap();
    let repository = Repository::open(&root).unwrap();
    let identity = repository.root_identity().clone();

    let moved = temp.path().join("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("value"), "replacement").unwrap();
    assert_eq!(
        repository.read_file(Path::new("value"), 64, 64).unwrap(),
        b"original"
    );
    assert!(Repository::open_expected(&root, &identity).is_err());

    fs::remove_dir_all(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    assert!(Repository::open_expected(&root, &identity).is_ok());
}

#[test]
fn pinned_writes_and_deletes_stay_on_the_original_root_after_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join(".precept/rules")).unwrap();
    fs::write(root.join(".precept/rules/value"), "original").unwrap();
    let repository = Repository::open(&root).unwrap();
    let identity = repository.root_identity().clone();

    let moved = temp.path().join("moved");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    repository
        .write_atomic(Path::new(".precept/rules/newvalue"), b"updated", None, None)
        .unwrap();
    assert_eq!(
        fs::read(moved.join(".precept/rules/newvalue")).unwrap(),
        b"updated"
    );
    assert_eq!(
        repository
            .read_file(Path::new(".precept/rules/newvalue"), 64, 64)
            .unwrap(),
        b"updated"
    );
    assert!(!root.join(".precept").exists());
    repository
        .write_atomic(
            Path::new(".precept/rules/value"),
            b"replacement",
            Some(
                fs::metadata(moved.join(".precept/rules/value"))
                    .unwrap()
                    .permissions(),
            ),
            Some(b"original".as_slice()),
        )
        .unwrap();
    assert_eq!(
        fs::read(moved.join(".precept/rules/value")).unwrap(),
        b"replacement"
    );
    repository
        .remove_file(Path::new(".precept/rules/newvalue"))
        .unwrap();
    assert!(!moved.join(".precept/rules/newvalue").exists());

    fs::remove_dir(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    assert!(Repository::open_expected(&root, &identity).is_ok());
}

#[test]
fn conditional_writes_refuse_changed_or_already_created_targets() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("value"), "original").unwrap();
    let repository = Repository::open(temp.path()).unwrap();
    assert!(matches!(
        repository.write_atomic(
            Path::new("value"),
            b"replacement",
            None,
            Some(b"stale".as_slice()),
        ),
        Err(OpenError::Conflict)
    ));
    assert_eq!(fs::read(temp.path().join("value")).unwrap(), b"original");
    repository
        .write_atomic(Path::new("new"), b"first", None, None)
        .unwrap();
    assert!(matches!(
        repository.write_atomic(Path::new("new"), b"second", None, None),
        Err(OpenError::Conflict)
    ));
    assert_eq!(fs::read(temp.path().join("new")).unwrap(), b"first");
}

#[cfg(unix)]
#[test]
fn replacement_keeps_restrictive_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("value");
    fs::write(&path, "original").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let repository = Repository::open(temp.path()).unwrap();
    repository
        .write_atomic(
            Path::new("value"),
            b"replacement",
            Some(fs::metadata(&path).unwrap().permissions()),
            Some(b"original".as_slice()),
        )
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(windows)]
fn windows_dacl(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, GetFileSecurityW};

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut needed = 0u32;
    unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    assert!(needed > 0);
    let mut bytes = vec![0u8; needed as usize];
    assert_ne!(
        unsafe {
            GetFileSecurityW(
                wide.as_ptr(),
                DACL_SECURITY_INFORMATION,
                bytes.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        },
        0
    );
    bytes
}

#[cfg(windows)]
#[test]
fn replacement_preserves_the_existing_windows_dacl() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            DACL_SECURITY_INFORMATION, SetFileSecurityW,
        },
    };

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("value");
    fs::write(&path, "original").unwrap();
    let sddl: Vec<u16> = std::ffi::OsStr::new("D:P(A;;FA;;;WD)")
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut descriptor = std::ptr::null_mut();
    let mut size = 0u32;
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                &mut size,
            )
        },
        0
    );
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    assert_ne!(
        unsafe { SetFileSecurityW(wide.as_ptr(), DACL_SECURITY_INFORMATION, descriptor.cast(),) },
        0
    );
    unsafe { LocalFree(descriptor.cast()) };
    let before = windows_dacl(&path);
    let repository = Repository::open(temp.path()).unwrap();
    repository
        .write_atomic(
            Path::new("value"),
            b"replacement",
            Some(fs::metadata(&path).unwrap().permissions()),
            Some(b"original".as_slice()),
        )
        .unwrap();
    assert_eq!(windows_dacl(&path), before);
}

#[test]
fn a_linked_parent_is_never_traversed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret"), "private").unwrap();
    make_link(&outside, &root.join("linked"));
    let repository = Repository::open(&root).unwrap();
    assert!(matches!(
        repository.open_file(Path::new("linked/secret")),
        Err(OpenError::Unsafe)
    ));
    assert_eq!(repository.selector_missing("linked/secret"), None);
    assert!(matches!(
        repository.open_dir_create(Path::new("linked")),
        Err(OpenError::Unsafe)
    ));
    assert!(matches!(
        repository.write_atomic(Path::new("linked/written"), b"unsafe", None, None),
        Err(OpenError::Unsafe)
    ));
    assert!(matches!(
        repository.remove_file(Path::new("linked/secret")),
        Err(OpenError::Unsafe)
    ));
    assert_eq!(
        fs::read_to_string(outside.join("secret")).unwrap(),
        "private"
    );
    assert!(!outside.join("written").exists());
}

#[test]
fn a_file_where_a_directory_is_expected_is_absent_but_unsafe_to_open() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(".precept"), "not a directory").unwrap();
    let repository = Repository::open(temp.path()).unwrap();
    assert_eq!(
        repository.selector_missing(".precept/rules/example.md"),
        Some(true)
    );
    assert!(matches!(
        repository.open_dir(Path::new(".precept")),
        Err(OpenError::Unsafe)
    ));
    assert!(matches!(
        repository.open_dir_create(Path::new(".precept/rules")),
        Err(OpenError::Unsafe)
    ));
}

#[test]
fn enumeration_limits_are_independent() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.md", "d.md"] {
        fs::write(temp.path().join(name), name).unwrap();
    }
    let repository = Repository::open(temp.path()).unwrap();
    let markdown = |name: &std::ffi::OsStr| name.as_encoded_bytes().ends_with(b".md");
    assert!(matches!(
        repository.entries(Path::new(""), 2, 10, markdown),
        Err(OpenError::TooMany)
    ));
    assert!(matches!(
        repository.entries(Path::new(""), 10, 1, markdown),
        Err(OpenError::TooMany)
    ));
    assert_eq!(
        repository
            .entries(Path::new(""), 10, 2, markdown)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn enumeration_and_identity_are_bounded_and_serializable() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("a"), "a").unwrap();
    fs::write(temp.path().join("b"), "b").unwrap();
    let repository = Repository::open(temp.path()).unwrap();
    assert!(matches!(
        repository.entries(Path::new(""), 2, 1, |_| true),
        Err(OpenError::TooMany)
    ));
    assert_eq!(
        repository
            .entries(Path::new(""), 2, 2, |_| true)
            .unwrap()
            .len(),
        2
    );
    let encoded = serde_json::to_vec(repository.root_identity()).unwrap();
    let decoded: RootIdentity = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, *repository.root_identity());
}

#[cfg(unix)]
fn make_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn make_link(target: &Path, link: &Path) {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "& { param([string]$link,[string]$target) New-Item -ItemType Junction -Path $link -Target $target | Out-Null }",
            &link.to_string_lossy(),
            &target.to_string_lossy(),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "Windows junction fixture failed");
}

#[cfg(not(any(unix, windows)))]
fn make_link(_target: &Path, _link: &Path) {
    panic!("link fixture unsupported");
}
