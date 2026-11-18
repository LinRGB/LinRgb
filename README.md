# LinRgb — 1.0 Beta 1

**AI disclosure:** LinRgb was built with AI assistance under the maintainer’s direction. AI generated much of the code and documentation, and the maintainer tested selected hardware and workflows. A passing software test does not mean an unlisted device is safe or supported.

LinRgb is a native Rust and GTK4 app for RGB lighting on Linux. NixOS on `x86_64-linux` is the primary platform for this beta. OpenRGB and SignalRGB are not required at runtime.

## Try the beta without installing it

With Nix Flakes enabled, run:

```bash
nix run github:Yuna404/LinRgb/v1.0.0-beta.1
```

This downloads or builds the pinned beta and opens LinRgb. It does not add a desktop launcher to your system profile or install udev rules. Close other RGB applications and their background servers before applying any lighting settings. Do not run the GUI as root.

## Install on NixOS with Flakes

1. Add the release as an input in your existing `flake.nix`:

   ```nix
   inputs.linrgb.url = "github:Yuna404/LinRgb/v1.0.0-beta.1";
   ```

2. Include `linrgb` in your existing `outputs` arguments, for example `outputs = { nixpkgs, linrgb, ... }: ...`. In the `modules` list of your `nixpkgs.lib.nixosSystem` call, add this inline module. Keep your existing modules:

   ```nix
   ({ pkgs, ... }: let
     package = linrgb.packages.${pkgs.stdenv.hostPlatform.system}.default;
   in {
     environment.systemPackages = [ package ];
     services.udev.packages = [ package ];
   })
   ```

3. From the directory containing your system flake, lock the input and rebuild. Replace `YOUR_HOST` with the name under `nixosConfigurations`:

   ```bash
   nix flake lock
   sudo nixos-rebuild switch --flake .#YOUR_HOST
   ```

4. Open **LinRgb** from the application menu or run `linrgb`. Reconnect the RGB USB controller after the rebuild so udev can apply the new permissions. Commit your updated `flake.lock` to keep the installation reproducible. To use a newer release, change the tag in the input URL before rebuilding.

The udev rules grant the active local session access to the supported ASUS Aura and Fractal USB IDs. GPU and RAM controllers using I²C still require access to their specific `/dev/i2c-*` nodes. The package does not grant blanket I²C access. If **Apply** reports `Permission denied`, verify the adapter and its permissions instead of launching LinRgb with `sudo`.

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
- **System tray:** on compatible desktops, the yellow window button hides LinRgb while the effect continues. Click the tray icon to reopen it. The red button quits.

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
