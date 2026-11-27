use precept_ipc::{Access, Client, Credential, Location, load_credential};
use serde_json::{Value, json};
use std::path::PathBuf;

/// `name version` from `clientInfo`, bounded and printable, or none.
pub(super) fn client_label(params: &Value) -> Option<String> {
    let info = params.get("clientInfo")?;
    let name = info.get("name")?.as_str()?;
    let version = info.get("version").and_then(Value::as_str).unwrap_or("");
    let label: String = format!("{name} {version}")
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_owned())
}

/// What every tool says outside a registered project.
pub(super) const UNBOUND: &str = "This folder is not part of a project registered in Precept (or its agent access is off), so Precept has nothing for it. Continue without Precept.";

/// The project whose registered root holds every one of `dirs`, through the
/// agent capabilities published in the runtime directory: each is asked for
/// its own root. `None` when no project or more than one matches.
pub(super) async fn resolve(location: &Location, dirs: &[PathBuf]) -> Option<(String, Credential)> {
    let dirs: Vec<_> = dirs.iter().filter_map(|d| d.canonicalize().ok()).collect();
    if dirs.is_empty() {
        return None;
    }
    let runtime = location.root().ok()?;
    let mut names: Vec<String> = std::fs::read_dir(&runtime)
        .ok()?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| {
            n.strip_prefix("agent-")?
                .strip_suffix(".cap")
                .map(str::to_owned)
        })
        .filter(|n| precept_ipc::valid_project(n))
        .take(256)
        .collect();
    names.sort();
    let mut found = None;
    for name in names {
        let Ok(path) = location.credential_path(Some(&name)) else {
            continue;
        };
        let Ok(credential) = load_credential(location, &path) else {
            continue;
        };
        if credential.access
            != (Access::Agent {
                project: name.clone(),
            })
        {
            continue;
        }
        let Ok(mut client) = Client::connect(location, &credential).await else {
            continue;
        };
        let Ok(answer) = client.request("project.root", json!({})).await else {
            continue;
        };
        let Some(root) = answer["root"]
            .as_str()
            .and_then(|r| std::path::Path::new(r).canonicalize().ok())
        else {
            continue;
        };
        if dirs.iter().all(|d| d.starts_with(&root)) {
            if found.is_some() {
                return None;
            }
            found = Some((name, credential));
        }
    }
    found
}

/// Local folders named by a `roots/list` result (`file://` URIs only).
pub(super) fn root_dirs(result: &Value) -> Vec<PathBuf> {
    let distribution = std::env::var("WSL_DISTRO_NAME").ok();
    root_dirs_in(result, distribution.as_deref())
}

/// [`root_dirs`] for a server running in the WSL distribution `distribution`
/// (if any). A URI names a local folder when its host is empty or
/// `localhost`, or, inside WSL, when it is this distribution's share as a
/// Windows client names it (`//wsl.localhost/<distribution>/...` or
/// `//wsl$/<distribution>/...`). Any other host is another machine's folder
/// and is skipped, never read as a relative path.
pub(super) fn root_dirs_in(result: &Value, distribution: Option<&str>) -> Vec<PathBuf> {
    result["roots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["uri"].as_str()?.strip_prefix("file://"))
        .map(percent_decode)
        .filter_map(|s| {
            let (host, path) = match s.find('/') {
                Some(at) => (&s[..at], &s[at..]),
                None => return None,
            };
            if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
                return Some(path.to_owned());
            }
            let share = host.eq_ignore_ascii_case("wsl.localhost") || host == "wsl$";
            let (name, rest) = path[1..].split_once('/').unwrap_or((&path[1..], ""));
            (share && distribution.is_some_and(|d| d.eq_ignore_ascii_case(name)))
                .then(|| format!("/{rest}"))
        })
        .map(|s| {
            // `file:///C:/...` leaves a leading slash before the drive letter;
            // Windows reads `/C:/...` as drive-relative and loses the folder.
            let b = s.as_bytes();
            if cfg!(windows)
                && b.len() >= 3
                && b[0] == b'/'
                && b[1].is_ascii_alphabetic()
                && b[2] == b':'
            {
                s[1..].to_owned()
            } else {
                s
            }
        })
        .map(PathBuf::from)
        .take(64)
        .collect()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
