#!/usr/bin/env bash
# Builds a local Arch Linux package (pacman) of the ACP daemon side for private
# dogfooding: acp-daemon, acp-query, acp-mcp, acp-wsl-bridge, acp-nsh-tui and the
# acp-daemon systemd user unit. Does not install anything; installing is an explicit
# `sudo pacman -U` by the owner.
#
#   tools/dogfood/arch-package.sh <new-attempt-name>
#
# Output: .tmp/artifacts/local-dogfood/<name>/ (ignored; never commit or upload).
set -euo pipefail
die() { echo "arch-package: $*" >&2; exit 1; }

here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
name=${1:-}
[[ $name =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]] || die "usage: $0 <new-attempt-name>"
[[ -z $(git -C "$repo" status --porcelain --untracked-files=all) ]] || die "clean committed source required"
[[ $(uname -m) == x86_64 && -f /etc/arch-release ]] || die "Arch Linux x86_64 builder required"

sha=$(git -C "$repo" rev-parse HEAD)
short=${sha:0:12}
pkgver="0.0.0.r$(git -C "$repo" rev-list --count HEAD).g${short}"
parent="$repo/.tmp/artifacts/local-dogfood"
mkdir -p -m 700 "$parent"
out="$parent/$name"
mkdir -m 700 "$out" || die "attempt $name already exists; use a new name"

git -C "$repo" archive --format=tar --prefix="acp-$short/" -o "$out/acp-$short.tar" "$sha"
cp "$here/acp-daemon.service" "$here/README.md" "$out/"
sums=""
for f in "acp-$short.tar" acp-daemon.service README.md; do
  sums+="'$(sha256sum "$out/$f" | cut -d' ' -f1)' "
done
# Runtime dependencies, verified against the built binaries below.
depends="glibc libgcc"
sed -e "s/@PKGVER@/$pkgver/" -e "s/@SHORT@/$short/g" -e "s/@SUMS@/${sums% }/" -e "s/@DEPENDS@/$depends/" \
  "$here/arch/PKGBUILD.in" > "$out/PKGBUILD"

(cd "$out" && BUILDDIR="$out/build" PKGDEST="$out" SRCDEST="$out" LOGDEST="$out" \
  makepkg --force --nocolor --log >/dev/null) || die "makepkg failed; see the private logs in $out"
pkg=$(ls "$out"/agent-control-plane-"$pkgver"-1-x86_64.pkg.tar.* 2>/dev/null | head -1)
[[ -n $pkg ]] || die "package file missing"

# Verify: declared dependencies cover every direct DT_NEEDED owner, and no builder path leaked.
verify="$out/verify"
mkdir -m 700 "$verify"
bsdtar -xf "$pkg" -C "$verify"
owners=$(for b in "$verify"/usr/bin/*; do
  ldd "$b" | awk '/=> \//{print $3}'
done | sort -u | xargs -r pacman -Qoq | sort -u)
for o in $owners; do
  [[ " $depends " == *" $o "* ]] || die "undeclared runtime dependency: $o"
done
for b in "$verify"/usr/bin/*; do
  for p in "$repo" "$HOME" "$out"; do
    ! grep -qaF "$p" "$b" || die "builder path bytes in $(basename "$b")"
  done
done

{
  echo "{"
  echo "  \"package\": \"$(basename "$pkg")\","
  echo "  \"sha256\": \"$(sha256sum "$pkg" | cut -d' ' -f1)\","
  echo "  \"source_sha\": \"$sha\","
  echo "  \"pkgver\": \"$pkgver\","
  echo "  \"depends\": \"$depends\","
  echo "  \"observed_owners\": \"$(echo $owners)\","
  echo "  \"builder_path_bytes\": false"
  echo "}"
} > "$out/package-result.json"
cat "$out/package-result.json"
echo "Install explicitly: sudo pacman -U $pkg"
