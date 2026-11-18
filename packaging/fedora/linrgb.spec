%global commit d61b8963050aea6d8aa5128b396d6260d6360cfb

Name:           linrgb
Version:        1.0.0~beta.1
Release:        1%{?dist}
Summary:        Native RGB lighting control for Linux
# Upstream has not declared a license; this spec is for local builds.
License:        LicenseRef-NotDeclared
URL:            https://github.com/Yuna404/LinRgb
Source0:        https://codeload.github.com/Yuna404/LinRgb/tar.gz/%{commit}#/linrgb-%{commit}.tar.gz
ExclusiveArch:  x86_64
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkgconfig(gtk4)

%description
LinRgb controls supported USB HID and I2C lighting controllers through a GTK4 desktop interface.

%prep
printf '82c80a1545263e8a63ae9a9dc5c087e9dc24eba001a2f3598dbd3617b6ed475f  %%s\n' '%{SOURCE0}' | sha256sum --check
%autosetup -n LinRgb-%{commit}

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

%changelog
* Wed Nov 18 2026 LinRgb maintainers <Yuna404@users.noreply.github.com> - 1.0.0~beta.1-1
- Add local Fedora packaging for the first LinRgb beta.
