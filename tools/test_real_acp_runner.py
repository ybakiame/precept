"""Content-free smoke guard regression; no installed agent or credential use."""

import unittest
from test_real_acp_smoke import token_received


class RealRunnerTests(unittest.TestCase):
    def test_streamed_synthetic_token_across_chunks_and_no_tool_or_user_echo(self):
        self.assertTrue(
            token_received(
                [
                    "agent_message_chunk: ACP_",
                    "current_mode_update: observed",
                    "agent_message_chunk: SMO",
                    "agent_message_chunk: KE_OK",
                ]
            )
        )
        for events in [
            ["user_message_chunk: ACP_SMOKE_OK"],
            ["tool_call: ACP_SMOKE_OK"],
            ["agent_message_chunk: ACP_", "user_message_chunk: SMOKE_OK"],
            ["agent_message_chunk: ACP_NOT_OK"],
        ]:
            self.assertFalse(token_received(events))


if __name__ == "__main__":
    unittest.main()
