"""Local-only gates. Never install packages, contact Actions, or open a user DB.

All Cargo commands use --offline --locked. Raw diagnostics are private; only
the deliberately bounded report is suitable for manually sanitized publication.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
PY = sys.executable


def gates(core, service, ui):
    cargo = ["cargo"]
    common = ["--offline", "--locked"]
    items = []
    if core:
        items += [
            ("root-fmt", cargo + ["fmt", "--all", "--", "--check"]),
            ("root-clippy", cargo + ["clippy", "--workspace", "--all-targets", "--all-features", *common, "--", "-D", "warnings"]),
            ("root-tests", cargo + ["test", "--workspace", "--all-targets", *common]),
            ("python", [PY, "-m", "unittest", "discover", "-s", "tests", "-v"]),
            ("parity-build", cargo + ["build", "--workspace", "--examples", *common]),
            ("json-parity", [PY, "tools/check_p1_parity.py"]),
            ("sqlite-parity", [PY, "tools/check_sqlite_parity.py"]),
            ("backup-rehearsal", cargo + ["run", "-p", "acp-rehearsal", "--example", "synthetic_rehearsal", *common, "--", "--synthetic-only"]),
            ("process-recovery", cargo + ["test", "-p", "acp-rehearsal", "--features", "recovery-tests", "--test", "process_recovery", *common]),
        ]
    if service or ui:
        items.append(("service-build", cargo + ["build", "--workspace", "--bins", "--examples", *common]))
    if service:
        items.append(("service-e2e", [PY, "tools/test_local_service.py", "-v"]))
    if ui:
        items += [("model-fmt", cargo + ["fmt", "--manifest-path", "spikes/p1-e/shared/Cargo.toml", "--", "--check"]),
                  ("nsh-fmt", cargo + ["fmt", "--manifest-path", "spikes/p1-e/nsh/Cargo.toml", "--", "--check"]),
                  ("nsh-clippy", cargo + ["clippy", "--manifest-path", "spikes/p1-e/nsh/Cargo.toml", "--all-targets", "--features", "native,tui", *common, "--", "-D", "warnings"])]
        for name, manifest, args in [
            ("model-tests", "shared", ["test", "--features", "native"]),
            ("snapshot-build", "shared", ["build", "--features", "native", "--bin", "acp-ui-snapshot"]),
            ("nsh-components", "nsh", ["test", "--lib"]),
            ("nsh-host-release", "nsh", ["build", "--release", "--features", "native,tui", "--bins"]),
        ]:
            command = cargo + [*args, "--manifest-path", f"spikes/p1-e/{manifest}/Cargo.toml", *common]
            if name == "nsh-components":
                command += ["--", "--nocapture"]
            items.append((name, command))
        items += [("ui-contract", [PY, "tools/test_ui_spike_contract.py"]),
                  ("local-package", [PY, "tools/package_local_console.py"])]
    return items


def unavailable(output):
    # Compiler errors/test failures are never classified as missing prerequisites.
    if re.search(r"error\[E\d+\]|test result: FAILED|FAILED \((?:failures|errors)=", output):
        return False
    return any(s in output for s in [
        "attempting to make an HTTP request, but --offline was specified",
        "you are in the offline mode", "offline mode (via `--offline`)",
        "no matching package named", "failed to download", "Unable to update",
        "is not installed", "no release found for", "could not find system library",
        "The system library `", "No module named 'pyte'", "No module named 'psutil'",
        "No module named 'winpty'",
        "No module named 'mss'", "failed to find tool", "could not execute process `rustc",
    ])


def run_gate(name, command, directory, environment):
    started = time.monotonic()
    path = directory / (name + ".log")
    try:
        with path.open("wb") as log:
            process = subprocess.run(command, cwd=ROOT, env=environment,
                                     stdout=log, stderr=subprocess.STDOUT, timeout=1800)
        output = path.read_text(encoding="utf-8", errors="replace")
        status = "PASS" if process.returncode == 0 else ("NOT_RUN" if unavailable(output) else "FAIL")
        # Tests that explicitly report unavailable desktop/SDK cannot become PASS.
        if process.returncode == 0 and re.search(r"^NOT_RUN:", output, re.MULTILINE):
            status = "NOT_RUN"
        reason = "dependency/toolchain/cache unavailable" if status == "NOT_RUN" else None
        if process.returncode == 0 and status == "NOT_RUN":
            reason = "one or more subchecks explicitly NOT_RUN; inspect the private local log"
        counts = re.findall(r"(?:test result: ok\. (\d+) passed|Ran (\d+) tests?|PASS: (\d+) (?:Python/Rust|exact SQLite))", output)
        count = sum(int(next(x for x in group if x)) for group in counts)
        result = dict(name=name, command=command, status=status, exit_code=process.returncode,
                      seconds=round(time.monotonic() - started, 3), observed_count=count)
        if reason:
            result["reason"] = reason
    except (FileNotFoundError, subprocess.TimeoutExpired) as error:
        result = dict(name=name, command=command, status="NOT_RUN" if isinstance(error, FileNotFoundError) else "FAIL",
                      reason="executable unavailable" if isinstance(error, FileNotFoundError) else "gate timed out",
                      seconds=round(time.monotonic() - started, 3))
    print(f"{name}: {result['status']} ({result['seconds']}s)", flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for switch in ("core", "service", "ui", "all", "local-project"):
        parser.add_argument("--" + switch, action="store_true")
    parser.add_argument("--report-dir", type=Path, default=ROOT / "artifacts/local-p1-e2")
    args = parser.parse_args()
    if not any((args.core, args.service, args.ui, args.all, args.local_project)):
        parser.error("select --core, --service, --ui, --all or --local-project")
    directory = args.report_dir.resolve()
    # Do not write diagnostics into any operator-selected checkout.
    external = os.environ.get("ACP_LOCAL_PROJECT_ROOT")
    if external and directory.is_relative_to(Path(external).resolve()):
        parser.error("report directory must be outside the selected checkout")
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    if os.name != "nt":
        directory.chmod(0o700)
    env = {**os.environ, "PYTHONPATH": os.pathsep.join(filter(None, [str(ROOT / "src"), os.environ.get("PYTHONPATH")])), "CARGO_NET_OFFLINE": "true", "RUSTUP_OFFLINE": "1"}
    results = []
    versions = {}
    for name, command in [("python", [PY, "--version"]), ("rustc", ["rustc", "--version"]),
                          ("cargo", ["cargo", "--version"]), ("codex", ["codex", "--version"])]:
        try:
            p = subprocess.run(command, capture_output=True, text=True, timeout=20, env=env)
            versions[name] = p.stdout.strip() if p.returncode == 0 else "NOT_RUN"
        except (OSError, subprocess.TimeoutExpired):
            versions[name] = "NOT_RUN"
    prerequisites = {"json-parity": "parity-build", "sqlite-parity": "parity-build", "service-e2e": "service-build", "ui-contract": "snapshot-build", "local-package": "nsh-host-release"}
    for name, command in gates(args.core or args.all, args.service or args.all, args.ui or args.all):
        dependency = prerequisites.get(name)
        if dependency and any(r["name"] == dependency and r["status"] != "PASS" for r in results):
            results.append(dict(name=name, command=command, status="NOT_RUN", reason=f"prerequisite {dependency} unavailable or failed"))
            continue
        results.append(run_gate(name, command, directory, env))
    if args.ui or args.all:
        suffix = ".exe" if os.name == "nt" else ""
        for name, script, binary in [
            ("tui-pty", "tools/test_local_console_tui.py", "acp-nsh-tui"),
            ("desktop-smoke", "tools/test_local_console_native.py", "acp-nsh-native"),
        ]:
            command = [PY, script, "--binary", f"spikes/p1-e/nsh/target/release/{binary}{suffix}", "--out", str(directory)]
            if any(r["name"] == "nsh-host-release" and r["status"] != "PASS" for r in results):
                results.append(dict(name=name, command=command, status="NOT_RUN", reason="fresh host release build did not pass"))
            elif name == "desktop-smoke" and platform.system() == "Linux" and not os.environ.get("DISPLAY"):
                results.append(dict(name=name, command=command, status="NOT_RUN", reason="no desktop DISPLAY"))
            elif not Path(command[3]).exists():
                results.append(dict(name=name, command=command, status="NOT_RUN", reason="release executable unavailable"))
            else:
                results.append(run_gate(name, command, directory, env))
        results += [dict(name="web-smoke", status="NOT_RUN", reason="optional offline synthetic web smoke not selected"),
                    dict(name="desktop-ime-screen-reader", status="NOT_RUN", reason="requires operator assistive technology session")]
    if args.local_project:
        if not external:
            results.append(dict(name="local-project", status="NOT_RUN", reason="ACP_LOCAL_PROJECT_ROOT not explicitly set"))
        else:
            results.append(run_gate("local-project", [PY, "tools/test_local_project.py", "--out", str(directory)], directory, env))
    sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, check=True).stdout)
    report = dict(tested_git_sha=sha, worktree_dirty=dirty, os=platform.system(), os_release=platform.release(), os_version=platform.version(), architecture=platform.machine(),
                  versions=versions, gates=results, cloud_ci="NOT_RUN: local-only; push commits with [skip ci]",
                  totals={status: sum(r["status"] == status for r in results) for status in ["PASS", "FAIL", "NOT_RUN"]})
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report["totals"]))
    return 1 if any(r["status"] == "FAIL" for r in results) else 0


if __name__ == "__main__":
    raise SystemExit(main())
