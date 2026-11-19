#!/bin/sh
set -eu

recipe=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
revision=7cb4af8352d93096925db2fc30b4747dca78c8a3
checksum=bff34a7f10209fca458d8b065a3c1f7c781ca305e140d532c7cde39efa9c4689
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

curl -fsSL "https://codeload.github.com/Yuna404/LinRgb/tar.gz/$revision" -o "$work/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work/source.tar.gz" | sha256sum --check --status
tar -xzf "$work/source.tar.gz" -C "$work"
cp -R "$recipe/debian" "$work/LinRgb-$revision/"
chmod +x "$work/LinRgb-$revision/debian/rules"
(cd "$work/LinRgb-$revision" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_1.0.0~beta.1-4_amd64.deb" .
