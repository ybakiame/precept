"""Exercise executable safety/acceptance paths with Python assertions disabled."""

import ast
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]


class OptimizedInteropTests(unittest.TestCase):
    def optimized(self, source, *args):
        env = dict(os.environ)
        env["PYTHONPATH"] = os.pathsep.join([str(ROOT / "src"), str(ROOT / "tools")])
        result = subprocess.run(
            [sys.executable, "-O", "-c", textwrap.dedent(source), *map(str, args)],
            env=env,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=20,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def test_no_optin_or_normal_home_cannot_reach_preflight_or_prompt(self):
        self.optimized(
            """
            from pathlib import Path
            from unittest import TestCase
            from unittest.mock import patch
            import evaluate_cross_agent as runner
            t = TestCase()
            for flags, error, message in [
                (["--kimi-home", str(Path.home().resolve())], RuntimeError, "opt-in"),
                (["--allow-kimi-subscription-turn"], RuntimeError, "opt-in"),
                (["--kimi-home", str(Path.home().resolve()),
                  "--allow-kimi-subscription-turn"], ValueError, "normal user HOME"),
            ]:
                with patch.object(runner.sys, "argv", ["runner", "--out", "unused", *flags]), \
                     patch.object(runner.subprocess, "check_output") as process, \
                     patch.object(runner, "Scenario") as fixture, \
                     patch.object(runner, "kimi_trial") as prompt:
                    with t.assertRaisesRegex(error, message):
                        runner.main()
                    process.assert_not_called()
                    fixture.assert_not_called()
                    prompt.assert_not_called()
        """
        )

    def test_permission_allowlist_and_budget_are_evaluated_before_approval(self):
        self.optimized(
            """
            import json
            from types import SimpleNamespace
            from unittest import TestCase
            from unittest.mock import Mock
            from evaluate_cross_agent import approve_permissions
            t = TestCase()
            def view(title):
                return dict(session_id="synthetic", permissions=[dict(
                    request_id="synthetic", allow_once="once", operation=json.dumps(dict(
                        toolCallId="synthetic", title=title, content=[dict(
                            type="content", content=dict(type="text",
                            text="Requesting approval to Approve " + title))])))])
            f = SimpleNamespace(query=Mock())
            with t.assertRaisesRegex(RuntimeError, "Unapproved"):
                approve_permissions(f, view("Bash"), 0)
            f.query.assert_not_called()
            safe = view("mcp__acp-context__get_handoff")
            t.assertEqual(approve_permissions(f, safe, 0), 1)
            f.query.assert_called_once_with("session.permission", session_id="synthetic",
                request_id="synthetic", option_id="once")
            f.query.reset_mock()
            for current in [2, 3]:
                with t.assertRaisesRegex(RuntimeError, "Unapproved"):
                    approve_permissions(f, safe, current)
                f.query.assert_not_called()
        """
        )

    def test_observer_rejects_paths_marker_and_command_before_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = base / "fixture"
            root.mkdir()
            marker = root / ".acp-interop-fixture"
            marker.touch()
            child = base / "child-was-launched"
            env = dict(os.environ)
            for case in ["outside", "marker", "command"]:
                with self.subTest(case=case):
                    out = (
                        base / "outside.jsonl"
                        if case == "outside"
                        else root / "trace.jsonl"
                    )
                    if case == "marker":
                        marker.unlink()
                    else:
                        marker.touch(exist_ok=True)
                    command = [
                        str(Path(sys.executable).resolve()),
                        "-c",
                        "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
                        str(child),
                    ]
                    if case == "command":
                        command[0] = "relative-command"
                    result = subprocess.run(
                        [
                            sys.executable,
                            "-O",
                            str(ROOT / "tools/interop_trace.py"),
                            "--synthetic-root",
                            str(root),
                            "--out",
                            str(out),
                            "--",
                            *command,
                        ],
                        env=env,
                        capture_output=True,
                        text=True,
                        input="",
                        timeout=20,
                    )
                    self.assertEqual(result.returncode, 2)
                    self.assertIn(
                        "Sanitized synthetic MCP observer failure", result.stderr
                    )
                    self.assertFalse(out.exists())
                    self.assertFalse(child.exists())

    def test_failed_handoff_and_client_result_invariants_still_raise(self):
        self.optimized(
            """
            from types import SimpleNamespace
            from unittest import TestCase
            from unittest.mock import Mock
            from evaluate_cross_agent import Codex, check_handoff
            t = TestCase()
            value = dict(validation=[dict(status="FAIL")], resume_warnings=[])
            f = SimpleNamespace(cli=Mock(return_value=value), validation=[dict(status="PASS")])
            client = SimpleNamespace(call=Mock(return_value=value))
            with t.assertRaises(RuntimeError):
                check_handoff(f, client, [])
            f.validation = value["validation"]
            # Partial overlap must fail too, even if the observed set has extra flags.
            value["resume_warnings"] = ["untrusted"]
            with t.assertRaises(RuntimeError):
                check_handoff(f, client, ["head", "historical"])
            codex = object.__new__(Codex)
            codex.thread = "synthetic"
            codex.rpc = Mock(return_value=dict(result=dict(isError=False)))
            with t.assertRaises(RuntimeError):
                codex.call("forbidden", {}, success=False)
            codex.rpc.return_value = dict(error=dict(message="synthetic"))
            with t.assertRaises(RuntimeError):
                codex.call("get_handoff", {})
        """
        )

    def test_source_security_and_output_preflight_still_reject(self):
        self.optimized(
            """
            import json
            from pathlib import Path
            from tempfile import TemporaryDirectory
            from types import SimpleNamespace
            from unittest import TestCase
            from unittest.mock import patch
            import evaluate_cross_agent as runner
            t = TestCase()
            with TemporaryDirectory() as tmp:
                root = Path(tmp).resolve()
                out = root / "must-not-exist"
                security = root / "security.json"
                passing = dict(tested_git_sha="synthetic-head", worktree_dirty=False,
                    gates=[dict(name=n, status="PASS") for n in
                        ["context-process", "lease-unit", "root-clippy"]])
                # Exercise same-source, dirty source, missing gates, and unrelated extra gates.
                reports = [passing | dict(tested_git_sha="old"),
                    passing | dict(worktree_dirty=True),
                    passing | dict(gates=[dict(name="unrelated", status="PASS")]),
                    passing | dict(gates=[]), passing]
                for dirty, report in [(True, passing)] + [(False, r) for r in reports]:
                    security.write_text(json.dumps(report))
                    flags = ["runner", "--out", str(out), "--kimi-home", str(root),
                        "--kimi-executable", str(Path(__import__("sys").executable).resolve()),
                        "--allow-kimi-subscription-turn", "--security-report", str(security)]
                    with patch.object(runner.sys, "argv", flags), \
                         patch.object(runner, "os", SimpleNamespace(name="posix")), \
                         patch.object(runner, "validate_kimi_home"), \
                         patch.object(runner.subprocess, "check_output",
                            side_effect=[b" M tracked" if dirty else b"", "synthetic-head"]), \
                         patch.object(runner, "Scenario") as fixture:
                        with t.assertRaises(RuntimeError):
                            runner.main()
                        fixture.assert_not_called()
                        t.assertFalse(out.exists())
        """
        )

    def test_shared_fixture_checks_survive_optimized_execution(self):
        self.optimized(
            """
            import struct
            from types import SimpleNamespace
            from unittest import TestCase
            from unittest.mock import Mock, patch
            from test_local_service import Fixture, Raw, MAX_FRAME
            from test_session_context import ContextFixture
            from test_agent_session import SessionFixture
            t = TestCase()
            raw = object.__new__(Raw)
            raw.read_exact = Mock(return_value=struct.pack(">I", MAX_FRAME + 1))
            with t.assertRaisesRegex(AssertionError, "framing"):
                raw.recv()
            raw.read_exact.assert_called_once_with(4)
            fixture = object.__new__(Fixture)
            fixture.location = []
            with patch("test_local_service.subprocess.run",
                       return_value=SimpleNamespace(returncode=1, stdout="{}")):
                with t.assertRaisesRegex(AssertionError, "exit outcome"):
                    fixture.cli("health")
            context = object.__new__(ContextFixture)
            context.runtime = SimpleNamespace(glob=lambda _: [])
            with t.assertRaisesRegex(AssertionError, "one private"):
                context.lease_file()
            peer = Mock()
            peer.__enter__ = Mock(return_value=peer)
            peer.__exit__ = Mock()
            peer.request.return_value = dict(error="Unauthorized", result={})
            with patch("test_agent_session.Raw", return_value=peer):
                with t.assertRaisesRegex(AssertionError, "operator query"):
                    SessionFixture.query(object.__new__(SessionFixture), "session.get")
        """
        )

    def test_executable_checks_cannot_regress_to_assert_statements(self):
        for name in ["evaluate_cross_agent.py", "interop_trace.py"]:
            tree = ast.parse((ROOT / "tools" / name).read_text())
            self.assertFalse(
                any(isinstance(n, ast.Assert) for n in ast.walk(tree)), name
            )
        tree = ast.parse((ROOT / "tools/test_real_acp_smoke.py").read_text())
        guard = next(
            n
            for n in tree.body
            if isinstance(n, ast.FunctionDef) and n.name == "private_home"
        )
        self.assertFalse(any(isinstance(n, ast.Assert) for n in ast.walk(guard)))
