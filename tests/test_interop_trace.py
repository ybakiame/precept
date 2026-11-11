"""Security regressions for the independent content-free MCP observer."""

import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from interop_trace import Observer, digest

sys.path.pop(0)


class InteropTraceTests(unittest.TestCase):
    def test_untrusted_request_and_error_text_never_reaches_trace(self):
        marker = "SENSITIVE_SYNTHETIC_DO_NOT_RECORD"
        observer = Observer()
        observer.request(
            dict(id=marker, method=marker, params=dict(name=marker, token=marker))
        )
        event = observer.response(dict(id=marker, error=dict(message=marker)))
        self.assertNotIn(marker, json.dumps(event))
        self.assertEqual(event["outcome"], "rpc_error")
        self.assertEqual(event["method"], "unknown")

    def test_response_correlation_and_historical_statuses_not_upgraded(self):
        observer = Observer()
        observer.request(
            dict(id=1, method="tools/call", params=dict(name="get_handoff"))
        )
        self.assertIsNone(observer.response(dict(id=2, result={})))
        value = dict(
            validation=[
                dict(status="FAIL"),
                dict(status="NOT_RUN"),
                dict(status="untrusted"),
            ],
            resume_warnings=[
                "Earlier validation is scoped to its recorded revision and environment",
                "SENSITIVE_SYNTHETIC",
            ],
        )
        event = observer.response(
            dict(id=1, result=dict(content=[dict(text=json.dumps(value))]))
        )
        self.assertEqual(event["validation_statuses"], ["FAIL", "NOT_RUN", "UNKNOWN"])
        self.assertEqual(event["freshness"], ["historical", "unknown"])
        self.assertEqual(event["result_sha256"], digest(value))
        self.assertNotIn("SENSITIVE_SYNTHETIC", json.dumps(event))
        self.assertIsNone(observer.response(dict(id=1, result={})))
