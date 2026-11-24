use super::{Check, Facts, PRODUCT_VERSION, Reason, Remediation, Status};
use serde_json::Value;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_MANIFEST_FILES: usize = 512;
const MAX_MANIFEST_BINARIES: usize = 16;
#[derive(Debug)]
pub(super) enum ManifestRead {
    Missing,
    Unreadable,
    Invalid,
    Value(Value),
}

pub(super) fn read_manifest(path: &Path) -> ManifestRead {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ManifestRead::Missing,
        Err(_) => return ManifestRead::Unreadable,
    };
    if !meta.file_type().is_file()
        || meta.file_type().is_symlink()
        || meta.len() > MAX_MANIFEST_BYTES
    {
        return ManifestRead::Invalid;
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return ManifestRead::Unreadable,
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_MANIFEST_BYTES
    {
        return ManifestRead::Invalid;
    }
    match serde_json::from_slice(&bytes) {
        Ok(value) => ManifestRead::Value(value),
        Err(_) => ManifestRead::Invalid,
    }
}

pub(super) fn manifest_format(value: &Value) -> Option<u32> {
    value
        .get("format")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok().filter(|value| *value <= 100))
}

pub(super) fn manifest_valid(value: &Value) -> bool {
    manifest_format(value) == Some(2)
        && value.get("layout").and_then(Value::as_str) == Some("Precept/{bin,runtime,notices}")
}

pub(super) fn package_checks_at(current: &Path) -> Vec<Check> {
    let manifest_path = current
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("runtime/manifest.json"));
    let manifest = manifest_path.as_deref().map(read_manifest);
    let manifest_check = match &manifest {
        None | Some(ManifestRead::Missing) => Check::new(
            "package.manifest",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        Some(ManifestRead::Unreadable) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestUnreadable,
            Remediation::ReextractPackage,
        ),
        Some(ManifestRead::Invalid) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestInvalid,
            Remediation::ReextractPackage,
        ),
        Some(ManifestRead::Value(value)) if manifest_valid(value) => Check::new(
            "package.manifest",
            Status::Ok,
            Reason::ManifestPresent,
            Remediation::None,
        )
        .with_facts(Facts {
            manifest_format: manifest_format(value),
            ..Facts::default()
        }),
        Some(ManifestRead::Value(value)) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestInvalid,
            Remediation::ReextractPackage,
        )
        .with_facts(Facts {
            manifest_format: manifest_format(value),
            ..Facts::default()
        }),
    };
    let version_check = match &manifest {
        Some(ManifestRead::Value(value)) if manifest_valid(value) => {
            match value.get("version").and_then(Value::as_str) {
                Some(version) if version == PRODUCT_VERSION => Check::new(
                    "package.version",
                    Status::Ok,
                    Reason::PackageVersionCompatible,
                    Remediation::None,
                )
                .with_facts(Facts {
                    product_version: Some(PRODUCT_VERSION),
                    expected_product_version: Some(PRODUCT_VERSION),
                    ..Facts::default()
                }),
                Some(_) => Check::new(
                    "package.version",
                    Status::Problem,
                    Reason::PackageVersionIncompatible,
                    Remediation::ReextractPackage,
                )
                .with_facts(Facts {
                    expected_product_version: Some(PRODUCT_VERSION),
                    ..Facts::default()
                }),
                None => Check::new(
                    "package.version",
                    Status::Problem,
                    Reason::PackageVersionUnknown,
                    Remediation::ReextractPackage,
                ),
            }
        }
        Some(ManifestRead::Missing) | None => Check::new(
            "package.version",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        _ => Check::new(
            "package.version",
            Status::Unknown,
            Reason::ManifestUnavailable,
            Remediation::ReextractPackage,
        ),
    };
    let inventory_check = match &manifest {
        Some(ManifestRead::Value(value)) if manifest_valid(value) => {
            package_inventory_check(value, current.parent().and_then(Path::parent))
        }
        Some(ManifestRead::Missing) | None => Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        _ => Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::ManifestUnavailable,
            Remediation::ReextractPackage,
        ),
    };
    vec![manifest_check, version_check, inventory_check]
}

pub(super) fn expected_binaries(bridge_shipped: bool) -> Vec<String> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let mut names = vec![
        format!("preceptd{suffix}"),
        format!("precept{suffix}"),
        format!("precept-mcp{suffix}"),
        format!("precept-hook{suffix}"),
        format!("precept-desktop{suffix}"),
    ];
    if cfg!(target_os = "linux") || bridge_shipped {
        names.push(format!("precept-bridge{suffix}"));
    }
    names
}

pub(super) fn safe_package_file(file: &str) -> bool {
    if file == "README.txt" || file == "LICENSE.txt" {
        return true;
    }
    let mut parts = file.split('/');
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    matches!(first, "bin" | "runtime" | "notices")
        && !second.is_empty()
        && parts.next().is_none()
        && !file.contains('\\')
        && file.len() <= 128
        && file.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
}

pub(super) fn hash_metadata(value: &Value) -> bool {
    value
        .get("sha256")
        .and_then(Value::as_str)
        .is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

pub(super) fn package_inventory_check(manifest: &Value, package_root: Option<&Path>) -> Check {
    let Some(files) = manifest.get("files").and_then(Value::as_array) else {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReextractPackage,
        );
    };
    let Some(binaries) = manifest.get("binaries").and_then(Value::as_array) else {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReextractPackage,
        );
    };
    if files.len() > MAX_MANIFEST_FILES || binaries.len() > MAX_MANIFEST_BINARIES {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageInventoryMismatch,
            Remediation::ReextractPackage,
        );
    }
    let bridge_name = format!("precept-bridge{}", std::env::consts::EXE_SUFFIX);
    let bridge_shipped = binaries.iter().any(|entry| {
        entry.get("file").and_then(Value::as_str) == Some(format!("bin/{bridge_name}").as_str())
    });
    let expected = expected_binaries(bridge_shipped);
    let mut actual = binaries
        .iter()
        .filter_map(|entry| entry.get("file").and_then(Value::as_str))
        .filter_map(|file| file.strip_prefix("bin/"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    let facts = Facts {
        binary_count: Some(binaries.len()),
        file_count: Some(files.len()),
        sizes_match: Some(true),
        hashes_recorded: Some(true),
        content_hashes_verified: Some(false),
        ..Facts::default()
    };
    if actual != expected_sorted
        || binaries.iter().any(|entry| {
            entry
                .get("file")
                .and_then(Value::as_str)
                .is_none_or(|file| !file.starts_with("bin/") || !safe_package_file(file))
        })
    {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageInventoryMismatch,
            Remediation::ReextractPackage,
        )
        .with_facts(facts);
    }
    if files.iter().any(|entry| {
        entry
            .get("file")
            .and_then(Value::as_str)
            .is_none_or(|file| !safe_package_file(file))
            || !hash_metadata(entry)
            || entry.get("size").and_then(Value::as_u64).is_none()
    }) {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageHashMetadataInvalid,
            Remediation::ReextractPackage,
        )
        .with_facts(facts);
    }
    let Some(root) = package_root else {
        return Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::PackageMetadataUnknown,
            Remediation::ReviewInstallation,
        );
    };
    let mut seen = std::collections::BTreeSet::new();
    for entry in files {
        let Some(file) = entry.get("file").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(file) {
            return Check::new(
                "package.inventory",
                Status::Problem,
                Reason::PackageInventoryMismatch,
                Remediation::ReextractPackage,
            )
            .with_facts(facts);
        }
        let path = root.join(file);
        let matches = fs::symlink_metadata(&path).is_ok_and(|meta| {
            meta.file_type().is_file()
                && !meta.file_type().is_symlink()
                && entry.get("size").and_then(Value::as_u64) == Some(meta.len())
        });
        if !matches {
            return Check::new(
                "package.inventory",
                Status::Problem,
                Reason::PackageSizeMismatch,
                Remediation::ReextractPackage,
            )
            .with_facts(facts);
        }
    }
    Check::new(
        "package.inventory",
        Status::Ok,
        Reason::PackageMetadataConsistent,
        Remediation::None,
    )
    .with_facts(facts)
}
