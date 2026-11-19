use super::*;
use std::{
    cell::Cell,
    sync::{Arc, Barrier},
};

#[test]
fn reported_minimum_interval_is_a_lower_bound_without_sleeping() {
    let caps = Capabilities::HidRange {
        lamps: NonZeroU16::new(1).unwrap(),
        min_interval: Duration::from_millis(200),
    };
    let now = Instant::now();
    assert!(caps.frame_due(None, now));
    assert!(!caps.frame_due(Some(now), now + Duration::from_millis(199)));
    assert!(caps.frame_due(Some(now), now + Duration::from_millis(200)));
}

#[test]
fn invalidation_in_worker_queue_never_invokes_driver() {
    let generation = Generation::default();
    let ticket = generation.ticket();
    generation.invalidate();
    let stop = AtomicBool::new(false);
    assert!(Session::connect(&stop, &ticket, || panic!("stale job opened a driver")).is_err());
}

#[test]
fn refresh_during_verification_discards_new_handle_before_first_write() {
    let generation = Generation::default();
    let ticket = generation.ticket();
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        let b = barrier.clone();
        let g = &generation;
        scope.spawn(move || {
            b.wait();
            g.invalidate();
            b.wait();
        });
        let stop = AtomicBool::new(false);
        let result = Session::connect(&stop, &ticket, || {
            barrier.wait();
            barrier.wait();
            Ok((
                Capabilities::LegacyDirect,
                Box::new(|_| panic!("stale handle wrote")),
            ))
        });
        assert!(result.is_err());
    });
}

#[test]
fn refresh_between_connect_and_first_color_revokes_the_pending_command() {
    let g = Generation::default();
    let t = g.ticket();
    let stop = AtomicBool::new(false);
    let mut s = Session::connect(&stop, &t, || {
        Ok((
            Capabilities::LegacyDirect,
            Box::new(|_| panic!("stale color")),
        ))
    })
    .unwrap();
    g.invalidate();
    assert!(s.set_rgb([0; 3]).is_err());
    assert!(s.sink.is_none());
}

#[test]
fn open_sessions_survive_unrelated_refresh_but_never_resume_after_cancellation() {
    let generation = Generation::default();
    let ticket = generation.ticket();
    let stop = AtomicBool::new(false);
    let colors = Cell::new(0);
    let mut s = Session::connect(&stop, &ticket, || {
        Ok((
            Capabilities::LegacyDirect,
            Box::new(|_| {
                colors.set(colors.get() + 1);
                Ok(())
            }),
        ))
    })
    .unwrap();
    assert!(s.set_rgb([1; 3]).unwrap());
    generation.invalidate();
    assert!(s.set_rgb([2; 3]).unwrap());
    stop.store(true, Ordering::Release);
    assert!(s.set_rgb([3; 3]).is_err());
    // Resume uses a fresh inventory and a new job. Even resetting the old flag cannot resurrect the handle.
    generation.invalidate();
    stop.store(false, Ordering::Release);
    assert!(s.set_rgb([3; 3]).is_err());
    assert_eq!(colors.get(), 2);
}

#[test]
fn disconnect_or_permission_error_is_terminal() {
    for code in [libc::ENODEV, libc::EACCES] {
        let t = Generation::default().ticket();
        let stop = AtomicBool::new(false);
        let writes = Cell::new(0);
        let mut s = Session::connect(&stop, &t, || {
            Ok((
                Capabilities::LegacyDirect,
                Box::new(|_| {
                    writes.set(writes.get() + 1);
                    Err(io::Error::from_raw_os_error(code).into())
                }),
            ))
        })
        .unwrap();
        let err = s.set_rgb([0; 3]).unwrap_err();
        assert_eq!(
            err.downcast_ref::<io::Error>().unwrap().raw_os_error(),
            Some(code)
        );
        assert!(s.set_rgb([1; 3]).is_err());
        assert_eq!(writes.get(), 1);
    }
}

#[test]
fn kernel_capabilities_are_rechecked_and_verify_does_not_change_color() {
    use std::{fs, os::unix::fs::MetadataExt};
    let dir = std::env::temp_dir().join(gtk::glib::uuid_string_random());
    fs::create_dir(&dir).unwrap();
    for (k, v) in [
        ("multi_index", "green red blue"),
        ("max_brightness", "31"),
        ("brightness", "0"),
        ("multi_intensity", "0 0 0"),
    ] {
        fs::write(dir.join(k), v).unwrap();
    }
    let layout = kernel_led::channels(&dir).unwrap();
    let t = Target::Kernel(dir.clone(), dir.metadata().unwrap().ino(), layout);
    let ticket = Generation::default().ticket();
    let stop = AtomicBool::new(false);
    let mut session = t.clone().connect(&stop, &ticket).unwrap();
    assert_eq!(fs::read_to_string(dir.join("brightness")).unwrap(), "0");
    assert!(matches!(session.capabilities(),Capabilities::Sysfs{layout:l} if l==layout));
    assert!(session.set_rgb([255, 0, 0]).unwrap());
    assert_eq!(
        fs::read_to_string(dir.join("multi_intensity")).unwrap(),
        "0 31 0\n"
    );
    drop(session);
    fs::write(dir.join("multi_index"), "red green blue").unwrap();
    assert!(t.connect(&stop, &ticket).is_err());
    assert_eq!(
        fs::read_to_string(dir.join("multi_intensity")).unwrap(),
        "0 31 0\n"
    );
    fs::remove_dir_all(dir).unwrap();
}
