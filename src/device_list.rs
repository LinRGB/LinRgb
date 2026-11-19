use crate::{
    discovery::{self, Access, Device},
    drivers::{Generation, Target, Ticket},
    i18n::tr,
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    os::fd::AsRawFd,
    path::Path,
    rc::Rc,
    time::Duration,
};

// GTK-thread only. An old scan may finish, but must never publish after invalidation.
#[derive(Default)]
struct Epoch {
    generation: Generation,
    sleeping: Cell<bool>,
    ready: Cell<bool>,
}
impl Epoch {
    fn invalidate(&self) {
        self.ready.set(false);
        self.generation.invalidate();
    }
    fn accept(&self, generation: u64) -> bool {
        !self.sleeping.get() && self.generation.current() == generation
    }
}

type Selection = (Ticket, Vec<(String, Target)>);

pub fn install<F: Fn(&[Device]) + 'static>(
    list: &gtk::Box,
    notes: &gtk::Label,
    search: &gtk::SearchEntry,
    refresh: &gtk::Button,
    all: &gtk::Button,
    reconcile: F,
) -> impl Fn() -> Option<Selection> + Clone + use<F> {
    let reconcile = Rc::new(reconcile);
    let rows = Rc::new(RefCell::new(Vec::<(Device, gtk::CheckButton)>::new()));
    let entries = rows.clone();
    search.connect_search_changed(move |s| {
        let q = s.text().trim().to_lowercase();
        for (d, c) in entries.borrow().iter() {
            c.set_visible(d.name.to_lowercase().contains(&q));
        }
    });
    let entries = rows.clone();
    all.connect_clicked(move |_| {
        for (d, c) in entries.borrow().iter() {
            if d.bulk_selectable() {
                c.set_active(true);
            }
        }
    });
    let (list_ref, notes_ref, search_ref) =
        (list.downgrade(), notes.downgrade(), search.downgrade());
    let epoch = Rc::new(Epoch::default());
    let busy = Rc::new(Cell::new(false));
    let entries = rows.clone();
    let gate = epoch.clone();
    let received = reconcile.clone();
    let update = Rc::new(move || {
        gate.invalidate();
        if gate.sleeping.get() || busy.replace(true) {
            return;
        }
        let (gate, busy, entries, received) = (
            gate.clone(),
            busy.clone(),
            entries.clone(),
            received.clone(),
        );
        let (list, notes, search) = (list_ref.clone(), notes_ref.clone(), search_ref.clone());
        glib::spawn_future_local(async move {
            loop {
                // Coalesce an entire hotplug burst into a single passive scan.
                glib::timeout_future(Duration::from_millis(120)).await;
                if gate.sleeping.get() || list.upgrade().is_none() {
                    break;
                }
                let generation = gate.generation.current();
                let scan =
                    gio::spawn_blocking(|| discovery::scan(Path::new("/sys"), Path::new("/dev")))
                        .await;
                if !gate.accept(generation) {
                    continue;
                }
                let (Some(list), Some(notes), Some(search)) =
                    (list.upgrade(), notes.upgrade(), search.upgrade())
                else {
                    break;
                };
                let mut entries = entries.borrow_mut();
                let old = std::mem::take(&mut *entries);
                for (_, c) in &old {
                    list.remove(c);
                }
                let Ok(snapshot) = scan else {
                    received(&[]);
                    notes.set_visible(true);
                    notes.set_text(tr("Device discovery failed", "Échec de l’inventaire"));
                    break;
                };
                received(&snapshot.devices);
                let query = search.text().trim().to_lowercase();
                for d in snapshot.devices {
                    let mut label = d.name.clone();
                    for hint in [
                        d.support_label(),
                        match d.access {
                            Access::Available => "",
                            Access::Denied => tr("permission denied", "accès refusé"),
                            Access::Missing => {
                                tr("interface unavailable", "interface indisponible")
                            }
                        },
                    ] {
                        if !hint.is_empty() {
                            label.push_str(" · ");
                            label.push_str(hint);
                        }
                    }
                    let c = gtk::CheckButton::with_label(&label);
                    c.set_sensitive(d.access == Access::Available);
                    c.set_active(
                        old.iter()
                            .any(|(prev, c)| c.is_active() && d.can_restore_from(prev)),
                    );
                    c.set_visible(d.name.to_lowercase().contains(&query));
                    c.add_css_class("device-row");
                    list.append(&c);
                    entries.push((d, c));
                }
                let text = if entries.is_empty() && snapshot.notes.is_empty() {
                    tr("No RGB devices found", "Aucun périphérique RGB trouvé").to_owned()
                } else {
                    snapshot.notes.join("\n")
                };
                notes.set_visible(!text.is_empty());
                notes.set_text(&text);
                gate.ready.set(true);
                break;
            }
            busy.set(false);
        });
    });
    let manual = update.clone();
    refresh.connect_clicked(move |_| manual());
    if let Err(e) = watch_udev(list, update.clone()) {
        eprintln!("udev events unavailable; use Refresh: {e}");
    }
    // Device ACLs can change without a topology uevent (e.g. active seat switch).
    if let Ok(m) = gio::File::for_path("/dev")
        .monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
    {
        let update = update.clone();
        m.connect_changed(move |_, file, _, event| {
            if matches!(
                event,
                gio::FileMonitorEvent::AttributeChanged
                    | gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::Deleted
            ) && file.basename().is_some_and(|p| is_bus_node(&p))
            {
                update();
            }
        });
        list.connect_destroy(move |_| {
            m.cancel();
        });
    }
    let (rescan, view, power) = (update.clone(), list.downgrade(), epoch.clone());
    glib::spawn_future_local(async move {
        let Ok(bus) = gio::bus_get_future(gio::BusType::System).await else {
            return;
        };
        let Some(view) = view.upgrade() else { return };
        let sub = bus.subscribe_to_signal(
            Some("org.freedesktop.login1"),
            Some("org.freedesktop.login1.Manager"),
            Some("PrepareForSleep"),
            Some("/org/freedesktop/login1"),
            None,
            gio::DBusSignalFlags::NONE,
            move |s| {
                if let Some((sleeping,)) = s.parameters.get::<(bool,)>() {
                    power.sleeping.set(sleeping);
                    reconcile(&[]);
                    rescan();
                }
            },
        );
        let lifetime = Cell::new(Some(sub));
        view.connect_destroy(move |_| {
            lifetime.take();
        });
    });
    update();
    move || {
        epoch.ready.get().then(|| {
            (
                epoch.generation.ticket(),
                rows.borrow()
                    .iter()
                    .filter(|(_, c)| c.is_active() && c.is_sensitive())
                    .map(|(d, _)| (d.name.clone(), d.target.clone()))
                    .collect(),
            )
        })
    }
}

fn watch_udev(view: &gtk::Box, update: Rc<impl Fn() + 'static>) -> std::io::Result<()> {
    let mut builder = udev::MonitorBuilder::new()?;
    for subsystem in ["leds", "hidraw", "hid", "usb", "i2c-dev", "pci"] {
        builder = builder.match_subsystem(subsystem)?;
    }
    let socket = builder.listen()?;
    let lifetime = Rc::new(Cell::new(None::<glib::SourceId>));
    let alive = lifetime.clone();
    let flags = glib::IOCondition::IN | glib::IOCondition::HUP | glib::IOCondition::ERR;
    let id = glib::source::unix_fd_add_local(socket.as_raw_fd(), flags, move |_, condition| {
        if condition
            .intersects(glib::IOCondition::HUP | glib::IOCondition::ERR | glib::IOCondition::NVAL)
        {
            alive.take();
            update();
            return glib::ControlFlow::Break;
        }
        // Limit work per GLib dispatch; any remaining datagrams keep the fd readable.
        if socket.iter().take(64).count() > 0 {
            update();
        }
        glib::ControlFlow::Continue
    });
    lifetime.set(Some(id));
    view.connect_destroy(move |_| {
        if let Some(id) = lifetime.take() {
            id.remove();
        }
    });
    Ok(())
}

fn is_bus_node(path: &Path) -> bool {
    let name = path.to_str().unwrap_or_default();
    name.strip_prefix("hidraw")
        .or_else(|| name.strip_prefix("i2c-"))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}

#[test]
fn ignores_unrelated_device_events() {
    for n in ["hidraw1", "i2c-10"] {
        assert!(is_bus_node(Path::new(n)));
    }
    for n in ["input", "hidraw", "i2c-test"] {
        assert!(!is_bus_node(Path::new(n)));
    }
}
#[test]
fn invalidated_scan_cannot_restore_removed_or_sleeping_endpoints() {
    let g = Epoch::default();
    let first = g.generation.current();
    g.invalidate();
    assert!(!g.accept(first));
    let awake = g.generation.current();
    g.sleeping.set(true);
    g.invalidate();
    assert!(!g.accept(awake));
    assert!(!g.accept(g.generation.current()));
    g.sleeping.set(false);
    g.invalidate();
    assert!(!g.accept(awake));
    assert!(g.accept(g.generation.current()));
}
