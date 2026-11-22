#!/usr/bin/env bash
# One command for a private dogfood round from WSL2 Arch: gates, the Arch
# package, then the Windows Desktop built from a Windows clone at the same
# commit and deployed into %LOCALAPPDATA%\Programs\Precept.
#
#   tools/dogfood/ship.sh <attempt-name> --windows-clone <wsl path> \
#       --distribution <wsl-distro> [--skip-gates] [--no-windows]
#
# The Windows clone (a checkout on the Windows filesystem whose `origin` is
# this repository) is fast-forwarded to HEAD; the build runs in a temporary
# worktree there and the worktree is removed afterwards. Installing the Arch
# package stays an explicit `sudo pacman -U` by the owner (printed at the end).
# The Desktop must be closed for the Windows deploy. Output and logs go to
# .tmp/artifacts/ship/<attempt-name>/ (ignored; never commit or upload).
set -euo pipefail
die() { echo "ship: $*" >&2; exit 1; }

here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
name=${1:-}
[[ $name =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,47}$ ]] || die "usage: $0 <attempt-name> --windows-clone DIR --distribution NAME [--skip-gates] [--no-windows]"
shift
clone="" distribution="" gates=1 windows=1
while (($#)); do
  case $1 in
    --windows-clone) clone=${2:-}; shift 2 ;;
    --distribution) distribution=${2:-}; shift 2 ;;
    --skip-gates) gates=0; shift ;;
    --no-windows) windows=0; shift ;;
    *) die "unknown argument: $1" ;;
  esac
done
if ((windows)); then
  [[ -n $clone && -d $clone/.git ]] || die "--windows-clone must be a Windows-side git checkout"
  [[ $distribution =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]] || die "--distribution required"
fi
[[ -z $(git -C "$repo" status --porcelain --untracked-files=all) ]] || die "clean committed source required"

sha=$(git -C "$repo" rev-parse HEAD)
short=${sha:0:12}
branch=$(git -C "$repo" rev-parse --abbrev-ref HEAD)
out="$repo/.tmp/artifacts/ship/$name"
[[ ! -e $out ]] || die "attempt $name already exists; use a new name"
mkdir -p -m 700 "$out"
step() { echo "ship: $*"; }

if ((gates)); then
  step "gates (log: $out/gates.log)"
  (
    cd "$repo"
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --all-targets --locked
    cd apps
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked --features precept-ui/native -- -D warnings
    cargo test --workspace --all-targets --locked --features precept-ui/native
    cd ..
    node tools/check_source.mjs
  ) >"$out/gates.log" 2>&1 || die "gates failed; see $out/gates.log"
fi

step "Arch package"
"$here/arch-package.sh" "ship-$name" >"$out/arch.log" 2>&1 || die "Arch package failed; see $out/arch.log"
pkg=$(ls "$repo/.tmp/artifacts/local-dogfood/ship-$name/"*.pkg.tar.zst)

windows_status="NOT_RUN"
if ((windows)); then
  step "Windows clone → $short"
  git -C "$clone" fetch -q origin "$branch"
  git -C "$clone" merge -q --ff-only "origin/$branch"
  [[ $(git -C "$clone" rev-parse HEAD) == "$sha" ]] || die "Windows clone is not at $short"
  build="$clone/.tmp/build/ship-$short"
  (cd "$clone" && git.exe worktree add --detach "$(wslpath -w "$build")" "$sha" >/dev/null)
  cleanup() { (cd "$clone" && git.exe worktree remove --force "$(wslpath -w "$build")" >/dev/null 2>&1) || true; }
  trap cleanup EXIT
  artifacts="$clone/.tmp/artifacts/ship"
  mkdir -p "$artifacts"
  step "Windows package"
  (cd "$build" && pwsh.exe -NoProfile -File 'tools\portable\windows-package.ps1' \
    -OutputParent "$(wslpath -w "$artifacts")" -Name "$name") >"$out/windows-package.log" 2>&1 \
    || die "Windows package failed; see $out/windows-package.log"
  localappdata=$(wslpath -u "$(cmd.exe /c 'echo %LOCALAPPDATA%' 2>/dev/null | tr -d '\r')")
  replace=()
  [[ -d $localappdata/Programs/Precept ]] && replace=(-Replace)
  step "Windows deploy"
  if (cd "$build" && pwsh.exe -NoProfile -File 'tools\dogfood\windows-deploy.ps1' \
    -AttemptDir "$(wslpath -w "$artifacts/$name")" -Distribution "$distribution" -User "$(id -un)" \
    "${replace[@]}") >"$out/windows-deploy.log" 2>&1; then
    windows_status="DEPLOYED"
  else
    windows_status="FAILED"
  fi
fi

cat >"$out/result.json" <<JSON
{"source_sha":"$sha","arch_package":"$pkg","windows":"$windows_status"}
JSON
step "done: $out/result.json"
echo "Install the daemon side explicitly:"
echo "  sudo pacman -U $pkg && systemctl --user daemon-reload && systemctl --user restart preceptd"
[[ $windows_status != FAILED ]] || die "Windows deploy failed; see $out/windows-deploy.log (is the Desktop closed?)"
