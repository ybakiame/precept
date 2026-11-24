+++
key = "retry-memory-discipline"
title = "Repeated failure memory discipline"
enforcement = "advisory"
tags = ["memory", "debugging"]
+++
# Repeated failure memory discipline

After two materially identical failed attempts in one work session, stop retrying variations. Record one concise `attempt` note with a stable title, the symptom, attempted actions, observed outcomes, and the next hypothesis. Do not create a new note for every retry.

After a verified fix, update the same note or record a `lesson` that states the working approach and the evidence that verified it. Keep the note bounded and useful to a future agent.

Never include secrets, credentials, raw conversations, private paths, preference values, or Handoff bodies. Use project memory only where the operator allows Agent writes. If memory writes are unavailable, record the blocker in the work session and tell the operator instead of retrying the write.
