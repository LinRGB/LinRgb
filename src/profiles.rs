use crate::i18n::tr;
use gtk::{glib, prelude::*};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
};

type Profiles = BTreeMap<String, Value>;

fn persist(path: &Path, profiles: &Profiles) -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(|error| error.to_string())?;
    let text = serde_json::to_vec_pretty(profiles).expect("JSON values serialize");
    glib::file_set_contents(path, &text).map_err(|error| error.to_string())
}

fn change(profiles: &mut Profiles, old: &str, new: Option<&str>) -> Result<(), &'static str> {
    if let Some(new) = new {
        if new.is_empty() || new.chars().count() > 48 {
            return Err(tr(
                "Enter a profile name of 1–48 characters.",
                "Saisissez un nom de profil de 1 à 48 caractères.",
            ));
        }
        if new == old {
            return Ok(());
        }
        if profiles.contains_key(new) {
            return Err(tr(
                "A profile with that name already exists.",
                "Un profil porte déjà ce nom.",
            ));
        }
    }
    let profile = profiles.remove(old).ok_or(tr(
        "This profile no longer exists.",
        "Ce profil n’existe plus.",
    ))?;
    if let Some(new) = new {
        profiles.insert(new.to_owned(), profile);
    }
    Ok(())
}

pub fn config_dir() -> PathBuf {
    glib::user_config_dir().join("LinRGB")
}

fn parse(value: &Value) -> Option<(&str, &str, u32, f64, bool, f64)> {
    let first = value["color_a"].as_str()?;
    let second = value["color_b"].as_str()?;
    let mode = if let Some(mode) = value.get("mode") {
        u32::try_from(mode.as_u64()?).ok()?
    } else {
        u32::from(value["fade"].as_bool()?)
    };
    let seconds = value["cycle_seconds"].as_f64()?;
    let alternate = value["alternate"].as_bool().unwrap_or(false);
    let saturation = value["saturation"].as_f64().unwrap_or(100.0);
    crate::effects::Effect::from_inputs(mode, first, second, alternate, saturation, seconds)?;
    Some((first, second, mode, seconds, alternate, saturation))
}

pub fn install(
    sidebar: &gtk::Box,
    mode: &gtk::DropDown,
    first: &gtk::Entry,
    second: &gtk::Entry,
    period: &gtk::SpinButton,
    alternate: &gtk::CheckButton,
    saturation: &gtk::SpinButton,
) -> gtk::ListBox {
    let path = config_dir().join("profiles.json");
    let loaded = match fs::read(&path) {
        Ok(data) => {
            serde_json::from_slice::<BTreeMap<String, Value>>(&data).map_err(io::Error::other)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error),
    };
    let choices = gtk::ListBox::new();
    choices.add_css_class("profile-list");
    choices.set_selection_mode(gtk::SelectionMode::Single);
    choices.update_property(&[gtk::accessible::Property::Label(tr(
        "Saved profiles",
        "Profils enregistrés",
    ))]);
    choices.set_placeholder(Some(&crate::label(
        tr("No saved profiles yet", "Aucun profil enregistré"),
        "dim-label",
    )));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&choices)
        .build();
    let name = gtk::Entry::new();
    name.set_placeholder_text(Some(tr("Profile name", "Nom du profil")));
    name.set_max_length(48);
    name.update_property(&[gtk::accessible::Property::Label(tr(
        "Profile name",
        "Nom du profil",
    ))]);
    let save = gtk::Button::with_label(tr("Save profile", "Enregistrer le profil"));
    let status = crate::label("", "dim-label");
    status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    status.set_width_chars(1);
    status.set_max_width_chars(23);
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    section.add_css_class("profiles-panel");
    section.set_margin_top(8);
    section.append(&crate::label(tr("Profiles", "Profils"), "sidebar-item"));
    section.append(&scroll);
    section.append(&name);
    section.append(&save);
    section.append(&status);
    sidebar.append(&section);
    let profiles = match loaded {
        Ok(profiles) => Rc::new(RefCell::new(profiles)),
        Err(error) => {
            choices.set_sensitive(false);
            save.set_sensitive(false);
            status.set_text(&format!(
                "{}: {error}",
                tr("Could not read profiles", "Impossible de lire les profils")
            ));
            return choices;
        }
    };
    let manage = {
        let profiles = profiles.clone();
        let path = path.clone();
        let input = name.clone();
        move |old: &str, new: Option<&str>| {
            let mut updated = profiles.borrow().clone();
            change(&mut updated, old, new).map_err(str::to_owned)?;
            persist(&path, &updated)?;
            profiles.replace(updated);
            if input.text().as_str() == old {
                input.set_text(new.unwrap_or(""));
            }
            Ok(())
        }
    };
    for (name, _) in profiles
        .borrow()
        .iter()
        .filter(|(_, profile)| parse(profile).is_some())
    {
        append_profile(&choices, name, &status, manage.clone());
    }
    let stored = profiles.clone();
    let (a, b, lighting, duration, alternating, intensity, input, message) = (
        first.clone(),
        second.clone(),
        mode.clone(),
        period.clone(),
        alternate.clone(),
        saturation.clone(),
        name.clone(),
        status.clone(),
    );
    choices.connect_row_selected(move |_, row| {
        let Some(label) = row.and_then(row_label) else {
            return;
        };
        let id = label.text();
        let stored = stored.borrow();
        let Some((first, second, mode, seconds, alternate, saturation)) =
            stored.get(id.as_str()).and_then(parse)
        else {
            return;
        };
        a.set_text(first);
        b.set_text(second);
        lighting.set_selected(mode);
        duration.set_value(seconds);
        alternating.set_active(alternate);
        intensity.set_value(saturation);
        input.set_text(&id);
        message.set_text(tr(
            "Profile loaded. Click Apply.",
            "Profil chargé. Cliquez sur Appliquer.",
        ));
    });
    let (first, second, mode, period, alternate, saturation) = (
        first.clone(),
        second.clone(),
        mode.clone(),
        period.clone(),
        alternate.clone(),
        saturation.clone(),
    );
    let list = choices.clone();
    save.connect_clicked(move |_| {
        let name = name.text();
        let name = name.trim();
        if name.is_empty() {
            status.set_text(tr("Enter a profile name.", "Saisissez un nom de profil."));
            return;
        }
        let profile = json!({"color_a": first.text().as_str(), "color_b": second.text().as_str(), "mode": mode.selected(), "cycle_seconds": period.value(), "alternate": alternate.is_active(), "saturation": saturation.value()});
        if parse(&profile).is_none() {
            status.set_text(tr("Enter valid HEX colors before saving.", "Saisissez des couleurs HEX valides avant d’enregistrer."));
            return;
        }
        let mut updated = profiles.borrow().clone();
        updated.insert(name.to_owned(), profile);
        if let Err(error) = persist(&path, &updated) {
            status.set_text(&format!("{}: {error}", tr("Could not save profile", "Impossible d’enregistrer le profil")));
            return;
        }
        profiles.replace(updated);
        let row = (0..).map_while(|index| choices.row_at_index(index))
            .find(|row| row_label(row).is_some_and(|label| label.text().as_str() == name))
            .unwrap_or_else(|| append_profile(&choices, name, &status, manage.clone()));
        choices.select_row(Some(&row));
        status.set_text(tr("Profile saved.", "Profil enregistré."));
    });
    list
}

pub fn row_label(row: &gtk::ListBoxRow) -> Option<gtk::Label> {
    row.child()?.first_child()?.downcast().ok()
}

fn append_profile(
    list: &gtk::ListBox,
    name: &str,
    status: &gtk::Label,
    manage: impl Fn(&str, Option<&str>) -> Result<(), String> + Clone + 'static,
) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let row = gtk::ListBoxRow::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .build();
    menu.set_tooltip_text(Some(tr("Profile actions", "Actions du profil")));
    menu.update_property(&[gtk::accessible::Property::Label(tr(
        "Profile actions",
        "Actions du profil",
    ))]);
    let popover = gtk::Popover::new();
    let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for (title, deleting) in [
        (tr("Rename", "Renommer"), false),
        (tr("Delete", "Supprimer"), true),
    ] {
        let button = gtk::Button::with_label(title);
        if deleting {
            button.add_css_class("destructive-action");
        }
        let (row, popover, list) = (row.downgrade(), popover.downgrade(), list.downgrade());
        let (status, manage) = (status.clone(), manage.clone());
        button.connect_clicked(move |_| {
            if let Some(popover) = popover.upgrade() {
                popover.popdown();
            }
            let Some(row) = row.upgrade() else {
                return;
            };
            let Some(label) = row_label(&row) else {
                return;
            };
            let Some(window) = row.root().and_downcast::<gtk::Window>() else {
                return;
            };
            let old = label.text();
            let dialog = gtk::Dialog::builder()
                .transient_for(&window)
                .modal(true)
                .title(if deleting {
                    tr("Delete profile?", "Supprimer le profil ?")
                } else {
                    tr("Rename profile", "Renommer le profil")
                })
                .build();
            dialog.add_css_class("linrgb");
            dialog.add_button(tr("Cancel", "Annuler"), gtk::ResponseType::Cancel);
            let accept = dialog.add_button(title, gtk::ResponseType::Accept);
            if deleting {
                accept.add_css_class("destructive-action");
            }
            dialog.set_default_response(if deleting {
                gtk::ResponseType::Cancel
            } else {
                gtk::ResponseType::Accept
            });
            let body = dialog.content_area();
            body.set_margin_top(12);
            body.set_margin_bottom(12);
            body.set_margin_start(12);
            body.set_margin_end(12);
            let entry = gtk::Entry::new();
            if deleting {
                body.append(&crate::label(
                    &format!(
                        "{old}\n{}",
                        tr(
                            "This deletion cannot be undone.",
                            "Cette suppression est irréversible."
                        )
                    ),
                    "",
                ));
            } else {
                entry.set_text(&old);
                entry.set_max_length(48);
                entry.set_activates_default(true);
                entry.update_property(&[gtk::accessible::Property::Label(tr(
                    "New profile name",
                    "Nouveau nom du profil",
                ))]);
                body.append(&entry);
                entry.grab_focus();
            }
            let message = crate::label("", "dim-label");
            body.append(&message);
            let (status, manage, list) = (status.clone(), manage.clone(), list.clone());
            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    let text = entry.text();
                    let new = (!deleting).then(|| text.trim());
                    if let Err(error) = manage(&old, new) {
                        message.set_text(&error);
                        return;
                    }
                    if let Some(new) = new {
                        label.set_text(new);
                        row.set_tooltip_text(Some(new));
                        status.set_text(tr("Profile renamed.", "Profil renommé."));
                    } else {
                        if let Some(list) = list.upgrade() {
                            list.remove(&row);
                        }
                        status.set_text(tr("Profile deleted.", "Profil supprimé."));
                    }
                }
                dialog.destroy();
            });
            dialog.present();
        });
        actions.append(&button);
    }
    popover.set_child(Some(&actions));
    menu.set_popover(Some(&popover));
    content.append(&label);
    content.append(&menu);
    row.set_child(Some(&content));
    row.set_tooltip_text(Some(name));
    list.append(&row);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_preserves_profiles_and_rejects_collisions() {
        let mut profiles = Profiles::from([("A".into(), json!(1)), ("B".into(), json!(2))]);
        assert!(change(&mut profiles, "A", Some("B")).is_err());
        assert_eq!(profiles["A"], json!(1));
        assert_eq!(profiles["B"], json!(2));
        change(&mut profiles, "A", Some("C")).unwrap();
        assert_eq!(profiles["C"], json!(1));
        assert!(!profiles.contains_key("A"));
        change(&mut profiles, "C", None).unwrap();
        assert_eq!(profiles, Profiles::from([("B".into(), json!(2))]));
    }

    #[test]
    fn validates_a_profile_before_loading() {
        let mut profile =
            json!({"color_a":"#FF0000", "color_b":"#0000FF", "fade":true, "cycle_seconds":5.0});
        assert_eq!(
            parse(&profile),
            Some(("#FF0000", "#0000FF", 1, 5.0, false, 100.0))
        );
        profile["mode"] = json!(2);
        profile["alternate"] = json!(true);
        profile["saturation"] = json!(75);
        assert_eq!(
            parse(&profile),
            Some(("#FF0000", "#0000FF", 2, 5.0, true, 75.0))
        );
        profile["cycle_seconds"] = json!(0);
        assert!(parse(&profile).is_none());
    }
}
