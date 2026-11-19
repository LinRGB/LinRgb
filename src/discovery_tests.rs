use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("linrgb-{}", gtk::glib::uuid_string_random()));
        for sub in [
            "sys/bus/pci/devices",
            "sys/bus/hid/devices",
            "sys/class/hidraw",
            "sys/class/leds",
            "dev",
        ] {
            fs::create_dir_all(root.join(sub)).unwrap();
        }
        Self(root)
    }
    fn put(&self, name: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn scan(&self) -> Inventory {
        scan(&self.0.join("sys"), &self.0.join("dev"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn board_name_alone_cannot_create_rgb_controls() {
    let f = Fixture::new();
    f.put("sys/class/dmi/id/board_name", board::B550);
    f.put(
        "sys/class/hidraw/hidraw7/device/uevent",
        "HID_ID=0003:00000B05:000018F3",
    );
    assert!(f.scan().devices.is_empty());
    f.put(
        "sys/class/hidraw/hidraw7/device/uevent",
        "HID_ID=0003:00000B05:00001939",
    );
    let snapshot = f.scan();
    assert_eq!(snapshot.devices.len(), 2);
    assert!(snapshot.devices.iter().all(|d| d.access == Access::Missing));
    f.put("dev/hidraw7", b"untouched");
    assert!(f.scan().devices.iter().all(|d| d.bulk_selectable()));
    assert_eq!(fs::read(f.0.join("dev/hidraw7")).unwrap(), b"untouched");
    f.put(
        "sys/class/hidraw/hidraw8/device/uevent",
        "HID_ID=0003:00000B05:00001939",
    );
    assert!(f.scan().devices.is_empty()); // ambiguous
}

#[test]
fn smbus_requires_exact_parent_and_one_adapter_and_slots_need_opt_in() {
    let f = Fixture::new();
    f.put("sys/class/dmi/id/board_name", board::B550);
    let pci = "sys/bus/pci/devices/0000:00:14.0";
    for (attr, id) in [
        ("vendor", "0x1022"),
        ("device", "0x790b"),
        ("subsystem_vendor", "0x1043"),
        ("subsystem_device", "0x0000"),
    ] {
        f.put(&format!("{pci}/{attr}"), id);
    }
    f.put(
        &format!("{pci}/i2c-42/name"),
        "SMBus PIIX4 adapter port 0 at 0b00\n",
    );
    f.put("dev/i2c-42", b"untouched");
    assert!(f.scan().devices.is_empty());
    f.put(&format!("{pci}/subsystem_device"), "0x87c0");
    let inv = f.scan();
    assert_eq!(inv.devices.len(), 2);
    assert!(
        inv.devices
            .iter()
            .all(|d| d.access == Access::Available && !d.bulk_selectable())
    );
    assert_eq!(fs::read(f.0.join("dev/i2c-42")).unwrap(), b"untouched");
    f.put(
        &format!("{pci}/i2c-43/name"),
        "SMBus PIIX4 adapter port 0 at 0b00",
    );
    assert!(f.scan().devices.is_empty());
}

#[test]
fn aliases_share_one_led_and_removal_clears_inventory() {
    let f = Fixture::new();
    let dir = f.0.join("sys/class/leds");
    f.put("sys/devices/private-led-name/multi_index", "green red blue");
    f.put("sys/devices/private-led-name/multi_intensity", "0 0 0");
    f.put("sys/devices/private-led-name/brightness", "0");
    f.put("sys/devices/private-led-name/max_brightness", "255");
    for name in ["alias", "private-led-name"] {
        symlink("../../devices/private-led-name", dir.join(name)).unwrap();
    }
    let inv = f.scan();
    assert_eq!(inv.devices.len(), 1);
    let json = inv.diagnostics().to_string();
    assert!(
        json.contains("linux-multicolor")
            && !json.contains("private-led-name")
            && !json.contains("alias")
    );
    let p = f.0.join("sys/devices/private-led-name/multi_intensity");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o444)).unwrap();
    // A root CI worker can bypass ordinary file permissions.
    if unsafe { libc::geteuid() } != 0 {
        assert_eq!(f.scan().devices[0].access, Access::Denied);
    }
    fs::remove_file(p).unwrap();
    assert_eq!(f.scan().devices[0].access, Access::Missing);
    fs::remove_file(dir.join("alias")).unwrap();
    fs::remove_file(dir.join("private-led-name")).unwrap();
    assert!(f.scan().devices.is_empty());
}

#[test]
fn changing_hid_instance_preserves_port_identity_but_not_transport_path() {
    let f = Fixture::new();
    let mut old = None;
    for instance in [1, 2] {
        let sub = format!("sys/devices/usb/1-2:1.0/0003:36BC:1001.{instance:04}");
        let hid = f.0.join(&sub);
        let alias =
            f.0.join("sys/bus/hid/devices")
                .join(hid.file_name().unwrap());
        f.put(
            &format!("{sub}/uevent"),
            "HID_ID=0003:000036BC:00001001\nHID_NAME=Private Test Hub\nHID_UNIQ=private-serial\n",
        );
        f.put(
            &format!("{sub}/report_descriptor"),
            include_bytes!("fractal-adjust-pro-wdl.hid"),
        );
        fs::create_dir_all(hid.join("hidraw/hidraw1")).unwrap();
        symlink(&hid, &alias).unwrap();
        let led = f.0.join("sys/class/leds/alias");
        for (attr, value) in [
            ("multi_index", "red green blue"),
            ("brightness", "0"),
            ("multi_intensity", "0 0 0"),
        ] {
            f.put(&format!("{sub}/leds/alias/{attr}"), value);
        }
        symlink(hid.join("leds/alias"), &led).unwrap();
        f.put("dev/hidraw1", b"untouched");
        let inv = f.scan();
        assert_eq!(inv.devices.len(), 1);
        assert!(!inv.diagnostics().to_string().contains("Private"));
        let d = inv.devices.into_iter().next().unwrap();
        if let Some(previous) = old.as_ref() {
            assert!(!d.can_restore_from(previous));
            assert!(d.key == previous.key && d.target != previous.target);
        }
        assert!(matches!(d.target, Target::LampArray(_))); // no second writer via LED class
        assert!(f.scan().devices[0].can_restore_from(&d));
        old = Some(d);
        fs::remove_file(led).unwrap();
        fs::remove_file(alias).unwrap();
        assert!(f.scan().devices.is_empty());
    }
}
