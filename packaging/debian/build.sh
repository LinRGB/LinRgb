#!/bin/sh
set -eu
recipe=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
version=1.0.0-beta.4
checksum=9a8d682f4980c422603cd28a74aeeff68bb39cb0de21a2bf834e8e8d382cb72f
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
if [ "$#" = 1 ]; then
  cp -- "$1" "$work/source.tar.gz"
else
  curl -fsSL "https://github.com/LinRGB/LinRgb/releases/download/v$version/linrgb-$version.tar.gz" -o "$work/source.tar.gz"
fi
printf '%s  %s\n' "$checksum" "$work/source.tar.gz" | sha256sum --check --status
tar -xzf "$work/source.tar.gz" -C "$work"
cp -R "$recipe/debian" "$work/linrgb-$version/"
chmod +x "$work/linrgb-$version/debian/rules"
(cd "$work/linrgb-$version" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_1.0.0~beta.4-1_amd64.deb" .
