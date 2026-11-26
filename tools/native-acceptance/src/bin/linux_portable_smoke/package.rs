use super::*;

pub(super) struct Package {
    pub(super) precept: PathBuf,
    pub(super) bins: PathBuf,
    pub(super) manifest: Value,
}
impl Package {
    /// Shipped executables, as `bin/<name>`.
    pub(super) fn shipped(&self) -> Vec<String> {
        self.manifest["binaries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["file"].as_str().unwrap().to_owned())
            .collect()
    }
    pub(super) fn hashes(&self) -> Vec<Value> {
        self.shipped()
            .into_iter()
            .map(|file| {
                let bytes = fs::read(self.precept.join(&file)).unwrap();
                json!({"file":file,"sha256":format!("{:x}", Sha256::digest(bytes))})
            })
            .collect()
    }
    /// Shared objects mapped by one owned process (`/proc/<pid>/maps`): every one
    /// must come from the user distribution, none from the archive or elsewhere.
    /// Includes dynamic GTK/GIO modules and GPU/display integration that
    /// DT_NEEDED alone does not show.
    pub(super) fn modules(&self, pid: u32, out: &Path, surface: &str) -> Vec<String> {
        let maps = fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
        let mut libs: Vec<String> = maps
            .lines()
            .filter_map(|l| l.split_whitespace().nth(5))
            .filter(|p| p.starts_with('/') && p.contains(".so"))
            .map(str::to_owned)
            .collect();
        libs.sort();
        libs.dedup();
        // Anything under the extracted Precept/ tree is refused, not only bin/.
        let precept = self.precept.to_str().unwrap();
        let mut distribution = Vec::new();
        let mut wsl_provided = Vec::new();
        for lib in &libs {
            assert!(
                !lib.starts_with(precept),
                "{surface}: library loaded from the archive: {lib}"
            );
            // WSL mounts its GPU/DXCore integration here; it is not a distro package.
            if lib.starts_with("/usr/lib/wsl/") {
                wsl_provided.push(lib.clone());
                continue;
            }
            assert!(
                [
                    "/usr/lib/",
                    "/lib/",
                    "/usr/lib64/",
                    "/lib64/",
                    "/usr/share/",
                    "/usr/libexec/"
                ]
                .iter()
                .any(|p| lib.starts_with(p)),
                "{surface}: library outside the user distribution: {lib}"
            );
            distribution.push(lib.clone());
        }
        fs::write(
            out.join(format!("{surface}-libraries.json")),
            serde_json::to_vec_pretty(
                &json!({"user_distribution":distribution,"wsl_provided":wsl_provided}),
            )
            .unwrap(),
        )
        .unwrap();
        libs
    }
}
