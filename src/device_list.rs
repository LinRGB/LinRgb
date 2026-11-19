use crate::{
    discovery::{self, Access, Device},
    effects::Target,
    i18n::tr,
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};

pub fn install(
    list: &gtk::Box,
    notes: &gtk::Label,
    search: &gtk::SearchEntry,
    refresh: &gtk::Button,
    all: &gtk::Button,
) -> impl Fn() -> Vec<(String, Target)> + Clone + use<> {
    let rows = Rc::new(RefCell::new(Vec::<(Device, gtk::CheckButton)>::new()));
    let entries = rows.clone();
    search.connect_search_changed(move |search| {
        let query = search.text().trim().to_lowercase();
        for (d, c) in entries.borrow().iter() {
            c.set_visible(d.name.to_lowercase().contains(&query));
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
    let (weak_list, weak_notes, weak_search) =
        (list.downgrade(), notes.downgrade(), search.downgrade());
    let busy = Rc::new(Cell::new(false));
    let pending = Rc::new(Cell::new(false));
    let entries = rows.clone();
    let update = Rc::new(move || {
        pending.set(true);
        if busy.replace(true) {
            return;
        }
        let (busy, pending, entries) = (busy.clone(), pending.clone(), entries.clone());
        let (list, notes, search) = (weak_list.clone(), weak_notes.clone(), weak_search.clone());
        glib::spawn_future_local(async move {
            while pending.replace(false) {
                let scan =
                    gio::spawn_blocking(|| discovery::scan(Path::new("/sys"), Path::new("/dev")))
                        .await;
                let (Some(list), Some(notes), Some(search)) =
                    (list.upgrade(), notes.upgrade(), search.upgrade())
                else {
                    break;
                };
                let Ok(snapshot) = scan else {
                    notes.set_visible(true);
                    notes.set_text(tr("Device discovery failed", "Échec de l’inventaire"));
                    break;
                };
                let mut entries = entries.borrow_mut();
                let old = std::mem::take(&mut *entries);
                for (_, check) in &old {
                    list.remove(check);
                }
                let query = search.text().trim().to_lowercase();
                for d in snapshot.devices {
                    let label = match d.access {
                        Access::Available => d.name.clone(),
                        Access::Denied => {
                            format!("{} · {}", d.name, tr("permission denied", "accès refusé"))
                        }
                        Access::Missing => format!(
                            "{} · {}",
                            d.name,
                            tr("interface unavailable", "interface indisponible")
                        ),
                    };
                    let c = gtk::CheckButton::with_label(&label);
                    c.set_sensitive(d.access == Access::Available);
                    c.set_active(
                        c.is_sensitive()
                            && old
                                .iter()
                                .any(|(prev, c)| prev.key == d.key && c.is_active()),
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
            }
            busy.set(false);
        });
    });
    let manual = update.clone();
    refresh.connect_clicked(move |_| manual());
    // devtmpfs reports node creation/removal and ACL changes; no idle inventory polling.
    if let Ok(monitor) = gio::File::for_path("/dev")
        .monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
    {
        let event_update = update.clone();
        let scheduled = Rc::new(Cell::new(false));
        monitor.connect_changed(move |_, file, _, event| {
            if !matches!(
                event,
                gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::Deleted
                    | gio::FileMonitorEvent::AttributeChanged
                    | gio::FileMonitorEvent::MovedIn
                    | gio::FileMonitorEvent::MovedOut
            ) {
                return;
            }
            if !file.basename().is_some_and(|p| is_bus_node(&p)) || scheduled.replace(true) {
                return;
            }
            let (update, scheduled) = (event_update.clone(), scheduled.clone());
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                scheduled.set(false);
                update();
            });
        });
        list.connect_destroy(move |_| {
            monitor.cancel();
        });
    }
    let resumed = update.clone();
    let view = list.downgrade();
    glib::spawn_future_local(async move {
        let Ok(bus) = gio::bus_get_future(gio::BusType::System).await else {
            return;
        };
        let Some(view) = view.upgrade() else {
            return;
        };
        let sub = bus.subscribe_to_signal(
            Some("org.freedesktop.login1"),
            Some("org.freedesktop.login1.Manager"),
            Some("PrepareForSleep"),
            Some("/org/freedesktop/login1"),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if signal.parameters.get::<(bool,)>() == Some((false,)) {
                    resumed();
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
        rows.borrow()
            .iter()
            .filter(|(_, c)| c.is_sensitive() && c.is_active())
            .map(|(d, _)| (d.name.clone(), d.target.clone()))
            .collect()
    }
}

fn is_bus_node(path: &Path) -> bool {
    let name = path.to_str().unwrap_or_default();
    name.strip_prefix("hidraw")
        .or_else(|| name.strip_prefix("i2c-"))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}

#[test]
fn ignores_unrelated_device_events() {
    for name in ["hidraw8", "i2c-10"] {
        assert!(is_bus_node(Path::new(name)));
    }
    for name in ["hidraw", "input", "i2c-test"] {
        assert!(!is_bus_node(Path::new(name)));
    }
}
