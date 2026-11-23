#!/usr/bin/env bash
# Builds and publishes a Precept release from this machine (WSL2 Arch Linux with
# a Windows clone), without hosted CI:
#
#   tools/release.sh <version> --windows-clone <wsl path> [--skip-gates] [--publish]
#
# 1. HEAD must be clean and on `dev`, `main` must be its ancestor, and the
#    version must be the one in Cargo.toml. An annotated tag v<version> is made
#    at HEAD when absent (local only until --publish).
# 2. The gates of both workspaces and the source check (unless --skip-gates).
# 3. The Arch package (`arch-package.sh --release`) and the Windows ZIP
#    (`windows-package.ps1 -Release`, built in a temporary worktree of the
#    Windows clone at the same commit), then SHA256SUMS, into
#    .tmp/artifacts/release/v<version>/.
# 4. With --publish only: `main` is fast-forwarded to HEAD, the tag is pushed
#    and a GitHub Release is created with the assets and the version's
#    CHANGELOG section as notes. Without it nothing leaves this machine.
set -euo pipefail
die() { echo "release: $*" >&2; exit 1; }
step() { echo "release: $*"; }

repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
version=${1:-}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "usage: $0 <version> --windows-clone DIR [--skip-gates] [--publish]"
shift
clone="" gates=1 publish=0
while (($#)); do
  case $1 in
    --windows-clone) clone=${2:-}; shift 2 ;;
    --skip-gates) gates=0; shift ;;
    --publish) publish=1; shift ;;
    *) die "unknown argument: $1" ;;
  esac
done
[[ -n $clone && -d $clone/.git ]] || die "--windows-clone must be a Windows-side git checkout"
cd "$repo"
[[ -z $(git status --porcelain --untracked-files=all) ]] || die "clean committed source required"
[[ $(git rev-parse --abbrev-ref HEAD) == dev ]] || die "release from dev"
cargo_version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml)
[[ $cargo_version == "$version" ]] || die "Cargo.toml says $cargo_version"
grep -q "^## $version\$" CHANGELOG.md || die "CHANGELOG.md has no '## $version' section"
sha=$(git rev-parse HEAD)
tag="v$version"
git fetch -q origin main dev
git merge-base --is-ancestor origin/main HEAD || die "origin/main is not an ancestor of HEAD"
[[ $(git rev-parse origin/dev) == "$sha" ]] || die "push dev first (the Windows clone builds the pushed commit)"
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  [[ $(git rev-parse "$tag^{commit}") == "$sha" ]] || die "$tag exists at another commit"
else
  git tag -a "$tag" -m "Precept $version" "$sha"
  step "tagged $tag at ${sha:0:12} (local)"
fi
out="$repo/.tmp/artifacts/release/$tag"
[[ ! -e $out ]] || die "$out exists; remove it to rebuild"
mkdir -p -m 700 "$out"

if ((gates)); then
  step "gates (log: $out/gates.log)"
  (
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
tools/dogfood/arch-package.sh "release-$tag" --release >"$out/arch.log" 2>&1 \
  || die "Arch package failed; see $out/arch.log"
cp "$repo/.tmp/artifacts/local-dogfood/release-$tag/precept-$version-1-x86_64.pkg.tar.zst" "$out/"

step "Windows ZIP (clone → ${sha:0:12})"
git -C "$clone" fetch -q origin dev
build="$clone/.tmp/build/release-$tag"
(cd "$clone" && git.exe worktree add --detach "$(wslpath -w "$build")" "$sha" >/dev/null)
cleanup() { (cd "$clone" && git.exe worktree remove --force "$(wslpath -w "$build")" >/dev/null 2>&1) || true; }
trap cleanup EXIT
artifacts="$clone/.tmp/artifacts/release"
mkdir -p "$artifacts"
[[ ! -e $artifacts/$tag ]] || die "$artifacts/$tag exists; remove it to rebuild"
(cd "$build" && pwsh.exe -NoProfile -File 'tools\portable\windows-package.ps1' \
  -OutputParent "$(wslpath -w "$artifacts")" -Name "$tag" -Release "$version") >"$out/windows.log" 2>&1 \
  || die "Windows package failed; see $out/windows.log"
cp "$artifacts/$tag/precept-$version-windows-x64.zip" "$out/"

(cd "$out" && sha256sum precept-* >SHA256SUMS)
awk -v v="## $version" '$0==v{on=1;next} on&&/^## /{exit} on' CHANGELOG.md >"$out/notes.md"
step "assets in $out:"
(cd "$out" && ls -l precept-* SHA256SUMS)

if ((publish)); then
  step "publishing $tag"
  git push -q origin "$sha:refs/heads/main"
  git push -q origin "refs/tags/$tag"
  gh release create "$tag" --verify-tag --title "Precept $version" --notes-file "$out/notes.md" \
    "$out"/precept-*.zip "$out"/precept-*.pkg.tar.zst "$out/SHA256SUMS"
  step "published: $(gh release view "$tag" --json url --jq .url)"
else
  step "not published (add --publish to push main, the tag and the Release)"
fi
