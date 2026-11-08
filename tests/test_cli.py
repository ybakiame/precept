import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class CLITests(unittest.TestCase):
    def test_cli_and_stdio_in_subprocess(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t) / "repo"
            root.mkdir()
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            env = {**os.environ, "AGENT_CP_HOME": str(Path(t) / "state")}
            def cli(*args, input=None):
                return subprocess.run([sys.executable, "-m", "agent_control_plane.cli", *args],
                                      input=input, capture_output=True, text=True, env=env, check=True)
            self.assertEqual("repo", json.loads(cli("project", "add", "repo", str(root)).stdout)["name"])
            pid = json.loads(cli("preference", "suggest", "--key", "style", "--value", "concise",
                                 "--source", "explicit-user").stdout)["id"]
            self.assertEqual([], json.loads(cli("context", "repo").stdout)["preferences"])
            cli("preference", "confirm", str(pid))
            self.assertEqual("concise", json.loads(cli("context", "repo").stdout)["preferences"][0]["value"])
            reqs = [
                {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}},
                {"jsonrpc": "2.0", "method": "notifications/initialized"},
                {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "get_effective_context", "arguments": {}}},
            ]
            out = cli("mcp", "--project", "repo", input="".join(json.dumps(x)+"\n" for x in reqs))
            responses = [json.loads(x) for x in out.stdout.splitlines()]
            self.assertEqual([1, 2], [x["id"] for x in responses])
            self.assertNotIn("preference", out.stderr.lower())
            self.assertEqual("concise", json.loads(responses[1]["result"]["content"][0]["text"])["preferences"][0]["value"])


if __name__ == "__main__":
    unittest.main()
