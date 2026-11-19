use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

const ICON: &[u8; 32 * 32 * 4] = include_bytes!("tray-icon.argb");
const ITEM: &str = "/StatusNotifierItem";
const MENU: &str = "/Menu";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const INTERFACE: &str = r#"
<node><interface name="org.kde.StatusNotifierItem">
  <method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
  <property name="Category" type="s" access="read"/>
  <property name="Id" type="s" access="read"/>
  <property name="Title" type="s" access="read"/>
  <property name="Status" type="s" access="read"/>
  <property name="IconName" type="s" access="read"/>
  <property name="IconPixmap" type="a(iiay)" access="read"/>
  <property name="ItemIsMenu" type="b" access="read"/>
  <property name="Menu" type="o" access="read"/>
</interface><interface name="com.canonical.dbusmenu">
  <method name="GetLayout"><arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="as" direction="in"/><arg type="u" direction="out"/><arg type="(ia{sv}av)" direction="out"/></method>
  <method name="GetGroupProperties"><arg type="ai" direction="in"/><arg type="as" direction="in"/><arg type="a(ia{sv})" direction="out"/></method>
  <method name="Event"><arg type="i" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="in"/><arg type="u" direction="in"/></method>
  <method name="AboutToShow"><arg type="i" direction="in"/><arg type="b" direction="out"/></method>
  <property name="Version" type="u" access="read"/>
  <property name="TextDirection" type="s" access="read"/>
  <property name="Status" type="s" access="read"/>
  <property name="IconThemePath" type="as" access="read"/>
</interface></node>"#;

type Props = BTreeMap<String, glib::Variant>;
type Layout = (i32, Props, Vec<glib::Variant>);
const PROFILES: i32 = 4;
const FIRST_PROFILE: i32 = 5;

fn items(names: &[String]) -> BTreeMap<i32, Props> {
    use crate::i18n::tr;
    let mut items = BTreeMap::from([(0, Props::new())]);
    for (id, label) in [
        (1, tr("Open", "Ouvrir")),
        (2, tr("Relaunch", "Relancer")),
        (3, tr("Exit", "Quitter")),
        (PROFILES, tr("Profiles", "Profils")),
    ] {
        items.insert(id, Props::from([("label".into(), label.to_variant())]));
    }
    let profiles = items.get_mut(&PROFILES).unwrap();
    profiles.insert("children-display".into(), "submenu".to_variant());
    profiles.insert("enabled".into(), (!names.is_empty()).to_variant());
    for (id, name) in (FIRST_PROFILE..).zip(names) {
        items.insert(id, Props::from([("label".into(), name.to_variant())]));
    }
    items
}

fn layout(items: &BTreeMap<i32, Props>, id: i32, depth: i32) -> Layout {
    let children = if depth == 0 {
        Vec::new()
    } else {
        let ids = match id {
            0 => vec![1, PROFILES, 2, 3],
            PROFILES => items.range(FIRST_PROFILE..).map(|(&id, _)| id).collect(),
            _ => Vec::new(),
        };
        ids.into_iter()
            .map(|id| layout(items, id, if depth < 0 { -1 } else { depth - 1 }).to_variant())
            .collect()
    };
    (id, items.get(&id).cloned().unwrap_or_default(), children)
}

fn icon_pixmap() -> glib::Variant {
    [(32i32, 32i32, ICON.as_slice())].as_slice().to_variant()
}

#[derive(Default)]
pub struct Menu {
    profiles: glib::WeakRef<gtk::ListBox>,
    apply: glib::WeakRef<gtk::Button>,
}

impl Menu {
    pub fn bind(&self, profiles: &gtk::ListBox, apply: &gtk::Button) {
        self.profiles.set(Some(profiles));
        self.apply.set(Some(apply));
    }
}

pub fn install(
    window: &gtk::ApplicationWindow,
    restart: Rc<Cell<bool>>,
) -> Result<Rc<Menu>, glib::Error> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)?;
    let host = bus.call_sync(
        Some(WATCHER),
        "/StatusNotifierWatcher",
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&(WATCHER, "IsStatusNotifierHostRegistered").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        500,
        gio::Cancellable::NONE,
    )?;
    if host
        .get::<(glib::Variant,)>()
        .and_then(|(value,)| value.get::<bool>())
        != Some(true)
    {
        return Err(glib::Error::new(
            gio::IOErrorEnum::NotSupported,
            "No system tray host",
        ));
    }
    let interface = gio::DBusNodeInfo::for_xml(INTERFACE)?;
    let menu = Rc::new(Menu::default());
    let state = menu.clone();
    let weak = window.downgrade();
    let names = RefCell::new(Vec::<String>::new());
    let revision = Cell::new(0u32);
    let menu_registration = bus
        .register_object(
            MENU,
            &interface
                .lookup_interface("com.canonical.dbusmenu")
                .unwrap(),
        )
        .method_call(move |_, _, _, _, method, params, invocation| {
            let reply = match method {
                "GetLayout" => {
                    let (id, depth, _) = params.get::<(i32, i32, Vec<String>)>().unwrap();
                    if id == 0 || id == PROFILES {
                        let latest = state
                            .profiles
                            .upgrade()
                            .map(|list| {
                                (0..)
                                    .map_while(|i| list.row_at_index(i))
                                    .filter_map(|row| crate::profiles::row_label(&row))
                                    .map(|label| label.text().to_string())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        if *names.borrow() != latest {
                            names.replace(latest);
                            revision.set(revision.get().wrapping_add(1));
                        }
                    }
                    Some((revision.get(), layout(&items(&names.borrow()), id, depth)).to_variant())
                }
                "GetGroupProperties" => {
                    let (ids, _) = params.get::<(Vec<i32>, Vec<String>)>().unwrap();
                    let entries: Vec<_> = items(&names.borrow())
                        .into_iter()
                        .filter(|(id, _)| ids.is_empty() || ids.contains(id))
                        .collect();
                    Some((entries,).to_variant())
                }
                "AboutToShow" => Some((true,).to_variant()),
                "Event" => {
                    let (id, event, _, _) =
                        params.get::<(i32, String, glib::Variant, u32)>().unwrap();
                    if event == "clicked"
                        && let Some(window) = weak.upgrade()
                    {
                        match id {
                            1 => window.present(),
                            2 | 3 => {
                                restart.set(id == 2);
                                let _ = gtk::prelude::WidgetExt::activate_action(
                                    &window, "win.quit", None,
                                );
                            }
                            FIRST_PROFILE.. => {
                                let name =
                                    names.borrow().get((id - FIRST_PROFILE) as usize).cloned();
                                if let (Some(name), Some(list), Some(apply)) =
                                    (name, state.profiles.upgrade(), state.apply.upgrade())
                                    && let Some(row) =
                                        (0..).map_while(|i| list.row_at_index(i)).find(|row| {
                                            crate::profiles::row_label(row)
                                                .is_some_and(|label| label.text() == name)
                                        })
                                {
                                    list.unselect_all();
                                    list.select_row(Some(&row));
                                    apply.emit_clicked();
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                _ => unreachable!(),
            };
            invocation.return_value(reply.as_ref());
        })
        .property(|_, _, _, _, property| match property {
            "Version" => 3u32.to_variant(),
            "TextDirection" => "ltr".to_variant(),
            "Status" => "normal".to_variant(),
            "IconThemePath" => Vec::<String>::new().to_variant(),
            _ => unreachable!(),
        })
        .build()?;
    let weak = window.downgrade();
    let registration = bus
        .register_object(
            ITEM,
            &interface
                .lookup_interface("org.kde.StatusNotifierItem")
                .unwrap(),
        )
        .method_call(move |_, _, _, _, _, _, invocation| {
            if let Some(window) = weak.upgrade() {
                window.present();
            }
            invocation.return_value(None);
        })
        .property(|_, _, _, _, property| match property {
            "Category" => "Hardware".to_variant(),
            "Id" => "linrgb".to_variant(),
            "Title" => "LinRgb".to_variant(),
            "Status" => "Active".to_variant(),
            "IconName" => "".to_variant(),
            "IconPixmap" => icon_pixmap(),
            "ItemIsMenu" => false.to_variant(),
            "Menu" => glib::variant::ObjectPath::try_from(MENU)
                .unwrap()
                .to_variant(),
            _ => unreachable!(),
        })
        .build();
    let registration = match registration {
        Ok(registration) => registration,
        Err(error) => {
            let _ = bus.unregister_object(menu_registration);
            return Err(error);
        }
    };
    if let Err(error) = bus.call_sync(
        Some(WATCHER),
        "/StatusNotifierWatcher",
        WATCHER,
        "RegisterStatusNotifierItem",
        Some(&(ITEM,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        1000,
        gio::Cancellable::NONE,
    ) {
        let _ = bus.unregister_object(registration);
        let _ = bus.unregister_object(menu_registration);
        return Err(error);
    }
    let registrations = Cell::new(Some([registration, menu_registration]));
    window.connect_destroy(move |_| {
        if let Some(registrations) = registrations.take() {
            for registration in registrations {
                let _ = bus.unregister_object(registration);
            }
        }
    });
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_notifier_signatures_and_submenu() {
        assert_eq!(icon_pixmap().type_().as_str(), "a(iiay)");
        let menu = items(&["Desk".into()]);
        let root = layout(&menu, 0, -1);
        assert_eq!(root.to_variant().type_().as_str(), "(ia{sv}av)");
        let profiles = root.2[1].get::<Layout>().unwrap();
        assert_eq!(profiles.0, PROFILES);
        let profile = profiles.2[0].get::<Layout>().unwrap();
        assert_eq!(profile.0, FIRST_PROFILE);
        assert_eq!(profile.1["label"].get::<String>().as_deref(), Some("Desk"));
        assert!(layout(&menu, PROFILES, 0).2.is_empty());
    }
}
