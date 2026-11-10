"""Real Rust daemon/child ACP v1 fixture, operator isolation. Synthetic state only."""

import json
import os
from pathlib import Path
import subprocess
import time
import unittest
from test_local_service import Fixture, Raw, Bridge, binary, stop

MODES = [
    "normal",
    "cancel",
    "permission",
    "malformed-permission",
    "duplicate",
    "duplicate-response",
    "wrong-id",
    "wrong-session",
    "huge",
    "malformed",
    "crash",
    "timeout",
    "version",
]


class SessionFixture(Fixture):
    def __init__(self):
        super().__init__(start=False)
        self.environment = dict(
            os.environ, ACP_TEST_PARENT_SENTINEL="synthetic-not-a-secret"
        )
        self.config = self.root / "programs.json"
        env = (
            {
                k: os.environ[k]
                for k in ["SystemRoot", "WINDIR", "TEMP", "TMP"]
                if k in os.environ
            }
            if os.name == "nt"
            else {}
        )
        self.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id=m, executable=binary("acp-fake-agent"), args=[m], env=env
                        )
                        for m in MODES[:8]
                    ],
                )
            )
        )
        # Eight configured programs maximum; each negative group replaces the config before startup.
        self.set_programs(MODES[:8])

    def set_programs(self, modes):
        env = (
            {
                k: os.environ[k]
                for k in ["SystemRoot", "WINDIR", "TEMP", "TMP"]
                if k in os.environ
            }
            if os.name == "nt"
            else {}
        )
        self.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id=m, executable=binary("acp-fake-agent"), args=[m], env=env
                        )
                        for m in modes
                    ],
                )
            )
        )
        if os.name == "nt":
            subprocess.run(
                [
                    "icacls",
                    str(self.config),
                    "/inheritance:r",
                    "/grant:r",
                    os.environ["USERNAME"] + ":F",
                ],
                check=True,
                capture_output=True,
                timeout=10,
            )
        else:
            self.config.chmod(0o600)

    def args(self, database=None):
        return super().args(database) + ["--agent-config", str(self.config)]

    def query(self, method, project="alpha", cap="operator", success=True, **params):
        with Raw(self, cap) as r:
            reply = r.request(
                method, dict(session_api_version=1, project=project, **params)
            )
        if success:
            assert (
                reply.get("error") is None
            ), "sanitized operator query failed: " + str(reply.get("error"))
            return (
                reply["result"]["session"]
                if method == "session.get"
                else reply["result"]
            )
        return reply["error"]

    def wait(self, state, project="alpha", timeout=8):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            v = self.query("session.get", project)
            if v and v["state"] == state:
                return v
            time.sleep(0.03)
        raise AssertionError("session did not reach " + state)

    def launch(self, mode="normal"):
        v = self.query("session.start", executable=mode, confirmed=True)
        return self.wait("ready") if mode not in ["timeout", "version", "auth"] else v

    def end(self, v):
        self.query("session.stop", session_id=v["session_id"])
        return self.wait("stopped")


class AgentSessionTests(unittest.TestCase):
    def test_prompt_two_clients_reattach_and_stop_no_database_write(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch()
            self.assertTrue(v["process_alive"])
            self.assertEqual(
                f.query(
                    "session.start", executable="normal", confirmed=True, success=False
                ),
                "Busy",
            )
            self.assertEqual(
                f.query(
                    "session.start",
                    project="beta",
                    executable="normal",
                    confirmed=True,
                    success=False,
                ),
                "Busy",
            )
            # Independent connections; closing one does not stop the runtime.
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="Synthetic prompt 中文",
            )
            v = f.wait("completed")
            self.assertIn("Synthetic ACP update 中文", " ".join(v["events"]))
            self.assertEqual(f.query("session.get")["session_id"], v["session_id"])
            self.assertEqual(f.query("session.get", project="beta"), None)
            self.assertEqual(
                f.query(
                    "session.prompt",
                    project="beta",
                    session_id=v["session_id"],
                    prompt="no",
                    success=False,
                ),
                "NotFound",
            )
            f.end(v)
            self.assertEqual(f.before, f.db.read_bytes())

    def test_cancel_and_old_generation_cannot_control_new_session(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch("cancel")
            f.query("session.prompt", session_id=v["session_id"], prompt="wait")
            f.wait("running")
            f.query("session.cancel", session_id=v["session_id"])
            v = f.wait("completed")
            self.assertEqual(v["stop_reason"], "cancelled")
            f.end(v)
            new = f.launch("normal")
            self.assertNotEqual(v["session_id"], new["session_id"])
            self.assertEqual(
                f.query("session.stop", session_id=v["session_id"], success=False),
                "NotFound",
            )
            f.end(new)

    def test_permission_default_deny_specific_allow_once_and_no_always(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch("permission")
            for choice in [None, "once"]:
                f.query(
                    "session.prompt", session_id=v["session_id"], prompt="permission"
                )
                pending = f.wait("waiting_permission")["permissions"][0]
                self.assertEqual(f.query("session.get")["permissions"][0], pending)
                self.assertEqual(
                    f.query(
                        "session.permission",
                        session_id=v["session_id"],
                        request_id=pending["request_id"],
                        option_id="always",
                        success=False,
                    ),
                    "InvalidRequest",
                )
                f.query(
                    "session.permission",
                    session_id=v["session_id"],
                    request_id=pending["request_id"],
                    option_id=choice,
                )
                done = f.wait("completed")
                self.assertIn(
                    (
                        "Permission explicitly allowed once"
                        if choice
                        else "Permission denied"
                    ),
                    " ".join(done["events"]),
                )
                self.assertEqual(
                    f.query(
                        "session.permission",
                        session_id=v["session_id"],
                        request_id=pending["request_id"],
                        option_id="once",
                        success=False,
                    ),
                    "InvalidRequest",
                )
            f.end(v)

    def test_permission_expiry_cancel_and_stop_revoke(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch("permission")
            f.query("session.prompt", session_id=v["session_id"], prompt="expire")
            f.wait("waiting_permission")
            v = f.wait("completed", timeout=34)
            self.assertIn("Permission expired: denied", v["events"])
            f.query("session.prompt", session_id=v["session_id"], prompt="cancel")
            p = f.wait("waiting_permission")["permissions"][0]
            f.query("session.cancel", session_id=v["session_id"])
            f.wait("completed")
            self.assertEqual(
                f.query(
                    "session.permission",
                    session_id=v["session_id"],
                    request_id=p["request_id"],
                    option_id="once",
                    success=False,
                ),
                "InvalidRequest",
            )
            f.end(v)

    def test_malformed_permission_is_denied_without_approval(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch("malformed-permission")
            f.query("session.prompt", session_id=v["session_id"], prompt="malformed")
            v = f.wait("completed")
            self.assertEqual(v["permissions"], [])
            self.assertIn("Unsupported permission request: denied", v["events"])
            f.end(v)

    def test_protocol_failures_are_bounded_and_sanitized(self):
        with SessionFixture() as f:
            f.set_programs(
                [
                    "duplicate",
                    "duplicate-response",
                    "wrong-id",
                    "wrong-session",
                    "huge",
                    "malformed",
                    "crash",
                    "normal",
                ]
            )
            f.start()
            for mode in [
                "duplicate",
                "duplicate-response",
                "wrong-id",
                "wrong-session",
                "huge",
                "malformed",
                "crash",
            ]:
                with self.subTest(mode=mode):
                    v = f.launch(mode)
                    f.query(
                        "session.prompt", session_id=v["session_id"], prompt="failure"
                    )
                    v = f.wait("failed")
                    deadline = time.monotonic() + 3
                    while v["process_alive"] and time.monotonic() < deadline:
                        time.sleep(0.03)
                        v = f.query("session.get")
                    self.assertFalse(v["process_alive"])
                    self.assertLessEqual(len(v["events"]), 64)
                    self.assertNotIn("must not leak", " ".join(v["events"]))
                    self.assertIn(
                        v["failure"], ["InvalidRequest", "TooLarge", "Unavailable"]
                    )

    def test_initialization_timeout_and_version(self):
        with SessionFixture() as f:
            f.set_programs(["timeout", "version", "auth"])
            f.start()
            for mode in ["timeout", "version", "auth"]:
                v = f.launch(mode)
                v = f.wait("failed")
                self.assertEqual(
                    v["failure"],
                    {
                        "timeout": "Timeout",
                        "version": "Version",
                        "auth": "Authentication required",
                    }[mode],
                )
                while f.query("session.get")["process_alive"]:
                    time.sleep(0.03)

    def test_mcp_two_tools_and_agent_credentials_cannot_control_or_enumerate(self):
        with SessionFixture() as f:
            f.start()
            v = f.launch("permission")
            f.query("session.prompt", session_id=v["session_id"], prompt="wait")
            p = f.wait("waiting_permission")["permissions"][0]
            for project in ["alpha", "beta"]:
                for method in [
                    "session.capabilities",
                    "session.list",
                    "session.get",
                    "session.start",
                    "session.prompt",
                    "session.cancel",
                    "session.stop",
                    "session.permission",
                    "session.unknown",
                ]:
                    self.assertEqual(
                        f.query(method, cap=project, success=False), "Unauthorized"
                    )
                b = Bridge(f, project)
                tools = b.request(
                    dict(
                        jsonrpc="2.0",
                        id=1,
                        method="tools/list",
                        params={
                            "_meta": {
                                "io.modelcontextprotocol/protocolVersion": "2026-07-28"
                            }
                        },
                    )
                )
                self.assertEqual(
                    [t["name"] for t in tools["result"]["tools"]],
                    ["get_effective_context", "get_handoff"],
                )
                denied = b.request(
                    dict(
                        jsonrpc="2.0",
                        id=2,
                        method="tools/call",
                        params={
                            "name": "session.start",
                            "arguments": {},
                            "_meta": {
                                "io.modelcontextprotocol/protocolVersion": "2026-07-28"
                            },
                        },
                    )
                )
                self.assertIn("error", denied)
            self.assertEqual(f.query("session.get")["permissions"][0], p)
            for params, error in [
                ({"session_api_version": 2, "project": "alpha"}, "Version"),
                ({"project": "alpha"}, "InvalidRequest"),
                ({"session_api_version": 1, "project": "other"}, "Unauthorized"),
                (
                    {"session_api_version": 1, "project": "alpha", "shell": "bad"},
                    "InvalidRequest",
                ),
            ]:
                with Raw(f) as r:
                    self.assertEqual(r.request("session.get", params)["error"], error)
            self.assertEqual(
                f.query(
                    "session.start", executable="normal", confirmed=False, success=False
                ),
                "InvalidRequest",
            )
            f.end(v)

    def test_registered_root_drift_revokes_prompt_and_allow_but_stop_remains(self):
        import sqlite3

        with SessionFixture() as f:
            f.start()
            v = f.launch("permission")
            f.query("session.prompt", session_id=v["session_id"], prompt="pending")
            pending = f.wait("waiting_permission")["permissions"][0]
            with sqlite3.connect(f.db) as db:
                db.execute(
                    "UPDATE projects SET root=? WHERE name=?",
                    (str(f.root / "fixture/beta"), "alpha"),
                )
            self.assertEqual(
                f.query(
                    "session.permission",
                    session_id=v["session_id"],
                    request_id=pending["request_id"],
                    option_id="once",
                    success=False,
                ),
                "UnsafeRuntime",
            )
            self.assertEqual(
                f.query(
                    "session.prompt",
                    session_id=v["session_id"],
                    prompt="no",
                    success=False,
                ),
                "UnsafeRuntime",
            )
            f.end(v)

    def test_cancel_deadline_kills_uncooperative_child(self):
        with SessionFixture() as f:
            f.set_programs(["cancel-timeout"])
            f.start()
            v = f.launch("cancel-timeout")
            f.query(
                "session.prompt", session_id=v["session_id"], prompt="cancel deadline"
            )
            f.wait("running")
            f.query("session.cancel", session_id=v["session_id"])
            v = f.wait("failed")
            self.assertEqual(v["failure"], "Timeout")
            deadline = time.monotonic() + 3
            while (
                f.query("session.get")["process_alive"] and time.monotonic() < deadline
            ):
                time.sleep(0.03)
            self.assertFalse(f.query("session.get")["process_alive"])

    def test_no_configuration_and_unsafe_config_fail_closed(self):
        with Fixture() as f:
            with Raw(f) as r:
                cap = r.request(
                    "session.capabilities", dict(session_api_version=1, project="alpha")
                )["result"]
                self.assertEqual(cap["executables"], [])
            with Raw(f) as r:
                self.assertEqual(
                    r.request(
                        "session.start",
                        dict(
                            session_api_version=1,
                            project="alpha",
                            executable="unknown",
                            confirmed=True,
                        ),
                    )["error"],
                    "Unauthorized",
                )
        for change in [
            "environment",
            "relative",
            "unknown-field",
            "too-many",
            "broad-mode",
            "project-config",
        ]:
            with self.subTest(change=change), SessionFixture() as f:
                data = json.loads(f.config.read_text())
                if change == "environment":
                    data["programs"][0]["env"] = {"ACP_OPERATOR_TOKEN": "not-a-token"}
                elif change == "relative":
                    data["programs"][0]["executable"] = "relative-agent"
                elif change == "unknown-field":
                    data["shell"] = "never-execute"
                elif change == "too-many":
                    data["programs"] *= 2
                f.config.write_text(json.dumps(data))
                if change == "broad-mode" and os.name != "nt":
                    f.config.chmod(0o644)
                elif change == "broad-mode":
                    subprocess.run(
                        ["icacls", str(f.config), "/grant", "*S-1-1-0:F"],
                        check=True,
                        capture_output=True,
                        timeout=10,
                    )
                if change == "project-config":
                    target = f.root / "fixture/alpha/programs.json"
                    target.write_bytes(f.config.read_bytes())
                    if os.name != "nt":
                        target.chmod(0o600)
                    f.config = target
                proc = subprocess.run(f.args(), capture_output=True, timeout=8)
                self.assertEqual(proc.returncode, 2)
                self.assertNotIn(b"not-a-token", proc.stderr)
                self.assertFalse(f.runtime.exists())

    def test_daemon_restart_loses_sessions_and_rejects_old_ids(self):
        with SessionFixture() as f:
            f.start()
            old = f.launch("normal")
            f.end(old)
            stop(f.proc)
            f.start()
            self.assertIsNone(f.query("session.get"))
            new = f.launch("normal")
            self.assertNotEqual(new["session_id"], old["session_id"])
            self.assertEqual(
                f.query("session.stop", session_id=old["session_id"], success=False),
                "NotFound",
            )
            f.end(new)


if __name__ == "__main__":
    unittest.main()
