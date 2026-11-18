# P1-M5/S1-D — third-party notice ledger review (G11)

2026-11-18. Scope: [Issue #76](https://github.com/ybakiame/agent-control-plane/issues/76),
parent [M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67),
[S0 contract](../p1/release-readiness-plan.md) decision D6. Follows the S1-A
[Windows](p1-m5-s1a-windows-portable.md) and S1-C
[Linux](p1-m5-s1c-wslg-wayland-portable.md) reports, whose ledgers left 21 / 18
third-party entries without notice text and labeled every third-party entry
`PENDING`. Those reports keep their original verdicts.

**Result: every third-party package in both shipped closures is reviewed: 419/419
Windows, 448/448 Linux, 0 pending.** Both recipes now refuse packaging instead of
shipping an unresolved entry. This is a notice ledger for the Cargo
normal-dependency closure, **not** a linker-derived SBOM. It is not legal advice,
not a publication or signing decision, and it does not cover OS-owned libraries;
those are not shipped. G11 remains BLOCKED for public distribution only, because
version/tag semantics and signing are undecided.

## Method

One shared module, [`tools/portable/notices.mjs`](../../tools/portable/notices.mjs),
replaces the duplicated Windows/Linux loops.

1. **License policy.** Each package's declared expression is parsed:
   - SPDX `AND`/`OR`/`WITH` and parentheses; legacy `/` is read as `OR`.
   - It must be satisfiable using only these permissive licenses: MIT, MIT-0,
     Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0,
     Unicode-DFS-2016, CC0-1.0, Unlicense, 0BSD, BSL-1.0, CDLA-Permissive-2.0.
   - The only `WITH` exception allowed is `LLVM-exception`, and only on Apache-2.0.
     Any other or mispaired exception refuses the whole expression, even when
     another `OR` branch would be permissive.
   - Unknown, unparsable or non-permissive expressions refuse.
2. **Notice source, in order:**
   1. Crate-local license texts (`LICENSE*`, `COPYING*`, `UNLICENSE*`). Attribution
      `NOTICE*` files ship alongside them but never resolve a package on their own.
      An empty or whitespace-only candidate refuses packaging.
   2. For Cargo git dependencies, the nearest license texts up to the pinned
      checkout root, under the same rules.
   3. A reviewed supplement for that exact package and version
      ([`notice-supplements/supplements.json`](../../tools/portable/notice-supplements/supplements.json)).
      Each text in it is:
      - byte-exact from a GitHub URL pinned to a full commit SHA
      - stored as `<sha256>.txt` and re-hashed at packaging time
      - a license text unless marked `notice`; each supplement needs at least one
        license text. An entry must have either a non-empty text list or an
        explicit no-text license, never neither or both.
3. **Record.** Each ledger entry records its notice source and a `REVIEWED` line.
   Texts are deduplicated as `notices/<sha256>.txt`. Each text also records:
   - its kind (`license` or `notice`)
   - for crate-local and checkout texts, the upstream file path; git packages also
     record their pinned commit (`git_commit`)
   - for supplements, the exact upstream URL, repository, full commit and path

   So the shipped `dependency-notices.json` is verifiable without `supplements.json`.

### Entries without crate-local text

| Package(s) | Version | Declared | Resolution |
| --- | --- | --- | --- |
| `nsh-base`, `nsh-core`, `nsh-engine`, `nsh-macros`, `nsh-platform-tui`, `nsh-updater`, plus `nsh-platform-windows` (Windows) / `nsh-platform-linux` (Linux) | 0.5.0 | Apache-2.0 | `LICENSE-APACHE` at the pinned NSH checkout root `b7f31a08…` (no upstream NOTICE file) |
| `ratex-font`, `ratex-layout`, `ratex-lexer`, `ratex-parser`, `ratex-types` | 0.1.15 | MIT | RaTeX `LICENSE` at tag `v0.1.15` (`5f7dc488…`). `ratex-font` embeds KaTeX-derived font metrics and symbol tables, so it also ships the KaTeX font notice and SIL OFL 1.1 from the same commit (conservative) |
| `rust-i18n-support`, `rust-i18n-macro` | 4.2.4 | MIT | `LICENSE` at tag `v4.2.4` (`5f3ea5ba…`) |
| `taffy` | 0.15.1 | MIT | `LICENSE` at tag `v0.15.1` (`0064bd54…`) |
| `wuff` | 0.2.10 | MIT | `LICENSE` at tag `v0.2.10` (`0bb77cb3…`) |
| `webview2-com`, `webview2-com-sys` (0.39.1), `webview2-com-macros` (0.8.1); Windows only | — | MIT | Upstream `LICENSE`, single revision `f55627c8…` since 2021; no per-version tag exists |
| `mac` | 0.1.1 | MIT/Apache-2.0 | Upstream ships no license file and no copyright line. **Apache-2.0 is elected**; its verbatim text is the hash-checked repository `LICENSE`. No copyright line is invented |
| `hexf-parse` | 0.2.1 | CC0-1.0 | Public-domain dedication, so no notice obligation. Upstream added a license file only for later releases. Recorded as no-notice-required; no text is invented |

All other third-party entries ship crate-local texts. Every declared expression in
both closures satisfies the policy. No copyleft license (GPL/LGPL/MPL) appears, and
`CDLA-Permissive-2.0` and `BSL-1.0` texts ship crate-locally.

## Results (clean source `c44f2f6`, after review fixes)

| Recipe | Third-party | Reviewed | Pending | By notice source | Archive |
| --- | ---: | ---: | ---: | --- | --- |
| Windows ZIP (`s1d-windows-02`, native Windows host) | 419 | 419 | 0 | crate-local 398; checkout root 7; pinned upstream 12; election 1; no-notice 1 | 264 entries, `e53ae598…0a09`, extracted inventory PASS |
| Linux `.tar.gz` (`s1d-linux-02`, builder `acp-s1b-ubuntu`) | 448 | 448 | 0 | crate-local 430; checkout root 7; pinned upstream 9; election 1; no-notice 1 | 276 entries, `7dbec3f1…8cfd`, extracted inventory and listing policy PASS |

Texts by kind: Windows 745 license / 1 notice, Linux 782 license / 1 notice (the
KaTeX font notice). No real crate in either closure ships a NOTICE-only or empty
license file. Each archive gained one entry over the first S1-D run (`18c8e87`:
263 / 275 entries): the shared `UNLICENSE` text of 7 crates (`aho-corasick`,
`byteorder-lite`, `globset`, `ignore`, `memchr`, `same-file`, `walkdir`). The old
filename pattern missed it, though their `LICENSE-MIT` text already shipped.

Archives stay local in ignored `artifacts/local-p1-m5/`; they are not committed or
uploaded. Product binaries are unchanged by this slice; only `notices/`, the manifest
notice summary and the manifest limitation text change. The clean-target acceptance
in S1-A/S1-C therefore still applies to the binaries, but it was **not** rerun on
these archives.

## Validation

| Check | Platform | Result |
| --- | --- | --- |
| `node --test tools/portable/*.test.mjs` | Linux/WSL | **25 PASS** |
| Same | Windows | **24 PASS, 1 skipped** (the `readelf` comparison is Linux-only) |
| New tests | both | license policy (slash, nesting, `WITH`, refusals); checkout-root detection (POSIX and Windows paths); committed supplements hash-exact and commit-pinned; every notice source; refusal of unresolved texts, another version, non-permissive licenses, and an undeclared no-text license |
| Review regressions | both | unknown/mispaired exception behind a permissive `OR`; NOTICE-only crate and checkout; empty and whitespace-only license or notice; empty, ambiguous and notice-only supplements; non-pinned supplement URL; serialized kinds, upstream paths, git commit and supplement provenance |
| Both recipes end to end | Windows; builder | PASS, 0 pending (above) |

Not run: clean-target re-acceptance of these archives, legal review, signing,
publication.

## Remaining for G11

- Version/tag semantics and the RC identifier (D6/S4).
- Signing and publication decision (D7). Until then, G11 stays BLOCKED for public
  distribution.
