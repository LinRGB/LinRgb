#!/usr/bin/env bash
# Recreate the native source asset from a Git revision, not the working directory.
set -euo pipefail
rev=${1:?usage: release-source.sh REV OUTPUT.tar.gz}
out=${2:?missing output path}
version=$(git show "$rev:Cargo.toml" | awk -F '"' '/^version = / { print $2; exit }')
test -n "$version"
# Exclude the distro recipe wrappers: they pin the hash of this very archive.
# All build inputs, the locked deps and shared install files come from the named tree.
git archive --format=tar --mtime=1970-01-01T00:00:00Z --prefix="linrgb-$version/" "$rev^{tree}" \
  LICENSE Cargo.toml Cargo.lock src data design/theme.example.json packaging/70-linrgb.rules \
  packaging/io.github.linrgb.LinRgb.desktop packaging/io.github.linrgb.LinRgb.svg notices | gzip -n > "$out"
