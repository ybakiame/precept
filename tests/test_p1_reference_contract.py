"""Cross-language golden fixture for future Rust core; exercises Python P0 oracle.

The fixture is deliberately independent of path, commit SHA, time and ID order.
"""
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from agent_control_plane.mcp import MCPServer
from agent_control_plane.store import Store

FIXTURE = Path(__file__).parent / "fixtures" / "p1-preference-contract.v1.json"


class P1ReferenceContractTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads(FIXTURE.read_text(encoding="utf-8"))
        self.temp = tempfile.TemporaryDirectory(prefix="acp-p1-contract-")
        self.addCleanup(self.temp.cleanup)
        base = Path(self.temp.name)
        self.store = Store(base / "state")
        self.addCleanup(self.store.close)
        for name, marker in self.data["projects"].items():
            root = base / name
            root.mkdir()
            (root / "AGENTS.md").write_text(marker, encoding="utf-8")
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            self.store.register_project(name, str(root))
        for item in self.data["preferences"]:
            project = item["scope"] if item["scope"] != "global" else None
            pref_id = self.store.suggest_preference(item["key"], item["value"], "fixture", project)
            status = item["status"]
            if status == "deprecated":
                self.store.change_preference(pref_id, "confirmed")
                self.store.change_preference(pref_id, "deprecated")
            elif status != "suggested":
                self.store.change_preference(pref_id, status)

    @staticmethod
    def call(server, name, arguments):
        return server.handle({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"},
                "name": name, "arguments": arguments,
            },
        })

    def test_preference_contract_and_live_document_scope(self):
        for name, expected in self.data["expected_effective"].items():
            server = MCPServer(self.store, name)
            answer = self.call(server, "get_effective_context", {"include_documents": True})
            self.assertFalse(answer["result"]["isError"])
            payload = json.loads(answer["result"]["content"][0]["text"])
            actual = {p["key"]: {"value": p["value"], "scope": p["scope"]}
                      for p in payload["preferences"]}
            self.assertEqual(expected, actual)
            serialized = json.dumps(payload, ensure_ascii=False)
            self.assertIn(self.data["projects"][name], serialized)
            if name == "alpha":
                for marker in self.data["forbidden_alpha_strings"]:
                    self.assertNotIn(marker, serialized)

    def test_tool_calls_do_not_override_process_scope_or_write(self):
        server = MCPServer(self.store, "alpha")
        for call in self.data["must_reject_tool_calls"]:
            response = self.call(server, call["name"], call["arguments"])
            self.assertTrue(
                "error" in response or response["result"].get("isError"),
                (call, response),
            )
        context = self.call(server, "get_effective_context", {})
        self.assertFalse(context["result"]["isError"])


if __name__ == "__main__":
    unittest.main()
