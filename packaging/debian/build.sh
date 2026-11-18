#!/bin/sh
set -eu

recipe=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
revision=85db1703c58563a9f37ebe98a20e14bf999f686d
checksum=79286e7e745dcce45e5a3709abbb9e42cafcd2050e0a2a2525fc7aee448caa92
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

curl -fsSL "https://codeload.github.com/Yuna404/LinRgb/tar.gz/$revision" -o "$work/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work/source.tar.gz" | sha256sum --check --status
tar -xzf "$work/source.tar.gz" -C "$work"
cp -R "$recipe/debian" "$work/LinRgb-$revision/"
chmod +x "$work/LinRgb-$revision/debian/rules"
(cd "$work/LinRgb-$revision" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_1.0.0~beta.1-2_amd64.deb" .
