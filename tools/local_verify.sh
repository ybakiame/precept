#!/usr/bin/env bash
# Native Cargo gates, synthetic process tests only. No Profile/model bootstrap.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
suite=${1:-all}
case "$suite" in core|ui|all) ;; *) exit 2 ;; esac
if [[ $# -ne 3 ]]; then echo 'Usage: tools/local_verify.sh core|ui|all NEW_REPORT_DIR FULL_BASE_COMMIT' >&2; exit 2; fi
if [[ -n $(git status --porcelain) ]]; then echo 'Refuse: commit tracked/source changes before exact-SHA verification' >&2; exit 2; fi
baseline=$3
if [[ ! $baseline =~ ^[0-9a-fA-F]{40}$ ]] || [[ $(git cat-file -t "$baseline" 2>/dev/null) != commit ]]; then echo 'Refuse: baseline must be one existing full commit SHA' >&2; exit 2; fi
baseline=$(git rev-parse --verify "$baseline")
if [[ $baseline == $(git rev-parse HEAD) ]] || ! git merge-base --is-ancestor "$baseline" HEAD; then echo 'Refuse: baseline must be an ancestor with a nonempty branch delta' >&2; exit 2; fi
if git diff --quiet "$baseline" HEAD; then
  echo 'Refuse: baseline must have a nonempty branch delta' >&2; exit 2
else
  delta_status=$?
  if [[ $delta_status -ne 1 ]]; then echo 'Refuse: cannot inspect branch delta' >&2; exit 2; fi
fi
report=$2
mkdir -m 700 -- "$report"
report=$(realpath -- "$report")
{
  git rev-parse HEAD
  printf 'diff_base=%s\n' "$baseline"
  uname -srm
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
run_gate branch-diff git diff --check "$baseline" HEAD
rustc --version >> "$report/source.txt"
cargo --version >> "$report/source.txt"
if [[ $suite != ui ]]; then
  run_gate workspace-fmt cargo fmt --all -- --check
  run_gate workspace-build cargo build --workspace --bins --locked
  run_gate workspace-clippy cargo clippy --workspace --all-targets --locked -- -D warnings
  run_gate workspace-tests cargo test --workspace --all-targets --locked
else
  run_gate process-binaries cargo build --workspace --bins --locked
fi
if [[ $suite != core ]]; then
  run_gate apps-fmt cargo fmt --manifest-path apps/Cargo.toml --all -- --check
  run_gate apps-build cargo build --manifest-path apps/Cargo.toml --workspace --features precept-ui/native --bins --locked
  run_gate apps-clippy cargo clippy --manifest-path apps/Cargo.toml --workspace --all-targets --features precept-ui/native --locked -- -D warnings
  run_gate apps-tests cargo test --manifest-path apps/Cargo.toml --workspace --all-targets --features precept-ui/native --locked
fi
run_gate diff git diff --check
# No native window/PTY/model/hosted CI claim is inferred from these gates.
