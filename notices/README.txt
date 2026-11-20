Licensing scope
===============

The root LICENSE applies to original LinRGB source code and documentation
owned or controlled by its contributors. It does not relicense the following
bundled material or grant trademark rights.

* OpenRGB-derived data/openrgb-pci.tsv and the generated ENE matches in
  packaging/70-linrgb.rules: see OpenRGB-COPYING and the provenance header in
  the catalogue. The source registrations are from OpenRGB contributors,
  revision 5bcfa9ae01536d2dd45b0c6e5b15677513f7a6d5 (GPL-2.0-or-later):
  https://github.com/CalcProgrammer1/OpenRGB/tree/5bcfa9ae01536d2dd45b0c6e5b15677513f7a6d5
  The import and rule-generation scripts are in this repository.
* HID report fixtures src/drivers/aura-interface.hid and
  src/fractal-adjust-pro-wdl.hid: hardware interface data, not covered by the
  MIT grant. The Aura capture is documented in docs/aura-usb.md.
* Artwork: ressources/LinRGB-banner.png, the embedded artwork in
  packaging/io.github.linrgb.LinRgb.svg, and src/tray-icon.argb. No open
  artwork license is declared here. This list includes derived icon formats.
* Rust and system dependencies retain their upstream terms. See Cargo.lock
  for Rust versions and the distribution packages for GTK and system libraries.

LinRGB does not link to or require OpenRGB or SignalRGB. Protocol references
are attributed in the hardware documentation. Earlier release assets were
not rebuilt or relabeled by this licensing change.
