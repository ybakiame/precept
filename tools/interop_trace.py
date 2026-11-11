"""Test-only MCP stdio observer. Never persist raw frames, IDs or arguments."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

LIMIT = 1024 * 1024
TOOLS = {"get_effective_context", "get_handoff"}
WARNINGS = {
    "HEAD changed or cannot be verified; re-evaluate all prior checks": "head",
    "Branch differs from handoff; inspect worktree before proceeding": "branch",
    "Current worktree has changes or cannot be inspected; preserve user edits": "dirty",
    "Handoff has no worktree fingerprint; uncommitted changes cannot be compared": "missing_fingerprint",
    "Worktree fingerprint unavailable; inspect differences manually": "unknown_fingerprint",
    "Worktree fingerprint changed since handoff; inspect uncommitted edits": "fingerprint",
    "Earlier validation is scoped to its recorded revision and environment": "historical",
}


def digest(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, ensure_ascii=False).encode()
    ).hexdigest()


class Observer:
    def __init__(self):
        self.pending = {}
        self.serial = 0

    def request(self, value):
        if not isinstance(value, dict) or "id" not in value:
            return
        ident = value["id"]
        if (
            type(ident) not in (str, int)
            or len(str(ident)) > 128
            or len(self.pending) >= 1024
        ):
            raise ValueError("Observer correlation bound")
        method = value.get("method")
        method = (
            method
            if isinstance(method, str)
            and method in {"initialize", "ping", "tools/list", "tools/call"}
            else "unknown"
        )
        params = value.get("params")
        tool = params.get("name") if isinstance(params, dict) else None
        tool = tool if isinstance(tool, str) and tool in TOOLS else "unknown"
        self.serial += 1
        self.pending[json.dumps(ident)] = dict(
            sequence=self.serial, method=method, tool=tool
        )

    def response(self, value):
        if not isinstance(value, dict):
            return None
        event = self.pending.pop(json.dumps(value.get("id")), None)
        if event is None:
            return None
        result = value.get("result")
        event["outcome"] = "rpc_error" if "error" in value else "success"
        if isinstance(result, dict):
            if result.get("isError"):
                event["outcome"] = "tool_error"
            if event["method"] == "tools/list":
                tools = result.get("tools", [])
                event["exact_two_tools"] = (
                    isinstance(tools, list)
                    and len(tools) == 2
                    and {t.get("name") for t in tools if isinstance(t, dict)} == TOOLS
                )
            if (
                event["method"] == "tools/call"
                and event["tool"] in TOOLS
                and event["outcome"] == "success"
            ):
                try:
                    data = json.loads(result["content"][0]["text"])
                    if not isinstance(data, dict):
                        raise ValueError("Object required")
                    event["result_sha256"] = digest(data)
                    warnings = data.get("resume_warnings", [])
                    event["freshness"] = sorted(
                        {
                            WARNINGS.get(w, "unknown")
                            for w in warnings
                            if isinstance(w, str)
                        }
                    )
                    event["validation_statuses"] = [
                        (
                            v.get("status")
                            if v.get("status") in {"PASS", "FAIL", "SKIPPED", "NOT_RUN"}
                            else "UNKNOWN"
                        )
                        for v in data.get("validation", [])
                        if isinstance(v, dict)
                    ]
                except (KeyError, IndexError, TypeError, ValueError):
                    event["outcome"] = "unclassified_payload"
        return event


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--synthetic-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    root = args.synthetic_root.resolve(strict=True)
    assert (root / ".acp-interop-fixture").is_file()
    assert args.out.resolve().is_relative_to(root) and not args.out.is_symlink()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    assert command and Path(command[0]).is_absolute() and Path(command[0]).is_file()
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(args.out, flags, 0o600)
    observer = Observer()
    lock = threading.Lock()
    proc = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )

    def forward():
        try:
            while raw := sys.stdin.buffer.readline(LIMIT + 1):
                if len(raw) > LIMIT:
                    raise ValueError("Frame bound")
                with lock:
                    observer.request(json.loads(raw))
                proc.stdin.write(raw)
                proc.stdin.flush()
        except (OSError, ValueError, TypeError):
            pass
        finally:
            proc.stdin.close()

    threading.Thread(target=forward, daemon=True).start()
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as trace:
            while raw := proc.stdout.readline(LIMIT + 1):
                if len(raw) > LIMIT:
                    raise ValueError("Frame bound")
                with lock:
                    event = observer.response(json.loads(raw))
                if event is not None:
                    trace.write(json.dumps(event, sort_keys=True) + "\n")
                    trace.flush()
                sys.stdout.buffer.write(raw)
                sys.stdout.buffer.flush()
    finally:
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("Sanitized synthetic MCP observer failure", file=sys.stderr)
        raise SystemExit(2)
