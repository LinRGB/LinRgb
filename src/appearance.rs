use crate::i18n::tr;
use gtk::{gio, prelude::*};
use serde_json::Value;
use std::{cell::RefCell, fs, io::Read, path::Path, rc::Rc};

const ROLES: [&str; 9] = [
    "base",
    "surface",
    "raised",
    "text",
    "muted",
    "accent",
    "accent-ink",
    "accent-label",
    "wall",
];
const DARK: [&str; 9] = [
    "#141414", "#1d1d1d", "#2a2a2a", "#f5f5f5", "#bdbdbd", "#73e4c6", "#0b3027", "#9debd8",
    "#0b0b0b",
];
const LIGHT: [&str; 9] = [
    "#ffffff", "#ffffff", "#f0f0f0", "#191919", "#535353", "#087365", "#ffffff", "#07584e",
    "#eeeeee",
];

const THEMES: [(&str, &str, [&str; 9]); 5] = [
    ("dark", "LinRgb Dark", DARK),
    ("light", "LinRgb Light", LIGHT),
    (
        "midnight",
        "Midnight Blue",
        [
            "#111827", "#19243a", "#243551", "#eef5ff", "#b6c8e1", "#89b9ff", "#122848", "#a6cbff",
            "#0a1220",
        ],
    ),
    (
        "lavender",
        "Lavender Dusk",
        [
            "#1c1924", "#262131", "#342d43", "#f4eefc", "#c7bad7", "#d8bbff", "#2c1747", "#e8d3ff",
            "#14111c",
        ],
    ),
    (
        "paper",
        "Warm Paper",
        [
            "#fffaf0", "#fff6e7", "#f2e4cd", "#30221a", "#624e40", "#8c442b", "#ffffff", "#773820",
            "#eae0ce",
        ],
    ),
];

fn palette(value: &Value) -> Option<[&str; 9]> {
    let name = value["name"].as_str()?;
    if name.trim().is_empty()
        || name.chars().count() > 48
        || !matches!(value["mode"].as_str()?, "dark" | "light")
    {
        return None;
    }
    let mut colors = [""; 9];
    for (slot, role) in colors.iter_mut().zip(ROLES) {
        let hex = value["colors"][role].as_str()?;
        if !hex.starts_with('#') || crate::color_wheel::parse_hex(hex).is_none() || hex.len() != 7 {
            return None;
        }
        *slot = hex;
    }
    Some(colors)
}

fn load(path: &Path) -> Option<Value> {
    if !fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(16_385)
        .read_to_end(&mut data)
        .ok()?;
    (data.len() <= 16_384)
        .then(|| serde_json::from_slice(&data).ok())
        .flatten()
}

fn saved_zoom(value: &Value) -> u64 {
    value
        .as_u64()
        .filter(|zoom| (80..=150).contains(zoom))
        .unwrap_or(100)
}

fn watch_wallpaper(path: &Path, on_palette: impl Fn(Value) + 'static) -> Option<gio::FileMonitor> {
    // Watch the directory so atomic replacements of the JSON are detected too.
    let file = gio::File::for_path(path);
    let monitor = file
        .parent()?
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        .ok()?;
    let path = path.to_owned();
    monitor.connect_changed(move |_, changed, destination, event| {
        use gio::FileMonitorEvent::*;
        if !matches!(
            event,
            Changed | ChangesDoneHint | Created | MovedIn | Renamed
        ) {
            return;
        }
        if !changed.equal(&file) && !destination.is_some_and(|target| target.equal(&file)) {
            return;
        }
        if let Some(theme) = load(&path).filter(|theme| palette(theme).is_some()) {
            on_palette(theme);
        }
    });
    Some(monitor)
}

fn render(
    provider: &gtk::CssProvider,
    mode: &str,
    custom: Option<&Value>,
    wallpaper: Option<&Value>,
    zoom: u64,
) {
    let colors = match mode {
        "custom" => custom.and_then(palette).unwrap_or(DARK),
        "wallpaper" => wallpaper.and_then(palette).unwrap_or(DARK),
        _ => THEMES
            .iter()
            .find(|(id, _, _)| *id == mode)
            .map_or(DARK, |(_, _, colors)| *colors),
    };
    let mut css = String::new();
    for (role, color) in ROLES.into_iter().zip(colors) {
        css.push_str(&format!("@define-color linrgb_{role} {color};\n"));
    }
    css.push_str(&format!(".linrgb {{ font-size: {zoom}%; }}\n"));
    css.push_str(include_str!("style.css"));
    provider.load_from_data(&css);
}

pub fn install(window: &gtk::ApplicationWindow, header: &gtk::HeaderBar) {
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(window),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let path = crate::profiles::config_dir().join("appearance.json");
    let themes = crate::profiles::config_dir().join("themes");
    if let Err(error) = fs::create_dir_all(&themes) {
        eprintln!("Could not create themes directory: {error}");
    }
    let saved = load(&path)
        .or_else(|| load(&gtk::glib::user_config_dir().join("linrgb/appearance.json")))
        .unwrap_or_default();
    crate::i18n::init(saved["language"].as_str().unwrap_or("en"));
    let custom = Rc::new(RefCell::new(
        load(&themes.join("imported.json"))
            .or_else(|| saved.get("theme").cloned())
            .filter(|theme| palette(theme).is_some()),
    ));
    let wallpaper_path = themes.join("wallpaper.json");
    let wallpaper = Rc::new(RefCell::new(
        load(&wallpaper_path).filter(|theme| palette(theme).is_some()),
    ));
    let options = gtk::ComboBoxText::new();
    options.update_property(&[gtk::accessible::Property::Label(tr("Theme", "Thème"))]);
    for (id, name, _) in THEMES {
        options.append(Some(id), name);
    }
    options.append(
        Some("wallpaper"),
        tr("Noctalia Wallpaper", "Fond d’écran Noctalia"),
    );
    if let Some(theme) = custom.borrow().as_ref() {
        options.append(Some("custom"), theme["name"].as_str().unwrap_or("Custom"));
    }
    if !options.set_active_id(saved["mode"].as_str()) {
        options.set_active_id(Some("dark"));
    }
    let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
    settings.set_tooltip_text(Some(tr("Settings", "Paramètres")));
    settings.update_property(&[gtk::accessible::Property::Label(tr(
        "Settings",
        "Paramètres",
    ))]);
    let dialog = gtk::Dialog::builder()
        .title(tr("Settings", "Paramètres"))
        .transient_for(window)
        .modal(true)
        .hide_on_close(true)
        .default_width(390)
        .build();
    dialog.add_css_class("linrgb");
    dialog.add_button(tr("Done", "Terminé"), gtk::ResponseType::Close);
    dialog.connect_response(|dialog, _| dialog.hide());
    let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
    controls.add_css_class("settings-panel");
    controls.append(&crate::label(tr("Theme", "Thème"), "heading"));
    controls.append(&options);
    controls.append(&crate::label(
        tr("Interface zoom (%)", "Zoom de l’interface (%)"),
        "heading",
    ));
    let zoom = gtk::SpinButton::with_range(80.0, 150.0, 5.0);
    zoom.set_value(saved_zoom(&saved["zoom"]) as f64);
    zoom.set_numeric(true);
    zoom.update_property(&[gtk::accessible::Property::Label(tr(
        "Interface zoom (%)",
        "Zoom de l’interface (%)",
    ))]);
    controls.append(&zoom);
    let language = gtk::ComboBoxText::new();
    language.append(Some("en"), "English");
    language.append(Some("fr"), "Français");
    language.set_active_id(Some(if saved["language"].as_str() == Some("fr") {
        "fr"
    } else {
        "en"
    }));
    language.update_property(&[gtk::accessible::Property::Label(tr("Language", "Langue"))]);
    controls.append(&crate::label(tr("Language", "Langue"), "heading"));
    controls.append(&language);
    controls.append(&crate::label(
        tr(
            "Restart LinRgb to change the display language.",
            "Relancez LinRgb pour changer la langue de l’interface.",
        ),
        "dim-label",
    ));
    dialog.content_area().append(&controls);
    render(
        &provider,
        options.active_id().as_deref().unwrap_or("dark"),
        custom.borrow().as_ref(),
        wallpaper.borrow().as_ref(),
        zoom.value_as_int() as u64,
    );
    let selection = options.downgrade();
    let scale = zoom.downgrade();
    let live_provider = provider.clone();
    let live = wallpaper.clone();
    let watcher = watch_wallpaper(&wallpaper_path, move |theme| {
        if live.borrow().as_ref().and_then(palette) == palette(&theme) {
            return;
        }
        live.replace(Some(theme));
        if let (Some(options), Some(zoom)) = (selection.upgrade(), scale.upgrade())
            && options.active_id().as_deref() == Some("wallpaper")
        {
            render(
                &live_provider,
                "wallpaper",
                None,
                live.borrow().as_ref(),
                zoom.value_as_int() as u64,
            );
        }
    });
    if watcher.is_none() {
        eprintln!("Could not watch {}", wallpaper_path.display());
    }
    let state = custom.clone();
    let apply = move |options: &gtk::ComboBoxText, zoom: u64, language: &str| {
        let Some(mode) = options.active_id() else {
            return;
        };
        let theme = state.borrow();
        render(
            &provider,
            &mode,
            theme.as_ref(),
            wallpaper.borrow().as_ref(),
            zoom,
        );
        let settings = serde_json::json!({"mode": mode.as_str(), "theme": *theme, "zoom": zoom, "language": language});
        // The themes directory is created at startup and shares this parent.
        if let Err(error) = gtk::glib::file_set_contents(&path, settings.to_string().as_bytes()) {
            eprintln!("Could not save appearance: {error}");
        }
    };
    let update = apply.clone();
    let (scale, locale) = (zoom.downgrade(), language.downgrade());
    options.connect_changed(move |options| {
        if let (Some(scale), Some(locale)) = (scale.upgrade(), locale.upgrade()) {
            update(
                options,
                scale.value_as_int() as u64,
                locale.active_id().as_deref().unwrap_or("en"),
            );
        }
    });
    let (appearance, locale) = (options.downgrade(), language.downgrade());
    let update = apply.clone();
    zoom.connect_value_changed(move |zoom| {
        if let (Some(options), Some(locale)) = (appearance.upgrade(), locale.upgrade()) {
            update(
                &options,
                zoom.value_as_int() as u64,
                locale.active_id().as_deref().unwrap_or("en"),
            );
        }
    });
    let (appearance, scale) = (options.downgrade(), zoom.downgrade());
    language.connect_changed(move |language| {
        if let (Some(options), Some(scale)) = (appearance.upgrade(), scale.upgrade()) {
            apply(
                &options,
                scale.value_as_int() as u64,
                language.active_id().as_deref().unwrap_or("en"),
            );
        }
    });
    let import = gtk::Button::with_label(tr("Import JSON theme…", "Importer un thème JSON…"));
    controls.prepend(&import);
    let parent = dialog.downgrade();
    let selection = options.clone();
    import.connect_clicked(move |_| {
        let Some(window) = parent.upgrade() else {
            return;
        };
        let chooser = gtk::FileChooserNative::new(
            Some(tr("Import theme", "Importer un thème")),
            Some(&window),
            gtk::FileChooserAction::Open,
            Some(tr("Import", "Importer")),
            Some(tr("Cancel", "Annuler")),
        );
        let filter = gtk::FileFilter::new();
        filter.add_pattern("*.json");
        chooser.add_filter(&filter);
        let _ = chooser.set_current_folder(Some(&gio::File::for_path(&themes)));
        let destination = themes.join("imported.json");
        let state = custom.clone();
        let options = selection.clone();
        let parent = parent.clone();
        gtk::glib::MainContext::default().spawn_local(async move {
            if chooser.run_future().await == gtk::ResponseType::Accept {
                let data = chooser
                    .file()
                    .and_then(|file: gio::File| file.path())
                    .and_then(|path| load(&path))
                    .filter(|theme| palette(theme).is_some());
                if let Some(theme) = data {
                    if let Err(error) =
                        gtk::glib::file_set_contents(&destination, theme.to_string().as_bytes())
                    {
                        eprintln!("Could not copy the theme to the config folder: {error}");
                    }
                    let name = theme["name"].as_str().unwrap_or("Custom").to_owned();
                    let previous = state.replace(Some(theme));
                    if previous.is_some() {
                        options.remove((THEMES.len() + 1) as i32);
                    }
                    options.append(Some("custom"), &name);
                    options.set_active_id(Some("custom"));
                } else if let Some(window) = parent.upgrade() {
                    let dialog = gtk::MessageDialog::builder()
                        .transient_for(&window)
                        .modal(true)
                        .buttons(gtk::ButtonsType::Close)
                        .text(tr("Invalid theme file", "Fichier de thème invalide"))
                        .secondary_text(tr(
                            "Choose a LinRgb JSON theme with all nine HEX colors.",
                            "Choisissez un thème JSON LinRgb contenant les neuf couleurs HEX.",
                        ))
                        .build();
                    dialog.add_css_class("linrgb");
                    dialog.connect_response(|dialog, _| dialog.close());
                    dialog.present();
                }
            }
            chooser.destroy();
        });
    });
    window.connect_destroy(move |_| {
        if let Some(watcher) = &watcher {
            watcher.cancel();
        }
    });
    settings.connect_clicked(move |_| dialog.present());
    header.pack_end(&settings);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_watch_detects_an_atomic_replacement() {
        let dir =
            std::env::temp_dir().join(format!("linrgb-theme-{}", gtk::glib::uuid_string_random()));
        fs::create_dir(&dir).unwrap();
        let context = gtk::glib::MainContext::new();
        let received = context
            .with_thread_default(|| {
                let path = dir.join("wallpaper.json");
                fs::write(&path, "invalid JSON").unwrap();
                let theme_name = Rc::new(RefCell::new(None));
                let result = theme_name.clone();
                let monitor = watch_wallpaper(&path, move |theme| {
                    result.replace(theme["name"].as_str().map(str::to_owned));
                })
                .unwrap();
                let incoming = dir.join("incoming.json");
                fs::write(&incoming, include_str!("../design/theme.example.json")).unwrap();
                fs::rename(incoming, &path).unwrap();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while theme_name.borrow().is_none() && std::time::Instant::now() < deadline {
                    context.iteration(false);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                monitor.cancel();
                theme_name.take()
            })
            .unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(received.as_deref(), Some("Lavender Dusk"));
    }

    #[test]
    fn zoom_restores_valid_values_and_ignores_invalid_settings() {
        assert_eq!(saved_zoom(&serde_json::json!(80)), 80);
        assert_eq!(saved_zoom(&serde_json::json!(150)), 150);
        assert_eq!(saved_zoom(&serde_json::json!(151)), 100);
        assert_eq!(saved_zoom(&Value::Null), 100);
    }

    #[test]
    fn accepts_the_preview_theme_but_rejects_css_injection() {
        let mut theme: Value =
            serde_json::from_str(include_str!("../design/theme.example.json")).unwrap();
        assert!(palette(&theme).is_some());
        theme["colors"]["accent"] = "red; } * { color: red".into();
        assert!(palette(&theme).is_none());
    }
}
