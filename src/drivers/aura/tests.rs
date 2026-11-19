use super::*;
use crate::drivers::{Generation, Session};

// Synthetic sysfs and a regular file as a packet sink. No /dev access.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(gtk::glib::uuid_string_random());
        fs::create_dir_all(root.join("usb/hid")).unwrap();
        fs::write(root.join("usb/bInterfaceNumber"), "02\n").unwrap();
        fs::write(root.join("usb/hid/uevent"), Profile::Strix.hid()).unwrap();
        fs::write(root.join("usb/hid/report_descriptor"), DESCRIPTOR).unwrap();
        fs::write(root.join("hidraw7"), []).unwrap();
        Self(root)
    }
    fn attachment(&self) -> Attachment {
        let instance = self.0.join("usb/hid");
        Attachment {
            inode: instance.metadata().unwrap().ino(),
            instance,
            profile: Profile::Strix,
        }
    }
    fn open(&self, a: &Attachment) -> Result<fs::File> {
        a.verify(B550)?;
        Ok(fs::OpenOptions::new()
            .append(true)
            .open(self.0.join("hidraw7"))?)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn two_zone_sessions_share_one_locked_fd_without_initialization_writes() {
    let f = Fixture::new();
    let a = f.attachment();
    let mut ports = Connections::default();
    let stop = AtomicBool::new(false);
    let ticket = Generation::default().ticket();
    let mut fixed = Session::connect(&stop, &ticket, || {
        ports.bind_with(a.clone(), Zone::Fixed, &stop, |a| f.open(a))
    })
    .unwrap();
    let mut header = Session::connect(&stop, &ticket, || {
        ports.bind_with(a.clone(), Zone::Header1, &stop, |_| panic!("second open"))
    })
    .unwrap();
    assert!(matches!(
        fixed.capabilities(),
        Capabilities::Aura(Layout::Fixed2)
    ));
    assert!(matches!(
        header.capabilities(),
        Capabilities::Aura(Layout::Header100)
    ));
    let path = f.0.join("hidraw7");
    assert!(fs::read(&path).unwrap().is_empty());
    let mut competing = Connections::default();
    assert!(
        competing
            .bind_with(a.clone(), Zone::Fixed, &stop, |a| f.open(a))
            .is_err()
    );
    let rgb = [1, 2, 3];
    fixed.set_rgb(rgb).unwrap();
    header.set_rgb(rgb).unwrap();
    header.set_rgb(rgb).unwrap();
    let data = fs::read(&path).unwrap();
    let (packets, rest) = data.as_chunks::<65>();
    assert!(rest.is_empty());
    assert_eq!(packets.len(), 12);
    assert_eq!(
        &packets[0][..11],
        &[0xec, 0x40, 0x84, 0, 2, 1, 2, 3, 1, 2, 3]
    );
    assert!(packets[0][11..].iter().all(|&b| b == 0));
    assert_eq!(&packets[1][..5], &[0xec, 0x52, 0x53, 0, 1]);
    assert!(packets[1][5..].iter().all(|&b| b == 0));
    for (i, p) in packets[2..].iter().enumerate() {
        let slot = i % 5;
        assert_eq!(
            &p[..5],
            &[
                0xec,
                0x40,
                if slot == 4 { 0x80 } else { 0 },
                (slot * 20) as u8,
                20
            ]
        );
        assert_eq!(&p[5..], rgb.repeat(20));
    }
    drop((ports, fixed, header));
    assert!(
        competing
            .bind_with(a.clone(), Zone::Fixed, &stop, |_| panic!(
                "retry after lock failure"
            ))
            .is_err()
    );
    assert!(
        Connections::default()
            .bind_with(a, Zone::Fixed, &stop, |a| f.open(a))
            .is_ok()
    );
}
#[test]
fn crosshair_exposes_only_eight_fixed_positions() {
    assert_eq!(
        Profile::Crosshair.layout(Zone::Fixed).unwrap(),
        Layout::Fixed8
    );
    assert!(Profile::Crosshair.layout(Zone::Header1).is_err());
    let f = Fixture::new();
    let mut a = f.attachment();
    a.profile = Profile::Crosshair;
    fs::write(a.instance.join("uevent"), a.profile.hid()).unwrap();
    let mut ports = Connections::default();
    let stop = AtomicBool::new(false);
    assert!(
        ports
            .bind_with(a.clone(), Zone::Header1, &stop, |_| panic!(
                "invalid zone opened"
            ))
            .is_err()
    );
    let (_, mut write) = ports
        .bind_with(a, Zone::Fixed, &stop, |a| {
            a.verify(CROSSHAIR)?;
            Ok(fs::OpenOptions::new()
                .append(true)
                .open(f.0.join("hidraw7"))?)
        })
        .unwrap();
    write([1, 2, 3]).unwrap();
    let p = fs::read(f.0.join("hidraw7")).unwrap();
    assert_eq!(p.len(), 65);
    assert_eq!(&p[..5], &[0xec, 0x40, 0x84, 0, 8]);
    assert_eq!(&p[5..29], [1, 2, 3].repeat(8));
    assert!(p[29..].iter().all(|&b| b == 0));
}
#[test]
fn wrong_model_product_interface_or_descriptor_cannot_bind() {
    let f = Fixture::new();
    let a = f.attachment();
    assert!(a.verify(B550).is_ok());
    assert!(a.verify(CROSSHAIR).is_err());
    for (path, wrong) in [
        ("usb/hid/uevent", Profile::Crosshair.hid().as_bytes()),
        ("usb/bInterfaceNumber", b"01"),
        ("usb/hid/report_descriptor", &DESCRIPTOR[..35]),
    ] {
        let p = f.0.join(path);
        let expected = fs::read(&p).unwrap();
        fs::write(&p, wrong).unwrap();
        assert!(a.verify(B550).is_err());
        fs::write(p, expected).unwrap();
    }
}
#[test]
fn identical_replacement_at_same_path_needs_a_new_attachment_and_apply() {
    let f = Fixture::new();
    let old = f.attachment();
    let mut ports = Connections::default();
    let stop = AtomicBool::new(false);
    let gen_id = Generation::default();
    let ticket = gen_id.ticket();
    let mut session = Session::connect(&stop, &ticket, || {
        ports.bind_with(old.clone(), Zone::Fixed, &stop, |a| f.open(a))
    })
    .unwrap();
    session.set_rgb([1; 3]).unwrap();
    fs::rename(f.0.join("hidraw7"), f.0.join("detached")).unwrap();
    fs::write(f.0.join("hidraw7"), []).unwrap();
    fs::rename(&old.instance, f.0.join("usb/old")).unwrap();
    fs::create_dir(&old.instance).unwrap();
    for name in ["uevent", "report_descriptor"] {
        fs::copy(f.0.join("usb/old").join(name), old.instance.join(name)).unwrap();
    }
    let new = f.attachment();
    assert_ne!(old, new);
    assert!(old.verify(B550).is_err());
    assert!(new.verify(B550).is_ok());
    // Even before the topology event arrives, the old writer cannot resolve the replacement.
    session.set_rgb([2; 3]).unwrap();
    assert_eq!(fs::metadata(f.0.join("detached")).unwrap().len(), 130);
    gen_id.invalidate();
    assert!(Session::connect(&stop, &ticket, || panic!("stale apply")).is_err());
    assert!(
        ports
            .bind_with(new, Zone::Fixed, &stop, |_| panic!("rebind"))
            .is_err()
    );
    stop.store(true, Ordering::Release);
    assert!(session.set_rgb([3; 3]).is_err());
    assert!(fs::read(f.0.join("hidraw7")).unwrap().is_empty());
}

struct Mock<F>(F);
impl<F: FnMut(&[u8]) -> io::Result<usize>> Write for Mock<F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn partial_write_disconnect_or_cancel_between_packets_revokes_both_zones() {
    use std::cell::Cell;
    for fault in ["short", "disconnect", "cancel"] {
        let calls = Cell::new(0);
        let stop = AtomicBool::new(false);
        let port = Rc::new(RefCell::new(Some(Mock(|buf: &[u8]| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                match fault {
                    "short" => return Ok(4),
                    "disconnect" => return Err(io::Error::from_raw_os_error(libc::ENODEV)),
                    _ => stop.store(true, Ordering::Release),
                }
            }
            Ok(buf.len())
        }))));
        let mut header = writer(port.clone(), Layout::Header100, &stop);
        let mut fixed = writer(port.clone(), Layout::Fixed2, &stop);
        assert!(header([1; 3]).is_err());
        assert!(port.borrow().is_none());
        assert!(fixed([2; 3]).is_err());
        assert!(header([3; 3]).is_err());
        assert_eq!(calls.get(), 3);
    }
}
