use super::*;
use crate::drivers::{Generation, Session};
use std::cell::RefCell;

struct Mock<'a> {
    reads: &'a RefCell<Vec<u8>>,
    sig: [u8; 2],
    info: Option<([u8; 32], u8)>,
    frames: &'a RefCell<Vec<[u8; 32]>>,
    disconnect: bool,
}
impl Transport for Mock<'_> {
    fn signature_byte(&mut self, reg: u8) -> Result<u8> {
        self.reads.borrow_mut().push(reg);
        Ok(self.sig[match reg {
            0x43 => 0,
            0x44 => 1,
            _ => panic!("unexpected register"),
        }])
    }
    fn readonly_info(&mut self) -> Result<([u8; 32], u8)> {
        self.info
            .take()
            .ok_or_else(|| "no read-only model evidence".into())
    }
    fn direct(&mut self, frame: &[u8; 32]) -> Result<()> {
        self.frames.borrow_mut().push(*frame);
        if self.disconnect {
            return Err(io::Error::from_raw_os_error(libc::ENXIO).into());
        }
        Ok(())
    }
}
fn info() -> [u8; 32] {
    let mut i = [0; 32];
    i[..4].copy_from_slice(&[0x1c, 0x1b, 1, 3]);
    i[28] = 4;
    i
}
#[test]
fn model_evidence_is_mandatory_and_family_reads_short_circuit() {
    for (sig, expected) in [
        ([0, 3], &[0x43][..]),
        ([0x1b, 0], &[0x43, 0x44]),
        ([0x1b, 3], &[0x43, 0x44]),
    ] {
        let reads = RefCell::default();
        let frames = RefCell::default();
        let t = Generation::default().ticket();
        let stop = AtomicBool::new(false);
        assert!(
            bind(
                Mock {
                    reads: &reads,
                    sig,
                    info: None,
                    frames: &frames,
                    disconnect: false
                },
                &stop,
                &t
            )
            .is_err()
        );
        assert_eq!(reads.into_inner(), expected);
        assert!(frames.into_inner().is_empty());
    }
}
#[test]
fn invalid_vid_pid_protocol_or_crc_never_yields_a_writer() {
    for offset in [0, 2, 28, 32] {
        let mut i = info();
        if offset < 32 {
            i[offset] = 0xff;
            if offset == 28 {
                i[offset] = 3;
            }
        }
        let sum = crc(&i) ^ u8::from(offset == 32);
        let reads = RefCell::default();
        let frames = RefCell::default();
        let t = Generation::default().ticket();
        assert!(
            bind(
                Mock {
                    reads: &reads,
                    sig: [0x1a, 1],
                    info: Some((i, sum)),
                    frames: &frames,
                    disconnect: false
                },
                &AtomicBool::new(false),
                &t
            )
            .is_err()
        );
        assert!(frames.into_inner().is_empty());
    }
}
#[test]
fn synthetic_verified_module_writes_only_on_apply_and_does_not_retry_errors() {
    assert_eq!(crc(b"123456789"), 0xf4);
    for disconnect in [false, true] {
        let reads = RefCell::default();
        let frames = RefCell::default();
        let t = Generation::default().ticket();
        let stop = AtomicBool::new(false);
        let i = info();
        let mut s = Session::connect(&stop, &t, || {
            bind(
                Mock {
                    reads: &reads,
                    sig: [0x1b, 4],
                    info: Some((i, crc(&i))),
                    frames: &frames,
                    disconnect,
                },
                &stop,
                &t,
            )
        })
        .unwrap();
        assert!(frames.borrow().is_empty());
        assert!(matches!(
            s.capabilities(),
            Capabilities::Corsair { protocol: 4 }
        ));
        assert_eq!(s.set_rgb([1, 2, 3]).is_err(), disconnect);
        stop.store(true, Ordering::Release);
        assert!(s.set_rgb([0; 3]).is_err());
        drop(s);
        let data = frames.into_inner();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0][0], 10);
        assert_eq!(&data[0][1..31], [1, 2, 3].repeat(10));
        assert_eq!(crc(&data[0]), 0);
    }
}
struct FamilyOnly<F>(F);
impl<F: FnMut(u8) -> Result<u8>> Transport for FamilyOnly<F> {
    fn signature_byte(&mut self, reg: u8) -> Result<u8> {
        (self.0)(reg)
    }
    fn direct(&mut self, _: &[u8; 32]) -> Result<()> {
        panic!("family signature must not authorize RGB")
    }
}
#[test]
fn linux_policy_rejects_family_without_selector_writes() {
    let t = Generation::default().ticket();
    let result = bind(
        FamilyOnly(|reg| {
            Ok(match reg {
                0x43 => 0x1a,
                0x44 => 4,
                _ => panic!("unexpected register"),
            })
        }),
        &AtomicBool::new(false),
        &t,
    );
    assert!(result.err().unwrap().to_string().contains("RGB withheld"));
}
#[test]
fn stale_or_cancelled_verification_stops_before_second_read() {
    for stale in [false, true] {
        let g = Generation::default();
        let t = g.ticket();
        let stop = AtomicBool::new(false);
        let result = bind(
            FamilyOnly(|reg| {
                assert_eq!(reg, 0x43);
                if stale {
                    g.invalidate()
                } else {
                    stop.store(true, Ordering::Release)
                }
                Ok(0x1a)
            }),
            &stop,
            &t,
        );
        let message = result.err().unwrap().to_string();
        assert!(message.contains(if stale {
            "Inventory changed"
        } else {
            "cancelled"
        }));
    }
}
#[test]
fn signature_io_error_is_propagated_without_retry() {
    let t = Generation::default().ticket();
    let stop = AtomicBool::new(false);
    for code in [libc::EACCES, libc::ENXIO, libc::EBUSY] {
        let mut calls = 0;
        let result = bind(
            FamilyOnly(|reg| {
                calls += 1;
                assert_eq!(reg, 0x43);
                Err(io::Error::from_raw_os_error(code).into())
            }),
            &stop,
            &t,
        );
        let error = result.err().unwrap();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().raw_os_error(),
            Some(code)
        );
        assert_eq!(calls, 1);
    }
}
#[test]
fn locks_follow_the_open_fd_and_distinct_modules_do_not_conflict() {
    let path = std::env::temp_dir().join(gtk::glib::uuid_string_random());
    let first = fs::File::create_new(&path).unwrap();
    let second = fs::OpenOptions::new().write(true).open(&path).unwrap();
    fs::remove_file(path).unwrap();
    lock_slot(&first, Slot::A).unwrap();
    assert!(lock_slot(&second, Slot::A).is_err());
    lock_slot(&second, Slot::B).unwrap();
    drop(first);
    lock_slot(&second, Slot::A).unwrap();
}
#[test]
fn same_bus_number_different_attachment_is_rejected() {
    let root = std::env::temp_dir().join(gtk::glib::uuid_string_random());
    let bus = root.join("i2c-7");
    fs::create_dir_all(&bus).unwrap();
    let a = Adapter::observe(&root, 7).unwrap();
    a.check_parent(&bus).unwrap();
    fs::rename(&bus, root.join("old")).unwrap();
    fs::create_dir(&bus).unwrap();
    assert!(a.check_parent(&bus).is_err());
    fs::remove_dir_all(root).unwrap();
}
