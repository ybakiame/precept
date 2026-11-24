use super::harness::{Cli, inspect};
use super::support::{Fixture, suggest};

#[test]
fn s3_concurrent_cli_cas_and_identical_replay_commit_one_decision() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    for same_operation in [false, true] {
        let proposed = f
            .daemon
            .mutate(
                &binding,
                if same_operation {
                    "propose-replay"
                } else {
                    "propose-cas"
                },
                suggest(),
            )
            .unwrap();
        let id = proposed["affected"][0]["record_id"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let exact = inspect(&f, id, 1);
        let digest = exact["record"]["proposal_digest"].as_str().unwrap();
        let id_text = id.to_string();
        let args = [
            "preference-confirm",
            &id_text,
            "--scope",
            "global",
            "--expected-revision",
            "1",
            "--proposal-digest",
            digest,
            "--confirm",
        ];
        // Two independent CLI processes submit before either is waited on.
        let left_operation = if same_operation {
            "race-replay"
        } else {
            "race-left"
        };
        let left = Cli::start(&f, &binding, left_operation, &args);
        let right = Cli::start(
            &f,
            &binding,
            if same_operation {
                "race-replay"
            } else {
                "race-right"
            },
            &args,
        );
        let left = left.finish();
        let right = right.finish();
        if same_operation {
            assert!(left.0 && right.0);
            assert_eq!(left.1, right.1);
        } else {
            assert_ne!(left.0, right.0);
            assert_eq!(if left.0 { &right.1 } else { &left.1 }["error"], "Conflict");
        }
        let exact = inspect(&f, id, 2);
        assert_eq!(exact["record"]["status"], "confirmed");
        assert_eq!(exact["decisions"].as_array().unwrap().len(), 1);
        assert_eq!(exact["decisions"][0]["channel"], "cli");
        assert_eq!(exact["decisions"][0]["to_revision"], 2);
        assert_eq!(
            exact["decisions"][0]["receipt_commit_sequence"],
            exact["commit_sequence"]
        );
    }
}
