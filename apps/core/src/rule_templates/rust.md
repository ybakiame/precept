+++
key = "rust"
title = "Rust changes"
tags = ["rust", "review"]
enforcement = "advisory"
priority = 0
+++
# Rust changes

Keep Rust changes focused and preserve the crate's existing ownership boundaries.

- Handle errors explicitly; do not hide failures behind fallbacks.
- Add a regression test when behavior or validation changes.
- Run formatting and strict Clippy for the affected workspace before review.
