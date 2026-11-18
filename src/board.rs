use std::{error::Error, fs, io::Write};

type BoardResult<T> = Result<T, Box<dyn Error>>;

fn report(rgb: [u8; 3]) -> [u8; 65] {
    let mut report = [0; 65];
    report[..5].copy_from_slice(&[0xec, 0x40, 0x84, 0, 8]);
    for color in report[5..29].as_chunks_mut::<3>().0 {
        color.copy_from_slice(&rgb);
    }
    report
}

pub fn check_board() -> Result<(), Box<dyn Error>> {
    if fs::read_to_string("/sys/class/dmi/id/board_name")?.trim()
        != "ROG CROSSHAIR VIII HERO (WI-FI)"
    {
        return Err("Unsupported motherboard.".into());
    }
    Ok(())
}

pub fn open() -> BoardResult<impl FnMut([u8; 3]) -> BoardResult<()>> {
    check_board()?;
    let mut paths = Vec::new();
    for entry in fs::read_dir("/sys/class/hidraw")? {
        let entry = entry?;
        if fs::read_to_string(entry.path().join("device/uevent"))?
            .lines()
            .any(|line| line == "HID_ID=0003:00000B05:000018F3")
        {
            paths.push(entry.file_name());
        }
    }
    let [path] = paths.as_slice() else {
        return Err("Expected exactly one Aura USB controller.".into());
    };
    let mut hid = fs::OpenOptions::new()
        .write(true)
        .open(std::path::Path::new("/dev").join(path))?;
    Ok(move |rgb| {
        if hid.write(&report(rgb))? != 65 {
            return Err("Incomplete Aura report.".into());
        }
        Ok(())
    })
}

pub fn open_ram(address: u16) -> Result<crate::astral::Ene, Box<dyn Error>> {
    check_board()?;
    let mut buses = Vec::new();
    for entry in fs::read_dir("/sys/bus/pci/devices/0000:00:14.0")? {
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
    crate::astral::open_ene(*bus, address, b"DIMM_LED-0103", 0x8000, Some(5))
}

#[cfg(test)]
mod tests {
    #[test]
    fn board_report_targets_fixed_zone_only() {
        let report = super::report([1, 2, 3]);
        assert_eq!(&report[..5], &[0xec, 0x40, 0x84, 0, 8]);
        assert_eq!(&report[5..29], &[1, 2, 3].repeat(8));
        assert!(report[29..].iter().all(|&byte| byte == 0));
    }
}
