"""Safety regressions for local gates; no SDK, desktop or real checkout access."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("local_verify", ROOT / "tools/local_verify.py")
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class LocalValidationTests(unittest.TestCase):
    def test_missing_cache_is_not_a_regression_but_compiler_and_assertions_are(self):
        for output in ["you are in the offline mode (--offline)", "no matching package named fake",
                       "candidate versions found; offline mode (via `--offline`)"]:
            self.assertTrue(VERIFY.unavailable(output))
        for output in ["error[E0308]: mismatched types; offline mode (via `--offline`)", "test failed: assertion failed", "permission denied"]:
            self.assertFalse(VERIFY.unavailable(output))

    def test_all_rust_builds_pin_dependencies_offline_and_never_dispatch_actions(self):
        for _, command in VERIFY.gates(True, True, True):
            self.assertNotIn("gh", command)
            self.assertNotIn("workflow", command)
            if command[:2] in [["cargo", name] for name in ["clippy", "test", "build", "run"]]:
                self.assertIn("--offline", command)
                self.assertIn("--locked", command)

    def test_gate_status_preserves_failure_and_explicit_unavailable(self):
        import sys
        with tempfile.TemporaryDirectory(prefix="acp-gate-") as raw:
            output = Path(raw)
            result = VERIFY.run_gate("failed", [sys.executable, "-c", "raise SystemExit(3)"], output, None)
            self.assertEqual(result["status"], "FAIL")
            self.assertEqual(result["exit_code"], 3)
            result = VERIFY.run_gate("skipped", [sys.executable, "-c", "print('NOT_RUN: missing desktop')"], output, None)
            self.assertEqual(result["status"], "NOT_RUN")
            result = VERIFY.run_gate("blocked", [sys.executable, "-c", "print('BLOCKED: native Wayland unavailable'); raise SystemExit(2)"], output, None)
            self.assertEqual(result["status"], "BLOCKED")
            result = VERIFY.run_gate("passed", [sys.executable, "-c", "print('Ran 21 tests')"], output, None)
            self.assertEqual(result["status"], "PASS")
            self.assertEqual(result["observed_count"], 21)
            result = VERIFY.run_gate("with-optional-note", [sys.executable, "-c", "print('{\"status\":\"PASS\",\"accessibility\":\"NOT_RUN: optional\"}')"], output, None)
            self.assertEqual(result["status"], "PASS")
