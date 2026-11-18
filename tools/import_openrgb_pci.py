#!/usr/bin/env python3
"""Generate LinRgb's identification-only PCI catalog from a local OpenRGB checkout."""

import argparse
from pathlib import Path
import re
import subprocess


MACRO = re.compile(r"^\s*REGISTER_I2C_PCI_DETECTOR\s*\(", re.MULTILINE)
RECORD = re.compile(r'\s*"([^"\\\r\n\t]+)"\s*,\s*\w+\s*,\s*([^,]+),([^,]+),([^,]+),([^,]+),')
DEFINE = re.compile(r"^\s*#define[ \t]+(\w+)[ \t]+(0[xX][0-9a-fA-F]+|[0-9]+)\b", re.MULTILINE)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root = args.checkout.resolve()
    revision = subprocess.check_output(["git", "-C", root, "rev-parse", "HEAD"], text=True).strip()
    if subprocess.check_output(["git", "-C", root, "status", "--porcelain"], text=True).strip():
        parser.error("OpenRGB checkout must be clean to record a reproducible source revision")
    defines = dict(DEFINE.findall((root / "pci_ids/pci_ids.h").read_text(encoding="utf-8")))
    rows = set()
    skipped = 0
    controllers = root / "Controllers"
    for path in sorted(controllers.rglob("*.cpp")):
        family = path.relative_to(controllers).parts[0]
        text = path.read_text(encoding="utf-8", errors="replace")
        for start in MACRO.finditer(text):
            record = RECORD.match(text, start.end())
            if not record:
                skipped += 1
                continue
            name, *tokens = record.groups()
            values = [defines.get(token.strip(), token.strip()) for token in tokens]
            if not all(re.fullmatch(r"0[xX][0-9a-fA-F]{1,4}|[0-9]{1,5}", v) for v in values):
                skipped += 1
                continue
            numbers = [int(v, 16 if v.lower().startswith("0x") else 10) for v in values]
            if not all(0 < num < 0xffff for num in numbers):
                skipped += 1
                continue
            rows.add((*[f"0x{num:04x}" for num in numbers], family, name.rstrip()))
    if not rows:
        parser.error("no exact PCI registrations found")
    header = f"# https://github.com/CalcProgrammer1/OpenRGB/tree/{revision}\n# vendor\tdevice\tsubvendor\tsubdevice\tfamily\tmodel\n"
    args.output.write_text(header + "".join("\t".join(row) + "\n" for row in sorted(rows)), encoding="utf-8")
    print(f"{len(rows)} registrations, {len({row[4] for row in rows})} families, {skipped} skipped")


if __name__ == "__main__":
    main()
