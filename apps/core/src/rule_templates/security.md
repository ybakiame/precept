+++
key = "security"
title = "Security boundaries"
tags = ["security", "review"]
enforcement = "advisory"
priority = 0
+++
# Security boundaries

Preserve least authority and keep private data out of diagnostics and logs.

- Validate paths and scope before filesystem or process operations.
- Do not log credentials, personal paths, raw conversations or preference values.
- Treat hooks as cooperative compliance, not as a security boundary.
