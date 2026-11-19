use super::*;
use crate::drivers::{Generation, Session};
use std::collections::VecDeque;

// Expected wire transcript built independently of the driver. An extra transaction panics.
#[derive(Debug)]
enum Step {
    Read(u8, u8),
    Select(u8),
    Frame([u8; 32]),
    FailRead(u8, i32),
    FailSelect(u8),
    FailFrame,
}
struct Mock<'a>(&'a mut VecDeque<Step>);
impl Transport for Mock<'_> {
    fn read(&mut self, reg: u8) -> Result<u8> {
        match self.0.pop_front() {
            Some(Step::Read(r, v)) if r == reg => Ok(v),
            Some(Step::FailRead(r, code)) if r == reg => {
                Err(io::Error::from_raw_os_error(code).into())
            }
            s => panic!("unexpected read {reg:x}: {s:?}"),
        }
    }
    fn write_zero(&mut self, reg: u8) -> Result<()> {
        match self.0.pop_front() {
            Some(Step::Select(r)) if r == reg => Ok(()),
            Some(Step::FailSelect(r)) if r == reg => {
                Err(io::Error::from_raw_os_error(libc::ENXIO).into())
            }
            s => panic!("unexpected selector {reg:x}: {s:?}"),
        }
    }
    fn direct(&mut self, frame: &[u8; 32]) -> Result<()> {
        match self.0.pop_front() {
            Some(Step::Frame(expected)) => {
                assert_eq!(&expected, frame);
                Ok(())
            }
            Some(Step::FailFrame) => Err(io::Error::from_raw_os_error(libc::ENODEV).into()),
            s => panic!("unexpected frame: {s:?}"),
        }
    }
}
fn info(pid: u16) -> [u8; 32] {
    let mut i = [0; 32];
    i[..2].copy_from_slice(&0x1b1cu16.to_le_bytes());
    i[2..4].copy_from_slice(&pid.to_le_bytes());
    i[28] = 4;
    i
}
fn transcript(i: [u8; 32], sum: u8) -> VecDeque<Step> {
    use Step::*;
    let mut q = VecDeque::from([Read(0x43, 0x1b), Read(0x44, 4), Select(0x61), Select(0x21)]);
    q.extend(i.map(|b| Read(0x40, b)));
    q.push_back(Read(0x42, sum));
    q
}
fn reject(mut q: VecDeque<Step>) {
    let t = Generation::default().ticket();
    assert!(bind(Mock(&mut q), &AtomicBool::new(false), &t).is_err());
    assert!(q.is_empty());
}
#[test]
fn unrelated_signatures_never_reach_selectors() {
    use Step::*;
    reject([Read(0x43, 0)].into());
    reject([Read(0x43, 0x1b), Read(0x44, 2)].into());
}
#[test]
fn family_match_does_not_authorize_dominator_rs_or_unknown_modules() {
    for pid in [0x0400, 0x0200, 0x0201, 0x0401, 0x0701, 0xffff] {
        let i = info(pid);
        reject(transcript(i, crc(&i)));
    }
}
#[test]
fn wrong_vendor_crc_or_protocol_never_enables_rgb() {
    for (offset, value) in [(0, 0), (28, 3), (28, 5)] {
        let mut i = info(0x0300);
        i[offset] = value;
        reject(transcript(i, crc(&i)));
    }
    let i = info(0x0301);
    reject(transcript(i, crc(&i) ^ 1));
}
#[test]
fn any_incomplete_or_failed_transaction_stops_without_retry() {
    let i = info(0x0301);
    let full = transcript(i, crc(&i));
    for (n, step) in full.iter().enumerate() {
        let mut q = transcript(i, crc(&i));
        q.truncate(n);
        q.push_back(match *step {
            Step::Read(r, _) => Step::FailRead(r, libc::ENXIO),
            Step::Select(r) => Step::FailSelect(r),
            _ => unreachable!(),
        });
        reject(q);
    }
}
fn expected(rgb: [u8; 3]) -> [u8; 32] {
    let mut p = [0; 32];
    p[0] = 10;
    p[1..31].copy_from_slice(&rgb.repeat(10));
    p[31] = crc(&p[..31]);
    p
}
#[test]
fn both_pro_sl_ids_accept_repeated_direct_frames_until_first_failure() {
    assert_eq!(crc(b"123456789"), 0xf4);
    for pid in [0x0300, 0x0301] {
        let i = info(pid);
        let mut q = transcript(i, crc(&i));
        q.extend([
            Step::Frame(expected([1, 2, 3])),
            Step::Frame(expected([3, 2, 1])),
            Step::FailFrame,
        ]);
        let stop = AtomicBool::new(false);
        let t = Generation::default().ticket();
        let mut s = Session::connect(&stop, &t, || bind(Mock(&mut q), &stop, &t)).unwrap();
        assert!(matches!(
            s.capabilities(),
            Capabilities::Corsair {
                protocol: 4,
                model: "Vengeance RGB Pro SL"
            }
        ));
        assert!(s.set_rgb([1, 2, 3]).unwrap());
        assert!(s.set_rgb([3, 2, 1]).unwrap());
        assert!(s.set_rgb([1; 3]).is_err());
        assert!(s.set_rgb([2; 3]).is_err());
        drop(s);
        assert!(q.is_empty());
    }
}
#[test]
fn pro_ddr4_ten_positions_require_the_direct_protocol() {
    for pid in [0x0100, 0x0101] {
        let mut i = info(pid);
        i[28] = 3;
        reject(transcript(i, crc(&i)));
        i[28] = 4;
        let mut q = transcript(i, crc(&i));
        q.push_back(Step::Frame(expected([13, 29, 101])));
        let stop = AtomicBool::new(false);
        let t = Generation::default().ticket();
        let mut s = Session::connect(&stop, &t, || bind(Mock(&mut q), &stop, &t)).unwrap();
        assert!(matches!(
            s.capabilities(),
            Capabilities::Corsair {
                model: "Vengeance RGB Pro",
                protocol: 4
            }
        ));
        assert!(s.set_rgb([13, 29, 101]).unwrap());
        drop(s);
        assert!(q.is_empty());
    }
}
#[test]
fn two_live_slots_keep_distinct_frames_and_a_failure_is_local() {
    assert_eq!([Slot::A as u16, Slot::B as u16], [0x5a, 0x5b]);
    let stop = AtomicBool::new(false);
    let t = Generation::default().ticket();
    let a = info(0x0300);
    let b = info(0x0301);
    let mut qa = transcript(a, crc(&a));
    let mut qb = transcript(b, crc(&b));
    qa.extend([Step::Frame(expected([20, 0, 0])), Step::FailFrame]);
    qb.extend([
        Step::Frame(expected([0, 20, 0])),
        Step::Frame(expected([0, 0, 20])),
    ]);
    let mut first = Session::connect(&stop, &t, || bind(Mock(&mut qa), &stop, &t)).unwrap();
    let mut second = Session::connect(&stop, &t, || bind(Mock(&mut qb), &stop, &t)).unwrap();
    assert!(first.set_rgb([20, 0, 0]).unwrap());
    assert!(second.set_rgb([0, 20, 0]).unwrap());
    assert!(first.set_rgb([1; 3]).is_err());
    assert!(first.set_rgb([2; 3]).is_err());
    assert!(second.set_rgb([0, 0, 20]).unwrap());
    drop((first, second));
    assert!(qa.is_empty() && qb.is_empty());
}
#[test]
fn invalidated_ticket_after_identification_prevents_first_color() {
    let i = info(0x0300);
    let mut q = transcript(i, crc(&i));
    let g = Generation::default();
    let t = g.ticket();
    let stop = AtomicBool::new(false);
    let mut s = Session::connect(&stop, &t, || bind(Mock(&mut q), &stop, &t)).unwrap();
    g.invalidate();
    assert!(s.set_rgb([0; 3]).is_err());
    drop(s);
    assert!(q.is_empty());
}
#[test]
fn cancellation_between_selectors_blocks_binary_start() {
    struct Cancel<'a>(&'a AtomicBool);
    impl Transport for Cancel<'_> {
        fn read(&mut self, r: u8) -> Result<u8> {
            Ok(match r {
                0x43 => 0x1b,
                0x44 => 4,
                _ => panic!(),
            })
        }
        fn write_zero(&mut self, r: u8) -> Result<()> {
            assert_eq!(r, 0x61);
            self.0.store(true, Ordering::Release);
            Ok(())
        }
        fn direct(&mut self, _: &[u8; 32]) -> Result<()> {
            panic!()
        }
    }
    let stop = AtomicBool::new(false);
    let t = Generation::default().ticket();
    assert!(bind(Cancel(&stop), &stop, &t).is_err());
    assert!(stop.load(Ordering::Acquire));
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
