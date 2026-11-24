use super::super::support::{Fixture, logical, private_bytes, suggest};
use precept_ipc::Error;
use serde_json::json;
use std::fs;

#[test]
fn selected_file_and_lease_substitution_refuse_valid_private_replacements() {
    for replace_db in [true, false] {
        let f = Fixture::created();
        let b = f.daemon.enable();
        let original = if replace_db {
            f.db.clone()
        } else {
            fs::read_dir(f.db.parent().unwrap())
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with(".acp-writer-")
                })
                .unwrap()
        };
        let copied = f.db.parent().unwrap().join("replacement");
        // Windows byte-range locking refuses reading the held empty lease.
        // Its replacement is deliberately empty; only DB copies need contents.
        let bytes = if replace_db {
            fs::read(&original).unwrap()
        } else {
            Vec::new()
        };
        private_bytes(&copied, &bytes);
        if replace_db {
            let copy = precept_store::SqliteStore::open(&copied)
                .unwrap()
                .metadata()
                .unwrap()
                .binding;
            assert_eq!(copy, b);
        }
        fs::rename(&original, original.with_extension("saved")).unwrap();
        fs::rename(&copied, &original).unwrap();
        let before = logical(&f.db);
        assert_eq!(
            f.daemon.mutate(&b, "replacement-denied", suggest()),
            Err(Error::UnsafeRuntime)
        );
        if replace_db {
            assert_eq!(
                f.daemon.control("profile.info", json!({})),
                Err(Error::UnsafeRuntime)
            );
        }
        assert_eq!(logical(&f.db), before);
        f.daemon
            .control("profile.write.disable", json!({}))
            .unwrap();
    }
}
