#!/bin/sh
set -eu

recipe=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
revision=d61b8963050aea6d8aa5128b396d6260d6360cfb
checksum=82c80a1545263e8a63ae9a9dc5c087e9dc24eba001a2f3598dbd3617b6ed475f
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

curl -fsSL "https://codeload.github.com/Yuna404/LinRgb/tar.gz/$revision" -o "$work/source.tar.gz"
printf '%s  %s\n' "$checksum" "$work/source.tar.gz" | sha256sum --check --status
tar -xzf "$work/source.tar.gz" -C "$work"
cp -R "$recipe/debian" "$work/LinRgb-$revision/"
chmod +x "$work/LinRgb-$revision/debian/rules"
(cd "$work/LinRgb-$revision" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_1.0.0~beta.1-1_amd64.deb" .
