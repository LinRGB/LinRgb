use gtk::{gio, glib, prelude::*};

const ITEM: &str = "/StatusNotifierItem";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const INTERFACE: &str = r#"
<node><interface name="org.kde.StatusNotifierItem">
  <method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
  <property name="Category" type="s" access="read"/>
  <property name="Id" type="s" access="read"/>
  <property name="Title" type="s" access="read"/>
  <property name="Status" type="s" access="read"/>
  <property name="IconName" type="s" access="read"/>
  <property name="ItemIsMenu" type="b" access="read"/>
</interface></node>"#;

pub fn install(window: &gtk::ApplicationWindow) -> Result<(), glib::Error> {
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
    let info = interface
        .lookup_interface("org.kde.StatusNotifierItem")
        .expect("static interface");
    let weak = window.downgrade();
    let registration = bus
        .register_object(ITEM, &info)
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
            "IconName" => "preferences-desktop-color".to_variant(),
            "ItemIsMenu" => false.to_variant(),
            _ => unreachable!(),
        })
        .build()?;
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
        return Err(error);
    }
    let registration = std::cell::Cell::new(Some(registration));
    window.connect_destroy(move |_| {
        if let Some(registration) = registration.take() {
            let _ = bus.unregister_object(registration);
        }
    });
    Ok(())
}
