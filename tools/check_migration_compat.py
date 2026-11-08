"""Called by the Rust-owned temporary rehearsal; never a profile migration tool.

Only accepts --synthetic-only and the owned temporary fixture root. Tests actual
Python Store initialization/writes on v1, plus exact Rust read parity.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from contextlib import closing
from unittest.mock import patch
from check_sqlite_parity import ReadOnlyOracle
from agent_control_plane.store import Store
from agent_control_plane.context import effective_context, inspect_handoff


def snapshot(path):
    with ReadOnlyOracle(path) as store:
        version = store.db.execute("PRAGMA user_version").fetchone()[0]
        rows = {}
        for table, order in [("projects", "name"), ("preferences", "id"), ("handoffs", "project,task_id"), ("sqlite_sequence", "name")]:
            rows[table] = [dict(row) for row in store.db.execute(f"SELECT * FROM {table} ORDER BY {order}")]
        for handoff in rows["handoffs"]:
            handoff.setdefault("worktree_fingerprint", None)
        return version, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--synthetic-only", action="store_true", required=True)
    parser.add_argument("workspace", type=Path)
    args = parser.parse_args()
    raw_root = args.workspace.absolute()
    root = raw_root.resolve(strict=True)
    assert not raw_root.is_symlink()
    assert root.parent == Path(tempfile.gettempdir()).resolve()
    assert root.name.startswith("acp-synthetic-rehearsal-")
    paths = [root / "fixture/state/context.sqlite3", root / "backup.sqlite3",
             root / "output/context.sqlite3", root / "restored.sqlite3"]
    for path in paths:
        current = root
        for part in path.relative_to(root).parts:
            current = current / part
            assert not current.is_symlink()
            assert not getattr(current, "is_junction", lambda: False)()
        assert path.resolve(strict=True).is_relative_to(root)
        assert path.stat().st_nlink == 1
    source, backup, output, restored = paths
    original = snapshot(source)
    assert original[0] == 0
    assert snapshot(backup) == original
    assert snapshot(restored) == original
    assert snapshot(output) == (1, original[1])
    backup_bytes = backup.read_bytes()
    with closing(sqlite3.connect(output)) as db:
        schema_before = list(db.execute("SELECT name,sql FROM sqlite_schema ORDER BY name"))
    # This is the actual current Python constructor, not the read-only facade.
    with Store(output.parent) as store:
        assert snapshot(output) == (1, original[1])
        with closing(sqlite3.connect(output)) as db:
            assert list(db.execute("SELECT name,sql FROM sqlite_schema ORDER BY name")) == schema_before
        pref = store.suggest_preference("migration.rehearsal", "synthetic", "rehearsal", "alpha")
        assert all(r["id"] != pref for r in store.preferences("alpha"))
        store.change_preference(pref, "confirmed")
        assert any(r["id"] == pref for r in store.resolve_preferences("alpha"))
        store.change_preference(pref, "deprecated")
        reject = store.suggest_preference("migration.reject", "synthetic", "rehearsal")
        store.change_preference(reject, "rejected")
        store.save_handoff("alpha", "python-v1", "synthetic", "inspect", "base", "main", "clean", "",
                           [{"check": "python", "status": "PASS", "evidence": {"id": "preserved", "nested": [None, True]}}])
    current_version, current = snapshot(output)
    assert current_version == 1
    assert current["projects"] == original[1]["projects"]
    for table in ["preferences", "handoffs"]:
        for row in original[1][table]:
            assert row in current[table], "Original metadata/evidence changed"
    assert snapshot(source) == original and snapshot(backup) == original
    assert backup.read_bytes() == backup_bytes
    # Run the compiled Rust read-only adapter on the exact Python-written v1 DB.
    repo = Path(__file__).resolve().parents[1]
    reader = repo / "target/debug/examples" / ("sqlite_reader.exe" if __import__("os").name == "nt" else "sqlite_reader")
    state = dict(head="base", branch="main", dirty=False, change_count=0, worktree_fingerprint=None)
    requests = [dict(project=p, name="get_effective_context", arguments={"include_documents": True}) for p in ["alpha", "beta"]]
    requests += [dict(project="alpha", name="get_handoff", arguments={"task_id": t}) for t in ["shared", "python-v1"]]
    expected = []
    def worktree(_root, *, include_fingerprint=False):
        return state.copy() if include_fingerprint else {k: v for k, v in state.items() if k != "worktree_fingerprint"}
    with ReadOnlyOracle(output) as store, patch("agent_control_plane.context.worktree_snapshot", worktree):
        for request in requests:
            data = (effective_context(store, request["project"], **request["arguments"])
                    if request["name"] == "get_effective_context" else
                    inspect_handoff(store, request["project"], request["arguments"]["task_id"]))
            expected.append({"data": data})
    result = subprocess.run([str(reader)], input=json.dumps(dict(database=str(output), requests=requests,
                            worktrees={"alpha": state, "beta": state})), capture_output=True,
                            text=True, encoding="utf-8", check=True, timeout=30)
    actual = json.loads(result.stdout)
    assert actual == {"schema": "Version1", "answers": expected}
    print("PASS: v1 actual Python constructor and writes, exact records/evidence and 4 Rust read comparisons")


if __name__ == "__main__":
    main()
