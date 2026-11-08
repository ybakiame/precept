import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from agent_control_plane.context import effective_context, inspect_handoff
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

    def test_unknown_project(self):
        with self.assertRaises(ValueError):
            effective_context(self.store, "other")


if __name__ == "__main__":
    unittest.main()
