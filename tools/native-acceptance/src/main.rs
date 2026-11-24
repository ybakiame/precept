//! Owned synthetic integration only. No profile discovery, model or user credentials.
mod baseline;
mod desktop;
mod evidence;
mod fixture;
mod relay;
mod scenarios;
mod wsl;
use baseline::run;
use fixture::binary_fingerprints;
pub(crate) use fixture::{Fixture, Owned, Ui};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn main() {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.first().is_some_and(|s| s == "relay") {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(relay::run(&a[1..]));
        return;
    }
    if a.first().is_some_and(|s| s == "wsl-desktop") {
        wsl_desktop(&a[1..]);
        return;
    }
    assert!(a.len() >= 2, "surface and NEW_OUTPUT required");
    assert_eq!(a[0], "desktop");
    let out = PathBuf::from(&a[1]);
    assert!(out.is_absolute());
    let repo = std::env::current_dir().unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git").args(args).output().unwrap();
        assert!(result.status.success(), "source metadata unavailable");
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let development = std::env::var_os("PRECEPT_S5_DEVELOPMENT").is_some();
    let only = std::env::var("PRECEPT_S3_CASE").ok();
    assert!(
        only.is_none() || development,
        "case selection is development-only, never full acceptance"
    );
    let sha = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain"]).is_empty();
    assert!(!dirty || development, "clean source required");
    let artifact_root = repo
        .join(".tmp/artifacts/local-p1-m4")
        .canonicalize()
        .expect("existing owned artifact root");
    assert!(
        out.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&artifact_root),
        "owned artifact tree required"
    );
    assert!(
        !out.components()
            .any(|part| matches!(part, std::path::Component::ParentDir)),
        "literal new artifact path required"
    );
    fs::create_dir(&out).expect("new report directory required");
    let nsh = repo.join("apps/target/debug").join(format!(
        "precept-nsh-{}{}",
        "native",
        std::env::consts::EXE_SUFFIX
    ));
    let binary_paths = [
        std::env::current_exe().unwrap(),
        Fixture::bin(&repo, "preceptd"),
        Fixture::bin(&repo, "precept"),
        nsh,
    ];
    let binaries = binary_fingerprints(&binary_paths);
    evidence::init(&out, &a[0], &sha, json!(binaries));
    std::panic::set_hook(Box::new(|_| {
        eprintln!("NATIVE_SCENARIO_FAILED detail=redacted; see ledger and owned captures")
    }));
    let baseline = if only.is_some() {
        Ok(json!({"status":"NOT_RUN","reason":"explicit-development-case-selection"}))
    } else {
        std::panic::catch_unwind(|| {
            run(
                &a[0],
                &out,
                a.get(2).map_or("", String::as_str),
                development,
            )
        })
    };
    if baseline.is_err() {
        evidence::finish("FAIL");
    }
    scenarios::run(&out, a.get(2).map_or("", String::as_str), only.as_deref());
    assert_eq!(
        binary_fingerprints(&binary_paths),
        binaries,
        "binaries changed during native acceptance"
    );
    let clean_after = git(&["status", "--porcelain"]).is_empty();
    let ledger = evidence::snapshot();
    let complete = ledger["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| only.as_ref().is_none_or(|id| c["id"] == *id))
        .all(|c| c["status"] == "PASS");
    let pass = baseline.is_ok() && complete && (development || clean_after);
    let result = json!({
        "status":if !pass { "INCOMPLETE" } else if only.is_some() { "DEV_PARTIAL_PASS" } else if development { "DEV_PASS" } else { "PASS" },
        "tested_git_sha":sha,"worktree_dirty":dirty,"clean_after":clean_after,
        "development_case":only,
        "surface":a[0],"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "binaries":binaries,"baseline":baseline.unwrap_or(json!({"status":"FAIL","detail":"see-partial-ledger"})),
        "ledger":"ledger.json","cells":ledger["cells"],
        "visual_review":"NOT_RUN (owned screenshots retained for independent inspection)",
        "historical_source_substitution":false
    });
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
    if !pass {
        std::process::exit(1);
    }
}

/// S1-B: `wsl-desktop NEW_OUTPUT WINDOW_SCRIPT DISTRIBUTION USER PREFIX`.
fn wsl_desktop(a: &[String]) {
    assert_eq!(
        a.len(),
        5,
        "wsl-desktop NEW_OUTPUT WINDOW_SCRIPT DISTRIBUTION USER PREFIX"
    );
    let out = PathBuf::from(&a[0]);
    assert!(out.is_absolute());
    let repo = std::env::current_dir().unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git").args(args).output().unwrap();
        assert!(result.status.success(), "source metadata unavailable");
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let development = std::env::var_os("PRECEPT_S5_DEVELOPMENT").is_some();
    let sha = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain"]).is_empty();
    assert!(!dirty || development, "clean source required");
    let artifact_root = repo
        .join(".tmp/artifacts/local-p1-m5")
        .canonicalize()
        .expect("existing owned artifact root");
    assert!(
        out.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&artifact_root)
            && !out
                .components()
                .any(|p| matches!(p, std::path::Component::ParentDir)),
        "owned literal artifact path required"
    );
    fs::create_dir(&out).expect("new report directory required");
    let gui = repo
        .join("apps/target/debug")
        .join(format!("precept-desktop{}", std::env::consts::EXE_SUFFIX));
    let target = wsl::Wsl {
        distribution: a[2].clone(),
        user: a[3].clone(),
        prefix: a[4].clone(),
    };
    let windows = binary_fingerprints(&[std::env::current_exe().unwrap(), gui.clone()]);
    let linux = wsl::linux_binaries(&target);
    evidence::init_with(
        &out,
        "wsl-desktop",
        &sha,
        json!({"windows":windows,"linux":linux}),
        wsl::CELLS,
    );
    std::panic::set_hook(Box::new(|info| {
        eprintln!(
            "WSL_SCENARIO_FAILED at {}",
            info.location().map(|l| l.to_string()).unwrap_or_default()
        )
    }));
    let outcome = std::panic::catch_unwind(|| wsl::run(&gui, &out, &a[1], target));
    if outcome.is_err() {
        evidence::finish("FAIL");
    }
    let ledger = evidence::snapshot();
    let pass = outcome.is_ok()
        && ledger["cells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["status"] == "PASS")
        && (development || git(&["status", "--porcelain"]).is_empty());
    let result = json!({
        "status": if !pass { "INCOMPLETE" } else if development { "DEV_PASS" } else { "PASS" },
        "tested_git_sha": sha, "worktree_dirty": dirty, "surface": "wsl-desktop",
        "platform": std::env::consts::OS, "windows_binaries": windows, "linux_binaries": linux,
        "cells": ledger["cells"], "ledger": "ledger.json",
        "visual_review": "owned screenshots retained for inspection"
    });
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
    if !pass {
        std::process::exit(1);
    }
}
