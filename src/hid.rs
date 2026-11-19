//! A port identifies a row. Only the exact kernel HID instance identifies an open request.
use std::{
    fs, io,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt},
    },
    path::{Path, PathBuf},
};

pub fn node(instance: &Path) -> io::Result<PathBuf> {
    let mut nodes = fs::read_dir(instance.join("hidraw"))?;
    let first = nodes
        .next()
        .transpose()?
        .ok_or_else(|| io::Error::other("Missing hidraw node"))?;
    if nodes.next().is_some() {
        return Err(io::Error::other("Ambiguous hidraw nodes"));
    }
    Ok(first.path())
}

fn verify_instance(expected: &Path, actual: &Path) -> io::Result<()> {
    if actual.canonicalize()? != expected {
        return Err(io::Error::other(
            "HID attachment changed; refresh inventory",
        ));
    }
    Ok(())
}

/// All checks refer to the opened fd or its exact (counter-bearing) kernel instance.
/// Linux disconnects an existing hidraw fd; never reopen it to retry a failed write.
pub fn open(instance: &Path, id: &str, read: bool) -> io::Result<fs::File> {
    let raw = node(instance)?;
    let name = raw
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid hidraw path"))?;
    let file = fs::OpenOptions::new()
        .read(read)
        .write(true)
        .open(Path::new("/dev").join(name))?;
    let metadata = file.metadata()?;
    let numbers = format!(
        "{}:{}",
        libc::major(metadata.rdev()),
        libc::minor(metadata.rdev())
    );
    if !metadata.file_type().is_char_device()
        || fs::read_to_string(raw.join("dev"))?.trim() != numbers
    {
        return Err(io::Error::other("hidraw device number changed"));
    }
    verify_instance(
        instance,
        &Path::new("/sys/dev/char").join(numbers).join("device"),
    )?;
    // Linux HIDIOCGRAWINFO (_IOR('H', 3, struct hidraw_devinfo)), on our x86_64 target.
    let mut info = HidrawInfo::default();
    if unsafe { libc::ioctl(file.as_raw_fd(), 0x80084803, &mut info) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if !matches_hid(info, id) {
        return Err(io::Error::other("HID bus, vendor or product changed"));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hid_fd_must_match_usb_bus_vendor_and_product() {
        let raw = || HidrawInfo {
            bus: 3,
            vendor: 0x0b05,
            product: 0x1939,
        };
        assert!(matches_hid(raw(), "HID_ID=0003:00000B05:00001939"));
        assert!(!matches_hid(raw(), "HID_ID=0003:00000B05:000018F3"));
    }
    #[test]
    fn reused_node_cannot_route_to_new_same_model_instance() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(gtk::glib::uuid_string_random());
        let old = root.join("0003:0B05:1939.0001");
        let new = root.join("0003:0B05:1939.0002");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        let link = root.join("device");
        symlink(&old, &link).unwrap();
        verify_instance(&old, &link).unwrap();
        fs::remove_file(&link).unwrap();
        symlink(&new, &link).unwrap();
        let mut writes = 0;
        assert!(verify_instance(&old, &link).map(|_| writes += 1).is_err());
        assert_eq!(writes, 0);
        fs::remove_dir_all(root).unwrap();
    }
}
