"""Exercise the installed local Codex MCP client without a model/inference turn.

Uses app-server stdio and its installed JSON schema, an isolated temporary Codex
home, disabled telemetry and a provider pointing to a closed loopback port.
Never copies credentials/auth/config from the operator's existing Codex home.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import time
from test_local_service import collect, stop, binary


class RpcRejected(Exception):
    pass


def smoke(fixture, project, expected_context):
    if not shutil.which("codex"):
        return dict(status="NOT_RUN", reason="local Codex executable unavailable")
    with tempfile.TemporaryDirectory(prefix="acp-codex-") as raw:
        root = Path(raw)
        version = subprocess.run(["codex", "--version"], capture_output=True, text=True, timeout=10, check=True).stdout.strip()
        schema = root / "schema"
        generated = subprocess.run(["codex", "app-server", "generate-json-schema", "--out", str(schema)], capture_output=True, timeout=20)
        if generated.returncode or not (schema / "v2/McpServerToolCallParams.json").is_file():
            return dict(status="NOT_RUN", reason="installed Codex lacks direct local MCP call API", version=version)
        # This is the documented purpose of CODEX_HOME, scoped only to this child.
        # No user settings, history, databases or login credentials are opened.
        home = root / "codex-state"
        home.mkdir(mode=0o700)
        cwd = root / "empty-workspace"
        cwd.mkdir(mode=0o700)
        args = [*fixture.location, "--project", project, "--capability-file", str(fixture.runtime / f"agent-{project}.cap")]
        config = '\n'.join([
            'model = "local-validation-no-inference"', 'model_provider = "local_validation"',
            '[model_providers.local_validation]', 'name = "Local validation only"',
            'base_url = "http://127.0.0.1:9/v1"', 'wire_api = "responses"',
            '[analytics]', 'enabled = false', '[feedback]', 'enabled = false',
            '[otel]', 'exporter = "none"', 'trace_exporter = "none"',
            '[mcp_servers.acp_validation]', 'command = ' + json.dumps(binary("acp-mcp")),
            'args = ' + json.dumps(args), 'startup_timeout_sec = 15',
        ]) + '\n'
        (home / "config.toml").write_text(config, encoding="utf-8")
        env = {**os.environ, "CODEX_HOME": str(home)}
        # Prevent inherited API keys/exporters from being used by the smoke harness.
        for key in list(env):
            if key.startswith(("OPENAI_", "OTEL_", "CODEX_API_")):
                env.pop(key)
        with (root / "app-server.log").open("wb") as log:
            proc = subprocess.Popen(["codex", "app-server", "--stdio"], cwd=cwd, env=env,
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log)
            lines = collect(proc.stdout)
            serial = 0
            def rpc(method, params):
                nonlocal serial
                serial += 1
                proc.stdin.write(json.dumps(dict(id=serial, method=method, params=params)).encode() + b"\n")
                proc.stdin.flush()
                deadline = time.monotonic() + 45
                while time.monotonic() < deadline:
                    line = lines.get(timeout=max(.1, deadline - time.monotonic()))
                    if line is None:
                        raise AssertionError("local Codex app-server exited")
                    response = json.loads(line)
                    if response.get("id") == serial:
                        if "error" in response:
                            raise RpcRejected("Codex direct MCP request rejected")
                        return response["result"]
                raise AssertionError("Codex direct MCP call timeout")
            try:
                rpc("initialize", dict(clientInfo=dict(name="acp_local_validation", version="1"), capabilities=dict(experimentalApi=True)))
                proc.stdin.write(b'{"method":"initialized","params":{}}\n')
                proc.stdin.flush()
                thread = rpc("thread/start", dict(cwd=str(cwd), ephemeral=True, sandbox="read-only", approvalPolicy="never"))
                thread_id = thread["thread"]["id"]
                inventory = rpc("mcpServerStatus/list", dict(threadId=thread_id, serverName="acp_validation"))
                entries = inventory["data"]
                assert len(entries) == 1 and entries[0]["name"] == "acp_validation"
                tools = entries[0]["tools"]
                names = set(tools) if isinstance(tools, dict) else {t["name"] for t in tools}
                assert names == {"get_effective_context", "get_handoff"}, "Codex tool inventory expanded"
                definitions = list(tools.values()) if isinstance(tools, dict) else tools
                for tool in definitions:
                    assert tool["inputSchema"]["additionalProperties"] is False
                    assert "project" not in tool["inputSchema"]["properties"]
                def call(name, arguments):
                    return rpc("mcpServer/tool/call", dict(threadId=thread_id, server="acp_validation", tool=name, arguments=arguments))
                context = call("get_effective_context", {})
                assert not context.get("isError")
                value = json.loads(context["content"][0]["text"])
                assert value == expected_context, "Codex/context CLI parity differs"
                for task in ["missing", "beta-only"]:
                    assert call("get_handoff", dict(task_id=task))["isError"]
                for name, arguments in [
                    ("get_effective_context", dict(project="beta")),
                    ("get_handoff", dict(task_id="shared", project="beta")),
                    ("get_handoff", dict(task_id=["shared"])),
                    ("handoff.list", {}), ("project.list", {}),
                ]:
                    try:
                        result = call(name, arguments)
                    except RpcRejected:
                        # The local Codex harness may reject schemas/tool names itself.
                        continue
                    assert result.get("isError"), "hostile MCP call unexpectedly succeeded"
                return dict(status="PASS", version=version, transport="local Codex app-server stdio; no model turn", tools=sorted(names),
                            scope="exact context equality with operator query", denial_cases=7,
                            user_configuration_unchanged=True, remote_inference="NOT_RUN by design; no real data uploaded")
            finally:
                stop(proc, hard=True)
