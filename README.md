![LinRGB banner](ressources/LinRGB-banner.png)

# LinRGB

A native Linux RGB controller written in Rust and GTK4. Preview a color or effect, choose the devices you want, then **Apply**. LinRGB lists hardware from Linux metadata without scanning SMBus addresses. It does not need OpenRGB or SignalRGB to run.

**Beta for external testing · x86-64 Linux.** Start with [the tester guide](docs/testing-beta.md). An inventory match is not permission to write to a device.

## Install

Get the package for your distribution and `SHA256SUMS` from the [beta releases](https://github.com/LinRGB/LinRgb/releases). From your downloads directory, verify the file with `sha256sum --ignore-missing -c SHA256SUMS`; make sure **your package** is listed as `OK`. Install one package, not all three:

| System | Install the downloaded beta 2 file |
|---|---|
| Arch Linux x86_64 | `sudo pacman -U ./linrgb-1.0.0-beta.2-arch-x86_64.pkg.tar.zst` |
| Debian 13 amd64 | `sudo apt install ./linrgb-1.0.0-beta.2-debian13-amd64.deb` |
| Fedora 44 x86_64 | `sudo dnf install ./linrgb-1.0.0-beta.2-fedora44-x86_64.rpm` |

These are standalone packages, not an AUR submission, Debian repository or Fedora COPR. They install the desktop launcher, icon, targeted udev rules and `i2c-dev` modules-load entry. Reboot if the new device permissions have not taken effect. Run `linrgb` as your **normal desktop user**, never as root.

### NixOS

For an inventory-only check on the tagged release: `nix run github:LinRGB/LinRgb/v1.0.0-beta.2 -- --diagnostics`. A one-off run or Home Manager package does not install system device rules.

For system integration, add `inputs.linrgb.url = "github:LinRGB/LinRgb/v1.0.0-beta.2";` to your existing flake. Keep LinRGB's own pinned nixpkgs. With `outputs = inputs@{ nixpkgs, ... }:`, include `inputs.linrgb.nixosModules.default` in your host's `modules` list alongside `./configuration.nix`. Enable it in that configuration:

```nix
programs.linrgb.enable = true;
```

From the directory containing your flake run `nix flake lock`, then `sudo nixos-rebuild switch --flake .#my-host` (replace `my-host`). The module supplies the application, udev rules and kernel module. To upgrade to a later beta, change the tag and run `nix flake update linrgb` before rebuilding.

## Hardware: what works, and what is only a candidate?

| Controller | Identification and available control | Evidence / limits |
|---|---|---|
| Linux multicolor LED class | RGB/RGBW channel order and scale rechecked on the same sysfs attachment; uniform color | Temporary filesystem tests; depends on the kernel driver |
| Fractal Adjust Pro Hub | WDL mode, exact HID descriptor, lamp count and update interval from the open device | Older setup exercised; new lifecycle tested with fixtures, not physically on this build |
| ASUS Aura USB | Crosshair VIII Hero (Wi-Fi) fixed zone; Strix B550-A fixed and addressable header 1 on a matching interface | Earlier Crosshair solid color test. B550-A layout experimental; counts are profile positions, not measured LEDs |
| Corsair Vengeance RGB Pro SL / Pro DDR4 | Select slot `0x5A` or `0x5B` individually on the matched B550-A adapter. Requires a valid information packet, exact product and protocol 4 | SL direct color reported on earlier software; restored sessions and new ten-position **Pro** variant tested in simulation. Protocol 3 and Vengeance RS have **no RGB writer** |
| ENE memory | Crosshair setup, two opt-in slots, firmware signature required | Earlier solid-color tests at `0x72`/`0x73`; not universal DIMM discovery |
| ASUS NVIDIA ENE GPU | Four matching PCI IDs plus compatible controller firmware and count | Earlier Astral RTX 5080 OC test; other catalogued cards experimental |
| Other PCI / HID matches | Inventory only without a matching driver | No direct AMD/XFX GPU backend; a card connected to a motherboard-sync header is controlled via that header |

**Discovery is passive; Apply is not.** Close other lighting tools and their background services before testing. A new attachment or an I/O failure does not automatically inherit or restart an effect. A command already submitted to the kernel cannot be recalled. I²C device permissions grant access to an adapter, not just its RGB chip. See [Aura](docs/aura-usb.md) and [Corsair](docs/corsair-sessions.md) for the exact checks.

## Everyday use

- Pick a fixed HEX color, two-color fade, breathing effect or spectrum cycle. The preview does not touch hardware. **Reset to white** sends white; it is not a factory reset.
- Save colors and effects as profiles. Loading a profile does not apply it. The existing configuration and profile formats are unchanged.
- Use built-in palettes or a [custom theme / Noctalia wallpaper palette](design/README.md). Interface themes do not change your lighting.
- Close to the tray if your desktop provides one; use **Ctrl+Q** to quit completely.

## Diagnostics and feedback

Run `linrgb --version`, then `linrgb --diagnostics > linrgb-diagnostics.json`. This works without a graphical session. It reports allowlisted IDs, backend matches and an access state; no RGB writes, serial numbers or automatic upload. `available` means an interface appears accessible, not that a controller has passed active verification. For `denied`, check local-session access and package/module installation rather than using `sudo linrgb`.

See the [testing guide](docs/testing-beta.md) and [report compatibility](https://github.com/LinRGB/LinRgb/issues/new?template=hardware.yml). Review the JSON yourself before sharing it.

## Development, credits and licenses

With flakes enabled: `nix develop`, `cargo test --all-targets --locked`, `cargo run --locked -- --diagnostics`. For the packaged build use `nix build`. Native recipes are in [`packaging/`](packaging); release-source and verification instructions are in [the release checklist](docs/releasing.md).

LinRGB was built with substantial AI assistance under the maintainer's direction. People tested selected hardware; a successful build or simulation is not a physical compatibility claim. Thanks to OpenRGB contributors for protocol research and PCI identification data. See [third-party notices](notices/README.txt); LinRGB has not declared a general license for its own code or artwork.
