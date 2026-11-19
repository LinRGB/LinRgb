{
  lib,
  rustPlatform,
  pkg-config,
  wrapGAppsHook4,
  gtk4,
  udev,
}:
rustPlatform.buildRustPackage {
  pname = "linrgb";
  version = (lib.importTOML ../Cargo.toml).package.version;
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../packaging
      ../data/openrgb-pci.tsv
      ../design/theme.example.json
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;
  nativeBuildInputs = [ pkg-config wrapGAppsHook4 ];
  buildInputs = [ gtk4 udev ];
  preFixup = ''
    gappsWrapperArgs+=(--set-default GSK_RENDERER cairo)
  '';
  postInstall = ''
    install -Dm644 packaging/io.github.linrgb.LinRgb.desktop $out/share/applications/io.github.linrgb.LinRgb.desktop
    install -Dm644 packaging/io.github.linrgb.LinRgb.svg $out/share/icons/hicolor/scalable/apps/io.github.linrgb.LinRgb.svg
    install -Dm644 packaging/70-linrgb.rules $out/lib/udev/rules.d/70-linrgb.rules
  '';
  meta = {
    description = "Native RGB lighting control for Linux";
    mainProgram = "linrgb";
    platforms = [ "x86_64-linux" ];
  };
}
