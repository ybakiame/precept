"""Negative checks for the opt-in real smoke permission selector; no model calls."""

import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from test_real_context_smoke import approved_readonly_request, READONLY_REQUESTS

sys.path.pop(0)


class RealContextOptInTests(unittest.TestCase):
    def test_exact_readonly_tool_claim_and_separate_permission_optin(self):
        base = dict(opt_in=True, agent="kimi", remaining=2, allow_once="once")
        for title in READONLY_REQUESTS:
            call = dict(
                toolCallId="synthetic-call",
                title=title,
                content=[
                    {
                        "type": "content",
                        "content": {
                            "type": "text",
                            "text": "Requesting approval to Approve " + title,
                        },
                    }
                ],
            )
            self.assertTrue(approved_readonly_request(call, **base))
            for change in [
                dict(opt_in=False),
                dict(agent="opencode"),
                dict(remaining=0),
                dict(allow_once=None),
            ]:
                self.assertFalse(approved_readonly_request(call, **(base | change)))
            for changed in [
                None,
                [],
                call | dict(title=[title]),
                call | dict(title=title + " extra"),
                call | dict(rawInput={"project": "beta"}),
                call | dict(content=[]),
                call | dict(content=[{"type": "diff", "path": "synthetic"}]),
            ]:
                self.assertFalse(approved_readonly_request(changed, **base))
