#!/usr/bin/env bash
# Starts a Precept release; the hosted Release workflow builds it:
#
#   tools/release.sh <version>            check, fast-forward main, push the tag
#   tools/release.sh <version> --publish  publish the accepted draft Release
#
# Starting: HEAD must be clean, on `dev` and pushed; `main` must be its
# ancestor; the version must be Cargo.toml's and have a CHANGELOG section; the
# Gates workflow must have passed for HEAD. `main` is then fast-forwarded to
# HEAD and the annotated tag v<version> is pushed, which runs the Release
# workflow: gates, the musl Linux archive and the Windows ZIP, one signed
# update manifest for both, and a draft GitHub Release. Nothing is built or
# signed on this machine, and no key is read here.
#
# Publishing: after accepting the draft's assets (for example in Windows
# Sandbox), --publish makes it the latest Release, which Desktop updates and
# the WSL installer then offer.
set -euo pipefail
die() { echo "release: $*" >&2; exit 1; }
step() { echo "release: $*"; }

repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
version=${1:-}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "usage: $0 <version> [--publish]"
shift
publish=0
while (($#)); do
  case $1 in
    --publish) publish=1; shift ;;
    *) die "unknown argument: $1" ;;
  esac
done
tag="v$version"
cd "$repo"
command -v gh >/dev/null || die "the GitHub CLI (gh) is required"

if ((publish)); then
  [[ $(gh release view "$tag" --json isDraft --jq .isDraft) == true ]] || die "$tag is not a draft Release"
  gh release edit "$tag" --draft=false --latest
  step "published: $(gh release view "$tag" --json url --jq .url)"
  exit 0
fi

[[ -z $(git status --porcelain --untracked-files=all) ]] || die "clean committed source required"
[[ $(git rev-parse --abbrev-ref HEAD) == dev ]] || die "release from dev"
cargo_version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"$/\1/p' Cargo.toml)
[[ $cargo_version == "$version" ]] || die "Cargo.toml says $cargo_version"
grep -q "^## $version\$" CHANGELOG.md || die "CHANGELOG.md has no '## $version' section"
sha=$(git rev-parse HEAD)
git fetch -q origin main dev
[[ $(git rev-parse origin/dev) == "$sha" ]] || die "push dev first"
git merge-base --is-ancestor origin/main HEAD || die "origin/main is not an ancestor of HEAD"
! git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null || die "$tag already exists on origin"
gates=$(gh run list --workflow pr.yml --commit "$sha" --event push --json conclusion --jq '.[0].conclusion // ""')
[[ $gates == success ]] || die "the Gates workflow has not passed for ${sha:0:12} (state: ${gates:-none})"

git rev-parse -q --verify "refs/tags/$tag" >/dev/null || git tag -a "$tag" -m "Precept $version" "$sha"
[[ $(git rev-parse "$tag^{commit}") == "$sha" ]] || die "local $tag is at another commit"
git push -q origin "$sha:refs/heads/main"
git push -q origin "refs/tags/$tag"
step "pushed main and $tag at ${sha:0:12}; the Release workflow builds the draft:"
step "  gh run watch \$(gh run list --workflow release.yml --commit $sha --json databaseId --jq '.[0].databaseId')"
step "then accept its assets and run: $0 $version --publish"
