use super::*;

/// Exact post-stop removal from the original inventory: preflight every listed
/// file (allowlisted name, unaliased regular file, hash) and refuse while any
/// process runs a product executable, before the first unlink; then remove only
/// empty known directories. Unknown files are never touched.
pub(super) fn remove_owned(
    root: &Path,
    files: &[Value],
) -> Result<(usize, Vec<&'static str>), &'static str> {
    let bins = [
        "preceptd",
        "precept",
        "precept-mcp",
        "precept-desktop",
        "precept-bridge",
    ];
    let allowed = |f: &str| {
        bins.iter().any(|b| f == format!("Precept/bin/{b}"))
            || [
                "Precept/runtime/manifest.json",
                "Precept/notices/NOTICE.txt",
                "Precept/notices/dependency-notices.json",
                "Precept/README.txt",
                "Precept/LICENSE.txt",
            ]
            .contains(&f)
            || f.strip_prefix("Precept/notices/")
                .and_then(|n| n.strip_suffix(".txt"))
                .is_some_and(|h| {
                    h.len() == 64
                        && h.bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                })
    };
    if files.is_empty()
        || !bins.iter().all(|b| {
            files
                .iter()
                .any(|f| f["file"] == format!("Precept/bin/{b}"))
        })
    {
        return Err("incomplete removal inventory");
    }
    for f in files {
        let name = f["file"].as_str().ok_or("invalid inventory")?;
        if !allowed(name) {
            return Err("unsafe removal member");
        }
        let path = root.join(name);
        let mut dir = path.parent();
        while let Some(d) = dir {
            if fs::symlink_metadata(d)
                .map_err(|_| "missing directory")?
                .file_type()
                .is_symlink()
            {
                return Err("alias refused");
            }
            dir = d.parent();
        }
        if !fs::symlink_metadata(&path)
            .map_err(|_| "missing product file")?
            .is_file()
        {
            return Err("regular file required");
        }
        if format!(
            "{:x}",
            Sha256::digest(fs::read(&path).map_err(|_| "unreadable")?)
        ) != f["sha256"].as_str().unwrap_or_default()
        {
            return Err("changed file retained");
        }
    }
    // Linux permits unlinking a running executable; refuse instead while any
    // visible process runs one of these exact product files.
    let products: Vec<PathBuf> = bins
        .iter()
        .map(|b| root.join("Precept/bin").join(b).canonicalize().unwrap())
        .collect();
    for entry in fs::read_dir("/proc").unwrap().flatten() {
        if let Ok(exe) = fs::read_link(entry.path().join("exe"))
            && products.contains(&exe)
        {
            return Err("product process running");
        }
    }
    for f in files {
        fs::remove_file(root.join(f["file"].as_str().unwrap())).map_err(|_| "unlink failed")?;
    }
    let mut retained = Vec::new();
    for d in [
        "Precept/bin",
        "Precept/runtime",
        "Precept/notices",
        "Precept",
    ] {
        if fs::remove_dir(root.join(d)).is_err() && root.join(d).exists() {
            retained.push(d);
        }
    }
    Ok((files.len(), retained))
}
