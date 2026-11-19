# LinRGB 1.0.0-beta.2 — hardware testing beta

This beta introduces passive hardware inventory, searchable device selection and opt-in lighting sessions. USB/HID and Linux LED events refresh the list. Sleep, stale queued requests and device loss no longer authorize replay on a replacement attachment. Diagnostics is a headless, allowlisted JSON report; `--version` identifies the installed build.

- Linux multicolor LEDs and Fractal Adjust Pro WDL recheck their interface layout before color. Aura fixed/ARGB zones on the B550-A share one verified connection.
- Corsair Vengeance RGB Pro SL control is restored after a complete, CRC-checked information query. The two DIMM slots remain independent. The same ten-position direct protocol now supports Vengeance RGB Pro DDR4 IDs `0100/0101` **only with protocol 4**. Both Pro variants have simulated transaction coverage; this release has not been tested on physical DIMMs.
- RS DDR4 `0400/0401` is identified on explicit query but cannot receive colors. OpenRGB's [unified driver](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/91714d1aaca683488402505beb5f7d5ebc858d1e/Controllers/CorsairDRAMController) constructs a 20-byte six-position packet yet requests a 32-byte block write. That does not establish valid padding or prove a 20-byte SMBus block works. No speculative frame was added. Older Pro protocol 3 also stays unsupported.
- Arch, Debian 13 and Fedora 44 native packages, a pinned NixOS flake, a [short testing guide](testing-beta.md) and a hardware feedback form are provided for external testing. Existing settings, profiles, theme format and tray behavior are unchanged.

## Known limitations

No automatic SMBus address scan. No new motherboard association: another host needs independently verified PCI/adapter routing and a tester; matching DMI is not sufficient. No general-purpose HID/LampArray or GPU writer. B550-A's exact Aura descriptor and suspend/reconnect behavior still need owner confirmation. Simulations, CI and package installs do not establish physical RGB behavior. Start with diagnostics, not Apply.
