{
  description = "LinRgb package and development environment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/1a08097fc1ed85575db39763b2c4a5d8970a37ba";
  outputs = { self, nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      packages.${system}.default = pkgs.callPackage ./nix/package.nix {};
      checks.${system}.package = self.packages.${system}.default;
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          system = pkgs.stdenv.hostPlatform.system;
          package = self.packages.${system}.default;
        in {
          options.programs.linrgb.enable = lib.mkEnableOption "LinRgb and its device access rules";
          config = lib.mkIf config.programs.linrgb.enable {
            environment.systemPackages = [ package ];
            services.udev.packages = [ package ];
            boot.kernelModules = [ "i2c-dev" ];
          };
        };
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [ cargo rustc rustfmt clippy pkg-config ];
        buildInputs = [ pkgs.gtk4 ];
        shellHook = ''
          # Cairo avoids loading the GPU rendering stack for the mostly static interface.
          export GSK_RENDERER="''${GSK_RENDERER:-cairo}"
          export XDG_DATA_DIRS="${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
        '';
      };
    };
}
