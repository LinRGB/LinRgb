#!/usr/bin/env bash
# Run in the named distro container with /repo read-only and /out writable.
set -euo pipefail
: "${LINRGB_VERSION:?}" "${LINRGB_SHA256:?}" "${SOURCE_DATE_EPOCH:?}"
archived="linrgb-$LINRGB_VERSION.tar.gz"
source="/input/$archived"
printf '%s  %s\n' "$LINRGB_SHA256" "$source" | sha256sum --check
export LINRGB_NATIVE_VERSION=${LINRGB_VERSION/-/\~}
export LINRGB_LICENSE='MIT AND GPL-2.0-or-later'
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

case "$1" in
  arch) pacman -Syu --noconfirm --needed rust pkgconf gtk4 systemd systemd-libs ;;
  debian)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y --no-install-recommends build-essential ca-certificates debhelper libgtk-4-dev libudev-dev pkgconf rustup udev
    ;;
  fedora) dnf -y install cargo gcc gtk4-devel systemd-devel systemd-udev rpm-build ;;
  *) exit 2 ;;
esac
useradd --create-home builder
cp -R /repo/packaging/. "$work/"
chown -R builder:builder "$work"

case "$1" in
  arch)
    cd "$work/arch"
    install -o builder "$source" "$archived"
    export LINRGB_NATIVE_VERSION=${LINRGB_VERSION/-/} LINRGB_LOCAL_ARCHIVE=$archived
    runuser -u builder -- makepkg --noconfirm
    package="linrgb-$LINRGB_NATIVE_VERSION-1-x86_64.pkg.tar.zst"
    test "$(bsdtar -xOf "$package" .PKGINFO | grep '^pkgver = ')" = "pkgver = $LINRGB_NATIVE_VERSION-1"
    pacman -Up --print-format '%n %v' "$package" | grep -Fx "linrgb $LINRGB_NATIVE_VERSION-1"
    pacman -U --noconfirm "$package"
    output="linrgb-$LINRGB_VERSION-arch-x86_64.pkg.tar.zst"
    ;;
  debian)
    cd "$work/debian"
    runuser -u builder -- rustup toolchain install 1.99.1 --profile minimal
    runuser -u builder -- sh ./build.sh "$source"
    package="linrgb_${LINRGB_NATIVE_VERSION}-1_amd64.deb"
    test "$(dpkg-deb -f "$package" Package Version Architecture)" = "$(printf 'Package: linrgb\nVersion: %s-1\nArchitecture: amd64' "$LINRGB_NATIVE_VERSION")"
    test -n "$(dpkg-deb -f "$package" Depends)"
    apt-get install -y "./$package"
    output="linrgb-$LINRGB_VERSION-debian13-amd64.deb"
    ;;
  fedora)
    cd "$work/fedora"
    install -d -o builder SOURCES
    install -o builder "$source" "SOURCES/$archived"
    runuser -u builder -- rpmbuild -bb linrgb.spec --define "_topdir $PWD" \
      --define "upstream_version $LINRGB_VERSION" --define "native_version $LINRGB_NATIVE_VERSION" \
      --define "source_sha256 $LINRGB_SHA256" --define 'automated_release 1'
    package="RPMS/x86_64/linrgb-$LINRGB_NATIVE_VERSION-1.fc44.x86_64.rpm"
    test "$(rpm -qp --qf '%{NAME} %{VERSION}-%{RELEASE} %{ARCH}' "$package")" = "linrgb $LINRGB_NATIVE_VERSION-1.fc44 x86_64"
    rpm -qpR "$package"
    dnf -y install "./$package"
    output="linrgb-$LINRGB_VERSION-fedora44-x86_64.rpm"
    ;;
esac

test -x /usr/bin/linrgb
ldd /usr/bin/linrgb > "$work/libraries"
cat "$work/libraries"
! grep -F 'not found' "$work/libraries" || exit 1
for path in share/applications/io.github.linrgb.LinRgb.desktop \
    share/icons/hicolor/scalable/apps/io.github.linrgb.LinRgb.svg lib/udev/rules.d/70-linrgb.rules; do
  cmp "/usr/$path" "/repo/packaging/${path##*/}"
done
grep -Fx i2c-dev /usr/lib/modules-load.d/linrgb.conf
udevadm verify /usr/lib/udev/rules.d/70-linrgb.rules
test "$(runuser -u builder -- /usr/bin/linrgb --version)" = "LinRGB $LINRGB_VERSION"
runuser -u builder -- /usr/bin/linrgb --diagnostics
install -m644 "$package" "/out/$output"
