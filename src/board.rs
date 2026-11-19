//! Motherboard-specific SMBus guards. USB lighting is in drivers::aura.
use crate::drivers::aura::{B550, CROSSHAIR};
use std::{error::Error, fs, path::Path};
type BoardResult<T> = Result<T, Box<dyn Error>>;
pub fn b550_present() -> bool {
    fs::read_to_string("/sys/class/dmi/id/board_name").is_ok_and(|name| name.trim() == B550)
}
pub fn open_ram(address: u16) -> Result<crate::astral::Ene, Box<dyn Error>> {
    if fs::read_to_string("/sys/class/dmi/id/board_name")?.trim() != CROSSHAIR {
        return Err("Unsupported motherboard.".into());
    }
    if !matches!(address, 0x72 | 0x73) {
        return Err("Unexpected ENE RAM address".into());
    }
    crate::astral::open_ene(ram_bus()?, address, &[b"DIMM_LED-0103"], 0x8000, Some(5))
}

pub fn ram_bus() -> BoardResult<u32> {
    ram_bus_at(Path::new("/sys/bus/pci/devices/0000:00:14.0"))
}

pub fn ram_bus_at(controller: &Path) -> BoardResult<u32> {
    for (attr, id) in [
        ("vendor", "0x1022"),
        ("device", "0x790b"),
        ("subsystem_vendor", "0x1043"),
        ("subsystem_device", "0x87c0"),
    ] {
        if fs::read_to_string(controller.join(attr))?.trim() != id {
            return Err("Unsupported RAM SMBus controller".into());
        }
    }
    let mut buses = Vec::new();
    for entry in fs::read_dir(controller)? {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(number) = name
            .to_str()
            .and_then(|name| name.strip_prefix("i2c-"))
            .and_then(|n| n.parse().ok())
            && fs::read_to_string(entry.path().join("name"))?.trim()
                == "SMBus PIIX4 adapter port 0 at 0b00"
        {
            buses.push(number);
        }
    }
    let [bus] = buses.as_slice() else {
        return Err("Expected exactly one matching RAM SMBus adapter.".into());
    };
    Ok(*bus)
}
