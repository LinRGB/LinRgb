# Corsair Vengeance RGB Pro SL DDR4: verified direct RGB

**Status: available on explicit Apply for the existing B550-A configuration; exercised in simulation, not on physical RAM in this milestone.** Select one known slot individually. Discovery, refresh and resume do not query the module or restore a color. There is no override for a mismatching identity packet.

## What changed

LinRGB `85db170` introduced the direct driver; `8811a5d` removed a configuration busy wait that interfered with repeated direct updates. `dff035c` added the preliminary family reads. All three used `src/corsair.rs`. Milestone 5 (`990d3a8`, `src/drivers/corsair.rs`) retained the adapter/slot checks and RGB frame but required full identity without selector writes. The Linux transport could never supply that evidence. Milestone 6 restores the known **device-info transaction**, not a family-wide RGB fallback. The difference is which command is authorized when: the family match allows an info query, only the validated response allows color.

## Evidence from OpenRGB

- [MR 932](https://gitlab.com/CalcProgrammer1/OpenRGB/-/merge_requests/932), head `8a57e5c1`: closed, not merged. Its detector change would also accept `0x1b` at register `0x43` in the Vengeance Pro driver. Dominator can return that same byte; assigning its 12-position layout to a ten-position Pro SL is wrong. The old Vengeance and Dominator detectors used different overlapping family checks, not a reliable model decoder.
- [MR 3241](https://gitlab.com/CalcProgrammer1/OpenRGB/-/merge_requests/3241), head `4b1a5749`, merged 9 April 2026, squash `9a6ff775`. It combined those drivers and collected identification packets. The initial description includes examples for other kits; the evidence for **Pro SL** is the explicit mapping in the merged device table, not the assumption that a DDR5 example also applies to DDR4.
- Checked the [official master snapshot `91714d1a`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/91714d1aaca683488402505beb5f7d5ebc858d1e/Controllers/CorsairDRAMController) and the [mirror snapshot `5bcfa9ae`](https://github.com/CalcProgrammer1/OpenRGB/tree/5bcfa9ae01536d2dd45b0c6e5b15677513f7a6d5/Controllers/CorsairDRAMController). `CorsairDRAMControllerDetect.cpp` checks the family. `CorsairDRAMController.h/.cpp` define the commands, `ReadDeviceInfo`, direct frame and configuration paths. `CorsairDRAMDevices.h/.cpp` map product IDs to layouts; `RGBController_CorsairDRAM.cpp` exposes effects. These are reverse-engineered protocol references, not a vendor guarantee. No OpenRGB code or runtime dependency imported.

| Address or transaction | Meaning and limits |
|---|---|
| `0x5a`, `0x5b` | Seven-bit **slave addresses** in the existing B550-A profile; neither is an identity register. No other address is tried. |
| Read byte-data `0x43`, then `0x44` | Family gate: respectively `0x1a`–`0x1c` and one of `1,3,4`. Their full firmware semantics are unknown. In particular, reading `4` here is not the protocol-version check. |
| Write `0x61=0` | Select the device-information buffer. This changes protocol selection state; it is not a passive register read. |
| Write `0x21=0` | Start the binary transfer for the selected buffer. Treat this as starting the stream, not as a standalone model ID, a firmware reset or a save. Exact cursor implementation is unknown. |
| Read `0x40` × 32, then `0x42` once | Consume the selected info stream and its CRC-8. Unlike upstream's log-and-continue CRC mismatch path, LinRGB rejects any I/O error or mismatch. Reading an arbitrary previously selected buffer is not an alternative. |
| Write block `0x31` | After verification only: ten positions, each with the requested RGB, followed by CRC (32 bytes including leading count). Reused for effects. No hardware effect configuration. |

In the checked source, both selectors are common to device-info acquisition in the unified driver. Its configuration/update paths are separate (e.g. `0x82` to apply configuration, `WaitReady` there rather than in `ReadDeviceInfo`). There is **no save, bootloader switch, RGB payload or config commit** in the identity query. A temporary selection/transfer state is the interpretation supported by that code and by LinRGB's earlier operation, not a proof of zero side effects for every firmware or for an unrelated chip imitating the family bytes. No speculative register, retry, reset, busy polling or compensating write was added.

## What is actually identified?

The info packet contains little-endian vendor and product IDs (offsets 0 and 2), firmware fields at 8–11 and protocol version at 28. OpenRGB maps DDR4 `0100/0101` to Vengeance Pro, `0200/0201` to Dominator Platinum, `0300/0301` to Vengeance Pro SL, and `0400/0401` to Vengeance RS (hex). LinRGB requires VID `1b1c`, one of the two **SL** PIDs, a valid packet CRC and protocol **4**. Upstream allows direct mode from version 4; this restoration does not assume an unknown future protocol. Ten is the SL profile size, not a measured LED count. Firmware version is not used as a model discriminator.

This identifies the **controller profile**, not a unique serial, exact DIMM SKU or continuous physical presence. No reliable strictly read-only equivalent for this packet was found in the examined drivers. Already available SPD/SMBIOS part labels could corroborate the installed kit, but not bind a particular lighting endpoint to its reported model. No SPD reads, additional probes, manual identity assertion or compatibility switch are necessary.

## Lifecycle and validation

- Passive checks still require the supported DMI and PCI host IDs, unique PIIX4 adapter and exact attachment. Apply rechecks the opened character device, sysfs parent and inode, uses ordinary (non-forced) slave selection and takes a nonblocking per-slot OFD lock. Only the selected slot is queried. Unlike OpenRGB discovery, no address range is enumerated.
- The same fd carries the query and direct frames. The memory worker serializes the two selected slots. Cooperating processes cannot interleave requests to the same slot; other bus clients need not honor the lock and can disturb the selected stream. Close other RGB tools. CRC is an integrity check, not authentication. Cancellation/stale inventory is checked between identification transactions; an already submitted transaction cannot be recalled. No reopening on failure.
- Synthetic transaction transcripts cover both SL PIDs, ambiguous models, unrecognized protocols, corrupt CRC, failure at every selector/read including truncated transfers, stale ticket before first RGB, cancellation between selectors, repeated frames and terminal write failure. No physical SMBus transactions were performed for this milestone.
- **Physical follow-up (owner approval required):** on the known B550-A kit, stop competing clients, select one known slot, apply two distinct solid colors in succession, and verify only the intended module responds. Repeat for the other slot, then both together. Confirm an effect stops on suspend without replay on resume. Restore the original color explicitly. Never hot-swap DIMMs.
