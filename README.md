# LinRgb — 1.0 Beta 1

LinRgb controls RGB lighting on Linux, with NixOS as the primary platform. It is written in Rust with GTK4 and does not require OpenRGB or SignalRGB at runtime.

## Getting started

```bash
nix develop -c cargo run
```

Run LinRgb without `sudo`. Close other RGB applications and their background services before applying lighting settings.

1. Select your devices.
2. Choose **Solid color**, **Two-color fade**, **Breathing**, or **Color cycle**. Breathing fades to black between pulses and can alternate between two colors. Color cycle sweeps the spectrum with adjustable saturation. The preview uses the same effect as the selected devices.
3. Click **Apply**. Applying another setting replaces the running effect. **Reset to white** stops the effect and sets the selected devices to solid white; it does not restore factory settings.

The yellow window button hides LinRgb on desktops with a compatible system tray. Click the tray icon to reopen it. The red button exits LinRgb. Lighting settings are not saved to the controllers.

Run the packaged app from this checkout with `nix run .` on `x86_64-linux`. The package includes a desktop launcher, a GTK runtime wrapper, and opt-in USB permission rules. Verify the locked build with `nix flake check --no-update-lock-file`.

### NixOS device access

Add the same LinRgb package to `environment.systemPackages` and `services.udev.packages` in your NixOS configuration. Rebuild and reconnect the USB controller so the rules apply. The rules match only the supported ASUS Aura and Fractal USB IDs and grant access to the active local session. `nix run .` alone does not install udev rules.

For GPU and RAM controllers using I²C, LinRgb still requires access to the relevant `/dev/i2c-*` node. The package does not grant blanket I²C access. Verify the exact adapter and permissions with your administrator if **Apply** reports an access error. Never run the GUI as root.

## Profiles and themes

Enter a name in the **Profiles** sidebar and click **Save profile** to store both colors, the lighting mode, and the cycle duration. Saving under the same name updates the profile. Selecting a profile loads its settings without changing the LEDs. Click **Apply** to send the settings to the selected devices.

Open **Settings** from the title bar to choose a built-in theme, import a JSON palette, or set the interface zoom from 80% to 150%. English is the default language; restart LinRgb after switching to French or English. The custom palette format and Matugen template are in [`design/`](design/README.md).

Profiles are stored in `profiles.json`, appearance settings in `appearance.json`, and the latest imported palette in `themes/imported.json`. These paths are relative to `$XDG_CONFIG_HOME/LinRGB`, or `~/.config/LinRGB` by default.

Choose **Noctalia Wallpaper** under **Settings → Theme** to follow `themes/wallpaper.json` automatically. Configure Noctalia to generate that file from the supplied Matugen template. LinRgb keeps the last valid palette if the JSON is temporarily missing or invalid, and uses LinRgb Dark if no valid palette has been read yet. This changes the interface only, not hardware lighting.

## Hardware support

Solid color has been confirmed on the ASUS ROG Astral GeForce RTX 5080 OC, the fixed Aura zone of the ASUS ROG Crosshair VIII Hero (Wi-Fi), and two ENE RGB memory modules at SMBus addresses `0x72` and `0x73`. The motherboard's addressable RGB headers are not yet supported.

The Fractal Adjust Pro Hub (`36BC:1001`) can be selected in Windows Dynamic Lighting mode when it exposes the supported HID LampArray report layout. LinRgb sends the same color to all lamps on the hub. Other LampArray layouts are listed for identification only. Access to the hub's `/dev/hidraw*` node is required.

Other Astral RTX 5080 variants are recognized by PCI ID, but are not verified on hardware. LinRgb rejects unrecognized ENE firmware. It also detects RGB LEDs exposed under `/sys/class/leds`, where driver support and permissions allow it. For RGBW devices, the white channel is kept off. See the [Linux multicolor LED documentation](https://docs.kernel.org/leds/leds-class-multicolor.html).

## PCI catalog

`data/openrgb-pci.tsv` is generated from OpenRGB's `REGISTER_I2C_PCI_DETECTOR` registrations. Entries are matched on all four PCI IDs. A match identifies a candidate device and controller family; it does not enable RGB writes.

To regenerate the catalog from a clean OpenRGB checkout:

```bash
python3 tools/import_openrgb_pci.py /path/to/OpenRGB data/openrgb-pci.tsv
```

The generated file records the source revision. It covers exact PCI registrations, not USB detection or controller protocols. Python is not required at runtime.
