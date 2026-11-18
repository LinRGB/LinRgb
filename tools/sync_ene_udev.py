#!/usr/bin/env python3
"""Refresh or check NVIDIA ENE adapter rules from the exact PCI catalogue."""

import argparse
from collections import defaultdict
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
path = root / "packaging/70-linrgb.rules"
start = "# BEGIN generated ASUS NVIDIA ENE rules\n"
end = "# END generated ASUS NVIDIA ENE rules\n"
groups = defaultdict(set)
for row in (root / "data/openrgb-pci.tsv").read_text().splitlines():
    if row.startswith("#") or not row:
        continue
    vendor, device, subvendor, subsystem, family, _ = row.split("\t", 5)
    if (vendor, subvendor, family) == ("0x10de", "0x1043", "ENESMBusController"):
        if not all(re.fullmatch(r"0x[0-9a-f]{4}", value) for value in (device, subsystem)):
            raise SystemExit("Invalid PCI ID in ENE catalogue")
        groups[device].add(subsystem)
if not groups:
    raise SystemExit("No ASUS NVIDIA ENE entries found")
lines = []
for device, subsystems in sorted(groups.items()):
    ids = "|".join(sorted(subsystems))
    lines.append(f'SUBSYSTEM=="i2c-dev", ATTR{{name}}=="NVIDIA i2c adapter 1 at *", ATTRS{{vendor}}=="0x10de", ATTRS{{device}}=="{device}", ATTRS{{subsystem_vendor}}=="0x1043", ATTRS{{subsystem_device}}=="{ids}", TAG+="uaccess"\n')
original = path.read_text()
head, tail = original.split(start, 1)
_, tail = tail.split(end, 1)
updated = head + start + "".join(lines) + end + tail
if args.check:
    if updated != original:
        raise SystemExit("ENE rules are stale; run tools/sync_ene_udev.py")
else:
    path.write_text(updated)
print(f"{sum(map(len, groups.values()))} exact PCI matches in {len(groups)} rules")
