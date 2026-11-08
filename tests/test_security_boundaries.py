"""Read-only MCP and context-isolation negative tests, with isolated local state.

These are cooperative application-level boundaries, not an OS sandbox.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from agent_control_plane.mcp import MCPServer
from agent_control_plane.store import Store


STATELESS_META = {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}


class SecurityBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="acp-security-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.alpha = self._project("alpha", "ALPHA_CONTRACT_ONLY")
        self.beta = self._project("beta", "BETA_CONTRACT_ONLY")
        self.store = Store(self.root / "state")
        self.addCleanup(self.store.close)
        self.store.register_project("alpha", str(self.alpha))
        self.store.register_project("beta", str(self.beta))

    def _project(self, name, marker):
        root = self.root / name
        root.mkdir()
        (root / "AGENTS.md").write_text(marker, encoding="utf-8")
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        subprocess.run(["git", "-C", str(root), "-c", "user.name=Test",
                        "-c", "user.email=test@example.invalid", "commit",
                        "--allow-empty", "-qm", "init"], check=True)
        return root

    def _request(self, project, name, arguments):
        server = MCPServer(self.store, project)
        response = server.handle({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"_meta": STATELESS_META, "name": name, "arguments": arguments},
        })
        return response

    def _context(self, project, include_documents=False):
        response = self._request(project, "get_effective_context",
                                 {"include_documents": include_documents})
        self.assertFalse(response["result"]["isError"])
        return json.loads(response["result"]["content"][0]["text"])

    def test_only_confirmed_is_visible_to_mcp(self):
        suggested = self.store.suggest_preference(
            "test.pending", "PENDING_MARKER_NEVER_EXPOSE", "isolated-test")
        rejected = self.store.suggest_preference(
            "test.rejected", "REJECTED_MARKER_NEVER_EXPOSE", "isolated-test")
        self.store.change_preference(rejected, "rejected")
        deprecated = self.store.suggest_preference(
            "test.deprecated", "DEPRECATED_MARKER_NEVER_EXPOSE", "isolated-test")
        self.store.change_preference(deprecated, "confirmed")
        self.store.change_preference(deprecated, "deprecated")
        approved = self.store.suggest_preference(
            "interaction.language", "zh-CN", "isolated-test")
        self.store.change_preference(approved, "confirmed")
        self.assertEqual("suggested", next(x for x in self.store.preferences(
            "alpha", all_statuses=True) if x["id"] == suggested)["status"])
        for project in ("alpha", "beta"):
            payload = json.dumps(self._context(project), ensure_ascii=False)
            self.assertIn("zh-CN", payload)
            for marker in ("PENDING_MARKER_NEVER_EXPOSE",
                           "REJECTED_MARKER_NEVER_EXPOSE",
                           "DEPRECATED_MARKER_NEVER_EXPOSE"):
                self.assertNotIn(marker, payload)

    def test_project_preferences_and_docs_are_isolated(self):
        global_id = self.store.suggest_preference("commit.style", "global", "isolated-test")
        self.store.change_preference(global_id, "confirmed")
        alpha_id = self.store.suggest_preference(
            "commit.style", "alpha-override", "isolated-test", "alpha")
        self.store.change_preference(alpha_id, "confirmed")
        beta_id = self.store.suggest_preference(
            "project.secret", "BETA_PREF_ONLY", "isolated-test", "beta")
        self.store.change_preference(beta_id, "confirmed")
        alpha = self._context("alpha", include_documents=True)
        beta = self._context("beta", include_documents=True)
        self.assertEqual({"commit.style": "alpha-override"},
                         {p["key"]: p["value"] for p in alpha["preferences"]})
        self.assertEqual({"commit.style": "global", "project.secret": "BETA_PREF_ONLY"},
                         {p["key"]: p["value"] for p in beta["preferences"]})
        self.assertIn("ALPHA_CONTRACT_ONLY", json.dumps(alpha))
        self.assertNotIn("BETA_CONTRACT_ONLY", json.dumps(alpha))
        self.assertNotIn("BETA_PREF_ONLY", json.dumps(alpha))
        self.assertIn("BETA_CONTRACT_ONLY", json.dumps(beta))
        self.assertNotIn("ALPHA_CONTRACT_ONLY", json.dumps(beta))

    def test_project_selector_injection_is_rejected(self):
        for name, arguments in (
            ("get_effective_context", {"project": "beta", "include_documents": True}),
            ("get_effective_context", {"include_documents": True, "project_id": "beta"}),
            ("get_handoff", {"task_id": "shared", "project": "beta"}),
        ):
            response = self._request("alpha", name, arguments)
            self.assertTrue(response["result"]["isError"])
            self.assertNotIn("BETA_CONTRACT_ONLY", json.dumps(response))

    def test_same_task_id_cannot_cross_project(self):
        for project, marker in (("alpha", "ALPHA_OBJECTIVE_ONLY"),
                                ("beta", "BETA_OBJECTIVE_ONLY")):
            self.store.save_handoff(
                project, "shared", marker, "inspect", "example-sha", "main",
                "clean", "none", [{"check": "device", "status": "NOT_RUN"}])
        alpha_response = self._request("alpha", "get_handoff", {"task_id": "shared"})
        beta_response = self._request("beta", "get_handoff", {"task_id": "shared"})
        self.assertIn("ALPHA_OBJECTIVE_ONLY", json.dumps(alpha_response))
        self.assertNotIn("BETA_OBJECTIVE_ONLY", json.dumps(alpha_response))
        self.assertIn("BETA_OBJECTIVE_ONLY", json.dumps(beta_response))
        self.assertNotIn("ALPHA_OBJECTIVE_ONLY", json.dumps(beta_response))

    def test_other_project_unique_handoff_is_not_found(self):
        self.store.save_handoff("beta", "beta-only", "BETA_PRIVATE_TASK", "none",
                                "example-sha", "main", "clean", "none", [])
        response = self._request("alpha", "get_handoff", {"task_id": "beta-only"})
        self.assertTrue(response["result"]["isError"])
        self.assertNotIn("BETA_PRIVATE_TASK", json.dumps(response))

    def test_unregistered_project_and_mutation_tools_rejected(self):
        with self.assertRaises(ValueError):
            MCPServer(self.store, "unregistered")
        pending = self.store.suggest_preference("test.pending", "still-suggested", "test")
        for name in ("preference_confirm", "preference_suggest", "project_add"):
            response = self._request("alpha", name, {"id": pending})
            self.assertEqual(-32602, response["error"]["code"])
        row = next(p for p in self.store.preferences("alpha", all_statuses=True)
                   if p["id"] == pending)
        self.assertEqual("suggested", row["status"])

    def test_real_stdio_subprocess_scoped_and_read_only(self):
        beta_id = self.store.suggest_preference("beta.private", "BETA_PREF_ONLY", "test", "beta")
        self.store.change_preference(beta_id, "confirmed")
        pending = self.store.suggest_preference("test.pending", "PENDING_MARKER", "test")
        messages = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "security-test", "version": "1"}}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                "name": "get_effective_context", "arguments": {"include_documents": True}}},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
                "name": "get_effective_context", "arguments": {"project": "beta"}}},
        ]
        env = os.environ.copy()
        env["PYTHONPATH"] = str(Path(__file__).resolve().parents[1] / "src")
        proc = subprocess.run(
            [sys.executable, "-m", "agent_control_plane.cli", "--data-dir",
             str(self.root / "state"), "mcp", "--project", "alpha"],
            input="\n".join(json.dumps(m) for m in messages) + "\n",
            text=True, capture_output=True, timeout=12, env=env,
        )
        self.assertEqual(0, proc.returncode, proc.stderr)
        replies = [json.loads(line) for line in proc.stdout.splitlines()]
        self.assertEqual([1, 2, 3], [r["id"] for r in replies])
        payload = json.dumps(replies, ensure_ascii=False)
        self.assertIn("ALPHA_CONTRACT_ONLY", payload)
        self.assertNotIn("BETA_CONTRACT_ONLY", payload)
        self.assertNotIn("BETA_PREF_ONLY", payload)
        self.assertNotIn("PENDING_MARKER", payload)
        self.assertTrue(replies[2]["result"]["isError"])
        self.assertEqual("suggested", next(
            p["status"] for p in self.store.preferences("alpha", all_statuses=True)
            if p["id"] == pending))


if __name__ == "__main__":
    unittest.main()
