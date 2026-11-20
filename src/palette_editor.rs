//! Palette editing stays on GTK's thread. Workers receive only the compiled RGB array.
use crate::{effects::Palette, i18n::tr};
use gtk::{gdk, prelude::*};

#[derive(Clone)]
pub struct Controls {
    pub saturation: gtk::SpinButton,
    pub custom: gtk::CheckButton,
    pub cycle: gtk::StringList,
    pub aurora: gtk::StringList,
}
impl Controls {
    pub fn new(saturation: &gtk::SpinButton) -> Self {
        Self {
            saturation: saturation.clone(),
            custom: gtk::CheckButton::with_label(tr(
                "Use custom palette",
                "Utiliser une palette personnalisée",
            )),
            cycle: gtk::StringList::new(&["#FF7066", "#FFCA61", "#5784FF"]),
            aurora: gtk::StringList::new(&["#136B83", "#64E6B0", "#7956C8"]),
        }
    }
    pub fn model(&self, mode: u32) -> Option<&gtk::StringList> {
        match mode {
            3 if self.custom.is_active() => Some(&self.cycle),
            4 => Some(&self.aurora),
            _ => None,
        }
    }
    pub fn colors(&self, mode: u32) -> Option<Vec<String>> {
        self.model(mode).map(|m| {
            (0..m.n_items())
                .filter_map(|i| m.string(i).map(|s| s.to_string()))
                .collect()
        })
    }
    pub fn palette(&self, mode: u32) -> Option<Palette> {
        compile(self.model(mode)?)
    }
    pub fn restore(&self, mode: u32, colors: Option<&[String]>) {
        if mode == 3 {
            self.custom.set_active(colors.is_some());
        }
        if let (Some(m), Some(colors)) = (self.model(mode), colors) {
            m.splice(
                0,
                m.n_items(),
                &colors.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
    }
}

pub fn compile(m: &gtk::StringList) -> Option<Palette> {
    let colors = (0..m.n_items())
        .filter_map(|i| m.string(i))
        .collect::<Vec<_>>();
    Palette::parse(colors.iter().map(|s| s.as_str()))
}

fn wheel(model: &gtk::StringList, i: u32) -> gtk::Box {
    let title = format!("{} {}", tr("Color", "Couleur"), i + 1);
    let (picker, hex) = crate::color_wheel::picker(&title, [0; 3]);
    let m = model.clone();
    let handler = hex.connect_changed(move |entry| {
        let text = entry.text();
        if m.string(i).is_some_and(|old| old != text) {
            m.splice(i, 1, &[text.as_str()]);
        }
    });
    let hex = hex.downgrade();
    let refresh = move |m: &gtk::StringList| {
        if let (Some(entry), Some(text)) = (hex.upgrade(), m.string(i))
            && entry.text() != text
        {
            entry.block_signal(&handler);
            entry.set_text(&text);
            entry.unblock_signal(&handler);
        }
    };
    refresh(model);
    model.connect_items_changed(move |m, _, _, _| refresh(m));
    picker
}

pub fn row(model: &gtk::StringList, wheels: bool) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.append(&crate::label(
        if wheels {
            tr("Aurora palette", "Palette de l’aurore")
        } else {
            tr(
                "Palette · click a color to edit",
                "Palette · cliquez sur une couleur pour la modifier",
            )
        },
        "dim-label",
    ));
    let chips = gtk::FlowBox::new();
    chips.set_selection_mode(gtk::SelectionMode::None);
    chips.set_min_children_per_line(1);
    if wheels {
        chips.set_max_children_per_line(2);
    }
    row.append(&chips);
    let add = gtk::Button::with_label(tr("Add color", "Ajouter une couleur"));
    add.set_halign(gtk::Align::Start);
    row.append(&add);
    let m = model.clone();
    add.connect_clicked(move |_| {
        if m.n_items() < 5 {
            m.append("#F4A96C");
        }
    });
    let mut widgets = Vec::new();
    for i in 0..5 {
        let pair = gtk::Box::new(
            if wheels {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            },
            1,
        );
        let remove = gtk::Button::with_label("−");
        remove.set_tooltip_text(Some(tr("Remove this color", "Retirer cette couleur")));
        let m = model.clone();
        remove.connect_clicked(move |_| {
            if i < m.n_items() && m.n_items() > 2 {
                m.remove(i);
            }
        });
        let button = if wheels {
            pair.append(&wheel(model, i));
            None
        } else {
            let button = gtk::ColorButton::new();
            button.set_use_alpha(false);
            button.set_title(tr("Palette color", "Couleur de la palette"));
            let m = model.clone();
            button.connect_color_set(move |b| {
                if i < m.n_items() {
                    let rgb = b.rgba();
                    let [r, g, b] =
                        [rgb.red(), rgb.green(), rgb.blue()].map(|x| (255. * x).round() as u8);
                    m.splice(i, 1, &[&format!("#{r:02X}{g:02X}{b:02X}")]);
                }
            });
            pair.append(&button);
            Some(button.downgrade())
        };
        pair.append(&remove);
        let child = gtk::FlowBoxChild::new();
        child.set_child(Some(&pair));
        chips.insert(&child, -1);
        widgets.push((child.downgrade(), button, remove.downgrade()));
    }
    let add = add.downgrade();
    let update = move |m: &gtk::StringList| {
        if let Some(add) = add.upgrade() {
            add.set_sensitive(m.n_items() < 5);
        }
        for (i, (child, btn, remove)) in widgets.iter().enumerate() {
            if let (Some(child), Some(remove)) = (child.upgrade(), remove.upgrade()) {
                let c = m.string(i as u32);
                child.set_visible(c.is_some());
                if let Some(btn) = btn.as_ref().and_then(|b| b.upgrade())
                    && let Some(c) = c
                    && let Ok(rgb) = gdk::RGBA::parse(c)
                {
                    btn.set_rgba(&rgb);
                }
                remove.set_sensitive(m.n_items() > 2);
            }
        }
    };
    update(model);
    model.connect_items_changed(move |m, _, _, _| update(m));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GTK display; no hardware access"]
    fn edit_palette_widgets() {
        gtk::init().unwrap();
        let controls = Controls::new(&gtk::SpinButton::with_range(0., 100., 1.));
        assert!(controls.palette(3).is_none());
        let panel = row(&controls.aurora, true);
        let add = panel
            .last_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        let chips = add
            .prev_sibling()
            .unwrap()
            .downcast::<gtk::FlowBox>()
            .unwrap();
        let pair = chips.child_at_index(1).unwrap().child().unwrap();
        let picker = pair.first_child().unwrap();
        let mut children = std::iter::successors(picker.first_child(), |w| w.next_sibling());
        assert!(
            children.any(|w| w.is::<gtk::DrawingArea>()),
            "Aurora needs an inline color wheel"
        );
        let hex = children
            .find_map(|w| w.downcast::<gtk::Entry>().ok())
            .unwrap();
        let brightness = children
            .find_map(|w| w.downcast::<gtk::Scale>().ok())
            .unwrap();
        let remove = pair
            .last_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        add.emit_clicked();
        add.emit_clicked();
        assert_eq!(controls.aurora.n_items(), 5);
        assert!(!add.is_sensitive());
        brightness.set_value(0.);
        assert_eq!(controls.aurora.string(1).unwrap(), "#000000");
        hex.set_text("ABC");
        assert!(controls.palette(4).is_none());
        hex.set_text("#AACC33");
        let saved = controls.colors(4).unwrap();
        assert_eq!(saved[1], "#AACC33");
        remove.emit_clicked();
        remove.emit_clicked();
        remove.emit_clicked();
        assert_eq!(controls.aurora.n_items(), 2);
        assert!(!remove.is_sensitive());
        controls.restore(4, Some(&saved));
        assert_eq!(controls.colors(4).unwrap(), saved);
        assert_eq!(hex.text(), saved[1]);
        controls.restore(3, Some(&saved));
        assert_eq!(controls.palette(3), controls.palette(4));
        controls.restore(3, None);
        assert!(!controls.custom.is_active());
    }
}
