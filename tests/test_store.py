import json
import os
import sqlite3
import subprocess
import tempfile
import unittest
from contextlib import closing
from pathlib import Path

from agent_control_plane.context import effective_context, inspect_handoff, worktree_snapshot
from agent_control_plane.store import Store


class StoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "test-repo"
        self.root.mkdir()
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Test", "-c", "user.email=a@b.c",
                        "commit", "--allow-empty", "-qm", "init"], check=True)
        (self.root / "AGENTS.md").write_text("Follow project rules", encoding="utf8")
        self.store = Store(Path(self.tmp.name) / "data")
        self.addCleanup(self.store.close)
        self.store.register_project("sample", str(self.root))

    def test_only_confirmed_and_project_override(self):
        a = self.store.suggest_preference("commit.style", "Conventional", "explicit user")
        b = self.store.suggest_preference("commit.style", "Project-specific", "project rule", "sample")
        self.assertEqual([], self.store.resolve_preferences("sample"))
        self.store.change_preference(a, "confirmed")
        self.store.change_preference(b, "confirmed")
        ctx = effective_context(self.store, "sample")
        self.assertEqual("Project-specific", ctx["preferences"][0]["value"])
        self.assertEqual("project", ctx["preferences"][0]["scope"])
        self.assertNotIn("content", ctx["documents"][0])
        content = effective_context(self.store, "sample", include_documents=True)
        self.assertEqual("Follow project rules", content["documents"][0]["content"])
        self.store.change_preference(b, "deprecated")
        self.assertEqual("Conventional", self.store.resolve_preferences("sample")[0]["value"])

    def test_rejection_and_deletion(self):
        n = self.store.suggest_preference("tool", "npm", "inferred")
        self.store.change_preference(n, "rejected")
        self.assertEqual([], self.store.preferences("sample"))
        with self.assertRaises(ValueError):
            self.store.change_preference(n, "confirmed")
        self.store.delete_preference(n)
        self.assertEqual([], self.store.preferences(all_statuses=True))

    def test_prevent_doc_symlink_escape(self):
        (self.root / "AGENTS.md").unlink()
        secret = Path(self.tmp.name) / "secret.txt"
        secret.write_text("not-for-agents")
        (self.root / "AGENTS.md").symlink_to(secret)
        ctx = effective_context(self.store, "sample", include_documents=True)
        self.assertNotIn("not-for-agents", json.dumps(ctx))
        self.assertEqual("unavailable_or_outside_root", ctx["documents"][0]["status"])

    def test_handoff_verifies_head_and_worktree(self):
        sha = subprocess.check_output(["git", "-C", str(self.root), "rev-parse", "HEAD"], text=True).strip()
        branch = subprocess.check_output(["git", "-C", str(self.root), "branch", "--show-current"], text=True).strip()
        self.store.save_handoff("sample", "TASK-1", "Fix bug", "Run tests", sha, branch, "dirty", "added test",
                                [{"check": "unit", "status": "NOT_RUN"}])
        h = inspect_handoff(self.store, "sample", "TASK-1")
        self.assertEqual("NOT_RUN", h["validation"][0]["status"])
        self.assertEqual(sha, h["current_repository"]["head"])
        subprocess.run(["git", "-C", str(self.root), "-c", "user.name=Test", "-c", "user.email=a@b.c",
                        "commit", "--allow-empty", "-qm", "second"], check=True)
        h = inspect_handoff(self.store, "sample", "TASK-1")
        self.assertTrue(any("HEAD changed" in w for w in h["resume_warnings"]))


    def test_handoff_detects_dirty_content_change_without_head_change(self):
        # AGENTS.md is untracked; name/count/HEAD remain unchanged.
        before = worktree_snapshot(self.root, include_fingerprint=True)
        self.assertTrue(before["dirty"])
        self.assertIsNotNone(before["worktree_fingerprint"])
        self.store.save_handoff(
            "sample", "TASK-2", "Resume test", "Read diff",
            before["head"], before["branch"], "dirty", "setup",
            [{"check": "host", "status": "PASS"}],
            worktree_fingerprint=before["worktree_fingerprint"],
        )
        (self.root / "AGENTS.md").write_text("Changed untracked file", encoding="utf8")
        after = worktree_snapshot(self.root, include_fingerprint=True)
        self.assertEqual(before["head"], after["head"])
        self.assertEqual(before["change_count"], after["change_count"])
        self.assertNotEqual(before["worktree_fingerprint"], after["worktree_fingerprint"])
        resumed = inspect_handoff(self.store, "sample", "TASK-2")
        self.assertTrue(any("Worktree fingerprint changed" in warning
                            for warning in resumed["resume_warnings"]))
        self.assertEqual("PASS", resumed["validation"][0]["status"])

    def test_handoff_detects_tracked_content_change(self):
        subprocess.run(["git", "-C", str(self.root), "add", "AGENTS.md"], check=True)
        subprocess.run(["git", "-C", str(self.root),
                        "-c", "user.name=Test", "-c", "user.email=a@b.c",
                        "commit", "-qm", "track AGENTS"], check=True)
        (self.root / "AGENTS.md").write_text("First tracked edit", encoding="utf8")
        before = worktree_snapshot(self.root, include_fingerprint=True)
        self.store.save_handoff(
            "sample", "TASK-3", "Resume test", "Read diff",
            before["head"], before["branch"], "dirty", "setup", [],
            worktree_fingerprint=before["worktree_fingerprint"],
        )
        (self.root / "AGENTS.md").write_text("Second tracked edit", encoding="utf8")
        resumed = inspect_handoff(self.store, "sample", "TASK-3")
        self.assertTrue(any("Worktree fingerprint changed" in warning
                            for warning in resumed["resume_warnings"]))

    def test_sqlite_migration_adds_nullable_fingerprint(self):
        legacy_dir = Path(self.tmp.name) / "legacy"
        legacy_dir.mkdir()
        # The SQLite transaction context does not close the connection. Close it
        # before reopening the store and before TemporaryDirectory cleanup on Windows.
        with closing(sqlite3.connect(legacy_dir / "context.sqlite3")) as db, db:
            db.execute(
                "CREATE TABLE handoffs ("
                "project TEXT, task_id TEXT, objective TEXT, base_sha TEXT,"
                "branch TEXT, worktree_state TEXT, completed TEXT,"
                "next_action TEXT, validation_json TEXT, updated_at TEXT,"
                "PRIMARY KEY(project,task_id))"
            )
        with Store(legacy_dir) as store:
            columns = {row[1] for row in store.db.execute("PRAGMA table_info(handoffs)")}
            self.assertIn("worktree_fingerprint", columns)
        # Idempotent second open.
        with Store(legacy_dir) as store:
            columns = {row[1] for row in store.db.execute("PRAGMA table_info(handoffs)")}
            self.assertIn("worktree_fingerprint", columns)

    def test_unknown_project(self):
        with self.assertRaises(ValueError):
            effective_context(self.store, "other")


if __name__ == "__main__":
    unittest.main()
