import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from agent_control_plane.mcp import MCPServer
from agent_control_plane.store import Store


class MCPTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name) / "repo"
        root.mkdir()
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        self.store = Store(Path(self.tmp.name) / "state")
        self.addCleanup(self.store.close)
        self.store.register_project("one", str(root))
        self.srv = MCPServer(self.store, "one")

    def request(self, method, params=None, id=1):
        return self.srv.handle({"jsonrpc": "2.0", "id": id, "method": method, "params": params or {}})

    def test_legacy_handshake_then_tools(self):
        r = self.request("tools/list")
        self.assertIn("error", r)
        hello = self.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                            "clientInfo": {"name": "test", "version": "1"}})
        self.assertEqual("2025-11-25", hello["result"]["protocolVersion"])
        self.srv.handle({"jsonrpc": "2.0", "method": "notifications/initialized"})
        tools = self.request("tools/list")
        self.assertEqual(2, len(tools["result"]["tools"]))
        result = self.request("tools/call", {"name": "get_effective_context", "arguments": {}})
        self.assertEqual("one", json.loads(result["result"]["content"][0]["text"])["project"])

    def test_stateless_mode_and_scoped_tools(self):
        meta = {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                          "io.modelcontextprotocol/clientCapabilities": {}}}
        result = self.request("tools/list", meta)
        self.assertIn("_meta", result["result"])
        params = {**meta, "name": "get_effective_context", "arguments": {"project": "other"}}
        result = self.request("tools/call", params)
        self.assertTrue(result["result"]["isError"])
        self.assertNotIn("preferences", result["result"]["content"][0]["text"])

    def test_tool_error_and_notifications(self):
        self.assertIsNone(self.srv.handle({"jsonrpc": "2.0", "method": "notifications/progress"}))
        meta = {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}
        result = self.request("tools/call", {**meta, "name": "not_an_actual_tool", "arguments": {}})
        self.assertEqual(-32602, result["error"]["code"])


if __name__ == "__main__":
    unittest.main()
