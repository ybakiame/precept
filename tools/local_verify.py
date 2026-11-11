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
            (
                "root-clippy",
                cargo
                + [
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    *common,
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
            ("native-test-build", cargo + ["build", "--workspace", "--bins", *common]),
            ("root-tests", cargo + ["test", "--workspace", "--all-targets", *common]),
            ("python", [PY, "-m", "unittest", "discover", "-s", "tests", "-v"]),
            ("parity-build", cargo + ["build", "--workspace", "--examples", *common]),
            ("json-parity", [PY, "tools/check_p1_parity.py"]),
            ("sqlite-parity", [PY, "tools/check_sqlite_parity.py"]),
            (
                "backup-rehearsal",
                cargo
                + [
                    "run",
                    "-p",
                    "acp-rehearsal",
                    "--example",
                    "synthetic_rehearsal",
                    *common,
                    "--",
                    "--synthetic-only",
                ],
            ),
            (
                "process-recovery",
                cargo
                + [
                    "test",
                    "-p",
                    "acp-rehearsal",
                    "--features",
                    "recovery-tests",
                    "--test",
                    "process_recovery",
                    *common,
                ],
            ),
        ]
    if service or ui:
        items.append(
            (
                "service-build",
                cargo + ["build", "--workspace", "--bins", "--examples", *common],
            )
        )
    if service:
        items.append(("service-e2e", [PY, "tools/test_local_service.py", "-v"]))
    if ui:
        items += [
            (
                "model-fmt",
                cargo
                + [
                    "fmt",
                    "--manifest-path",
                    "spikes/p1-e/shared/Cargo.toml",
                    "--",
                    "--check",
                ],
            ),
            (
                "nsh-fmt",
                cargo
                + [
                    "fmt",
                    "--manifest-path",
                    "spikes/p1-e/nsh/Cargo.toml",
                    "--",
                    "--check",
                ],
            ),
            (
                "nsh-clippy",
                cargo
                + [
                    "clippy",
                    "--manifest-path",
                    "spikes/p1-e/nsh/Cargo.toml",
                    "--all-targets",
                    "--features",
                    "native,tui",
                    *common,
                    "--",
                    "-D",
                    "warnings",
                ],
            ),
        ]
        for name, manifest, args in [
            ("model-tests", "shared", ["test", "--features", "native"]),
            (
                "snapshot-build",
                "shared",
                ["build", "--features", "native", "--bin", "acp-ui-snapshot"],
            ),
            ("nsh-components", "nsh", ["test", "--lib"]),
            (
                "nsh-host-release",
                "nsh",
                ["build", "--release", "--features", "native,tui", "--bins"],
            ),
        ]:
            command = cargo + [
                *args,
                "--manifest-path",
                f"spikes/p1-e/{manifest}/Cargo.toml",
                *common,
            ]
            if name == "nsh-components":
                command += ["--", "--nocapture"]
            items.append((name, command))
        items += [
            ("ui-contract", [PY, "tools/test_ui_spike_contract.py"]),
        ]
    return items


def unavailable(output):
    # Compiler errors/test failures are never classified as missing prerequisites.
    if re.search(
        r"error\[E\d+\]|test result: FAILED|FAILED \((?:failures|errors)=", output
    ):
        return False
    return any(
        s in output
        for s in [
            "attempting to make an HTTP request, but --offline was specified",
            "you are in the offline mode",
            "offline mode (via `--offline`)",
            "no matching package named",
            "failed to download",
            "Unable to update",
            "is not installed",
            "no release found for",
            "could not find system library",
            "The system library `",
            "No module named 'pyte'",
            "No module named 'psutil'",
            "No module named 'winpty'",
            "No module named 'mss'",
            "failed to find tool",
            "could not execute process `rustc",
        ]
    )


def run_gate(name, command, directory, environment):
    started = time.monotonic()
    path = directory / (name + ".log")
    try:
        with path.open("wb") as log:
            process = subprocess.run(
                command,
                cwd=ROOT,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=1800,
            )
        output = path.read_text(encoding="utf-8", errors="replace")
        status = (
            "PASS"
            if process.returncode == 0
            else ("NOT_RUN" if unavailable(output) else "FAIL")
        )
        # Tests that explicitly report unavailable desktop/SDK cannot become PASS.
        if process.returncode == 0 and re.search(r"^NOT_RUN:", output, re.MULTILINE):
            status = "NOT_RUN"
        if process.returncode == 2 and re.search(r"^BLOCKED:", output, re.MULTILINE):
            status = "BLOCKED"
        reason = (
            "dependency/toolchain/cache unavailable" if status == "NOT_RUN" else None
        )
        if process.returncode == 0 and status == "NOT_RUN":
            reason = "one or more subchecks explicitly NOT_RUN; inspect the private local log"
        counts = re.findall(
            r"(?:test result: ok\. (\d+) passed|Ran (\d+) tests?|PASS: (\d+) (?:Python/Rust|exact SQLite))",
            output,
        )
        count = sum(int(next(x for x in group if x)) for group in counts)
        result = dict(
            name=name,
            command=command,
            status=status,
            exit_code=process.returncode,
            seconds=round(time.monotonic() - started, 3),
            observed_count=count,
        )
        if reason:
            result["reason"] = reason
    except (FileNotFoundError, subprocess.TimeoutExpired) as error:
        result = dict(
            name=name,
            command=command,
            status="NOT_RUN" if isinstance(error, FileNotFoundError) else "FAIL",
            reason=(
                "executable unavailable"
                if isinstance(error, FileNotFoundError)
                else "gate timed out"
            ),
            seconds=round(time.monotonic() - started, 3),
        )
    print(f"{name}: {result['status']} ({result['seconds']}s)", flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for switch in ("core", "service", "ui", "all", "local-project", "wslg", "ui-full"):
        parser.add_argument("--" + switch, action="store_true")
    parser.add_argument("--migration-mutations", action="store_true")
    parser.add_argument(
        "--owner-reviewed",
        action="store_true",
        help="record an already completed explicit owner UI review",
    )
    parser.add_argument(
        "--windows-report",
        type=Path,
        help="private clean same-SHA Windows companion report",
    )
    parser.add_argument(
        "--report-dir", type=Path, default=ROOT / "artifacts/local-p1-e2"
    )
    args = parser.parse_args()
    if not any(
        (
            args.core,
            args.service,
            args.ui,
            args.all,
            args.local_project,
            args.wslg,
            args.ui_full,
        )
    ):
        parser.error(
            "select --core, --service, --ui, --all, --wslg, --ui-full or --local-project"
        )
    directory = args.report_dir.resolve()
    # Do not write diagnostics into any operator-selected checkout.
    external = os.environ.get("ACP_LOCAL_PROJECT_ROOT")
    if external and directory.is_relative_to(Path(external).resolve()):
        parser.error("report directory must be outside the selected checkout")
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    if os.name != "nt":
        directory.chmod(0o700)
    env = {
        **os.environ,
        "PYTHONPATH": os.pathsep.join(
            filter(None, [str(ROOT / "src"), os.environ.get("PYTHONPATH")])
        ),
        "CARGO_NET_OFFLINE": "true",
        "RUSTUP_OFFLINE": "1",
        "ACP_TEST_PYTHON": PY,
    }
    results = []
    versions = {}
    for name, command in [
        ("python", [PY, "--version"]),
        ("rustc", ["rustc", "--version"]),
        ("cargo", ["cargo", "--version"]),
        ("codex", ["codex", "--version"]),
    ]:
        try:
            p = subprocess.run(
                command, capture_output=True, text=True, timeout=20, env=env
            )
            versions[name] = p.stdout.strip() if p.returncode == 0 else "NOT_RUN"
        except (OSError, subprocess.TimeoutExpired):
            versions[name] = "NOT_RUN"
    prerequisites = {
        "root-tests": "native-test-build",
        "json-parity": "parity-build",
        "sqlite-parity": "parity-build",
        "service-e2e": "service-build",
        "ui-contract": "snapshot-build",
        "local-package": "nsh-host-release",
    }
    ui = args.ui or args.all or args.ui_full or args.wslg
    wslg = args.wslg or args.ui_full
    for name, command in gates(
        args.core or args.all or args.ui_full,
        args.service or args.all or args.ui_full,
        ui,
    ):
        dependency = prerequisites.get(name)
        if dependency and any(
            r["name"] == dependency and r["status"] != "PASS" for r in results
        ):
            results.append(
                dict(
                    name=name,
                    command=command,
                    status="NOT_RUN",
                    reason=f"prerequisite {dependency} unavailable or failed",
                )
            )
            continue
        results.append(run_gate(name, command, directory, env))
    if args.migration_mutations or args.ui_full:
        results.append(
            run_gate(
                "rust-migration-mutations",
                [
                    PY,
                    "tools/check_rust_migration_mutations.py",
                    "--out",
                    str(directory / "mutations"),
                ],
                directory,
                env,
            )
        )
    if ui:
        suffix = ".exe" if os.name == "nt" else ""
        for name, script, binary in [
            ("tui-pty", "tools/test_local_console_tui.py", "acp-nsh-tui"),
            ("desktop-smoke", "tools/test_local_console_native.py", "acp-nsh-native"),
        ]:
            command = [
                PY,
                script,
                "--binary",
                f"spikes/p1-e/nsh/target/release/{binary}{suffix}",
                "--out",
                str(directory),
            ]
            if any(
                r["name"] == "nsh-host-release" and r["status"] != "PASS"
                for r in results
            ):
                results.append(
                    dict(
                        name=name,
                        command=command,
                        status="NOT_RUN",
                        reason="fresh host release build did not pass",
                    )
                )
            elif (
                name == "desktop-smoke"
                and platform.system() == "Linux"
                and not os.environ.get("WAYLAND_DISPLAY")
            ):
                results.append(
                    dict(
                        name=name,
                        command=command,
                        status="NOT_RUN",
                        reason="no native Wayland session; X11 fallback forbidden",
                    )
                )
            elif not Path(command[3]).exists():
                results.append(
                    dict(
                        name=name,
                        command=command,
                        status="NOT_RUN",
                        reason="release executable unavailable",
                    )
                )
            else:
                if name == "desktop-smoke" and platform.system() == "Linux":
                    name = "wayland-native"
                results.append(run_gate(name, command, directory, env))
        for name, command in [
            ("terminal-cells", [PY, "tools/test_terminal_screen.py"]),
            (
                "p1f-settings",
                [
                    PY,
                    "tools/test_brand_settings.py",
                    "--product",
                    "--desktop",
                    f"spikes/p1-e/nsh/target/release/acp-nsh-native{suffix}",
                    "--tui",
                    f"spikes/p1-e/nsh/target/release/acp-nsh-tui{suffix}",
                    "--out",
                    str(directory / "settings"),
                ],
            ),
            (
                "p1f-chrome",
                [
                    PY,
                    "tools/test_brand_chrome.py",
                    "--product",
                    "--binary",
                    f"spikes/p1-e/nsh/target/release/acp-nsh-native{suffix}",
                    "--out",
                    str(directory / "chrome"),
                ],
            ),
            (
                "p1f-previews",
                [
                    PY,
                    "tools/capture_product_previews.py",
                    "--desktop",
                    f"spikes/p1-e/nsh/target/release/acp-nsh-native{suffix}",
                    "--tui",
                    f"spikes/p1-e/nsh/target/release/acp-nsh-tui{suffix}",
                    "--locale",
                    "zh-CN",
                    "--out",
                    str(directory / "previews"),
                ],
            ),
        ]:
            if not any(
                directory.is_relative_to(ROOT / folder)
                for folder in [
                    "artifacts/local-p1-f",
                    "artifacts/local-p1-g",
                    "artifacts/local-p1-h",
                    "artifacts/local-p1-i",
                ]
            ):
                results.append(
                    dict(
                        name=name,
                        status="NOT_RUN",
                        reason="Product captures require a private P1-F/P1-G report directory; legacy gates remain available",
                    )
                )
            elif any(
                r["name"] == "nsh-host-release" and r["status"] == "PASS"
                for r in results
            ):
                results.append(run_gate(name, command, directory, env))
            else:
                results.append(
                    dict(
                        name=name,
                        status="NOT_RUN",
                        reason="fresh host release unavailable",
                    )
                )
        if directory.is_relative_to(ROOT / "artifacts/local-p1-i"):
            results.append(
                run_gate(
                    "lease-unit",
                    ["cargo", "test", "-p", "acp-ipc", "--offline", "--locked"],
                    directory,
                    env,
                )
            )
            results.append(
                run_gate(
                    "context-process",
                    [PY, "tools/test_session_context.py", "-v"],
                    directory,
                    env,
                )
            )
        if any(
            directory.is_relative_to(ROOT / folder)
            for folder in [
                "artifacts/local-p1-g",
                "artifacts/local-p1-h",
                "artifacts/local-p1-i",
            ]
        ):
            results.append(
                run_gate(
                    "agent-session-process",
                    [PY, "tools/test_agent_session.py", "-v"],
                    directory,
                    env,
                )
            )
            results.append(
                run_gate(
                    "agent-session-ui",
                    [
                        PY,
                        "tools/test_agent_session_ui.py",
                        "--desktop",
                        f"spikes/p1-e/nsh/target/release/acp-nsh-native{suffix}",
                        "--tui",
                        f"spikes/p1-e/nsh/target/release/acp-nsh-tui{suffix}",
                        "--out",
                        str(directory / "sessions"),
                        *(
                            ["--auth"]
                            if any(
                                directory.is_relative_to(ROOT / folder)
                                for folder in [
                                    "artifacts/local-p1-h",
                                    "artifacts/local-p1-i",
                                ]
                            )
                            else []
                        ),
                        *(
                            ["--context"]
                            if directory.is_relative_to(ROOT / "artifacts/local-p1-i")
                            else []
                        ),
                    ],
                    directory,
                    env,
                )
            )
        if any(
            directory.is_relative_to(ROOT / folder)
            for folder in ["artifacts/local-p1-h", "artifacts/local-p1-i"]
        ):
            results.append(
                run_gate(
                    "real-runner-guards",
                    [
                        PY,
                        "-m",
                        "unittest",
                        "discover",
                        "-s",
                        "tools",
                        "-p",
                        "test_real_acp_runner.py",
                        "-v",
                    ],
                    directory,
                    env,
                )
            )
        package = [
            PY,
            "tools/package_local_console.py",
            "--out",
            str(directory / "package"),
        ]
        if platform.system() == "Linux":
            package += ["--wayland-report", str(directory / "wayland-result.json")]
        package_ready = any(
            r["name"] == "nsh-host-release" and r["status"] == "PASS" for r in results
        )
        if platform.system() == "Linux":
            package_ready &= any(
                r["name"] == "wayland-native" and r["status"] == "PASS" for r in results
            )
        if package_ready:
            results.append(run_gate("local-package", package, directory, env))
        else:
            results.append(
                dict(
                    name="local-package",
                    status="NOT_RUN",
                    reason="fresh release and required host render acceptance did not pass",
                )
            )
        results += [
            dict(
                name="web-smoke",
                status="NOT_RUN",
                reason="optional offline synthetic web smoke not selected",
            ),
            dict(
                name="desktop-ime-screen-reader",
                status="NOT_RUN",
                reason="requires operator assistive technology session",
            ),
        ]
    if wslg:
        if platform.system() == "Linux":
            results.append(
                run_gate(
                    "wayland-doctor",
                    [
                        PY,
                        "tools/wslg_doctor.py",
                        "--probe",
                        "--out",
                        str(directory / "doctor"),
                    ],
                    directory,
                    env,
                )
            )
        else:
            results.append(
                dict(
                    name="wayland-doctor",
                    status="NOT_RUN",
                    reason="run --wslg inside the explicit WSL owner session",
                )
            )
    if args.ui_full:
        results.append(
            run_gate(
                "codex-synthetic",
                [PY, "tools/test_console_integrations.py", "--out", str(directory)],
                directory,
                env,
            )
        )
    if args.local_project:
        if not external:
            results.append(
                dict(
                    name="local-project",
                    status="NOT_RUN",
                    reason="ACP_LOCAL_PROJECT_ROOT not explicitly set",
                )
            )
        else:
            results.append(
                run_gate(
                    "local-project",
                    [PY, "tools/test_local_project.py", "--out", str(directory)],
                    directory,
                    env,
                )
            )
    sha = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    dirty = bool(
        subprocess.run(
            ["git", "status", "--porcelain"], cwd=ROOT, capture_output=True, check=True
        ).stdout
    )
    report = dict(
        tested_git_sha=sha,
        worktree_dirty=dirty,
        os=platform.system(),
        os_release=platform.release(),
        os_version=platform.version(),
        architecture=platform.machine(),
        versions=versions,
        gates=results,
        cloud_ci="NOT_RUN: local-only; push commits with [skip ci]",
        totals={
            status: sum(r["status"] == status for r in results)
            for status in ["PASS", "FAIL", "BLOCKED", "NOT_RUN"]
        },
    )
    wayland_pass = any(
        r["name"] == "wayland-native" and r["status"] == "PASS" for r in results
    )
    required_failure = any(r["status"] in ("FAIL", "BLOCKED") for r in results)
    required = {
        "root-tests",
        "python",
        "json-parity",
        "sqlite-parity",
        "process-recovery",
        "service-e2e",
        "model-tests",
        "nsh-components",
        "ui-contract",
        "tui-pty",
        "wayland-native",
        "codex-synthetic",
    }
    passed = {r["name"] for r in results if r["status"] == "PASS"}
    report["e3_acceptance"] = (
        "PASS_LOCAL_WSL"
        if wayland_pass and required <= passed and not required_failure
        else "INCOMPLETE"
    )
    migration_markers = {
        "fingerprint": "v1_oracle_clean_staged_unstaged_binary_rename_unicode_and_head_branch",
        "untracked": "v1_oracle_untracked_same_name_count_changed_bytes_and_modes",
        "limits": "v1_oracle_aggregate_file_count_limits_and_missing_head",
        "on_demand": "fingerprint_is_requested_only_after_valid_handoff_scope_check",
        "confirmed": "confirmed_only_statuses_and_complete_operator_parity",
        "documents": "global_override_and_documents_never_cross_scope",
        "injection": "project_selector_injection_and_malformed_arguments_are_refused",
        "tasks": "same_task_id_and_foreign_only_tasks_are_project_bound",
        "tools": "unregistered_unknown_and_mutating_tools_have_no_authority",
        "protocol": "real_stdio_two_schema_versions_and_notifications_remain_compatible",
        "authority": "bound_ipc_agents_cannot_list_or_escalate_after_restart_and_rotation",
    }
    if os.name == "nt":
        migration_markers["platform_paths"] = (
            "v1_oracle_windows_symlink_targets_modes_and_outside_parent"
        )
    else:
        migration_markers["platform_paths"] = (
            "v1_oracle_non_utf8_symlink_targets_and_unsafe_parent_do_not_read_outside"
        )
    root_log = (
        (directory / "root-tests.log").read_text(errors="replace")
        if (directory / "root-tests.log").is_file()
        else ""
    )
    report["rust_migrations"] = {
        key: (
            "PASS"
            if "root-tests" in passed
            and re.search(r"test [^\n]*" + re.escape(test) + r" \.\.\. ok", root_log)
            else "NOT_RUN"
        )
        for key, test in migration_markers.items()
    }
    local_required = {
        "root-fmt",
        "root-clippy",
        "native-test-build",
        "root-tests",
        "python",
        "json-parity",
        "sqlite-parity",
        "backup-rehearsal",
        "process-recovery",
        "service-e2e",
        "model-fmt",
        "nsh-fmt",
        "nsh-clippy",
        "model-tests",
        "nsh-components",
        "nsh-host-release",
        "ui-contract",
        "tui-pty",
        "local-package",
        "rust-migration-mutations",
    }
    local_required.add("desktop-smoke" if os.name == "nt" else "wayland-native")
    if os.name != "nt":
        local_required.update({"wayland-doctor", "codex-synthetic"})
    migrations_pass = all(v == "PASS" for v in report["rust_migrations"].values())
    local_pass = (
        local_required <= passed
        and migrations_pass
        and not required_failure
        and not dirty
    )
    report["e4_host_acceptance"] = (
        ("PASS_LOCAL_WINDOWS" if os.name == "nt" else "PASS_LOCAL_WSL")
        if local_pass
        else "INCOMPLETE"
    )
    report["p1f_host_checks"] = (
        "PASS"
        if local_pass
        and {"terminal-cells", "p1f-settings", "p1f-chrome", "p1f-previews"} <= passed
        else "INCOMPLETE"
    )
    if any(
        directory.is_relative_to(ROOT / folder)
        for folder in [
            "artifacts/local-p1-g",
            "artifacts/local-p1-h",
            "artifacts/local-p1-i",
        ]
    ):
        report["p1g_host_checks"] = (
            "PASS"
            if report["p1f_host_checks"] == "PASS"
            and {"agent-session-process", "agent-session-ui"} <= passed
            else "INCOMPLETE"
        )
        report["session_owner_ui_review"] = "PENDING"
        report["real_acp_agent"] = (
            "NOT_RUN by generic runner; see separate explicit opt-in smoke report"
        )
    if any(
        directory.is_relative_to(ROOT / folder)
        for folder in ["artifacts/local-p1-h", "artifacts/local-p1-i"]
    ):
        report["p1h_host_checks"] = (
            "PASS"
            if report.get("p1g_host_checks") == "PASS"
            and "real-runner-guards" in passed
            else "INCOMPLETE"
        )
        report["session_owner_ui_review"] = "DEFERRED_BY_OWNER_AT_PR25"
        report["context_mcp_bridge"] = (
            "NOT_RUN: explicitly not enabled; mcpServers empty"
        )
    if directory.is_relative_to(ROOT / "artifacts/local-p1-i"):
        report["p1i_host_checks"] = (
            "PASS"
            if report.get("p1h_host_checks") == "PASS"
            and {"lease-unit", "context-process"} <= passed
            else "INCOMPLETE"
        )
        report["context_mcp_bridge"] = (
            "PASS deterministic scoped calls/revocation"
            if "context-process" in passed
            else "INCOMPLETE"
        )
        report["real_context_agent"] = (
            "NOT_RUN by generic runner; explicit opt-in smoke is separate"
        )
    report["owner_ui_review"] = "ACCEPTED_BASE_UI" if args.owner_reviewed else "PENDING"
    report["windows_companion"] = "NOT_RUN"
    if args.windows_report:
        companion = json.loads(args.windows_report.read_text(encoding="utf-8"))
        report["windows_companion"] = (
            "PASS"
            if companion.get("tested_git_sha") == sha
            and companion.get("worktree_dirty") is False
            and companion.get("e4_host_acceptance") == "PASS_LOCAL_WINDOWS"
            else "INCOMPLETE"
        )
    report["e4_acceptance"] = (
        "PASS_LOCAL_WINDOWS_WSL_OWNER_REVIEWED"
        if os.name != "nt"
        and local_pass
        and report["windows_companion"] == "PASS"
        and args.owner_reviewed
        else "INCOMPLETE"
    )
    report["acceptance_scope"] = (
        "this host and explicitly configured clients; Windows host report is separate"
    )
    report["niri"] = "NOT_RUN: no active niri session selected"
    (directory / "report.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(report["totals"]))
    return (
        1
        if any(r["status"] == "FAIL" for r in results)
        else (
            2
            if any(r["status"] == "BLOCKED" for r in results)
            or (wslg and not wayland_pass)
            else 0
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
