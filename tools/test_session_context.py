"""Revocable native daemon/IPC/MCP process tests. Disposable synthetic state only."""

from concurrent.futures import ThreadPoolExecutor
import json
import os
import subprocess
import sqlite3
from contextlib import closing
import time
import unittest
from test_agent_session import SessionFixture
from test_local_service import Raw, binary, stop, collect


class ContextFixture(SessionFixture):
    def __init__(self, ttl=300, bridge=True):
        self.ttl, self.bridge_enabled = ttl, bridge
        super().__init__()
        self.set_programs(
            ["mcp-passive", "mcp", "mcp-cancel", "mcp-crash", "mcp-reject", "normal"]
        )

    def args(self, database=None):
        return super().args(database) + (
            [
                "--context-mcp",
                binary("acp-mcp"),
                "--context-lease-seconds",
                str(self.ttl),
            ]
            if self.bridge_enabled
            else []
        )

    def launch_context(self, mode="mcp-passive", project="alpha"):
        self.query(
            "session.start",
            project=project,
            executable=mode,
            confirmed=True,
            share_context=True,
        )
        return self.wait("ready", project)

    def lease_file(self):
        files = list(self.runtime.glob("session-*.cap"))
        if len(files) != 1:
            raise AssertionError("expected one private session capability")
        return files[0]

    def lease_peer(self, credential=None):
        credential = credential or json.loads(self.lease_file().read_text())
        r = Raw(self, hello=False)
        r.send(dict(api_version=1, credential=credential["token"]))
        ack = r.recv()
        if ack.get("ready") is not True:
            r.close()
            raise PermissionError("session capability rejected")
        return r

    def rejected(self, credential):
        with Raw(self, hello=False) as r:
            r.send(dict(api_version=1, credential=credential["token"]))
            return r.recv().get("error") == "Unauthorized"

    def revoked(self):
        deadline = time.monotonic() + 3
        while list(self.runtime.glob("session-*.cap")) and time.monotonic() < deadline:
            time.sleep(0.02)
        if list(self.runtime.glob("session-*.cap")):
            raise AssertionError("private lease file not cleaned")
        if self.query("session.context")["active"] is not False:
            raise AssertionError("Synthetic helper validation failed")
        if self.db.read_bytes() != self.before:
            raise AssertionError("Synthetic helper validation failed")


class SessionContextTests(unittest.TestCase):
    def test_default_disabled_and_explicit_operator_start_required(self):
        with ContextFixture(bridge=False) as f:
            f.start()
            self.assertEqual(
                f.query(
                    "session.start",
                    executable="normal",
                    confirmed=True,
                    share_context=True,
                    success=False,
                ),
                "Unavailable",
            )
            v = f.launch("normal")
            self.assertEqual(
                f.query("session.context"),
                dict(enabled=False, active=False, context_calls=0, handoff_calls=0),
            )
            self.assertFalse(list(f.runtime.glob("session-*.cap")))
            f.end(v)
        with ContextFixture() as f:
            f.start()
            for params in [
                dict(confirmed=False),
                dict(confirmed=True, mcpServers=[]),
                dict(confirmed=True, share_context="yes"),
            ]:
                self.assertEqual(
                    f.query(
                        "session.start", executable="normal", success=False, **params
                    ),
                    "InvalidRequest",
                )
            self.assertEqual(
                f.query(
                    "session.start",
                    cap="alpha",
                    executable="normal",
                    confirmed=True,
                    share_context=True,
                    success=False,
                ),
                "Unauthorized",
            )
            self.assertFalse(list(f.runtime.glob("session-*.cap")))

    def test_stop_revokes_established_peer_reconnect_and_old_generation(self):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context()
            path = f.lease_file()
            old = json.loads(path.read_text())
            self.assertEqual(old["format"], 2)
            self.assertEqual(
                old["binding"], dict(session=v["session_id"], agent="mcp-passive")
            )
            self.assertNotEqual(old["token"], f.credential("operator")["token"])
            self.assertNotEqual(old["token"], f.credential("alpha")["token"])
            with f.lease_peer() as peer:
                context = peer.request(
                    "context.get", {"include_documents": True}, ident="before"
                )["result"]
                self.assertEqual(
                    context, f.cli("context", "alpha", "--include-documents")
                )
                f.end(v)
                self.assertEqual(
                    peer.request("context.get", {}, ident="after")["error"],
                    "Unauthorized",
                )
            f.revoked()
            self.assertTrue(f.rejected(old))
            fresh = f.launch_context(project="beta")
            self.assertNotEqual(fresh["session_id"], v["session_id"])
            self.assertTrue(f.rejected(old))
            with f.lease_peer() as peer:
                self.assertEqual(
                    peer.request("context.get", {})["result"]["project"], "beta"
                )
            self.assertEqual(
                f.query("session.stop", session_id=v["session_id"], success=False),
                "NotFound",
            )
            f.query("session.stop", project="beta", session_id=fresh["session_id"])
            f.wait("stopped", project="beta")

    def test_in_flight_read_is_discarded_when_revoked_before_publication(self):
        with ContextFixture() as f:
            with closing(sqlite3.connect(f.db)) as db:
                db.execute("PRAGMA journal_mode=DELETE")
            f.before = f.db.read_bytes()
            f.start()
            v = f.launch_context()
            with f.lease_peer() as peer, closing(sqlite3.connect(f.db)) as writer:
                writer.execute("BEGIN EXCLUSIVE")
                peer.send(
                    dict(
                        api_version=1,
                        id="blocked-read",
                        method="context.get",
                        params={},
                    )
                )
                with ThreadPoolExecutor(max_workers=1) as pool:
                    pending = pool.submit(peer.recv)
                    time.sleep(0.2)
                    self.assertFalse(
                        pending.done(), "synthetic exclusive lock must hold the read"
                    )
                    f.end(v)
                    writer.rollback()
                    self.assertEqual(pending.result(timeout=6)["error"], "Unauthorized")
            f.revoked()

    def test_operator_cli_requires_confirmation_and_exposes_only_content_free_audit(
        self,
    ):
        with ContextFixture() as f:
            f.start()
            self.assertEqual(
                f.cli(
                    "session-start",
                    "alpha",
                    "mcp-passive",
                    "--share-context",
                    success=False,
                ),
                {"error": "InvalidRequest"},
            )
            f.cli(
                "session-start",
                "alpha",
                "mcp-passive",
                "--confirm-start",
                "--share-context",
            )
            v = f.wait("ready")
            self.assertEqual(
                f.cli("session-context", "alpha"),
                dict(enabled=True, active=True, context_calls=0, handoff_calls=0),
            )
            f.cli("session-stop", "alpha", v["session_id"])
            f.wait("stopped")
            f.revoked()

    def test_unrepresentable_canonical_runtime_path_fails_closed_without_panic(self):
        with ContextFixture() as f:
            if os.name == "nt":
                f.runtime = f.root / ("nonunicode-" + chr(0xD800))
            else:
                target = f.root / os.fsdecode(b"nonunicode-\xff")
                target.mkdir()
                alias = f.root / "runtime-alias"
                alias.symlink_to(target, target_is_directory=True)
                f.runtime = alias / "run"
                f.endpoint = str(f.runtime / "control.sock")
            f.location = ["--runtime-dir", str(f.runtime), "--endpoint", f.endpoint]
            result = subprocess.run(
                f.args(), capture_output=True, timeout=8, env=f.environment
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn(b"UnsafeRuntime", result.stderr)
            self.assertNotIn(b"panicked", result.stderr)
            self.assertFalse(list(f.runtime.glob("*.cap")))
            self.assertEqual(f.db.read_bytes(), f.before)

    def test_expiry_denies_already_connected_peer_and_new_handshake(self):
        with ContextFixture(ttl=1) as f:
            f.start()
            v = f.launch_context()
            c = json.loads(f.lease_file().read_text())
            with f.lease_peer() as peer:
                time.sleep(1.1)
                self.assertEqual(
                    peer.request("handoff.get", {"task_id": "shared"})["error"],
                    "Unauthorized",
                )
            self.assertTrue(f.rejected(c))
            f.revoked()
            f.end(v)

    def test_cancel_revokes_whole_session_but_preserves_prompt_cancel_semantics(self):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context("mcp-cancel")
            c = json.loads(f.lease_file().read_text())
            with f.lease_peer() as peer:
                f.query(
                    "session.prompt",
                    session_id=v["session_id"],
                    prompt="synthetic waiting turn",
                )
                f.wait("running")
                f.query("session.cancel", session_id=v["session_id"])
                v = f.wait("completed")
                self.assertEqual(v["stop_reason"], "cancelled")
                self.assertTrue(v["process_alive"])
                self.assertEqual(
                    peer.request("context.get", {})["error"], "Unauthorized"
                )
            f.revoked()
            self.assertTrue(f.rejected(c))
            self.assertEqual(
                f.query(
                    "session.prompt",
                    session_id=v["session_id"],
                    prompt="x",
                    share_context=True,
                    success=False,
                ),
                "InvalidRequest",
            )
            f.end(v)

    def test_scope_operator_mutations_and_forged_parameters_are_denied(self):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context()
            attacks = [
                ("project.list", {}, "Unauthorized"),
                ("session.start", {"anything": "untrusted"}, "Unauthorized"),
                ("session.context", {}, "Unauthorized"),
                ("context.get", {"project": "beta"}, "InvalidRequest"),
                ("context.get", {"role": "operator"}, "InvalidRequest"),
                ("context.get", {"session_id": "old"}, "InvalidRequest"),
                (
                    "handoff.get",
                    {"task_id": "shared", "project": "beta"},
                    "InvalidRequest",
                ),
                ("profile.confirm", {}, "Unsupported"),
            ]
            with f.lease_peer() as peer:
                for i, (method, params, error) in enumerate(attacks):
                    self.assertEqual(
                        peer.request(method, params, ident=str(i))["error"], error
                    )
            self.assertEqual(f.query("session.context")["context_calls"], 0)
            # Existing startup project capability and operator retain their original behavior.
            with Raw(f, "alpha") as legacy:
                self.assertEqual(
                    legacy.request("context.get", {})["result"]["project"], "alpha"
                )
            self.assertEqual(f.cli("projects"), [{"name": "alpha"}, {"name": "beta"}])
            f.end(v)
            f.revoked()

    def test_concurrent_held_and_reconnecting_peers_cannot_resurrect_revoked_lease(
        self,
    ):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context()
            c = json.loads(f.lease_file().read_text())
            with f.lease_peer() as a, f.lease_peer() as b:
                f.end(v)
                self.assertEqual(a.request("context.get", {})["error"], "Unauthorized")
                self.assertEqual(
                    b.request("handoff.get", {"task_id": "shared"})["error"],
                    "Unauthorized",
                )
            with ThreadPoolExecutor(max_workers=4) as pool:
                self.assertTrue(all(pool.map(lambda _: f.rejected(c), range(8))))
            f.revoked()

    def test_process_death_and_agent_rejection_cleanup_without_secret_diagnostics(self):
        for mode in ["mcp-crash", "mcp-reject"]:
            with self.subTest(mode=mode), ContextFixture() as f:
                f.start()
                f.query(
                    "session.start", executable=mode, confirmed=True, share_context=True
                )
                if mode == "mcp-crash":
                    v = f.wait("ready")
                    f.query(
                        "session.prompt",
                        session_id=v["session_id"],
                        prompt="synthetic crash",
                    )
                f.wait("failed")
                f.revoked()
                self.assertNotIn(
                    "SENSITIVE_AUTH_TEST", json.dumps(f.query("session.get"))
                )

    def test_protocol_failure_diagnostics_do_not_retain_untrusted_method_or_body(self):
        with ContextFixture() as f:
            f.set_programs(["mcp-unsupported-notification"])
            f.start()
            v = f.launch_context("mcp-unsupported-notification")
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="synthetic protocol failure",
            )
            v = f.wait("failed")
            f.revoked()
            self.assertIn("ACP rejected: notification method", v["events"])
            self.assertNotIn("SENSITIVE_AUTH_TEST", json.dumps(v))
            self.assertNotIn("_UNTRUSTED_SHOULD_NOT_BE_LOGGED", json.dumps(v))

    def test_hard_kill_restart_removes_stale_file_and_refuses_saved_secret(self):
        with ContextFixture() as f:
            f.start()
            f.launch_context()
            c = json.loads(f.lease_file().read_text())
            path = f.lease_file()
            with f.lease_peer() as peer:
                f.proc.kill()
                f.proc.wait(timeout=5)
                self.assertTrue(
                    path.exists(), "hard kill leaves a recognized stale capability"
                )
                f.start()
                self.assertFalse(path.exists())
                self.assertTrue(f.rejected(c))
                try:
                    reply = peer.request("context.get", {})
                    self.assertNotIn("result", reply)
                except (EOFError, OSError):
                    pass
            self.assertIsNone(f.query("session.get"))
            self.assertEqual(f.db.read_bytes(), f.before)

    def test_registered_root_drift_revokes_instead_of_rebinding_context(self):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context()
            new_root = f.root / "replacement-root"
            new_root.mkdir()
            with f.lease_peer() as peer:
                with closing(sqlite3.connect(f.db)) as db, db:
                    root = db.execute(
                        "SELECT root FROM projects WHERE name='alpha'"
                    ).fetchone()[0]
                    db.execute(
                        "UPDATE projects SET root=? WHERE name='alpha'",
                        (str(new_root),),
                    )
                self.assertEqual(
                    peer.request("context.get", {}, ident="drift")["error"],
                    "Unauthorized",
                )
                with closing(sqlite3.connect(f.db)) as db, db:
                    db.execute("UPDATE projects SET root=? WHERE name='alpha'", (root,))
                # Only this fixture operator changes synthetic registration; service stays read-only.
                f.before = f.db.read_bytes()
                self.assertEqual(
                    peer.request("context.get", {}, ident="restored")["error"],
                    "Unauthorized",
                )
            f.revoked()
            f.end(v)

    def test_deterministic_acp_agent_discovers_and_calls_actual_two_tool_bridge(self):
        with ContextFixture() as f:
            f.start()
            v = f.launch_context("mcp")
            self.assertEqual(f.query("session.context")["context_calls"], 0)
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="call both synthetic context tools",
            )
            v = f.wait("completed")
            self.assertEqual(
                f.query("session.context"),
                dict(enabled=True, active=True, context_calls=1, handoff_calls=1),
            )
            # Independently inspect the same stdio bridge, not an agent-authored summary.
            proc = subprocess.Popen(
                [
                    binary("acp-mcp"),
                    *f.location,
                    "--project",
                    "alpha",
                    "--capability-file",
                    str(f.lease_file()),
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
            f.children.append(proc)
            lines = collect(proc.stdout)

            def send(message):
                proc.stdin.write(json.dumps(message).encode() + b"\n")
                proc.stdin.flush()
                return json.loads(lines.get(timeout=8))

            meta = {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}
            tools = send(
                dict(jsonrpc="2.0", id=1, method="tools/list", params={"_meta": meta})
            )["result"]["tools"]
            self.assertEqual(
                [t["name"] for t in tools], ["get_effective_context", "get_handoff"]
            )
            data = send(
                dict(
                    jsonrpc="2.0",
                    id=2,
                    method="tools/call",
                    params={
                        "_meta": meta,
                        "name": "get_effective_context",
                        "arguments": {"include_documents": True},
                    },
                )
            )["result"]
            self.assertEqual(
                json.loads(data["content"][0]["text"]),
                f.cli("context", "alpha", "--include-documents"),
            )
            for i, name in enumerate(
                ["profile.confirm", "session.start", "project.list"], 3
            ):
                self.assertEqual(
                    send(
                        dict(
                            jsonrpc="2.0",
                            id=i,
                            method="tools/call",
                            params={"_meta": meta, "name": name, "arguments": {}},
                        )
                    )["error"]["code"],
                    -32602,
                )
            f.end(v)
            data = send(
                dict(
                    jsonrpc="2.0",
                    id=7,
                    method="tools/call",
                    params={
                        "_meta": meta,
                        "name": "get_handoff",
                        "arguments": {"task_id": "shared"},
                    },
                )
            )["result"]
            self.assertTrue(data["isError"])
            f.revoked()


if __name__ == "__main__":
    unittest.main()
