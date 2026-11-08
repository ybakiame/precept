"""Compare the Rust core with the unmodified Python P0 using synthetic state only.

Run after cargo build --locked --example contract_reader:
  PYTHONPATH=src python tools/check_p1_parity.py
No real SQLite files, migrations or client configuration are accessed.
"""
from __future__ import annotations
import argparse
import copy
import json
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))
from test_p1_reference_contract import P1ReferenceContractTests  # noqa: E402
from agent_control_plane.mcp import MCPServer  # noqa: E402


def normalize(value):
    """Omit storage IDs, timestamps and host roots; retain relative document paths."""
    if isinstance(value, dict):
        return {k: normalize(v) for k, v in value.items()
                if k not in {"id", "created_at", "updated_at", "root"}}
    if isinstance(value, list):
        return [normalize(v) for v in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, default=ROOT / "target/debug/examples/contract_reader")
    args = parser.parse_args()
    oracle = P1ReferenceContractTests()
    oracle.setUp()
    count = 0
    try:
        store = oracle.store
        # Identical task IDs and a task existing only in beta test scope isolation.
        for project, task in [("alpha", "shared"), ("beta", "shared"), ("beta", "beta-only")]:
            store.save_handoff(project, task, f"synthetic-{project}", "inspect", "base", "main", "clean", "",
                               [{"check": status, "status": status} for status in ["PASS", "FAIL", "SKIPPED", "NOT_RUN"]],
                               worktree_fingerprint="saved")
        store.save_handoff("alpha", "legacy", "legacy", "inspect", "base", "main", "clean", "", [])
        preferences = []
        for row in store.db.execute("SELECT * FROM preferences"):
            item = dict(row)
            item["key"] = item.pop("pref_key")
            preferences.append(item)
        projects = store.projects()
        handoffs = [store.handoff(p, t) for p, t in [("alpha", "shared"), ("beta", "shared"), ("beta", "beta-only"), ("alpha", "legacy")]]
        requests = []
        for project in oracle.data["projects"]:
            for include in [False, True]:
                requests.append(dict(project=project, name="get_effective_context", arguments={"include_documents": include}))
        for call in oracle.data["must_reject_tool_calls"]:
            requests.append(dict(project="alpha", **call))
        for name, arguments in [("get_effective_context", {"project_id": "beta"}),
                                ("get_effective_context", {"include_documents": "true"}),
                                ("get_effective_context", None),
                                ("get_handoff", {"task_id": 1}),
                                ("get_handoff", {}),
                                ("get_handoff", {"task_id": "shared", "project_id": "beta"})]:
            requests.append(dict(project="alpha", name=name, arguments=arguments))
        for p, task in [("alpha", "shared"), ("beta", "shared"), ("alpha", "beta-only"), ("alpha", "legacy"), ("alpha", "missing")]:
            requests.append(dict(project=p, name="get_handoff", arguments={"task_id": task}))
        requests.append(dict(project="unknown", name="get_effective_context", arguments={}))
        snapshots = [
            dict(head="base", branch="main", dirty=False, change_count=0, worktree_fingerprint="saved"),
            dict(head="later", branch="other", dirty=True, change_count=2, worktree_fingerprint="changed"),
            dict(head=None, branch=None, dirty=None, change_count=None, worktree_fingerprint=None),
            dict(head="base", branch="main", dirty=False, change_count=0, worktree_fingerprint=None),
        ]
        def run(snapshot):
            nonlocal count
            data = dict(projects=projects, preferences=preferences, handoffs=handoffs,
                        worktrees={p["name"]: snapshot for p in projects}, requests=requests)
            expected = []
            def inspect(_root, *, include_fingerprint=False):
                state = copy.deepcopy(snapshot)
                if not include_fingerprint:
                    state.pop("worktree_fingerprint")
                return state
            with patch("agent_control_plane.context.worktree_snapshot", inspect):
                for r in requests:
                    try:
                        response = oracle.call(MCPServer(store, r["project"]), r["name"], r["arguments"])
                        if "error" in response or response["result"].get("isError"):
                            expected.append({"error": True})
                        else:
                            expected.append({"data": json.loads(response["result"]["content"][0]["text"])})
                    except ValueError:
                        expected.append({"error": True})
            result = subprocess.run([str(args.reader.resolve())], input=json.dumps(data), text=True,
                                    capture_output=True, encoding="utf-8", check=True, timeout=30)
            actual = json.loads(result.stdout)
            assert len(actual) == len(expected)
            for request, rust, python in zip(requests, actual, expected):
                assert normalize(rust) == normalize(python), (request, rust, python)
                if request["project"] == "alpha" and "data" in rust:
                    for marker in oracle.data["forbidden_alpha_strings"]:
                        assert marker not in json.dumps(rust)
            count += len(requests)
        for snapshot in snapshots:
            run(snapshot)
        # Live reread, exact document size boundary, invalid UTF-8, and outside symlink.
        alpha = Path(store.project("alpha")["root"])
        (alpha / "README.md").write_bytes(b"x" * 16384)
        (alpha / "docs").mkdir()
        (alpha / "docs/index.md").write_bytes(b"x" * 16385)
        (alpha / "docs/README.md").write_bytes(b"\xffinvalid\xfe")
        run(snapshots[0])
        (alpha / "README.md").unlink()
        try:
            (alpha / "README.md").symlink_to(Path(store.project("beta")["root"]) / "AGENTS.md")
        except (OSError, NotImplementedError):
            print("outside symlink parity: NOT_RUN on this platform")
        else:
            run(snapshots[0])
        print(f"PASS: {count} Python/Rust request comparisons; shared v1 fixture, handoffs, malformed calls and live documents")
    finally:
        oracle.doCleanups()


if __name__ == "__main__":
    main()
