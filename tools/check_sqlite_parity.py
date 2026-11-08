"""Python/Rust parity on the same disposable SQLite files, never real profiles.

Build: cargo build --locked -p acp-store --example sqlite_reader
Run: PYTHONPATH=src python tools/check_sqlite_parity.py [--reader PATH]
"""
from __future__ import annotations
import argparse
import copy
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
from unittest.mock import patch

from sqlite_fixture import create_fixture, ROOT
from agent_control_plane.context import effective_context, inspect_handoff
from agent_control_plane.store import Store


class ReadOnlyOracle(Store):
    """Reuse P0 read methods without calling its schema-writing constructor."""
    def __init__(self, path):
        self.db = sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA query_only=ON")

    def handoff(self, project, task_id):
        result = super().handoff(project, task_id)
        # Missing legacy field has the same logical value as a current NULL field.
        result.setdefault("worktree_fingerprint", None)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, default=ROOT / "target/debug/examples/sqlite_reader")
    args = parser.parse_args()
    comparisons = 0
    cases = 0

    def rust(path, requests, snapshot):
        payload = dict(database=str(path), requests=requests, worktrees={p: snapshot for p in ["alpha", "beta"]})
        proc = subprocess.run([str(args.reader.resolve())], input=json.dumps(payload), text=True,
                              encoding="utf-8", capture_output=True, check=True, timeout=30)
        return json.loads(proc.stdout)

    snapshot = dict(head="base", branch="main", dirty=False, change_count=0, worktree_fingerprint="saved")
    requests = []
    for project in ["alpha", "beta"]:
        for include in [False, True]:
            requests.append(dict(project=project, name="get_effective_context", arguments={"include_documents": include}))
        for task in ["shared", "beta-only", "null-fingerprint", "missing"]:
            requests.append(dict(project=project, name="get_handoff", arguments={"task_id": task}))
    requests.append(dict(project="unknown", name="get_effective_context", arguments={}))

    def compare(path, legacy, current):
        nonlocal comparisons
        before = hashlib.sha256(path.read_bytes()).digest()
        with ReadOnlyOracle(path) as oracle:
            sql_before = list(oracle.db.iterdump())
            def worktree(_root, *, include_fingerprint=False):
                state = copy.deepcopy(current)
                if not include_fingerprint:
                    state.pop("worktree_fingerprint")
                return state
            expected = []
            with patch("agent_control_plane.context.worktree_snapshot", worktree):
                for request in requests:
                    try:
                        if request["name"] == "get_effective_context":
                            data = effective_context(oracle, request["project"], **request["arguments"])
                        else:
                            data = inspect_handoff(oracle, request["project"], request["arguments"]["task_id"])
                        expected.append(dict(data=data))
                    except ValueError:
                        expected.append(dict(error=True))
            response = rust(path, requests, current)
            assert response["schema"] == ("Legacy" if legacy else "Fingerprinted")
            assert len(response["answers"]) == len(expected)
            for result, reference in zip(response["answers"], expected):
                if "error" in reference:
                    assert "error" in result
                else:
                    # Same DB means IDs, timestamps, provenance and nested JSON
                    # must match exactly; nothing inside evidence is normalized.
                    assert result == reference, "SQLite parity response differs"
            assert list(oracle.db.iterdump()) == sql_before, "SQL state changed during Rust reads"
        assert hashlib.sha256(path.read_bytes()).digest() == before, "Database bytes changed during Rust reads"
        comparisons += len(requests)

    with tempfile.TemporaryDirectory(prefix="acp-sqlite-parity-") as directory:
        base = Path(directory)
        for legacy in [False, True]:
            path = create_fixture(base / str(legacy), legacy=legacy)
            compare(path, legacy, snapshot)
            compare(path, legacy, dict(head=None, branch=None, dirty=None, change_count=None, worktree_fingerprint=None))
            # Keep a real writer alive so a committed update remains in WAL.
            with closing(sqlite3.connect(path)) as writer:
                writer.execute("PRAGMA journal_mode=WAL")
                writer.execute("PRAGMA wal_autocheckpoint=0")
                with writer:
                    writer.execute("UPDATE preferences SET value='wal-visible' WHERE source='fixture-new'")
                assert path.with_name(path.name + "-wal").exists()
                compare(path, legacy, snapshot)
            cases += 3
        for name, setup, expected in [
            ("missing", None, "OpenFailed"),
            ("empty-file", b"", "IncompatibleSchema"),
            ("corrupt", b"not sqlite", "IncompatibleSchema"),
        ]:
            path = base / name
            if setup is not None:
                path.write_bytes(setup)
            assert rust(path, [], snapshot) == {"open_error": expected}
            assert path.exists() == (setup is not None)
            if setup is not None:
                assert path.read_bytes() == setup
            cases += 1
        for name, sql, expected in [
            ("future", "PRAGMA user_version=99", "UnsupportedVersion"),
            ("unknown-shape", "ALTER TABLE projects ADD COLUMN future TEXT", "IncompatibleSchema"),
        ]:
            path = create_fixture(base / name)
            with closing(sqlite3.connect(path)) as writer, writer:
                writer.execute(sql)
            before = path.read_bytes()
            assert rust(path, [], snapshot) == {"open_error": expected}
            assert path.read_bytes() == before
            cases += 1
        path = create_fixture(base / "no-records", empty=True)
        assert all("error" in r for r in rust(path, requests, snapshot)["answers"])
        cases += 1
        path = create_fixture(base / "invalid-status")
        with closing(sqlite3.connect(path)) as writer, writer:
            writer.execute("PRAGMA ignore_check_constraints=ON")
            writer.execute("UPDATE preferences SET status='unknown' WHERE scope='global'")
        result = rust(path, requests[:1], snapshot)
        assert result["answers"] == [{"error": "InvalidRecord"}]
        cases += 1
    print(f"PASS: {comparisons} exact SQLite Python/Rust request comparisons; {cases} scenarios including legacy/current, WAL and explicit failures")


if __name__ == "__main__":
    main()
