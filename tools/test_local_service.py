"""Real Daemon + native CLI + two MCP subprocesses; synthetic databases only.
Build first: cargo build --workspace --bins --examples --locked
Run: PYTHONPATH=src python tools/test_local_service.py -v
All credentials remain in a private OS-local runtime directory, never in argv/logs.
"""
from __future__ import annotations
import concurrent.futures
import ctypes
import hashlib
import json
import os
from pathlib import Path
import queue
import signal
import socket
import sqlite3
import struct
import subprocess
import tempfile
import threading
import time
import unittest
from contextlib import closing
from unittest.mock import patch
from sqlite_fixture import create_fixture, ROOT
from check_sqlite_parity import ReadOnlyOracle
from agent_control_plane.mcp import MCPServer
from agent_control_plane.context import worktree_snapshot

MAX_FRAME = 1024 * 1024
BIN = ROOT / "target/debug"


def binary(name):
    return str(BIN / (name + (".exe" if os.name == "nt" else "")))


def collect(pipe, records=None):
    result = queue.Queue()
    def reader():
        for line in iter(pipe.readline, b""):
            if records is not None:
                records.append(line.decode("utf-8", "replace"))
            result.put(line)
        result.put(None)
    threading.Thread(target=reader, daemon=True).start()
    return result


def stop(proc, hard=False):
    if proc.poll() is None:
        if hard:
            proc.kill()
        else:
            proc.terminate()
        try:
            proc.wait(timeout=6)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    for pipe in [proc.stdin, proc.stdout, proc.stderr]:
        if pipe:
            pipe.close()


def windows_pipe(endpoint):
    # Query current process token; never print or persist the SID itself.
    from ctypes import wintypes
    adv = ctypes.WinDLL("advapi32", use_last_error=True)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.GetCurrentProcess.argtypes = []
    adv.OpenProcessToken.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)]
    adv.GetTokenInformation.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    token = wintypes.HANDLE()
    assert adv.OpenProcessToken(kernel.GetCurrentProcess(), 8, ctypes.byref(token))
    size = wintypes.DWORD()
    adv.GetTokenInformation(token, 1, None, 0, ctypes.byref(size))
    data = ctypes.create_string_buffer(size.value)
    assert adv.GetTokenInformation(token, 1, data, size, ctypes.byref(size))
    sid = ctypes.cast(data, ctypes.POINTER(ctypes.c_void_p))[0]
    text = ctypes.c_void_p()
    adv.ConvertSidToStringSidW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
    assert adv.ConvertSidToStringSidW(sid, ctypes.byref(text))
    value = ctypes.wstring_at(text)
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.LocalFree(text)
    kernel.CloseHandle(token)
    digest = hashlib.sha256(value.encode()).hexdigest()[:16]
    return rf"\\.\pipe\acp-{digest}-{endpoint}"


class Raw:
    def __init__(self, fixture, cap="operator", hello=True):
        self.fixture = fixture
        if os.name == "nt":
            import msvcrt
            deadline = time.monotonic() + 8
            while True:
                try:
                    self.io = open(windows_pipe(fixture.endpoint), "r+b", buffering=0)
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise
                    time.sleep(.02)
            self.handle = msvcrt.get_osfhandle(self.io.fileno())
        else:
            self.io = socket.socket(socket.AF_UNIX)
            self.io.settimeout(8)
            self.io.connect(fixture.endpoint)
        if hello:
            self.send({"api_version": 1, "credential": fixture.credential(cap)["token"]})
            ack = self.recv()
            assert ack.get("ready") is True, "IPC handshake refused"

    def send_bytes(self, data):
        if os.name == "nt":
            self.io.write(data)
        else:
            self.io.sendall(data)

    def send(self, value):
        raw = json.dumps(value, ensure_ascii=False).encode()
        self.send_bytes(struct.pack(">I", len(raw)) + raw)

    def read_exact(self, n):
        data = b""
        deadline = time.monotonic() + 8
        while len(data) < n:
            if os.name == "nt":
                from ctypes import wintypes
                available = wintypes.DWORD()
                peek = ctypes.windll.kernel32.PeekNamedPipe
                peek.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p,
                                 ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
                if not peek(self.handle, None, 0, None, ctypes.byref(available), None):
                    raise EOFError("pipe closed")
                if not available.value:
                    if time.monotonic() > deadline:
                        raise TimeoutError("pipe deadline")
                    time.sleep(.01)
                    continue
                part = self.io.read(min(n - len(data), available.value))
            else:
                part = self.io.recv(n - len(data))
            if not part:
                raise EOFError("channel closed")
            data += part
        return data

    def recv(self):
        n = struct.unpack(">I", self.read_exact(4))[0]
        assert n <= MAX_FRAME, "response framing bound violated"
        return json.loads(self.read_exact(n))

    def request(self, method, params=None, ident="r", version=1):
        self.send(dict(api_version=version, id=ident, method=method, params={} if params is None else params))
        return self.recv()

    def close(self):
        self.io.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


class Bridge:
    def __init__(self, fixture, project):
        self.proc = subprocess.Popen([binary("acp-mcp"), *fixture.location,
                                     "--project", project, "--capability-file", str(fixture.runtime / f"agent-{project}.cap")],
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.lines = collect(self.proc.stdout)
        self.errors = []
        collect(self.proc.stderr, self.errors)
        fixture.children.append(self.proc)

    def request(self, message):
        self.proc.stdin.write(json.dumps(message, ensure_ascii=False).encode() + b"\n")
        self.proc.stdin.flush()
        if isinstance(message, dict) and message.get("jsonrpc") == "2.0" and "id" not in message:
            return None
        raw = self.lines.get(timeout=12)
        assert raw is not None, "bridge exited without a JSON response"
        return json.loads(raw)


class Fixture:
    def __init__(self, legacy=False, v1=False, start=True, allow=("alpha", "beta")):
        # Short Unix socket address, also on macOS where TMPDIR can be long.
        self.tmp = tempfile.TemporaryDirectory(prefix="acpd-", dir="/tmp" if os.name != "nt" else None)
        self.root = Path(self.tmp.name)
        self.db = create_fixture(self.root / "fixture", legacy=legacy)
        for name in ["alpha", "beta"]:
            subprocess.run(["git", "init", "-q", str(self.root / "fixture" / name)], check=True, capture_output=True)
        self.runtime = self.root / "run"
        self.endpoint = "control-" + self.root.name if os.name == "nt" else str(self.runtime / "control.sock")
        self.location = ["--runtime-dir", str(self.runtime), "--endpoint", self.endpoint]
        self.children = []
        self.errors = []
        self.allow = allow
        self.environment = None
        if v1:
            with closing(sqlite3.connect(self.db)) as db, db:
                db.execute("PRAGMA user_version=1")
        self.before = self.db.read_bytes()
        self.proc = None
        if start:
            try:
                self.start()
            except BaseException:
                self.close()
                raise

    def args(self, database=None):
        return [binary("acp-daemon"), *self.location, "--database", str(database or self.db),
                *[arg for p in self.allow for arg in ("--allow-project", p)]]

    def start(self):
        self.proc = subprocess.Popen(self.args(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=self.environment,
                                     creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
        self.children.append(self.proc)
        collect(self.proc.stderr, self.errors)
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise AssertionError("daemon startup failed: " + "".join(self.errors)[-200:])
            if all((self.runtime / name).exists() for name in ["operator.cap", *[f"agent-{p}.cap" for p in self.allow]]):
                try:
                    if self.cli("health")["ready"]:
                        return
                except (AssertionError, OSError):
                    pass
            time.sleep(.03)
        raise AssertionError("daemon readiness deadline")

    def credential(self, who):
        filename = "operator.cap" if who == "operator" else f"agent-{who}.cap"
        return json.loads((self.runtime / filename).read_text(encoding="utf-8"))

    def cli(self, *query, success=True):
        p = subprocess.run([binary("acp-query"), *self.location, *query], capture_output=True,
                           text=True, encoding="utf-8", timeout=12)
        assert (p.returncode == 0) == success, "operator CLI exit outcome differs"
        return json.loads(p.stdout)

    def close(self):
        for proc in reversed(self.children):
            stop(proc)
        self.tmp.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


def normalize(response):
    response = json.loads(json.dumps(response))
    result = response.get("result")
    if isinstance(result, dict) and "content" in result and not result.get("isError"):
        result["content"][0]["text"] = json.loads(result["content"][0]["text"])
    return response


def oracle(server, message):
    try:
        return server.handle(message)
    except ValueError:
        return dict(jsonrpc="2.0", id=None, error=dict(code=-32700, message="Malformed input: ValueError"))


class LocalServiceTests(unittest.TestCase):
    def test_two_real_bridges_operator_and_python_protocol_parity_all_schemas(self):
        comparisons = 0
        for legacy, v1 in [(True, False), (False, False), (False, True)]:
            with self.subTest(legacy=legacy, v1=v1), Fixture(legacy, v1) as f, ReadOnlyOracle(f.db) as store:
                bridges = {p: Bridge(f, p) for p in ["alpha", "beta"]}
                servers = {p: MCPServer(store, p) for p in bridges}
                messages = [
                    dict(jsonrpc="2.0", id=1, method="ping"),
                    dict(jsonrpc="2.0", id=2, method="tools/list"),
                    dict(jsonrpc="2.0", id=3, method="initialize", params={"protocolVersion": "2025-11-25"}),
                    dict(jsonrpc="2.0", method="notifications/initialized"),
                    dict(jsonrpc="2.0", id=4, method="tools/list"),
                    dict(jsonrpc="2.0", id=5, method="tools/call", params={"name": "get_effective_context", "arguments": {"include_documents": True}}),
                    dict(jsonrpc="2.0", id=6, method="tools/call", params={"name": "get_handoff", "arguments": {"task_id": "shared"}}),
                    dict(jsonrpc="2.0", id=7, method="tools/call", params={"name": "get_handoff", "arguments": {"task_id": "missing"}}),
                    dict(jsonrpc="2.0", id=8, method="unknown"),
                    dict(jsonrpc="2.0", id=9, method="ping", params={"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}),
                    dict(jsonrpc="2.0", id=True, method="ping"),
                    dict(jsonrpc="2.0", id=None, method="ping"),
                    dict(jsonrpc="2.0", id=10, method="ping", params=[]), [],
                ]
                for arguments in [{"project": "beta"}, {"project_id": "beta"}, {"include_documents": "true"}, {"extra": 1}, [], None]:
                    messages.append(dict(jsonrpc="2.0", id=11, method="tools/call", params={"name": "get_effective_context", "arguments": arguments}))
                for name, arguments in [("get_handoff", {"project": "beta", "task_id": "shared"}),
                                        ("get_handoff", {"task_id": 1}), ("preference_confirm", {"id": 1})]:
                    messages.append(dict(jsonrpc="2.0", id=12, method="tools/call", params={"name": name, "arguments": arguments}))
                for project, bridge in bridges.items():
                    for message in messages:
                        actual = bridge.request(message)
                        reference = oracle(servers[project], message)
                        assert (normalize(actual) if actual else None) == (normalize(reference) if reference else None), "MCP protocol/payload parity drift"
                        comparisons += actual is not None
                with concurrent.futures.ThreadPoolExecutor(3) as pool:
                    values = list(pool.map(lambda p: f.cli("context", p, "--include-documents"), ["alpha", "beta", "alpha"]))
                    assert [v["project"] for v in values] == ["alpha", "beta", "alpha"]
                assert f.cli("projects") == [{"name": "alpha"}, {"name": "beta"}]
                health = f.cli("health")
                assert all(marker not in json.dumps(health) for marker in [str(f.db), "PENDING", "ALPHA_PROJECT_CONTRACT", "BETA_PRIVATE"])
                stop(bridges["alpha"].proc, hard=True)
                assert f.cli("health")["ready"]
                assert f.proc.poll() is None

        print(f"PASS: {comparisons} normalized MCP responses compared with Python; 3 SQLite schemas, 6 real bridges")

    def test_server_side_agent_capabilities_protocol_and_frames(self):
        with Fixture() as f:
            with Raw(f, "alpha") as raw:
                for method, params in [("project.list", {}), ("context.get", {"project": "beta"}),
                                       ("context.get", {"project_id": "beta"}), ("handoff.get", {"project": "beta", "task_id": "shared"}),
                                       ("preference.confirm", {"id": 1}), ("daemon.shutdown", {})]:
                    response = raw.request(method, params, ident=method+json.dumps(params))
                    assert response["error"] in ["Unauthorized", "InvalidRequest", "Unsupported"]
                    assert "BETA_PRIVATE" not in json.dumps(response)
            with Raw(f) as raw:
                assert raw.request("context.get", {"project": "missing"})["error"] == "Unauthorized"
            with Raw(f, hello=False) as raw:
                raw.send({"api_version": 1, "credential": [0]*32})
                assert raw.recv()["error"] == "Unauthorized"
            with Raw(f, hello=False) as raw:
                raw.send({"api_version": 1, "credential": f.credential("alpha")["token"], "role": "operator"})
                assert raw.recv()["error"] == "Unauthorized"
            for version in [0, 2]:
                with Raw(f) as raw:
                    assert raw.request("daemon.health", version=version)["error"] == "Version"
            with Raw(f) as raw:
                assert raw.request("daemon.health", ident="dup")["result"]["ready"]
                assert raw.request("daemon.health", ident="dup")["error"] == "InvalidRequest"
            for payload in [struct.pack(">I", MAX_FRAME+1), struct.pack(">I", 1)+b"["]:
                with Raw(f) as raw:
                    raw.send_bytes(payload)
                    assert raw.recv()["error"] in ["TooLarge", "InvalidRequest"]
            with Raw(f) as raw:
                raw.send_bytes(struct.pack(">I", 100)+b"{")
                assert raw.recv()["error"] == "Timeout"
            with Raw(f) as raw:
                raw.send_bytes(struct.pack(">I", 100)+b"{")
                if os.name != "nt":
                    raw.io.shutdown(socket.SHUT_WR)
                    assert raw.recv()["error"] == "InvalidRequest"
            assert f.cli("health")["ready"]
            wrong = subprocess.run([binary("acp-mcp"), *f.location, "--project", "beta", "--capability-file", str(f.runtime / "agent-alpha.cap")],
                                   input="", capture_output=True, text=True, timeout=10)
            assert wrong.returncode != 0 and not wrong.stdout

    def test_read_only_persistence_collision_and_restart(self):
        with Fixture() as f:
            before = f.db.read_bytes()
            expected = f.cli("handoff", "alpha", "shared")
            old = f.credential("alpha")["token"]
            collision = subprocess.run(f.args(), capture_output=True, timeout=10)
            assert collision.returncode != 0
            assert f.cli("health")["ready"]
            stop(f.proc, hard=True)
            f.start()
            assert f.cli("handoff", "alpha", "shared") == expected
            with Raw(f, hello=False) as raw:
                raw.send(dict(api_version=1, credential=old))
                assert raw.recv()["error"] == "Unauthorized"
            assert f.db.read_bytes() == before, "daemon changed database bytes"
            missing = f.root / "not-created.sqlite3"
            failed = subprocess.run(f.args(missing), capture_output=True, timeout=10)
            assert failed.returncode != 0 and not missing.exists()
            stop(f.proc)
            assert f.db.read_bytes() == before

    def test_limits_backpressure_and_wal_updates(self):
        with Fixture() as f:
            held = [Raw(f, hello=False) for _ in range(16)]
            try:
                overloaded = f.cli("health", success=False)
                assert "error" in overloaded
            finally:
                for raw in held:
                    raw.close()
            time.sleep(.2)
            assert f.cli("health")["ready"]
            with closing(sqlite3.connect(f.db)) as writer:
                writer.execute("PRAGMA journal_mode=WAL")
                with writer:
                    writer.execute("UPDATE preferences SET value='live-wal' WHERE source='fixture-new'")
                    for n in range(100):
                        writer.execute("INSERT INTO preferences(scope,project,pref_key,value,status,source,created_at,updated_at) "
                                       "VALUES('project','alpha',?,?,'confirmed','load-fixture','synthetic','synthetic')",
                                       (f"load.{n}", "x"*4000))
                assert any(p["value"] == "live-wal" for p in f.cli("context", "alpha")["preferences"])
                with Raw(f) as raw:
                    if os.name != "nt":
                        raw.io.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1024)
                    raw.send(dict(api_version=1, id="blocked", method="context.get", params={"project": "alpha"}))
                    time.sleep(2.5)
                assert f.cli("health")["ready"]
                operator = subprocess.Popen([binary("acp-query"), *f.location, "context", "alpha"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                f.children.append(operator)
                time.sleep(.2)
                stop(operator, hard=True)
                assert f.cli("health")["ready"]
                with writer:
                    writer.execute("PRAGMA user_version=99")
                assert f.cli("health", success=False)["error"] == "Unavailable"

    def test_private_runtime_and_non_socket_paths_refused(self):
        with Fixture(start=False) as f:
            if os.name != "nt":
                f.runtime.mkdir(mode=0o755)
                os.chmod(f.runtime, 0o755)
                denied = subprocess.run(f.args(), capture_output=True, timeout=10)
                assert denied.returncode != 0
                os.chmod(f.runtime, 0o700)
                path = Path(f.endpoint)
                path.write_text("preserve", encoding="utf-8")
                os.chmod(path, 0o600)
                denied = subprocess.run(f.args(), capture_output=True, timeout=10)
                assert denied.returncode != 0 and path.read_text() == "preserve"
            else:
                f.start()
                stop(f.proc, hard=True)
                changed = subprocess.run(["icacls", str(f.runtime), "/grant", "*S-1-1-0:(OI)(CI)F"], capture_output=True, timeout=10)
                assert changed.returncode == 0
                denied = subprocess.run(f.args(), capture_output=True, timeout=10)
                assert denied.returncode != 0, "broad runtime ACL accepted"

    def test_allowlist_and_private_agent_capability_are_not_operator_authority(self):
        with Fixture(allow=("alpha",)) as f:
            assert f.cli("projects") == [{"name": "alpha"}]
            assert f.cli("context", "beta", success=False)["error"] == "Unauthorized"
            assert not (f.runtime / "agent-beta.cap").exists()
            bad = subprocess.run([binary("acp-mcp"), *f.location, "--project", "alpha", "--capability-file", str(f.runtime / "operator.cap")],
                                 capture_output=True, timeout=10)
            assert bad.returncode != 0 and not bad.stdout
            bad = subprocess.run([binary("acp-daemon"), *f.location, "--database", str(f.db), "--allow-project", "missing"],
                                 capture_output=True, timeout=10)
            assert bad.returncode != 0
            assert f.cli("health")["ready"]

    def test_live_git_and_explicit_unavailable_fingerprint_delta(self):
        with Fixture() as f:
            root = f.root / "fixture/alpha"
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid",
                            "add", "AGENTS.md"], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(root), "-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid",
                            "commit", "-qm", "synthetic"], check=True, capture_output=True)
            reference = worktree_snapshot(root, include_fingerprint=True)
            context = f.cli("context", "alpha")
            assert context["repository"] == {k: v for k, v in reference.items() if k != "worktree_fingerprint"}
            handoff = f.cli("handoff", "alpha", "shared")
            assert reference["worktree_fingerprint"] is not None
            assert handoff["current_repository"]["worktree_fingerprint"] is None
            assert "Worktree fingerprint unavailable; inspect differences manually" in handoff["resume_warnings"]
            (root / "AGENTS.md").write_text("synthetic changed document", encoding="utf-8")
            assert f.cli("context", "alpha")["repository"]["dirty"] is True
            print("VERIFIED DELTA: Python computes a Git fingerprint; Rust reports null plus incomplete-evidence warning")
        with Fixture(start=False) as f:
            empty = f.root / "no-git"
            empty.mkdir()
            f.environment = {**os.environ, "PATH": str(empty)}
            f.start()
            assert f.cli("context", "alpha")["repository"] == dict(head=None, branch=None, dirty=None, change_count=None)

    def test_graceful_signal_and_runtime_credentials_cleanup(self):
        with Fixture() as f:
            if os.name == "nt":
                try:
                    f.proc.send_signal(signal.CTRL_BREAK_EVENT)
                    f.proc.wait(timeout=6)
                except (OSError, subprocess.TimeoutExpired):
                    print("NOT_RUN: Windows console CTRL_BREAK delivery unavailable; hard termination/restart covered separately")
                    stop(f.proc, hard=True)
                    return
            else:
                f.proc.send_signal(signal.SIGTERM)
                f.proc.wait(timeout=6)
            assert f.proc.returncode == 0
            assert not (f.runtime / "operator.cap").exists()
            assert not (f.runtime / "agent-alpha.cap").exists()
            if os.name != "nt":
                assert not Path(f.endpoint).exists()
            f.start()
            assert f.cli("health")["ready"]

    def test_database_and_control_runtime_must_be_disjoint(self):
        with Fixture(start=False) as f:
            f.runtime.mkdir(mode=0o700)
            inside = create_fixture(f.runtime / "data")
            before = inside.read_bytes()
            denied = subprocess.run(f.args(inside), capture_output=True, timeout=10)
            assert denied.returncode != 0
            assert inside.read_bytes() == before
            assert not (f.runtime / "daemon.lock").exists()
            assert not (f.runtime / "operator.cap").exists()


if __name__ == "__main__":
    unittest.main()
