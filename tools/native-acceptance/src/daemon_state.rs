//! The daemon's private `--state-dir` for an acceptance run: its project set
//! and, optionally, the Profile the operator asked to select at start. Written
//! the way the daemon expects (0700 directory, 0600 files, strict JSON);
//! synthetic names and roots only.
use std::{io::Write, path::Path};

/// Writes `projects.json` (canonical roots) and, when given, `startup.json`.
pub fn write(dir: &Path, projects: &[(&str, &Path)], startup: Option<&Path>) {
    precept_ipc::transport::private_dir(dir, true).unwrap();
    let list: Vec<serde_json::Value> = projects
        .iter()
        .map(|(name, root)| serde_json::json!({"name":name,"root":root.canonicalize().unwrap()}))
        .collect();
    put(
        dir,
        "projects.json",
        serde_json::json!({"format":1,"projects":list}),
    );
    if let Some(profile) = startup {
        put(
            dir,
            "startup.json",
            serde_json::json!({"format":1,"profile":profile}),
        );
    }
}
fn put(dir: &Path, name: &str, body: serde_json::Value) {
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    precept_ipc::transport::new_private(&path)
        .unwrap()
        .write_all(body.to_string().as_bytes())
        .unwrap();
}
