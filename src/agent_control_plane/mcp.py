"""Minimal read-only MCP stdio service.

Supports legacy MCP initialize (2025-11-25) and stateless request metadata
(2026-07-28); never writes preferences or task state from a tool invocation.
"""

from __future__ import annotations

import json
import sys
from typing import Any

from . import __version__
from .context import effective_context, inspect_handoff
from .store import Store

PROTOCOL_LEGACY = "2025-11-25"
PROTOCOL_STATELESS = "2026-07-28"
SERVER_INFO = {"name": "agent-control-plane", "version": __version__}


def tool_schema() -> list[dict]:
    return [
        {"name": "get_effective_context", "description": "Read confirmed developer preferences and live project contract references. Repository rules take priority.",
         "inputSchema": {"type": "object", "properties": {"include_documents": {"type": "boolean", "default": False}},
                         "additionalProperties": False}},
        {"name": "get_handoff", "description": "Read a task handoff with current git revision and stale-state warnings. Does not grant permissions.",
         "inputSchema": {"type": "object", "properties": {"task_id": {"type": "string"}},
                         "required": ["task_id"], "additionalProperties": False}},
    ]


class MCPServer:
    def __init__(self, store: Store, project: str):
        store.project(project)
        self.store = store
        self.project = project
        self.initialized = False
        self.legacy_ready = False

    def _handle_tool(self, params: dict) -> dict:
        name = params.get("name")
        args = params.get("arguments", {})
        if not isinstance(args, dict):
            raise ValueError("arguments must be an object")
        try:
            if name == "get_effective_context":
                if set(args) - {"include_documents"} or not isinstance(args.get("include_documents", False), bool):
                    raise ValueError("include_documents must be boolean")
                data = effective_context(self.store, self.project,
                                         include_documents=args.get("include_documents", False))
            elif name == "get_handoff":
                if set(args) != {"task_id"} or not isinstance(args["task_id"], str):
                    raise ValueError("task_id string required")
                data = inspect_handoff(self.store, self.project, args["task_id"])
            else:
                return {"error": {"code": -32602, "message": "Unknown tool: " + str(name)}}
        except (ValueError, OSError) as exc:
            return {"result": {"content": [{"type": "text", "text": str(exc)}], "isError": True}}
        return {"result": {"content": [{"type": "text", "text": json.dumps(data, ensure_ascii=False)}],
                              "isError": False}}

    def handle(self, message: Any) -> dict | None:
        if not isinstance(message, dict) or message.get("jsonrpc") != "2.0":
            return {"jsonrpc": "2.0", "id": message.get("id") if isinstance(message, dict) else None,
                    "error": {"code": -32600, "message": "Invalid JSON-RPC request"}}
        if "id" not in message:
            if message.get("method") == "notifications/initialized":
                self.legacy_ready = self.initialized
            return None
        rid = message["id"]
        if isinstance(rid, bool) or not isinstance(rid, (str, int)):
            return {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid id"}}
        method = message.get("method")
        params = message.get("params", {})
        if not isinstance(params, dict):
            return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32602, "message": "params must be object"}}
        meta = params.get("_meta", {})
        stateless = isinstance(meta, dict) and meta.get("io.modelcontextprotocol/protocolVersion") == PROTOCOL_STATELESS
        if method == "initialize":
            # v2026 does not use this; older clients still do.
            requested = params.get("protocolVersion")
            if not isinstance(requested, str):
                return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32602, "message": "protocolVersion required"}}
            self.initialized = True
            self.legacy_ready = False
            result = {"protocolVersion": PROTOCOL_LEGACY, "serverInfo": SERVER_INFO, "capabilities": {"tools": {}}}
        elif method == "ping":
            result = {}
        elif not (stateless or self.legacy_ready):
            return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": "MCP initialization required"}}
        elif method == "tools/list":
            result = {"tools": tool_schema()}
        elif method == "tools/call":
            answer = self._handle_tool(params)
            if "error" in answer:
                return {"jsonrpc": "2.0", "id": rid, "error": answer["error"]}
            result = answer["result"]
        else:
            return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "Method not found"}}
        if stateless:
            result["_meta"] = {"io.modelcontextprotocol/serverInfo": SERVER_INFO}
        return {"jsonrpc": "2.0", "id": rid, "result": result}


def serve_stdio(store: Store, project: str) -> None:
    server = MCPServer(store, project)
    for line in sys.stdin:
        try:
            msg = json.loads(line)
            response = server.handle(msg)
        except (ValueError, json.JSONDecodeError) as exc:
            response = {"jsonrpc": "2.0", "id": None,
                        "error": {"code": -32700, "message": "Malformed input: " + type(exc).__name__}}
        except Exception as exc:
            print(f"MCP internal error: {type(exc).__name__}", file=sys.stderr)
            response = {"jsonrpc": "2.0", "id": None,
                        "error": {"code": -32603, "message": "Internal error"}}
        if response is not None:
            sys.stdout.write(json.dumps(response, ensure_ascii=False, separators=(",", ":")) + "\n")
            sys.stdout.flush()
