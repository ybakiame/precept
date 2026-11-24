use super::harness::envelope;
use super::support::{Fixture, logical, suggest};
use precept_ipc::Error;
use serde_json::json;

#[test]
fn s3_stale_request_bindings_refuse_before_replay_and_strict_decode() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    let original = envelope(&binding, "committed", suggest());
    let write = f.daemon.credential("operator-write.cap");
    f.daemon
        .call(&write, "profile.mutate", original.clone())
        .unwrap();
    let before = logical(&f.db);
    for field in ["profile_id", "generation", "owner_epoch"] {
        let mut stale = original.clone();
        stale["binding"][field] = if field == "owner_epoch" {
            json!(binding.owner_epoch + 1)
        } else {
            let current = stale["binding"][field].as_str().unwrap();
            json!(if current == "a".repeat(32) {
                "b".repeat(32)
            } else {
                "a".repeat(32)
            })
        };
        assert_eq!(
            f.daemon.call(&write, "profile.mutate", stale),
            Err(Error::Unauthorized)
        );
    }
    for field in ["outer", "binding", "command"] {
        let mut unknown = original.clone();
        match field {
            "outer" => unknown["extra"] = json!(true),
            "binding" => unknown["binding"]["extra"] = json!(true),
            _ => unknown["command"]["extra"] = json!(true),
        }
        assert_eq!(
            f.daemon.call(&write, "profile.mutate", unknown),
            Err(Error::InvalidRequest)
        );
    }
    assert_eq!(
        f.daemon.control("profile.review.apply", json!({})),
        Err(Error::Unsupported)
    );
    assert_eq!(logical(&f.db), before);
}
