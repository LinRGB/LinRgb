//! Palette editing stays on GTK's thread. Workers receive only the compiled RGB array.
use crate::{effects::Palette, i18n::tr};
use gtk::prelude::*;

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

// An invalid custom cycle must not silently become a classic spectrum cycle.
pub fn effect(
    mode: u32,
    first: &str,
    second: &str,
    alternate: bool,
    saturation: f64,
    seconds: f64,
    model: Option<&gtk::StringList>,
) -> Option<crate::effects::Effect> {
    let palette = match model {
        None => None,
        Some(m) => {
            let colors = (0..m.n_items())
                .map(|i| m.string(i))
                .collect::<Option<Vec<_>>>()?;
            Some(Palette::parse(colors.iter().map(|s| s.as_str()))?)
        }
    };
    crate::effects::Effect::from_inputs(
        mode, first, second, alternate, saturation, seconds, palette,
    )
}

pub fn row(model: &gtk::StringList) -> gtk::Box {
    let rail = gtk::Box::new(gtk::Orientation::Vertical, 4);
    rail.set_halign(gtk::Align::Center);
    rail.set_valign(gtk::Align::Center);
    let slots = gtk::ListBox::new();
    slots.set_selection_mode(gtk::SelectionMode::Browse);
    slots.add_css_class("palette-slots");
    rail.append(&slots);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let add = gtk::Button::from_icon_name("list-add-symbolic");
    let remove = gtk::Button::from_icon_name("list-remove-symbolic");
    for (button, title) in [
        (&add, tr("Add color", "Ajouter une couleur")),
        (
            &remove,
            tr("Remove selected color", "Retirer la couleur sélectionnée"),
        ),
    ] {
        button.set_tooltip_text(Some(title));
        button.update_property(&[gtk::accessible::Property::Label(title)]);
        actions.append(button);
    }
    rail.append(&actions);
    let (panel, hex) =
        crate::color_wheel::picker_with_slots(tr("Palette", "Palette"), [0; 3], Some(&rail));
    for i in 0..5 {
        let chip = gtk::DrawingArea::new();
        chip.add_css_class("palette-chip");
        let m = model.clone();
        chip.set_draw_func(move |_, cr, w, h| {
            if let Some(rgb) = m.string(i).and_then(|s| crate::color_wheel::parse_hex(&s)) {
                let [r, g, b] = rgb.map(|c| f64::from(c) / 255.);
                cr.set_source_rgb(r, g, b);
                let _ = cr.paint();
            } else {
                cr.move_to(0., h as f64);
                cr.line_to(w as f64, 0.);
                cr.set_source_rgb(0.65, 0.45, 0.45);
                let _ = cr.stroke();
            }
        });
        let drawing = chip.downgrade();
        model.connect_items_changed(move |_, pos, _, _| {
            if pos <= i
                && let Some(chip) = drawing.upgrade()
            {
                chip.queue_draw();
            }
        });
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        strip.append(&crate::label(&(i + 1).to_string(), "dim-label"));
        strip.append(&chip);
        let slot = gtk::ListBoxRow::new();
        slot.set_child(Some(&strip));
        slots.append(&slot);
    }
    let m = model.clone();
    let list = slots.downgrade();
    let handler = hex.connect_changed(move |entry| {
        if let Some(slot) = list.upgrade().and_then(|l| l.selected_row()) {
            let i = slot.index() as u32;
            let text = entry.text();
            if m.string(i).is_some_and(|old| old != text) {
                m.splice(i, 1, &[&text]);
            }
        }
    });
    let handler = std::rc::Rc::new(handler);
    let entry = hex.downgrade();
    let load = move |m: &gtk::StringList, i: i32| {
        if let (Some(entry), Some(text)) = (entry.upgrade(), m.string(i as u32))
            && entry.text() != text
        {
            entry.block_signal(&handler);
            entry.set_text(&text);
            entry.unblock_signal(&handler);
        }
    };
    let select = load.clone();
    let m = model.clone();
    slots.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            select(&m, row.index());
        }
    });
    let list = slots.downgrade();
    let buttons = (add.downgrade(), remove.downgrade());
    let update = move |m: &gtk::StringList| {
        let Some(list) = list.upgrade() else {
            return;
        };
        let n = m.n_items() as i32;
        let active = list.selected_row().map_or(0, |r| r.index()).min(n - 1);
        for i in 0..5 {
            let slot = list.row_at_index(i).unwrap();
            slot.set_visible(i < n);
            if let Some(text) = m.string(i as u32) {
                let name = format!("{} {} · {}", tr("Color", "Couleur"), i + 1, text);
                slot.update_property(&[gtk::accessible::Property::Label(&name)]);
                slot.set_tooltip_text(Some(&name));
            }
        }
        list.select_row(list.row_at_index(active).as_ref());
        load(m, active);
        if let (Some(add), Some(remove)) = (buttons.0.upgrade(), buttons.1.upgrade()) {
            add.set_sensitive(n < 5);
            remove.set_sensitive(n > 2);
        }
    };
    update(model);
    model.connect_items_changed(move |m, _, _, _| update(m));
    let m = model.clone();
    let list = slots.downgrade();
    add.connect_clicked(move |_| {
        if m.n_items() < 5 {
            m.append("#F4A96C");
            if let Some(list) = list.upgrade() {
                list.select_row(list.row_at_index(m.n_items() as i32 - 1).as_ref());
            }
        }
    });
    let m = model.clone();
    remove.connect_clicked(move |_| {
        if m.n_items() > 2
            && let Some(row) = slots.selected_row()
        {
            m.remove(row.index() as u32);
        }
    });
    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a GTK display; no hardware access"]
    fn edit_palette_widgets() {
        gtk::init().unwrap();
        let controls = Controls::new(&gtk::SpinButton::with_range(0., 100., 1.));
        assert!(controls.model(3).is_none());
        let panel = row(&controls.aurora);
        fn visit(w: &gtk::Widget, all: &mut Vec<gtk::Widget>) {
            all.push(w.clone());
            for c in std::iter::successors(w.first_child(), |c| c.next_sibling()) {
                visit(&c, all);
            }
        }
        let mut all = Vec::new();
        visit(panel.upcast_ref(), &mut all);
        assert_eq!(
            all.iter()
                .filter(|w| w.has_css_class("color-wheel"))
                .count(),
            1
        );
        let slots = all
            .iter()
            .find_map(|w| w.downcast_ref::<gtk::ListBox>())
            .unwrap();
        let hex = all
            .iter()
            .find_map(|w| w.downcast_ref::<gtk::Entry>())
            .unwrap();
        let slider = all
            .iter()
            .find_map(|w| w.downcast_ref::<gtk::Scale>())
            .unwrap();
        let action = |icon: &str| {
            all.iter()
                .filter_map(|w| w.downcast_ref::<gtk::Button>())
                .find(|b| b.icon_name().as_deref() == Some(icon))
                .unwrap()
        };
        let (add, remove) = (action("list-add-symbolic"), action("list-remove-symbolic"));
        let original = controls.colors(4).unwrap();
        slots.select_row(slots.row_at_index(1).as_ref());
        assert_eq!(hex.text(), original[1]);
        assert_eq!(controls.colors(4).unwrap(), original);
        slider.set_value(0.);
        assert_eq!(controls.aurora.string(1).unwrap(), "#000000");
        hex.set_text("#AACC33");
        let saved = controls.colors(4).unwrap();
        assert_eq!(
            saved,
            [original[0].as_str(), "#AACC33", original[2].as_str()]
        );
        add.emit_clicked();
        add.emit_clicked();
        assert!(!add.is_sensitive());
        assert_eq!(slots.selected_row().unwrap().index(), 4);
        hex.set_text("#123456");
        assert_eq!(controls.aurora.string(4).unwrap(), "#123456");
        remove.emit_clicked();
        remove.emit_clicked();
        remove.emit_clicked();
        assert!(!remove.is_sensitive());
        assert_eq!(hex.text(), "#AACC33");
        controls.restore(4, Some(&saved));
        assert_eq!(controls.colors(4).unwrap(), saved);
        controls.restore(3, Some(&saved));
        assert_eq!(controls.colors(3), controls.colors(4));
        controls.cycle.splice(0, 1, &["#XYZ"]);
        assert!(effect(3, "", "", false, 100., 6., controls.model(3)).is_none());
        controls.restore(3, None);
        assert!(!controls.custom.is_active());
    }
}
