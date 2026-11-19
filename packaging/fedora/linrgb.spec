%global upstream_version 1.0.0-beta.4

Name:           linrgb
Version:        1.0.0~beta.4
Release:        1%{?dist}
Summary:        Native RGB lighting control for Linux
# Upstream has not declared a license; this spec is for local builds.
License:        LicenseRef-NotDeclared
URL:            https://github.com/LinRGB/LinRgb
Source0:        %{url}/releases/download/v%{upstream_version}/linrgb-%{upstream_version}.tar.gz
ExclusiveArch:  x86_64
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libudev)

%description
LinRgb controls supported USB HID and I2C lighting controllers through a GTK4 desktop interface.

%prep
printf '9a8d682f4980c422603cd28a74aeeff68bb39cb0de21a2bf834e8e8d382cb72f  %%s\n' '%{SOURCE0}' | sha256sum --check
%autosetup -n linrgb-%{upstream_version}

%build
cargo fetch --locked --target x86_64-unknown-linux-gnu
cargo build --frozen --release --target x86_64-unknown-linux-gnu

%check
cargo test --frozen --release --target x86_64-unknown-linux-gnu

%install
install -Dm755 target/x86_64-unknown-linux-gnu/release/linrgb %{buildroot}%{_bindir}/linrgb
install -Dm644 packaging/io.github.linrgb.LinRgb.desktop -t %{buildroot}%{_datadir}/applications
install -Dm644 packaging/io.github.linrgb.LinRgb.svg -t %{buildroot}%{_datadir}/icons/hicolor/scalable/apps
install -Dm644 packaging/70-linrgb.rules -t %{buildroot}%{_prefix}/lib/udev/rules.d
install -d %{buildroot}%{_prefix}/lib/modules-load.d
printf 'i2c-dev\n' > %{buildroot}%{_prefix}/lib/modules-load.d/linrgb.conf

%files
%{_bindir}/linrgb
%{_datadir}/applications/io.github.linrgb.LinRgb.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.linrgb.LinRgb.svg
%{_prefix}/lib/udev/rules.d/70-linrgb.rules
%{_prefix}/lib/modules-load.d/linrgb.conf

%doc notices/*

%changelog
* Thu Nov 19 2026 LinRGB maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.4-1
- Package the square LinRGB artwork.

* Thu Nov 19 2026 LinRGB maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.3-1
- Restore the native tray context menu.

* Thu Nov 19 2026 LinRGB maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.2-1
- Beta 2: passive inventory and verified hardware sessions.

* Thu Nov 19 2026 LinRgb maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.1-4
- Probe ENE memory before selecting RGB registers.

* Wed Nov 18 2026 LinRgb maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.1-3
- Fix repeated Corsair direct RGB updates.

* Wed Nov 18 2026 LinRgb maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.1-2
- Update to 85db170 with experimental Aura ARGB, Corsair RAM and ENE support.

* Wed Nov 18 2026 LinRgb maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.1-1
- Add local Fedora packaging for the first LinRgb beta.
