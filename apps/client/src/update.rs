use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

use nsh_updater::{Config, Download};
use serde::Deserialize;
use url::Url;

const MAX_METADATA: usize = 1024 * 1024;
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateError {
    Unavailable,
    Network,
    Manifest,
    Download,
    Install,
    UnsafeInstallation,
}

#[derive(Debug, Clone)]
pub struct Update {
    version: String,
    notes: Option<String>,
    url: String,
    sha256: String,
    size: Option<u64>,
}

impl Update {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }
}

#[derive(Debug, Clone)]
pub struct DownloadedUpdate {
    version: String,
    archive: PathBuf,
    sha256: String,
    size: Option<u64>,
}

impl DownloadedUpdate {
    pub fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Deserialize)]
struct AppConfig {
    id: String,
    name: String,
    version: String,
    build: u64,
    updater: SourceConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceConfig {
    github: Option<String>,
    url: Option<String>,
    #[serde(default)]
    channels: std::collections::BTreeMap<String, String>,
    public_key: Option<String>,
}

fn config_from(bytes: &[u8]) -> Result<Config, UpdateError> {
    let app: AppConfig = serde_json::from_slice(bytes).map_err(|_| UpdateError::Unavailable)?;
    let mut config = Config::default();
    config.id = app.id;
    config.name = Some(app.name);
    config.version = app.version;
    config.github = app.updater.github;
    config.url = app.updater.url;
    config.channels = app.updater.channels;
    config.public_key = app.updater.public_key;
    config.build = Some(app.build);
    config.validate().map_err(|_| UpdateError::Unavailable)?;
    Ok(config)
}

fn config() -> Result<Config, UpdateError> {
    config_from(include_bytes!("../../ui/nsh.json"))
}

fn secure_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => url
            .host_str()
            .is_some_and(|host| host == "127.0.0.1" || host == "localhost" || host == "::1"),
        _ => false,
    }
}

fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>, UpdateError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_global(Some(Duration::from_secs(180)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut current = Url::parse(url).map_err(|_| UpdateError::Network)?;
    let require_https = current.scheme() == "https";
    for _ in 0..=MAX_REDIRECTS {
        if !secure_url(&current) || (require_https && current.scheme() != "https") {
            return Err(UpdateError::Network);
        }
        let response = agent
            .get(current.as_str())
            .header("Accept-Encoding", "identity")
            .call()
            .map_err(|_| UpdateError::Network)?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("Location")
                .and_then(|value| value.to_str().ok())
                .ok_or(UpdateError::Network)?;
            current = current.join(location).map_err(|_| UpdateError::Network)?;
            continue;
        }
        if response.status() != 200 {
            return Err(UpdateError::Network);
        }
        return Ok(response);
    }
    Err(UpdateError::Network)
}

fn read_all(url: &str) -> Result<Vec<u8>, UpdateError> {
    let mut response = get(url)?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_METADATA as u64)
        .read_to_vec()
        .map_err(|_| UpdateError::Network)
}

pub fn check() -> Result<Option<Update>, UpdateError> {
    if !cfg!(target_os = "windows") {
        return Err(UpdateError::Unavailable);
    }
    let config = config()?;
    let manifest = read_all(&config.manifest_url())?;
    let signature = match config.signature_url() {
        Some(url) => Some(read_all(&url)?),
        None => None,
    };
    let available = config
        .check(&manifest, signature.as_deref(), &nsh_updater::platform())
        .map_err(|_| UpdateError::Manifest)?;
    let Some(available) = available else {
        return Ok(None);
    };
    Ok(Some(Update {
        version: available.version,
        notes: available.notes,
        url: available.url,
        sha256: available.sha256.ok_or(UpdateError::Manifest)?,
        size: available.size,
    }))
}

fn updates_dir() -> Result<PathBuf, UpdateError> {
    let state = crate::paths::state_dir().ok_or(UpdateError::Unavailable)?;
    if let Some(parent) = state.parent() {
        precept_ipc::transport::private_dir(parent, true).map_err(|_| UpdateError::Unavailable)?;
    }
    precept_ipc::transport::private_dir(&state, true).map_err(|_| UpdateError::Unavailable)?;
    let updates = state.join("updates");
    precept_ipc::transport::private_dir(&updates, true).map_err(|_| UpdateError::Unavailable)?;
    Ok(updates)
}

fn verify_archive(path: &Path, sha256: &str, size: Option<u64>) -> Result<(), UpdateError> {
    let mut file = fs::File::open(path).map_err(|_| UpdateError::Download)?;
    let mut download = Download::new(sha256, size);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| UpdateError::Download)?;
        if read == 0 {
            break;
        }
        download
            .update(&buffer[..read])
            .map_err(|_| UpdateError::Download)?;
    }
    download.finish().map_err(|_| UpdateError::Download)
}

pub fn download(
    update: &Update,
    mut progress: impl FnMut(u64),
) -> Result<DownloadedUpdate, UpdateError> {
    if update.size.is_some_and(|size| size > MAX_ARCHIVE) {
        return Err(UpdateError::Download);
    }
    let directory = updates_dir().map_err(|_| UpdateError::Download)?;
    let archive = directory.join(format!("precept-update-{}", update.sha256));
    if archive.is_file() && verify_archive(&archive, &update.sha256, update.size).is_ok() {
        progress(update.size.unwrap_or_default());
        return Ok(DownloadedUpdate {
            version: update.version.clone(),
            archive,
            sha256: update.sha256.clone(),
            size: update.size,
        });
    }
    if archive.exists() {
        fs::remove_file(&archive).map_err(|_| UpdateError::Download)?;
    }
    let partial = directory.join(format!("precept-update-{}.part", update.sha256));
    let result = (|| {
        let mut response = get(&update.url).map_err(|_| UpdateError::Download)?;
        let mut file = fs::File::create(&partial).map_err(|_| UpdateError::Download)?;
        let mut download = Download::new(&update.sha256, update.size);
        let mut reader = response.body_mut().as_reader().take(MAX_ARCHIVE + 1);
        let mut buffer = [0_u8; 64 * 1024];
        progress(0);
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|_| UpdateError::Download)?;
            if read == 0 {
                break;
            }
            let received = download
                .update(&buffer[..read])
                .map_err(|_| UpdateError::Download)?;
            if received > MAX_ARCHIVE {
                return Err(UpdateError::Download);
            }
            file.write_all(&buffer[..read])
                .map_err(|_| UpdateError::Download)?;
            progress(received);
        }
        download.finish().map_err(|_| UpdateError::Download)?;
        file.sync_all().map_err(|_| UpdateError::Download)?;
        drop(file);
        fs::rename(&partial, &archive).map_err(|_| UpdateError::Download)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result?;
    Ok(DownloadedUpdate {
        version: update.version.clone(),
        archive,
        sha256: update.sha256.clone(),
        size: update.size,
    })
}

#[derive(Deserialize)]
struct Inventory {
    files: Vec<InventoryFile>,
}

#[derive(Deserialize)]
struct InventoryFile {
    file: String,
}

fn relative_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return None;
    }
    Some(path.to_owned())
}

fn verify_managed_root(root: &Path) -> Result<(), UpdateError> {
    let manifest_path = root.join("runtime/manifest.json");
    if !fs::symlink_metadata(&manifest_path)
        .map_err(|_| UpdateError::Unavailable)?
        .file_type()
        .is_file()
    {
        return Err(UpdateError::UnsafeInstallation);
    }
    let inventory: Inventory =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|_| UpdateError::Unavailable)?)
            .map_err(|_| UpdateError::UnsafeInstallation)?;
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    for entry in inventory.files {
        let path = relative_path(&entry.file).ok_or(UpdateError::UnsafeInstallation)?;
        let mut parent = path.parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            directories.insert(directory.to_owned());
            parent = directory.parent();
        }
        files.insert(path);
    }
    files.insert(PathBuf::from("runtime/manifest.json"));
    directories.insert(PathBuf::from("runtime"));
    let mut pending = vec![root.to_owned()];
    let mut visited = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|_| UpdateError::UnsafeInstallation)? {
            let entry = entry.map_err(|_| UpdateError::UnsafeInstallation)?;
            visited += 1;
            if visited > 4096 {
                return Err(UpdateError::UnsafeInstallation);
            }
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|_| UpdateError::UnsafeInstallation)?;
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| UpdateError::UnsafeInstallation)?
                .to_owned();
            if metadata.file_type().is_symlink() {
                return Err(UpdateError::UnsafeInstallation);
            }
            if metadata.is_dir() {
                if !directories.contains(&relative) {
                    return Err(UpdateError::UnsafeInstallation);
                }
                pending.push(entry.path());
            } else if !metadata.is_file() || !files.contains(&relative) {
                return Err(UpdateError::UnsafeInstallation);
            }
        }
    }
    Ok(())
}

fn installation_at(executable: &Path) -> Result<nsh_updater::install::Installation, UpdateError> {
    let root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or(UpdateError::Unavailable)?;
    let installation = nsh_updater::install::Installation::directory(root, executable)
        .ok_or(UpdateError::Unavailable)?;
    if !installation.can_install() {
        return Err(UpdateError::Unavailable);
    }
    installation.remove_leftovers();
    verify_managed_root(root)?;
    Ok(installation)
}

pub fn install(update: &DownloadedUpdate) -> Result<(), UpdateError> {
    verify_archive(&update.archive, &update.sha256, update.size)
        .map_err(|_| UpdateError::Install)?;
    let executable = std::env::current_exe().map_err(|_| UpdateError::Unavailable)?;
    let installation = installation_at(&executable)?;
    installation
        .install(&update.archive)
        .map_err(|_| UpdateError::Install)?;
    let _ = fs::remove_file(&update.archive);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_configuration_is_fixed_and_matches_the_product_version() {
        let config = config().unwrap();
        assert_eq!(config.id, "org.precept.operator");
        assert_eq!(config.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(config.github.as_deref(), Some("ybakiame/precept"));
        assert!(config.public_key.is_some());
        assert!(config.url.is_none());
    }

    #[test]
    fn archive_verification_checks_size_and_hash() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("archive");
        fs::write(&file, b"abc").unwrap();
        verify_archive(
            &file,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            Some(3),
        )
        .unwrap();
        fs::write(&file, b"abd").unwrap();
        assert_eq!(
            verify_archive(
                &file,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                Some(3),
            ),
            Err(UpdateError::Download)
        );
    }

    #[test]
    fn managed_installations_refuse_unknown_files_and_links() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Precept");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("runtime")).unwrap();
        fs::write(root.join("bin/precept-desktop.exe"), b"app").unwrap();
        fs::write(
            root.join("runtime/manifest.json"),
            r#"{"files":[{"file":"bin/precept-desktop.exe"}]}"#,
        )
        .unwrap();
        verify_managed_root(&root).unwrap();
        fs::write(root.join("notes.txt"), b"keep").unwrap();
        assert_eq!(
            verify_managed_root(&root),
            Err(UpdateError::UnsafeInstallation)
        );
        fs::remove_file(root.join("notes.txt")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("bin/precept-desktop.exe", root.join("link")).unwrap();
            assert_eq!(
                verify_managed_root(&root),
                Err(UpdateError::UnsafeInstallation)
            );
        }
    }

    #[test]
    fn nsh_installation_replaces_a_managed_multi_binary_directory() {
        let directory = tempfile::tempdir().unwrap();
        let desktop = if cfg!(windows) {
            "precept-desktop.exe"
        } else {
            "precept-desktop"
        };
        let daemon = if cfg!(windows) {
            "preceptd.exe"
        } else {
            "preceptd"
        };
        let make_root = |root: &Path, version: &[u8]| {
            fs::create_dir_all(root.join("bin")).unwrap();
            fs::create_dir_all(root.join("runtime")).unwrap();
            fs::write(root.join("bin").join(desktop), version).unwrap();
            fs::write(root.join("bin").join(daemon), version).unwrap();
            #[cfg(unix)]
            for name in [desktop, daemon] {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    root.join("bin").join(name),
                    fs::Permissions::from_mode(0o755),
                )
                .unwrap();
            }
            fs::write(
                root.join("runtime/manifest.json"),
                format!(r#"{{"files":[{{"file":"bin/{desktop}"}},{{"file":"bin/{daemon}"}}]}}"#),
            )
            .unwrap();
        };
        let installed = directory.path().join("installed/Precept");
        let replacement_parent = directory.path().join("replacement");
        let replacement = replacement_parent.join("Precept");
        make_root(&installed, b"old");
        make_root(&replacement, b"new");
        let archive = directory.path().join("update.tar.gz");
        let status = std::process::Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&replacement_parent)
            .arg("Precept")
            .status()
            .unwrap();
        assert!(status.success());
        let installation = installation_at(&installed.join("bin").join(desktop)).unwrap();
        installation.install(&archive).unwrap();
        assert_eq!(
            fs::read(installed.join("bin").join(desktop)).unwrap(),
            b"new"
        );
        assert_eq!(
            fs::read(installed.join("bin").join(daemon)).unwrap(),
            b"new"
        );
    }
}
