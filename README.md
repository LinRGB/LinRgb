# LinRgb — 1.0 Beta 1

**AI disclosure:** LinRgb was built with AI assistance under the maintainer’s direction. AI generated much of the code and documentation, and the maintainer tested selected hardware and workflows. A passing software test does not mean an unlisted device is safe or supported.

**Why I built LinRgb:** I couldn't find a Linux RGB app that worked the way I wanted. I didn't have the programming experience to build it on my own, so I used AI to help make the tool I wanted for my own PC. If you'd rather not use software built this way, I understand. There's no pressure to use LinRgb.

LinRgb is a native Rust and GTK4 app for RGB lighting on Linux. NixOS on `x86_64-linux` is the primary platform for this beta. OpenRGB and SignalRGB are not required at runtime.

## Try the beta without installing it

With Nix Flakes enabled, run:

```bash
nix run github:Yuna404/LinRgb/main
```

This downloads or builds LinRgb from `main` and opens it. It does not add a desktop launcher to your system profile or install udev rules. Close other RGB applications and their background servers before applying any lighting settings. Do not run the GUI as root.

## Install with Flakes

Add LinRgb to your flake inputs:

```nix
inputs.linrgb = {
  url = "github:Yuna404/LinRgb/main";
};
```

The lock file pins the selected revision of `main`. To get newer fixes, run `nix flake update linrgb`, rebuild, and quit and reopen LinRgb so the old process does not stay in the tray.

Keep LinRgb's pinned `nixpkgs` input instead of setting `inputs.linrgb.inputs.nixpkgs.follows = "nixpkgs"`. The beta was tested with its own pinned dependencies.

### NixOS (recommended)

In your NixOS configuration, with `inputs` in scope, add the module to your existing `imports` list and enable it:

```nix
imports = [ inputs.linrgb.nixosModules.default ];
programs.linrgb.enable = true;
```

The module installs LinRgb, loads `i2c-dev`, and registers the bundled udev rules. The rules grant the active local session access to the supported USB controllers and specific Astral GPU and Crosshair SMBus adapters. An I²C permission applies to the entire matched adapter, not just its RGB controller. No blanket I²C access is granted.

Rebuild NixOS, then launch **LinRgb** from the application menu or run `linrgb`. You do not need to add the package or its rules separately. Reconnect USB devices or reboot if device access has not refreshed.

### Package only (Home Manager or custom setup)

In a module with `inputs` and `pkgs` in scope, add this package to `home.packages` or `environment.systemPackages`:

```nix
inputs.linrgb.packages.${pkgs.stdenv.hostPlatform.system}.default
```

Package-only installs and `nix run` do not activate system udev rules. On NixOS, you can register the package separately in `services.udev.packages`. The `v1.0.0-beta.1` tag predates the module and bundles USB rules only. Do not run LinRgb as root.

## Install on Arch Linux

The [PKGBUILD](packaging/arch/PKGBUILD) builds a local `x86_64` package from a pinned source archive. It is not an AUR package.

```bash
sudo pacman -Syu --needed base-devel git

git clone https://github.com/Yuna404/LinRgb.git
cd LinRgb/packaging/arch
makepkg -si
```

Run `makepkg` as your normal user. It asks for administrator access when installing dependencies and the built package. The package includes the desktop launcher, targeted USB and I²C udev rules, and a configuration that loads `i2c-dev` at boot.

Reboot after the first installation so the module and access rules apply. For immediate use, load `i2c-dev` with `sudo modprobe i2c-dev` and reconnect the supported USB controller. If the existing I²C nodes still report `Permission denied`, reboot. Launch `linrgb` without `sudo`.

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

- **Tested with solid color:** ASUS ROG Astral GeForce RTX 5080 OC, ASUS ROG Crosshair VIII Hero (Wi-Fi) fixed Aura zone, and two ENE RGB memory modules at SMBus addresses `0x72` and `0x73`. The motherboard’s addressable headers are not supported. Other Astral RTX 5080 variants are identified by PCI ID but not all have been verified on hardware. Unrecognized ENE firmware is rejected.
- **Fractal Adjust Pro Hub (`36BC:1001`):** selectable in Windows Dynamic Lighting (WDL) mode when it exposes the supported HID LampArray report layout. LinRgb sends one color to all lamps on the hub. Other LampArray layouts are detected but not controlled. In the hub’s standalone mode, it will not appear as a WDL target.
- **Linux multicolor LEDs:** LinRgb can use RGB devices exposed by the kernel under `/sys/class/leds`, subject to driver support and permissions. The white channel stays off on RGBW devices. See the [Linux multicolor LED documentation](https://docs.kernel.org/leds/leds-class-multicolor.html).
- **PCI inventory:** an identified GPU is not necessarily controllable. The [OpenRGB-derived catalog](data/openrgb-pci.tsv) matches the four PCI IDs and lists candidate controller families; it never authorizes RGB writes by itself.

## Develop and verify

```bash
nix develop -c cargo run --locked
nix flake check --no-update-lock-file
```

The Nix package is defined in [`nix/package.nix`](nix/package.nix). To regenerate the identification catalog from a clean OpenRGB checkout, run `python3 tools/import_openrgb_pci.py /path/to/OpenRGB data/openrgb-pci.tsv`. The output records the upstream revision. Python is not needed to run LinRgb.
