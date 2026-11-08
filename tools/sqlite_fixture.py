"""Synthetic SQLite builder shared by Rust integration tests and parity checks.

Only creates state in a caller-owned empty temporary directory. Never opens a
user profile or invokes schema initialization on an existing database.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sqlite3
from contextlib import closing
from agent_control_plane.store import Store

ROOT = Path(__file__).resolve().parents[1]


def create_fixture(base: Path, legacy: bool = False, empty: bool = False) -> Path:
    base.mkdir(parents=True, exist_ok=True)
    if any(base.iterdir()):
        raise ValueError("Fixture directory must be empty")
    data = json.loads((ROOT / "tests/fixtures/p1-preference-contract.v1.json").read_text(encoding="utf-8"))
    with Store(base / "state") as store:
        if not empty:
            for name, marker in data["projects"].items():
                root = base / name
                root.mkdir()
                (root / "AGENTS.md").write_text(marker, encoding="utf-8")
                store.register_project(name, str(root))
            for p in data["preferences"]:
                project = None if p["scope"] == "global" else p["scope"]
                pref = store.suggest_preference(p["key"], p["value"], "fixture", project)
                if p["status"] == "deprecated":
                    store.change_preference(pref, "confirmed")
                if p["status"] != "suggested":
                    store.change_preference(pref, p["status"])
            # P0 resolution also handles historical duplicate confirmed records.
            with store.db:
                store.db.execute("INSERT INTO preferences(scope,project,pref_key,value,status,source,created_at,updated_at) "
                                 "VALUES('project','alpha','commit.style','newest','confirmed','fixture-new','synthetic','synthetic')")
            validation = [{"check": status, "status": status,
                           "environment": "synthetic", "revision": "base",
                           "evidence": {"id": "record-id", "updated_at": "recorded", "nested": [None, True, 2, {"log": "ref"}]}}
                          for status in ["PASS", "FAIL", "SKIPPED", "NOT_RUN"]]
            for project, task in [("alpha", "shared"), ("beta", "shared"), ("beta", "beta-only")]:
                store.save_handoff(project, task, f"synthetic-{project}", "inspect", "base", "main", "clean", "",
                                   validation, worktree_fingerprint="saved")
            store.save_handoff("alpha", "null-fingerprint", "legacy", "inspect", "base", "main", "clean", "", [])
    path = base / "state/context.sqlite3"
    if legacy:
        # Synthetic fixture only. Rust must read this missing-column variant as-is.
        with closing(sqlite3.connect(path)) as db, db:
            db.execute("ALTER TABLE handoffs DROP COLUMN worktree_fingerprint")
    return path


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--legacy", action="store_true")
    parser.add_argument("--empty", action="store_true")
    args = parser.parse_args()
    create_fixture(args.directory, args.legacy, args.empty)
