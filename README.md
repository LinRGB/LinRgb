<p align="center">
  <img src="ressources/LinRGB-banner.png" alt="LinRgb banner" width="900">
</p>

# LinRgb — Your Linux RGB controller, written in Rust

**AI disclosure:** LinRgb was built with AI assistance under the maintainer’s direction. AI generated much of the code and documentation, and the maintainer tested selected hardware and workflows. A passing software test does not mean an unlisted device is safe or supported.

**Why I built LinRgb:** I couldn't find a Linux RGB app that worked the way I wanted. I didn't have the programming experience to build it on my own, so I used AI to help make the tool I wanted for my own PC. If you'd rather not use software built this way, I understand. There's no pressure to use LinRgb.

LinRgb is a native Rust and GTK4 app for RGB lighting on Linux. NixOS on `x86_64-linux` is the primary platform. OpenRGB and SignalRGB are not required at runtime.

## Try LinRgb without installing it

With Nix Flakes enabled, run:

```bash
nix run github:LinRGB/LinRgb/main
```

This downloads or builds LinRgb from `main` and opens it. It does not add a desktop launcher to your system profile or install udev rules. Close other RGB applications and their background servers before applying any lighting settings. Do not run the GUI as root.

## Install on NixOS (recommended)

This guide assumes you already manage NixOS with a flake. If the `nix` command reports that flakes are disabled, add `nix.settings.experimental-features = [ "nix-command" "flakes" ];` to your current NixOS configuration and rebuild it using your existing setup first.

### 1. Add LinRgb to your flake

Merge these changes into your existing `flake.nix`. Keep your existing `nixpkgs` input and host modules. Replace `my-host` with your entry under `nixosConfigurations`. Keep LinRgb's own pinned `nixpkgs`; don't make it follow your system input.

```nix
{
  inputs.linrgb.url = "github:LinRGB/LinRgb/main";

  outputs = { nixpkgs, linrgb, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      modules = [
        ./configuration.nix
        linrgb.nixosModules.default
      ];
    };
  };
}
```

If your `outputs` already binds `inputs` (for example, `inputs@{ nixpkgs, ... }`), use `inputs.linrgb.nixosModules.default` in the `modules` list instead.

### 2. Enable LinRgb on your host

Add this inside the existing attribute set in `configuration.nix` (or the equivalent host module). No additional `imports` entry is needed.

```nix
programs.linrgb.enable = true;
```

### 3. Rebuild and launch

From the directory containing `flake.nix`, run:

```bash
nix flake lock
sudo nixos-rebuild switch --flake .#my-host
```

Launch **LinRgb** from the application menu or run `linrgb` without `sudo`. Quit any other RGB apps and their background servers before applying lighting. The module installs the launcher and package, loads `i2c-dev`, and registers targeted udev rules for the active local session. No manual rules are needed. I²C permissions apply to each matched adapter as a whole, not just to its RGB controller. If access has not refreshed, reconnect the USB device or reboot.

### Update LinRgb

From the same directory, run `nix flake update linrgb` and repeat the rebuild command. `flake.lock` pins the selected revision of `main`. Quit LinRgb with `Ctrl+Q` and reopen it so you don't keep running the previous version in the tray.

### Package only (Home Manager or custom setup)

In a module with `inputs` and `pkgs` in scope, add this package to `home.packages` or `environment.systemPackages`:

```nix
inputs.linrgb.packages.${pkgs.stdenv.hostPlatform.system}.default
```

Package-only installs and `nix run` do not activate system udev rules. Use the NixOS module for automatic device access. If you manage permissions yourself on NixOS, register the package in `services.udev.packages` and load `i2c-dev` for supported SMBus devices. Do not run LinRgb as root.

## Install on Arch Linux

The [PKGBUILD](packaging/arch/PKGBUILD) builds a local `x86_64` package from a pinned source archive. It is not an AUR package.

```bash
sudo pacman -Syu --needed base-devel git

git clone https://github.com/LinRGB/LinRgb.git
cd LinRgb/packaging/arch
makepkg -si
```

Run `makepkg` as your normal user. It asks for administrator access when installing dependencies and the built package. The package includes the desktop launcher, targeted USB and I²C udev rules, and a configuration that loads `i2c-dev` at boot.

Reboot after the first installation so the module and access rules apply. For immediate use, load `i2c-dev` with `sudo modprobe i2c-dev` and reconnect the supported USB controller. If the existing I²C nodes still report `Permission denied`, reboot. Launch `linrgb` without `sudo`.

## Install on Debian 13 (Trixie)

The [Debian recipe](packaging/debian/) builds a local `amd64` package. Debian 13's default Rust compiler is too old for this source, so the recipe uses a pinned Rust toolchain via `rustup`.

```bash
sudo apt update
sudo apt install build-essential ca-certificates curl debhelper git libgtk-4-dev pkgconf rustup
rustup toolchain install 1.99.1 --profile minimal

git clone https://github.com/LinRGB/LinRgb.git
cd LinRgb
sh packaging/debian/build.sh
sudo apt install ./linrgb_1.0.0~beta.1-4_amd64.deb
```

## Install on Fedora 44

The [RPM spec](packaging/fedora/linrgb.spec) builds a local `x86_64` package. It is not published on COPR.

```bash
sudo dnf install cargo gcc git gtk4-devel rpm-build rpmdevtools
rpmdev-setuptree

git clone https://github.com/LinRGB/LinRgb.git
cd LinRgb
spectool -g -R packaging/fedora/linrgb.spec
rpmbuild -bb packaging/fedora/linrgb.spec
sudo dnf install "$HOME"/rpmbuild/RPMS/x86_64/linrgb-1.0.0~beta.1-4.fc44.x86_64.rpm
```

Build as your normal user on both distributions. Both packages include the launcher, targeted USB and I²C udev rules, and a configuration that loads `i2c-dev` at boot. Reboot after the first installation, then launch `linrgb` without `sudo`.

## Features

### Devices and lighting

- **Device selection:** search the list, select devices individually, or use **Select all**. The status area reports success or errors per device.
- **Solid color:** set one steady color using the wheel or a six-digit HEX code. The brightness control adjusts its intensity.
- **Two-color fade:** hold Color A, briefly fade to Color B, then repeat in reverse. The cycle duration covers the full round trip.
- **Breathing:** pulse from dark to the selected color and back to dark. Optionally alternate between Colors A and B on successive breaths.
- **Color cycle:** sweep through the color spectrum on the selected devices, with adjustable cycle duration and saturation.
- **Effect preview:** watch a simulated graphics card and memory modules respond to your settings. The preview uses the same effect calculation as the selected devices and does not send any hardware commands.

Click **Apply** to send the settings or replace a running effect. **Reset to white** stops the effect and sets the selected devices to solid white. It does not restore factory settings. LinRgb does not save lighting settings to the controllers.

### Profiles, appearance, and desktop integration

- **Profiles:** save both colors, the effect, cycle duration, and relevant options. Select a profile to load it without changing the LEDs. Use the profile’s menu to rename or delete it, then use **Apply** when you want to change the lighting.
- **Settings:** choose a built-in theme, import a custom JSON palette, or set the interface zoom from 80% to 150%. English is the default. Restart LinRgb after switching between English and French.
- **Live wallpaper colors:** select **Noctalia Wallpaper** to watch `themes/wallpaper.json`. Noctalia, Matugen, or another tool can generate it. LinRgb keeps the last valid palette if the file becomes invalid, and starts with LinRgb Dark if no valid palette exists. This changes the interface, not the LEDs. See the [theme template and format](design/README.md).
- **System monitor:** view GPU load, VRAM, temperature, and system memory usage when available. The monitor appears in the maximized layout or opens from the **System monitor** section in the stacked layout.
- **System tray:** on compatible desktops, closing the window hides LinRgb while the effect continues. Click the tray icon to reopen it. The yellow button minimizes the window normally. To stop the effect and quit, use **Settings → Quit LinRgb** or press `Ctrl+Q` in the main window. Closing the window quits normally when no compatible tray is available.

LinRgb creates `$XDG_CONFIG_HOME/LinRGB/themes/` on first launch, using `~/.config/LinRGB/themes/` by default. Saved profiles and appearance settings live in `profiles.json` and `appearance.json` in the same config directory. The last imported palette is stored as `themes/imported.json`.

## Hardware support and limits

- **Tested with solid color:** ASUS ROG Astral GeForce RTX 5080 OC, ASUS ROG Crosshair VIII Hero (Wi-Fi) fixed Aura zone, and two ENE RGB memory modules at SMBus addresses `0x72` and `0x73`. The Crosshair’s addressable headers are not supported. Other ASUS NVIDIA cards in the ENE catalogue are offered as experimental targets when all four PCI IDs match. LinRgb only applies lighting if the controller reports a recognized firmware and a supported LED count. This does not add direct support for AMD GPUs or other controller families.
- **Experimental:** ASUS ROG STRIX B550-A GAMING fixed RGB outputs and `Addressable RGB Header 1` (100 LED positions). Select the header to control devices synced to it, such as an XFX RX 9070 XT Mercury connected for motherboard sync. No direct XFX GPU driver is included. Corsair Vengeance RGB Pro SL DDR4 lighting is experimental on the B550-A setup at SMBus addresses `0x5A` and `0x5B`.
- **Fractal Adjust Pro Hub (`36BC:1001`):** selectable in Windows Dynamic Lighting (WDL) mode when it exposes the supported HID LampArray report layout. LinRgb sends one color to all lamps on the hub. Other LampArray layouts are detected but not controlled. In the hub’s standalone mode, it will not appear as a WDL target.
- **Linux multicolor LEDs:** LinRgb can use RGB devices exposed by the kernel under `/sys/class/leds`, subject to driver support and permissions. The white channel stays off on RGBW devices. See the [Linux multicolor LED documentation](https://docs.kernel.org/leds/leds-class-multicolor.html).
- **PCI inventory:** an identified GPU is not necessarily controllable. The [OpenRGB-derived catalog](data/openrgb-pci.tsv) matches the four PCI IDs and lists candidate controller families; it never authorizes RGB writes by itself.

## Develop and verify

```bash
nix develop -c cargo run --locked
nix flake check --no-update-lock-file
```

The Nix package is defined in [`nix/package.nix`](nix/package.nix). To regenerate the identification catalog from a clean OpenRGB checkout, run `python3 tools/import_openrgb_pci.py /path/to/OpenRGB data/openrgb-pci.tsv`. The output records the upstream revision. After updating the catalogue, run `python3 tools/sync_ene_udev.py` to regenerate the matching adapter permissions. Python is not needed to run LinRgb.
