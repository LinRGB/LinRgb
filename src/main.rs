mod adaptive;
mod appearance;
mod astral;
mod avatar;
mod board;
mod color_wheel;
mod corsair;
mod device_list;
mod discovery;
mod effects;
mod hardware;
mod hid;
mod i18n;
mod kernel_led;
mod lamp_array;
mod monitor;
mod preview;
mod profiles;
mod tray;

use gtk::{gio, glib, prelude::*};
use i18n::tr;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

fn main() -> glib::ExitCode {
    if std::env::args_os().skip(1).eq(["--diagnostics"]) {
        let inventory = discovery::scan(std::path::Path::new("/sys"), std::path::Path::new("/dev"));
        println!("{}", inventory.diagnostics());
        return glib::ExitCode::SUCCESS;
    }
    let app = gtk::Application::builder()
        .application_id("io.github.linrgb.LinRgb")
        .build();
    app.set_accels_for_action("win.quit", &["<Primary>q"]);
    app.connect_activate(|app| {
        if let Some(window) = app.windows().into_iter().find(|window| window.is::<gtk::ApplicationWindow>()) {
            window.present();
            return;
        }
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("LinRgb")
            .default_width(1080)
            .default_height(660)
            .build();
        window.add_css_class("linrgb");
        let header = gtk::HeaderBar::new();
        header.set_show_title_buttons(false);
        appearance::install(&window, &header);
        let traffic = gtk::Box::new(gtk::Orientation::Horizontal, 11);
        traffic.set_valign(gtk::Align::Center);
        let tray_available = tray::install(&window).is_ok();
        for (symbol, class, tooltip) in [
            ("×", "close", if tray_available {
                tr("Close to the system tray", "Fermer dans la zone de notification")
            } else {
                tr("Close LinRgb", "Fermer LinRgb")
            }),
            ("−", "minimize", tr("Minimize window", "Réduire la fenêtre")),
            ("+", "maximize", tr("Maximize or restore window", "Agrandir ou restaurer la fenêtre")),
        ] {
            let button = gtk::Button::with_label(symbol);
            button.add_css_class("traffic");
            button.add_css_class(class);
            button.set_tooltip_text(Some(tooltip));
            button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
            let weak = window.downgrade();
            button.connect_clicked(move |_| {
                if let Some(window) = weak.upgrade() {
                    match class {
                        "close" => window.close(),
                        "minimize" => window.minimize(),
                        _ if window.is_maximized() => window.unmaximize(),
                        _ => window.maximize(),
                    }
                }
            });
            traffic.append(&button);
        }
        header.pack_start(&traffic);
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some(tr("Refresh device inventory", "Actualiser les périphériques")));
        header.pack_end(&refresh);
        window.set_titlebar(Some(&header));

        let layout = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 12);
        sidebar.add_css_class("sidebar");
        let identity = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let avatar = avatar::widget();
        avatar.set_valign(gtk::Align::Center);
        identity.append(&avatar);
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        titles.append(&label("LinRgb", "title-2"));
        titles.append(&label(tr("Lighting Studio", "Atelier lumière"), "dim-label"));
        identity.append(&titles);
        sidebar.append(&identity);
        let sidebar_toggle = gtk::ToggleButton::new();
        sidebar_toggle.set_icon_name("sidebar-show-symbolic");
        sidebar_toggle.set_tooltip_text(Some(tr("Show or hide sidebar", "Afficher ou masquer la barre latérale")));
        sidebar_toggle.set_active(true);
        sidebar_toggle.set_margin_start(8);
        let rail = sidebar.clone();
        sidebar_toggle.connect_toggled(move |toggle| rail.set_visible(toggle.is_active()));
        header.pack_start(&sidebar_toggle);
        layout.append(&sidebar);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 14);
        content.add_css_class("workspace");
        content.set_hexpand(true);
        content.set_valign(gtk::Align::Start);
        let hero = gtk::Box::new(gtk::Orientation::Vertical, 5);
        hero.add_css_class("hero");
        hero.append(&label(tr("LIGHTING STUDIO / SELECTED MODE", "ATELIER LUMIÈRE / MODE CHOISI"), "eyebrow"));
        let solid = tr("Solid color", "Couleur fixe");
        let fade = tr("Two-color fade", "Fondu bicolore");
        let solid_description = tr("Keep one color on the selected devices. Adjust Color A, then click Apply.", "Gardez une couleur fixe sur les périphériques. Réglez la couleur A, puis cliquez sur Appliquer.");
        let mode_title = label(solid, "title-1");
        let mode_description = label(solid_description, "dim-label");
        hero.append(&mode_title);
        hero.append(&mode_description);
        content.append(&hero);
        let workspace = adaptive::pair(12, 40);
        workspace.set_vexpand(false);
        let cards = workspace.clone();
        let sync_height = move |window: &gtk::ApplicationWindow| {
            adaptive::set_equal_height(&cards, window.is_maximized() || window.is_fullscreen());
        };
        window.connect_maximized_notify(sync_height.clone());
        window.connect_fullscreened_notify(sync_height);
        content.append(&workspace);
        let devices = gtk::Box::new(gtk::Orientation::Vertical, 9);
        devices.add_css_class("device-card");
        devices.append(&label(tr("Devices", "Périphériques"), "title-2"));
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(tr("Find a device…", "Rechercher un périphérique…")));
        devices.append(&search);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 5);
        devices.append(&list);
        let inventory = label("", "dim-label");
        devices.append(&inventory);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let all = gtk::Button::with_label(tr("Select all", "Tout sélectionner"));
        all.set_halign(gtk::Align::Start);
        devices.append(&all);
        monitor::install(&devices, &workspace, &window);
        workspace.append(&devices);
        let lighting = gtk::Box::new(gtk::Orientation::Vertical, 10);
        lighting.add_css_class("lighting-card");
        lighting.append(&label(tr("Lighting", "Éclairage"), "title-2"));
        workspace.append(&lighting);
        let breathing = tr("Breathing", "Respiration");
        let cycle = tr("Color cycle", "Cycle de couleurs");
        let mode = gtk::DropDown::from_strings(&[solid, fade, breathing, cycle]);
        mode.update_property(&[gtk::accessible::Property::Label(tr("Lighting mode", "Mode d’éclairage"))]);
        lighting.append(&mode);
        let colors = adaptive::pair(9, 50);
        let (first, hex) = color_wheel::picker(tr("Color A", "Couleur A"), [53, 132, 228]);
        let (second, end) = color_wheel::picker(tr("Color B", "Couleur B"), [145, 65, 172]);
        colors.append(&first);
        colors.append(&second);
        second.set_visible(false);
        lighting.append(&colors);
        let settings = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let period = gtk::SpinButton::with_range(2.0, 60.0, 1.0);
        period.set_value(6.0);
        period.update_property(&[gtk::accessible::Property::Label(tr("Full cycle in seconds", "Cycle complet en secondes"))]);
        let alternate = gtk::CheckButton::with_label(tr("Alternate with Color B", "Alterner avec la couleur B"));
        let saturation = gtk::SpinButton::with_range(0.0, 100.0, 5.0);
        saturation.set_value(100.0);
        let saturation_label = tr("Saturation (%)", "Saturation (%)");
        saturation.update_property(&[gtk::accessible::Property::Label(saturation_label)]);
        let saturation_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        saturation_row.append(&label(saturation_label, "dim-label"));
        saturation_row.append(&saturation);
        settings.append(&label(tr("Cycle duration (s)", "Durée du cycle (s)"), "dim-label"));
        settings.append(&period);
        lighting.append(&settings);
        lighting.append(&alternate);
        lighting.append(&saturation_row);
        let update_mode = {
            let (mode, alternate) = (mode.downgrade(), alternate.downgrade());
            move || {
                let (Some(mode), Some(alternate)) = (mode.upgrade(), alternate.upgrade()) else { return; };
                let index = mode.selected();
                first.set_visible(index != 3);
                second.set_visible(index == 1 || (index == 2 && alternate.is_active()));
                settings.set_visible(index != 0);
                alternate.set_visible(index == 2);
                saturation_row.set_visible(index == 3);
                let (title, description) = match index {
                    1 => (fade, tr("Hold each color, then fade briefly and smoothly to the other.", "Chaque couleur reste fixe avant un court fondu.")),
                    2 => (breathing, tr("Light rises and falls to black. Optionally alternate colors between breaths.", "La lumière monte puis s’éteint. Alternez les couleurs à chaque respiration si souhaité.")),
                    3 => (cycle, tr("Cycle through the color spectrum in sync on all selected devices.", "Parcourez le spectre des couleurs en synchronisant les périphériques sélectionnés.")),
                    _ => (solid, solid_description),
                };
                mode_title.set_text(title);
                mode_description.set_text(description);
            }
        };
        update_mode();
        let update = update_mode.clone();
        mode.connect_selected_notify(move |_| update());
        alternate.connect_toggled(move |_| update_mode());
        profiles::install(&sidebar, &mode, &hex, &end, &period, &alternate, &saturation);
        let job = Rc::new(RefCell::new(
            None::<(Arc<AtomicBool>, std::thread::JoinHandle<()>, Vec<effects::Target>)>,
        ));
        let running = job.clone();
        let quitting = Rc::new(Cell::new(false));
        let explicit_quit = quitting.clone();
        let weak = window.downgrade();
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                explicit_quit.set(true);
                window.close();
            }
        });
        window.add_action(&quit);
        window.connect_close_request(move |window| {
            if tray_available && !quitting.get() {
                window.hide();
                return glib::Propagation::Stop;
            }
            if let Some((flag, _, _)) = running.borrow().as_ref() {
                flag.store(true, Ordering::Relaxed);
            }
            glib::Propagation::Proceed
        });
        let apply = gtk::Button::with_label(tr("Apply", "Appliquer"));
        apply.add_css_class("suggested-action");
        let reset = gtk::Button::with_label(tr("Reset to white", "Revenir au blanc"));
        reset.set_tooltip_text(Some(
            tr("Stop the effect and set the selected devices to solid white", "Arrêter l’effet et régler les périphériques sélectionnés sur un blanc fixe"),
        ));
        controls.append(&apply);
        controls.append(&reset);
        lighting.append(&controls);
        let status = label("", "dim-label");
        lighting.append(&status);
        content.append(&preview::panel(&mode, &hex, &end, &period, &alternate, &saturation));
        let buttons = [apply.downgrade(), reset.downgrade()];
        let lifecycle_job = job.clone();
        let lifecycle_status = status.downgrade();
        let lifecycle_buttons = buttons.clone();
        let selected_targets = device_list::install(&list, &inventory, &search, &refresh, &all, move |available| {
            if let Some((flag, worker, targets)) = lifecycle_job.borrow().as_ref()
                && !worker.is_finished()
                && !targets.iter().all(|t| available.iter().any(|d| d.access == discovery::Access::Available && d.target == *t))
                && !flag.swap(true, Ordering::Release) {
                if let Some(label) = lifecycle_status.upgrade() { label.set_text(tr("RGB operation stopped: device availability changed.", "Éclairage arrêté : disponibilité des périphériques modifiée.")); }
                for b in lifecycle_buttons.iter().filter_map(|b| b.upgrade()) { b.set_sensitive(true); }
            }
        });
        let submit = move |reset: bool| {
            if buttons.iter().filter_map(|button| button.upgrade()).any(|button| !button.is_sensitive()) {
                return;
            }
            let effect = if reset {
                Some(effects::Effect::Solid([255; 3]))
            } else {
                effects::Effect::from_inputs(mode.selected(), &hex.text(), &end.text(), alternate.is_active(), saturation.value(), period.value())
            };
            let Some(effect) = effect else {
                status.set_text(tr("Enter valid HEX colors for this effect.", "Saisissez des couleurs HEX valides pour cet effet."));
                return;
            };
            let Some(targets) = selected_targets() else { status.set_text(tr("Waiting for a fresh device inventory.", "En attente d’un inventaire à jour.")); return; };
            if targets.is_empty() {
                status.set_text(tr("No devices selected.", "Aucun périphérique sélectionné."));
                return;
            }
            let endpoints = targets.iter().map(|(_, target)| target.clone()).collect();
            let previous = job.borrow_mut().take().map(|(flag, worker, _)| {
                flag.store(true, Ordering::Relaxed);
                worker
            });
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = cancel.clone();
            let (send, receive) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                if let Some(previous) = previous {
                    let _ = previous.join();
                }
                if !flag.load(Ordering::Relaxed) {
                    effects::run(targets, effect, &flag, &|name, state| { let _ = send.send((name, state)); });
                }
            });
            *job.borrow_mut() = Some((cancel.clone(), worker, endpoints));
            if reset {
                mode.set_selected(0);
                hex.set_text("#FFFFFF");
            }
            status.set_text(if effect.animated() {
                tr("Starting effect…", "Démarrage de l’effet…")
            } else {
                tr("Applying color…", "Application de la couleur…")
            });
            for button in buttons.iter().filter_map(|button| button.upgrade()) { button.set_sensitive(false); }
            let (status, buttons) = (status.downgrade(), buttons.clone());
            let mut states = std::collections::BTreeMap::new();
            glib::timeout_add_local(Duration::from_millis(100), move || {
                use std::sync::mpsc::TryRecvError;
                let Some(status) = status.upgrade() else { return glib::ControlFlow::Break; };
                if cancel.load(Ordering::Relaxed) { return glib::ControlFlow::Break; }
                let mut changed = false;
                let finished = loop {
                    match receive.try_recv() {
                        Ok((name, state)) => { states.insert(name, state); changed = true; }
                        Err(TryRecvError::Empty) => break false,
                        Err(TryRecvError::Disconnected) => break true,
                    }
                };
                if changed {
                    status.set_text(&states.iter().map(|(name, state)| format!("{name}: {state}")).collect::<Vec<_>>().join("\n"));
                } else if finished && states.is_empty() {
                    status.set_text(tr("RGB operation interrupted.", "Opération RGB interrompue."));
                }
                if changed || finished {
                    for button in buttons.iter().filter_map(|button| button.upgrade()) { button.set_sensitive(true); }
                }
                if finished { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
            });
        };
        let apply_settings = submit.clone();
        apply.connect_clicked(move |_| apply_settings(false));
        reset.connect_clicked(move |_| submit(true));
        let scroll = gtk::ScrolledWindow::builder()
            .child(&content)
            .hexpand(true)
            .build();
        layout.append(&scroll);
        window.set_child(Some(&layout));
        window.present();
    });
    app.run()
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}
