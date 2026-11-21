#!/bin/sh
set -eu
recipe=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
version=${LINRGB_VERSION:-1.0.0-beta.4}
checksum=${LINRGB_SHA256:-9a8d682f4980c422603cd28a74aeeff68bb39cb0de21a2bf834e8e8d382cb72f}
native=${LINRGB_NATIVE_VERSION:-1.0.0~beta.4}
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
if [ -n "${LINRGB_NATIVE_VERSION:-}" ] &&
   [ "$(dpkg-parsechangelog -l "$work/linrgb-$version/debian/changelog" -S Version)" != "$native-1" ]; then
  changelog="$work/linrgb-$version/debian/changelog"
  {
    printf 'linrgb (%s-1) unstable; urgency=medium\n\n  * Package LinRGB %s.\n\n' "$native" "$version"
    printf ' -- LinRGB maintainers <Yuna404@users.noreply.github.com>  %s\n\n' "$(LC_ALL=C date -Ru -d "@${SOURCE_DATE_EPOCH:?}")"
    cat "$changelog"
  } > "$changelog.new"
  mv "$changelog.new" "$changelog"
fi
chmod +x "$work/linrgb-$version/debian/rules"
(cd "$work/linrgb-$version" && dpkg-buildpackage -b -us -uc)
cp -- "$work/linrgb_${native}-1_amd64.deb" .
