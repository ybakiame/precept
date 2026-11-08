//! Guarded rehearsal only: generates its own temporary fixtures, never imports DBs.
use acp_rehearsal::{Error, Fault, Outcome, SyntheticWorkspace};
use std::{fs, path::Path, process::Command};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--synthetic-only"] {
        return Err("SyntheticOnlyRequired (no source/profile path arguments accepted)".into());
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into());
    for legacy in [true, false] {
        let ws = SyntheticWorkspace::new()?;
        let mut builder = Command::new(&python);
        builder
            .arg(repo.join("tools/sqlite_fixture.py"))
            .arg(ws.path().join("fixture"))
            .env("PYTHONPATH", repo.join("src"));
        if legacy {
            builder.arg("--legacy");
        }
        if !builder.output()?.status.success() {
            return Err("SyntheticBuilderFailed".into());
        }
        let source = ws.path().join("fixture/state/context.sqlite3");
        let backup = ws.path().join("backup.sqlite3");
        fs::create_dir(ws.path().join("output"))?;
        let output = ws.path().join("output/context.sqlite3");
        assert_eq!(
            ws.rehearse(&source, &backup, &output, Fault::BeforeCommit),
            Err(Error::BeforeCommit)
        );
        assert_eq!(
            ws.retry(&backup, &output, Fault::AfterCommit),
            Err(Error::AfterCommit)
        );
        ws.restore(&backup, &ws.path().join("restored.sqlite3"))?;
        assert_eq!(ws.retry(&backup, &output, Fault::None)?, Outcome::AlreadyV1);
        let verified = Command::new(&python)
            .arg(repo.join("tools/check_migration_compat.py"))
            .arg("--synthetic-only")
            .arg(ws.path())
            .env("PYTHONPATH", repo.join("src"))
            .output()?;
        if !verified.status.success() {
            return Err("SyntheticPythonCompatibilityFailed".into());
        }
        println!(
            "PASS: {} v0 -> verified backup v0 -> precommit rollback v0 -> postcommit error v1 -> restored v0 -> idempotent retry v1; Python open/write and Rust parity PASS",
            if legacy { "legacy" } else { "fingerprinted" }
        );
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
