use std::{
    error::Error,
    fs,
    io::Write,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

type BoardResult<T> = Result<T, Box<dyn Error>>;

fn report(rgb: [u8; 3], count: u8) -> [u8; 65] {
    direct_report(rgb, 0x84, 0, count)
}

fn direct_report(rgb: [u8; 3], channel: u8, offset: u8, count: u8) -> [u8; 65] {
    let mut report = [0; 65];
    report[..5].copy_from_slice(&[0xec, 0x40, channel, offset, count]);
    for color in report[5..5 + usize::from(count) * 3].as_chunks_mut::<3>().0 {
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

pub const B550: &str = "ROG STRIX B550-A GAMING";

pub fn fixed_zone(model: &str) -> Option<(&'static str, u8)> {
    match model {
        "ROG CROSSHAIR VIII HERO (WI-FI)" => Some(("HID_ID=0003:00000B05:000018F3", 8)),
        B550 => Some(("HID_ID=0003:00000B05:00001939", 2)),
        _ => None,
    }
}

pub fn b550_present() -> bool {
    fs::read_to_string("/sys/class/dmi/id/board_name").is_ok_and(|name| name.trim() == B550)
}

pub fn open() -> BoardResult<impl FnMut([u8; 3]) -> BoardResult<()>> {
    let model = fs::read_to_string("/sys/class/dmi/id/board_name")?;
    let (hid_id, count) = fixed_zone(model.trim()).ok_or("Unsupported motherboard.")?;
    let mut hid = open_hid(hid_id)?;
    Ok(move |rgb| send(&mut hid, &report(rgb, count)))
}

fn b550_argb_reports(rgb: [u8; 3]) -> [[u8; 65]; 5] {
    // shortcut: use the reported 100-LED layout until Aura zone sizing is configurable.
    std::array::from_fn(|i| direct_report(rgb, if i == 4 { 0x80 } else { 0 }, (i * 20) as u8, 20))
}

pub fn open_argb() -> BoardResult<impl FnMut([u8; 3]) -> BoardResult<()>> {
    if !b550_present() {
        return Err("Unsupported motherboard for Aura ARGB.".into());
    }
    let (hid_id, _) = fixed_zone(B550).ok_or("Missing B550 Aura configuration.")?;
    let mut hid = open_hid(hid_id)?;
    let mut gen1 = [0; 65];
    gen1[..5].copy_from_slice(&[0xec, 0x52, 0x53, 0, 1]);
    send(&mut hid, &gen1)?;
    Ok(move |rgb| {
        for report in b550_argb_reports(rgb) {
            send(&mut hid, &report)?;
        }
        Ok(())
    })
}

pub fn hid_node(root: &Path, hid_id: &str) -> BoardResult<PathBuf> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let details = match fs::read_to_string(entry.path().join("device/uevent")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            result => result?,
        };
        if details.lines().any(|line| line == hid_id) {
            paths.push(entry.file_name());
        }
    }
    let [path] = paths.as_slice() else {
        return Err("Expected exactly one Aura USB controller.".into());
    };
    Ok(PathBuf::from(path))
}

fn open_hid(hid_id: &str) -> BoardResult<fs::File> {
    let path = hid_node(Path::new("/sys/class/hidraw"), hid_id)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .open(Path::new("/dev").join(path))?;
    // Linux HIDIOCGRAWINFO (_IOR('H', 3, struct hidraw_devinfo)), on our x86_64 target.
    // Check the open fd, not just the recyclable /dev/hidrawN name.
    let mut raw = HidrawInfo::default();
    if unsafe { libc::ioctl(file.as_raw_fd(), 0x80084803, &mut raw) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if !matches_hid(raw, hid_id) {
        return Err("Aura controller changed before opening.".into());
    }
    Ok(file)
}

#[repr(C)]
#[derive(Default)]
struct HidrawInfo {
    bus: u32,
    vendor: u16,
    product: u16,
}
const _: () = assert!(std::mem::size_of::<HidrawInfo>() == 8);

fn matches_hid(raw: HidrawInfo, id: &str) -> bool {
    format!(
        "HID_ID={:04X}:{:08X}:{:08X}",
        raw.bus, raw.vendor, raw.product
    ) == id
}

fn send(hid: &mut fs::File, report: &[u8; 65]) -> BoardResult<()> {
    if hid.write(report)? != report.len() {
        return Err("Incomplete Aura report.".into());
    }
    Ok(())
}

pub fn open_ram(address: u16) -> Result<crate::astral::Ene, Box<dyn Error>> {
    check_board()?;
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

#[cfg(test)]
mod tests {
    #[test]
    fn hid_fd_must_match_usb_bus_vendor_and_product() {
        let raw = || super::HidrawInfo {
            bus: 3,
            vendor: 0x0b05,
            product: 0x1939,
        };
        assert!(super::matches_hid(raw(), "HID_ID=0003:00000B05:00001939"));
        assert!(!super::matches_hid(raw(), "HID_ID=0003:00000B05:000018F3"));
    }

    #[test]
    fn board_report_targets_fixed_zone_only() {
        let report = super::report([1, 2, 3], 8);
        assert_eq!(&report[..5], &[0xec, 0x40, 0x84, 0, 8]);
        assert_eq!(&report[5..29], &[1, 2, 3].repeat(8));
        assert!(report[29..].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn b550_argb_commits_only_the_last_packet() {
        let reports = super::b550_argb_reports([1, 2, 3]);
        for (i, packet) in reports.iter().enumerate() {
            assert_eq!(
                &packet[..5],
                &[
                    0xec,
                    0x40,
                    if i == 4 { 0x80 } else { 0 },
                    (i * 20) as u8,
                    20
                ]
            );
            assert_eq!(&packet[5..], &[1, 2, 3].repeat(20));
        }
    }

    #[test]
    fn b550_report_targets_only_the_two_fixed_headers() {
        let (id, count) = super::fixed_zone("ROG STRIX B550-A GAMING").unwrap();
        assert_eq!(id, "HID_ID=0003:00000B05:00001939");
        let report = super::report([1, 2, 3], count);
        assert_eq!(&report[..11], &[0xec, 0x40, 0x84, 0, 2, 1, 2, 3, 1, 2, 3]);
        assert!(report[11..].iter().all(|&byte| byte == 0));
    }
}
