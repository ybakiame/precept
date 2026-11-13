#!/usr/bin/env bash
# Native Cargo gates, synthetic process tests only. No Profile/model bootstrap.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
suite=${1:-all}
case "$suite" in core|ui|all) ;; *) echo 'Usage: tools/local_verify.sh [core|ui|all] [NEW_REPORT_DIR]' >&2; exit 2 ;; esac
if [[ $# -gt 2 ]]; then exit 2; fi
if [[ -n $(git status --porcelain) ]]; then echo 'Refuse: commit tracked/source changes before exact-SHA verification' >&2; exit 2; fi
report=${2:-artifacts/local-p1-m2/s4-linux}
mkdir -- "$report"
report=$(realpath -- "$report")
{
  git rev-parse HEAD
  uname -srm
  rustc --version
  cargo --version
} > "$report/source.txt"
run_gate() {
  local name=$1
  shift
  printf '%q ' "$@" >> "$report/commands.txt"
  printf '\n' >> "$report/commands.txt"
  if "$@" > "$report/$name.log" 2>&1; then
    printf '%s\tPASS\n' "$name" | tee -a "$report/result.tsv"
  else
    local result=$?
    printf '%s\tFAIL\t%s\n' "$name" "$result" | tee -a "$report/result.tsv"
    exit "$result"
  fi
}
if [[ $suite != ui ]]; then
  run_gate workspace-fmt cargo fmt --all -- --check
  run_gate workspace-build cargo build --workspace --bins --locked
  run_gate workspace-clippy cargo clippy --workspace --all-targets --locked -- -D warnings
  run_gate workspace-tests cargo test --workspace --all-targets --locked
else
  run_gate process-binaries cargo build --workspace --bins --locked
fi
if [[ $suite != core ]]; then
  run_gate shared-fmt cargo fmt --manifest-path spikes/p1-e/shared/Cargo.toml -- --check
  run_gate shared-clippy cargo clippy --manifest-path spikes/p1-e/shared/Cargo.toml --all-targets --features native --locked -- -D warnings
  run_gate shared-tests cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked
  run_gate nsh-fmt cargo fmt --manifest-path spikes/p1-e/nsh/Cargo.toml -- --check
  run_gate nsh-build cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --features native,tui --bins --locked
  run_gate nsh-clippy cargo clippy --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked -- -D warnings
  run_gate nsh-tests cargo test --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked
fi
run_gate diff git diff --check
# No native window/PTY/model/hosted CI claim is inferred from these gates.
