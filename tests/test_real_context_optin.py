"""Negative checks for the opt-in real smoke permission selector; no model calls."""

import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from test_real_context_smoke import (
    approved_readonly_request,
    READONLY_REQUESTS,
    credential_mode_allowed,
    opencode_model_allowed,
)

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

    def test_anonymous_mode_never_uses_credentials_and_is_opencode_only(self):
        self.assertTrue(credential_mode_allowed("opencode", True, False))
        for agent in ["kimi", "opencode"]:
            self.assertTrue(credential_mode_allowed(agent, False, True))
            self.assertFalse(credential_mode_allowed(agent, False, False))
            self.assertFalse(credential_mode_allowed(agent, True, True))
        self.assertFalse(credential_mode_allowed("kimi", True, False))

    def test_official_deepseek_is_separate_from_free_and_requires_credentials(self):
        self.assertTrue(
            opencode_model_allowed("deepseek/deepseek-flash", "deepseek", False)
        )
        self.assertFalse(
            opencode_model_allowed("deepseek/deepseek-flash", "deepseek", True)
        )
        self.assertFalse(
            opencode_model_allowed(
                "opencode/deepseek-v4.1-flash-free", "deepseek", False
            )
        )
        self.assertFalse(
            opencode_model_allowed("deepseek/deepseek-flash", "free", False)
        )
        self.assertFalse(
            opencode_model_allowed("deepseek/deepseek-v4-pro", "deepseek", False)
        )
