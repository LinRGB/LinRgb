use std::{
    fs, io,
    num::NonZeroU16,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    time::Duration,
};

const FRACTAL_DESCRIPTOR: &[u8] = include_bytes!("../fractal-adjust-pro-wdl.hid");

const LIGHTING_PAGE: u32 = 0x59;
const LAMP_ARRAY: u32 = (LIGHTING_PAGE << 16) | 1;

fn is_lamp_array(mut descriptor: &[u8]) -> bool {
    let mut page = 0;
    let mut pages = Vec::new();
    let mut lamp_usage = false;
    let mut depth = 0u32;
    while let Some((&prefix, rest)) = descriptor.split_first() {
        if prefix == 0xfe {
            let Some(payload) = rest
                .first()
                .and_then(|length| rest.get(2 + usize::from(*length)..))
            else {
                return false;
            };
            descriptor = payload;
            continue;
        }
        let size = [0, 1, 2, 4][usize::from(prefix & 3)];
        let Some(data) = rest.get(..size) else {
            return false;
        };
        let mut bytes = [0; 4];
        bytes[..size].copy_from_slice(data);
        let value = u32::from_le_bytes(bytes);
        match prefix & 0xfc {
            0x04 => page = value,
            0xa4 => pages.push(page),
            0xb4 => {
                let Some(previous) = pages.pop() else {
                    return false;
                };
                page = previous;
            }
            0x08 => {
                lamp_usage = (if size == 4 {
                    value
                } else {
                    (page << 16) | value
                }) == LAMP_ARRAY
            }
            0xa0 if depth == 0 && value == 1 && lamp_usage => return true,
            0xa0 => depth += 1,
            0xc0 => depth = depth.saturating_sub(1),
            _ => {}
        }
        if prefix & 0x0c == 0 {
            lamp_usage = false;
        }
        descriptor = &rest[size..];
    }
    false
}

pub fn detect(root: &Path) -> io::Result<Vec<(String, Option<PathBuf>)>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let Ok(descriptor) = fs::read(path.join("report_descriptor")) else {
            continue;
        };
        if !is_lamp_array(&descriptor) {
            continue;
        }
        let id = entry.file_name();
        let details = fs::read_to_string(path.join("uevent")).unwrap_or_default();
        let name = details
            .lines()
            .find_map(|line| line.strip_prefix("HID_NAME="))
            .unwrap_or("LampArray");
        let supported = descriptor == FRACTAL_DESCRIPTOR
            && details
                .lines()
                .any(|line| line == "HID_ID=0003:000036BC:00001001");
        let name = if supported {
            name.to_owned()
        } else {
            format!("{name} · {}", id.to_string_lossy())
        };
        devices.push((name, supported.then_some(path)));
    }
    devices.sort();
    Ok(devices)
}

// The Fractal descriptor fixes the IDs and field layout of the standard HID feature reports.
fn range_report(lamp_count: u16, [r, g, b]: [u8; 3]) -> [u8; 10] {
    let [lo, hi] = (lamp_count - 1).to_le_bytes();
    [7, 1, 0, 0, lo, hi, r, g, b, 255]
}

fn feature(file: &fs::File, number: libc::c_ulong, report: &mut [u8]) -> io::Result<()> {
    // Linux generic hidraw ioctl encoding (_IOWR); the buffer stays live for the call.
    let request = (3 << 30)
        | ((report.len() as libc::c_ulong) << 16)
        | (libc::c_ulong::from(b'H') << 8)
        | number;
    let result = unsafe { libc::ioctl(file.as_raw_fd(), request as _, report.as_mut_ptr()) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if result as usize != report.len() {
        return Err(io::Error::other("Incomplete LampArray feature report"));
    }
    Ok(())
}

pub(super) fn open<'a>(
    path: &Path,
    stop: &'a std::sync::atomic::AtomicBool,
) -> io::Result<(
    super::Capabilities,
    impl FnMut([u8; 3]) -> io::Result<()> + use<'a>,
)> {
    let details = fs::read_to_string(path.join("uevent"))?;
    if !details
        .lines()
        .any(|line| line == "HID_ID=0003:000036BC:00001001")
        || fs::read(path.join("report_descriptor"))? != FRACTAL_DESCRIPTOR
    {
        return Err(io::Error::other("Unsupported LampArray report layout"));
    }
    let file = crate::hid::open(path, "HID_ID=0003:000036BC:00001001", true)?;
    crate::hid::verify_descriptor(&file, FRACTAL_DESCRIPTOR)?;
    bind_reports(stop, move |number, report| feature(&file, number, report))
}

// Called only after fd identity and descriptor checks. GET attributes is read-only;
// mode change and color data belong to the first explicit operation, not connect.
fn bind_reports<F: FnMut(libc::c_ulong, &mut [u8]) -> io::Result<()>>(
    stop: &std::sync::atomic::AtomicBool,
    mut transfer: F,
) -> io::Result<(super::Capabilities, impl FnMut([u8; 3]) -> io::Result<()>)> {
    let mut attributes = [0; 23];
    attributes[0] = 3;
    transfer(7, &mut attributes)?;
    let (lamps, min_interval) = attributes_caps(&attributes)?;
    let caps = super::Capabilities::HidRange {
        lamps,
        min_interval,
    };
    let mut host_control = false;
    Ok((caps, move |rgb| {
        if stop.load(std::sync::atomic::Ordering::Acquire) {
            return Err(io::Error::other("RGB operation cancelled"));
        }
        if !host_control {
            transfer(6, &mut [8, 0])?;
            host_control = true;
        } // Switch from autonomous lighting to host control.
        if stop.load(std::sync::atomic::Ordering::Acquire) {
            return Err(io::Error::other("RGB operation cancelled"));
        }
        transfer(6, &mut range_report(lamps.get(), rgb))
    }))
}

fn attributes_caps(attributes: &[u8; 23]) -> io::Result<(NonZeroU16, Duration)> {
    let lamps = NonZeroU16::new(u16::from_le_bytes([attributes[1], attributes[2]]))
        .ok_or_else(|| io::Error::other("Empty LampArray"))?;
    if attributes[0] != 3 {
        return Err(io::Error::other("Invalid LampArray attributes"));
    }
    let interval =
        Duration::from_micros(u32::from_le_bytes(attributes[19..23].try_into().unwrap()).into());
    if interval > Duration::from_secs(1) {
        return Err(io::Error::other("Unexpected LampArray update interval"));
    }
    Ok((lamps, interval))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_only_reads_attributes_and_first_color_takes_control() {
        use std::cell::RefCell;
        let stop = std::sync::atomic::AtomicBool::new(false);
        let calls = RefCell::new(Vec::new());
        let (caps, mut write) = bind_reports(&stop, |op, buf| {
            calls.borrow_mut().push((op, buf[0]));
            if op == 7 {
                buf[1] = 76;
                buf[19..23].copy_from_slice(&12500u32.to_le_bytes());
            }
            Ok(())
        })
        .unwrap();
        assert!(
            matches!(caps,super::super::Capabilities::HidRange { lamps, min_interval } if lamps.get()==76 && min_interval==Duration::from_micros(12500))
        );
        assert_eq!(*calls.borrow(), [(7, 3)]);
        write([1, 2, 3]).unwrap();
        write([3, 2, 1]).unwrap();
        assert_eq!(*calls.borrow(), [(7, 3), (6, 8), (6, 7), (6, 7)]);
    }
    #[test]
    fn malformed_attributes_never_switch_mode_or_write_color() {
        let stop = std::sync::atomic::AtomicBool::new(false);
        for (id, count, interval) in [(1, 1, 0u32), (3, 0, 0), (3, 1, 1_000_001)] {
            assert!(
                bind_reports(&stop, |op, buf| {
                    assert_eq!(op, 7);
                    buf[0] = id;
                    buf[1] = count;
                    buf[19..23].copy_from_slice(&interval.to_le_bytes());
                    Ok(())
                })
                .is_err()
            );
        }
    }
    #[test]
    fn fractal_range_report_updates_every_lamp_and_commits() {
        assert!(is_lamp_array(FRACTAL_DESCRIPTOR));
        assert_eq!(
            range_report(76, [10, 20, 30]),
            [7, 1, 0, 0, 75, 0, 10, 20, 30, 255]
        );
        assert_eq!(&range_report(258, [0; 3])[2..6], &[0, 0, 1, 1]);
    }

    #[test]
    fn recognizes_top_level_lamp_array_applications_only() {
        assert!(is_lamp_array(&[0x05, 0x59, 0x09, 1, 0xa1, 1, 0xc0]));
        assert!(is_lamp_array(&[0x0b, 1, 0, 0x59, 0, 0xa1, 1, 0xc0]));
        assert!(!is_lamp_array(&[0x05, 0x01, 0x09, 1, 0xa1, 1, 0xc0]));
        assert!(!is_lamp_array(&[0x05, 0x59, 0x09, 1, 0xa1, 0, 0xc0]));
        assert!(!is_lamp_array(&[0x05, 0x59, 0x09, 1, 0xa1]));
    }
}
