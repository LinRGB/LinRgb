#!/bin/sh
set -eu

recipe=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
revision=8811a5d2564d52df75767ea11bd6e3e831923e63
checksum=f272f27e0067f37a973fed9837e776c3aa9183749374a7843f1b36490389fede
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

curl -fsSL "https://codeload.github.com/Yuna404/LinRgb/tar.gz/$revision" -o "$work/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work/source.tar.gz" | sha256sum --check --status
tar -xzf "$work/source.tar.gz" -C "$work"
cp -R "$recipe/debian" "$work/LinRgb-$revision/"
chmod +x "$work/LinRgb-$revision/debian/rules"
(cd "$work/LinRgb-$revision" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_1.0.0~beta.1-3_amd64.deb" .
