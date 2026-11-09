"""Bounded guard mutation checks in an owned source copy; never edit source/user data."""

from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
    mutations = [
        (
            "fingerprint-unconditionally-null",
            "crates/acp-daemon/src/worktree.rs",
            "if safe && include_fingerprint && head.is_some()",
            "if false && safe && include_fingerprint && head.is_some()",
            [
                "-p",
                "acp-daemon",
                "v1_oracle_clean_staged_unstaged_binary_rename_unicode_and_head_branch",
            ],
        ),
        (
            "unconfirmed-core-guard",
            "crates/acp-core/src/lib.rs",
            "p.status == Status::Confirmed",
            "true",
            [
                "-p",
                "acp-core",
                "--test",
                "read_only",
                "shared_v1_preferences_documents_and_forbidden_markers",
            ],
        ),
        (
            "agent-can-list-projects",
            "crates/acp-daemon/src/main.rs",
            "if access != Access::Operator {",
            "if false && access != Access::Operator {",
            [
                "-p",
                "acp-mcp",
                "--test",
                "security_process",
                "bound_ipc_agents_cannot_list_or_escalate_after_restart_and_rotation",
            ],
        ),
    ]
    results = []
    with tempfile.TemporaryDirectory(prefix="acp-e4-mutants-") as raw:
        checkout = Path(raw) / "checkout"
        checkout.mkdir()
        tracked = subprocess.check_output(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=ROOT,
        ).split(b"\0")
        for item in tracked:
            if not item:
                continue
            relative = Path(os.fsdecode(item))
            source = ROOT / relative
            if not source.is_file():
                continue
            assert source.resolve().is_relative_to(ROOT)
            target = checkout / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        env = {
            **os.environ,
            "CARGO_NET_OFFLINE": "true",
            "PYTHONPATH": str(checkout / "src"),
        }
        # One stable private checkout/target, never shared with the live checkout.
        env.pop("CARGO_TARGET_DIR", None)
        for name, relative, before, after, selected in mutations:
            file = checkout / relative
            original = file.read_text()
            assert original.count(before) == 1, "mutation anchor drifted"
            file.write_text(original.replace(before, after))
            started = time.monotonic()
            try:
                with (args.out / (name + ".log")).open("wb") as log:
                    build = subprocess.run(
                        [
                            "cargo",
                            "build",
                            "--workspace",
                            "--bins",
                            "--offline",
                            "--locked",
                        ],
                        cwd=checkout,
                        env=env,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                        timeout=300,
                    )
                    assert (
                        build.returncode == 0
                    ), "mutated code must compile; compile failure is not guard evidence"
                    test = subprocess.run(
                        [
                            "cargo",
                            "test",
                            *selected,
                            "--offline",
                            "--locked",
                            "--",
                            "--nocapture",
                        ],
                        cwd=checkout,
                        env=env,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                        timeout=120,
                    )
                output = (args.out / (name + ".log")).read_text(errors="replace")
                assert (
                    test.returncode != 0
                    and "test result: FAILED" in output
                    and "panicked" in output
                ), "guard mutation survived or failed for an unrelated prerequisite"
                results.append(
                    dict(
                        name=name,
                        status="PASS",
                        mutant_rejected=True,
                        seconds=round(time.monotonic() - started, 3),
                    )
                )
            finally:
                file.write_text(original)
    (args.out / "mutations.json").write_text(json.dumps(results, indent=2) + "\n")
    print(
        "PASS: "
        + str(len(results))
        + " compiled Rust mutations rejected by fingerprint/core/real-process authority tests"
    )


if __name__ == "__main__":
    main()
