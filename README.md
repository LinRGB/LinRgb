![LinRGB banner](ressources/LinRGB-banner.png)

# LinRGB

A native Linux RGB controller written in Rust and GTK4. Preview a color or effect, choose the devices you want, then **Apply**. LinRGB lists hardware from Linux metadata without scanning SMBus addresses. It does not need OpenRGB or SignalRGB to run.

**Beta for external testing · x86-64 Linux.** Start with [the tester guide](docs/testing-beta.md). An inventory match is not permission to write to a device.

## Install

Get the package for your distribution and `SHA256SUMS` from the [beta releases](https://github.com/LinRGB/LinRgb/releases). From your downloads directory, verify the file with `sha256sum --ignore-missing -c SHA256SUMS`; make sure **your package** is listed as `OK`. Install one package, not all three:

| System | Install the downloaded beta 4 file |
|---|---|
| Arch Linux x86_64 | `sudo pacman -U ./linrgb-1.0.0-beta.4-arch-x86_64.pkg.tar.zst` |
| Debian 13 amd64 | `sudo apt install ./linrgb-1.0.0-beta.4-debian13-amd64.deb` |
| Fedora 44 x86_64 | `sudo dnf install ./linrgb-1.0.0-beta.4-fedora44-x86_64.rpm` |

These are standalone packages, not an AUR submission, Debian repository or Fedora COPR. They install the desktop launcher, icon, targeted udev rules and `i2c-dev` modules-load entry. Reboot if the new device permissions have not taken effect. Run `linrgb` as your **normal desktop user**, never as root.

### NixOS

**Just want to see what LinRGB detects?** Run this as your normal user. It does not install device access rules or change any colors:

```bash
nix run github:LinRGB/LinRgb/main -- --diagnostics
```

**Want the desktop app and device access?** If you already use a NixOS flake on x86-64, follow these three steps:

| ① `flake.nix` | ② `configuration.nix` | ③ Terminal |
|---|---|---|
| Add the LinRGB input and module | Enable LinRGB for your host | Rebuild, then check the version |

#### ① Connect the module in `flake.nix`

The `+` lines below are additions to your **existing** flake. Type the lines without the `+` signs. Keep your current `nixpkgs`, host name and other modules.

```diff
  inputs = {
+   linrgb.url = "github:LinRGB/LinRgb/main";
  };

- outputs = { self, nixpkgs, ... }: {
+ outputs = inputs@{ self, nixpkgs, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      modules = [
        ./configuration.nix
+       inputs.linrgb.nixosModules.default
      ];
```

If your `outputs` already binds `inputs`, leave that line unchanged. Let LinRGB use its own pinned nixpkgs.

#### ② Enable LinRGB in `configuration.nix`

Add this line **inside the existing `{ ... }`**, alongside your other settings:

```nix
programs.linrgb.enable = true;
```

#### ③ Rebuild and check

Open a terminal in the folder containing `flake.nix`. Replace `my-host` with the name after `nixosConfigurations.` in step ①.

```bash
nix flake lock
sudo nixos-rebuild switch --flake .#my-host
linrgb --version
```

**Expected:** `LinRGB` followed by the installed version. Open **LinRGB** from your application menu as your normal user. The module installs the app, registers the udev rules and loads `i2c-dev`; no manual permission changes are needed.

<details>
<summary>Updating LinRGB from main</summary>

The input follows `main`, not a release tag. Nix records the chosen commit in `flake.lock` so rebuilds are reproducible. To pick up newer commits, run `nix flake update linrgb` in your flake directory, then repeat the rebuild in step ③. No URL change is needed.

</details>

## Hardware: what works, and what is only a candidate?

**Real-PC results below refer to the specific operation tested on earlier software.** They are not a physical sign-off for every effect, firmware revision or beta.4 session.

| Evidence | Hardware | What the record shows |
|---|---|---|
| Real PC · solid color | ASUS ROG Astral GeForce RTX 5080 OC | The card responded to a fixed color. This result does not extend to other ASUS GPUs. |
| Real PC · solid color | ASUS ROG Crosshair VIII Hero (Wi-Fi) | Fixed Aura zone only. Its addressable headers are not supported. |
| Real PC · solid color | Two ENE RGB DIMMs on the Crosshair | Both modules at `0x72` and `0x73` responded. These are not universal DIMM addresses. |
| Real PC · earlier report | Corsair Vengeance RGB Pro **SL** DDR4 on the Strix B550-A | Direct color worked before the session migration. The restored driver and isolation between `0x5A` and `0x5B` were tested in simulation. |
| Real PC · earlier setup | Fractal Adjust Pro Hub in WDL mode | The hub was exercised on hardware; the new connection and update-interval checks were tested with simulated reports. |
| Simulation only | Corsair Vengeance RGB **Pro** DDR4 (non-SL); Linux multicolor LED class | Pro: ten-position direct frames on protocol 4. Kernel LEDs: RGB/RGBW channel order and scale tested with temporary sysfs files. |
| Simulation only for the migrated driver | ASUS ROG Strix B550-A Gaming | Fixed outputs and Addressable RGB Header 1 tested together on a mock. The reported XFX RX 9070 XT Mercury connection is **motherboard sync**, not direct GPU support. |
| Candidate · RGB unconfirmed | Other ASUS NVIDIA ENE catalogue matches | Experimental control requires four matching PCI IDs plus supported firmware and LED count. A catalogue entry is not a hardware test. |
| Identified or listed · no RGB writer | Corsair Vengeance RS DDR4, protocol 3 Pro, unsupported PCI/HID devices | A device can be listed or identified without a supported color command. A passive RAM slot is not proof that a DIMM is present. |

[Validation sources and outstanding hardware checks](docs/compatibility.md)

**Discovery is passive; Apply is not.** Close other lighting tools and their background services before testing. A new attachment or an I/O failure does not automatically inherit or restart an effect. A command already submitted to the kernel cannot be recalled. I²C device permissions grant access to an adapter, not just its RGB chip. See [Aura](docs/aura-usb.md) and [Corsair](docs/corsair-sessions.md) for the exact checks.

## Everyday use

- **Aurora** blends two overlapping passes through a palette of 2–5 colors. **Color cycle** visits each palette color in order, or uses the classic spectrum. Select a numbered swatch to edit just that color with the shared wheel, HEX field or brightness slider. Both palettes let you add or remove colors before applying.
- **Gradient flow** moves continuously from A to B and back. **Pulse** flashes A against background B. Solid, fade and breathing are still available. Set the cycle duration from 2 to 60 seconds.
- The preview and selected devices follow the same clock after **Apply**. Each target receives a single color, not a per-LED wave. Controllers can update at different rates. Editing or loading a profile changes only the preview until you apply again. **Reset to white** sends white; it is not a factory reset.
- Profiles remember the active palette. Older profiles still load, including spectrum cycles saved without a palette.
- Use built-in palettes or a [custom theme / Noctalia wallpaper palette](design/README.md). Interface themes do not change your lighting.
- Close to the tray if your desktop provides one; use **Ctrl+Q** to quit completely.

## Diagnostics and feedback

Run `linrgb --version`, then `linrgb --diagnostics > linrgb-diagnostics.json`. This works without a graphical session. It reports allowlisted IDs, backend matches and an access state; no RGB writes, serial numbers or automatic upload. `available` means an interface appears accessible, not that a controller has passed active verification. For `denied`, check local-session access and package/module installation rather than using `sudo linrgb`.

See the [testing guide](docs/testing-beta.md) and [report compatibility](https://github.com/LinRGB/LinRgb/issues/new?template=hardware.yml). Review the JSON yourself before sharing it.

## Development, credits and licenses

With flakes enabled: `nix develop`, `cargo test --all-targets --locked`, `cargo run --locked -- --diagnostics`. For the packaged build use `nix build`. Native recipes are in [`packaging/`](packaging). Version tags build, test and publish Arch, Debian 13 and Fedora 44 packages together with SHA-256 checksums. Use the manual Actions run to test without publishing; see the [release checklist](docs/releasing.md).

LinRGB was built with substantial AI assistance under the maintainer's direction. People tested selected hardware; a successful build or simulation is not a physical compatibility claim. Thanks to OpenRGB contributors for protocol research and PCI identification data. Original LinRGB code and documentation are available under the [MIT License](LICENSE). This grant does not cover the OpenRGB-derived catalogue and device matches, hardware descriptors, branding artwork or third-party dependencies. See the [licensing and attribution notes](notices/README.txt) before redistributing bundled material.
