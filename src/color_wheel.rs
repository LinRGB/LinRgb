use crate::i18n::tr;
use gtk::{cairo, gdk, glib, prelude::*};
use std::{cell::Cell, f64::consts::TAU, rc::Rc};

const TEXTURE_SIZE: i32 = 304;

fn geometry(width: i32, height: i32) -> (f64, f64, f64) {
    let (x, y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    (x, y, x.min(y) * 68.0 / 76.0)
}

thread_local! {
    static WHEEL: cairo::ImageSurface = {
        let stride = cairo::Format::ARgb32.stride_for_width(TEXTURE_SIZE as u32).expect("valid wheel size");
        let mut pixels = vec![0; (stride * TEXTURE_SIZE) as usize];
        let (cx, cy, radius) = geometry(TEXTURE_SIZE, TEXTURE_SIZE);
        for y in 0..TEXTURE_SIZE {
            for x in 0..TEXTURE_SIZE {
                let [r, g, b] = rgb_at((f64::from(x) + 0.5 - cx) / radius, (f64::from(y) + 0.5 - cy) / radius, 1.0);
                let offset = (y * stride + x * 4) as usize;
                pixels[offset..offset + 4].copy_from_slice(&u32::from_be_bytes([255, r, g, b]).to_ne_bytes());
            }
        }
        cairo::ImageSurface::create_for_data(pixels, cairo::Format::ARgb32, TEXTURE_SIZE, TEXTURE_SIZE, stride).expect("valid wheel surface")
    };
}

pub fn parse_hex(input: &str) -> Option<[u8; 3]> {
    let input = input.trim();
    let digits = input.strip_prefix('#').unwrap_or(input);
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let [_, red, green, blue] = u32::from_str_radix(digits, 16).ok()?.to_be_bytes();
    Some([red, green, blue])
}

fn rgb_at(x: f64, y: f64, value: f64) -> [u8; 3] {
    let hue = y.atan2(x).rem_euclid(TAU) / TAU * 6.0;
    let saturation = x.hypot(y).min(1.0);
    [5.0, 3.0, 1.0].map(|offset| {
        let k = (offset + hue) % 6.0;
        (255.0 * value * (1.0 - saturation * k.min(4.0 - k).clamp(0.0, 1.0))).round() as u8
    })
}

fn wheel_position(rgb: [u8; 3]) -> ((f64, f64), f64) {
    let [r, g, b] = rgb.map(f64::from);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    if delta == 0.0 {
        return ((0.0, 0.0), max / 255.0);
    }
    let sector = if max == r {
        (g - b) / delta
    } else if max == g {
        2.0 + (b - r) / delta
    } else {
        4.0 + (r - g) / delta
    };
    let angle = sector * TAU / 6.0;
    let saturation = delta / max;
    (
        (angle.cos() * saturation, angle.sin() * saturation),
        max / 255.0,
    )
}

pub fn picker(title: &str, initial: [u8; 3]) -> (gtk::Box, gtk::Entry) {
    picker_with_slots(title, initial, None)
}

pub fn picker_with_slots(
    title: &str,
    initial: [u8; 3],
    slots: Option<&gtk::Box>,
) -> (gtk::Box, gtk::Entry) {
    let area = gtk::DrawingArea::new();
    area.add_css_class("color-wheel");
    area.set_halign(gtk::Align::Center);
    area.set_focusable(true);
    area.set_tooltip_text(Some(tr(
        "Hue and saturation: mouse or arrow keys",
        "Teinte et saturation : souris ou flèches du clavier",
    )));
    area.update_property(&[gtk::accessible::Property::Label(title)]);
    let (position, value) = wheel_position(initial);
    let point = Rc::new(Cell::new(position));
    let brightness = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    brightness.set_value(value * 100.0);
    brightness.set_draw_value(true);
    brightness.update_property(&[gtk::accessible::Property::Label(tr(
        "Brightness",
        "Luminosité",
    ))]);
    let hex = gtk::Entry::new();
    hex.set_width_chars(9);
    hex.set_placeholder_text(Some("#RRGGBB"));
    let hex_label = format!("{title} · {}", tr("HEX code", "Code HEX"));
    hex.update_property(&[gtk::accessible::Property::Label(&hex_label)]);
    let syncing = Rc::new(Cell::new(false));
    let update_hex = {
        let point = point.clone();
        let syncing = syncing.clone();
        let brightness = brightness.downgrade();
        let hex = hex.downgrade();
        move || {
            if syncing.get() {
                return;
            }
            if let (Some(brightness), Some(hex)) = (brightness.upgrade(), hex.upgrade()) {
                let (x, y) = point.get();
                let [r, g, b] = rgb_at(x, y, brightness.value() / 100.0);
                syncing.set(true);
                hex.set_text(&format!("#{r:02X}{g:02X}{b:02X}"));
                hex.remove_css_class("error");
                syncing.set(false);
            }
        }
    };
    update_hex();
    let on_value = update_hex.clone();
    brightness.connect_value_changed(move |_| on_value());
    let selected = point.clone();
    let slider = brightness.downgrade();
    let drawing = area.downgrade();
    hex.connect_changed(move |hex| {
        if syncing.get() {
            return;
        }
        if let Some(rgb) = parse_hex(&hex.text()) {
            hex.remove_css_class("error");
            let (position, value) = wheel_position(rgb);
            selected.set(position);
            syncing.set(true);
            if let Some(slider) = slider.upgrade() {
                slider.set_value(value * 100.0);
            }
            syncing.set(false);
            if let Some(drawing) = drawing.upgrade() {
                drawing.queue_draw();
            }
        } else {
            hex.add_css_class("error");
        }
    });
    let selected = point.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let (cx, cy, radius) = geometry(width, height);
        let side = f64::from(width.min(height));
        let scale = side / f64::from(TEXTURE_SIZE);
        cr.set_antialias(cairo::Antialias::Best);
        if cr.save().is_ok() {
            cr.arc(cx, cy, radius, 0.0, TAU);
            cr.clip();
            cr.translate(cx - side / 2.0, cy - side / 2.0);
            cr.scale(scale, scale);
            WHEEL.with(|image| {
                if cr.set_source_surface(image, 0.0, 0.0).is_ok() {
                    cr.source().set_filter(cairo::Filter::Bilinear);
                    let _ = cr.paint();
                }
            });
            let _ = cr.restore();
        }
        let (x, y) = selected.get();
        cr.arc(cx + x * radius, cy + y * radius, side / 30.0, 0.0, TAU);
        cr.set_source_rgb(0.0, 0.0, 0.0);
        cr.set_line_width(3.0);
        let _ = cr.stroke_preserve();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.set_line_width(1.5);
        let _ = cr.stroke();
    });

    let drag = gtk::GestureDrag::new();
    let set_position = {
        let area = area.downgrade();
        let point = point.clone();
        let update_hex = update_hex.clone();
        move |x: f64, y: f64| {
            let Some(area) = area.upgrade() else {
                return;
            };
            let (cx, cy, radius) = geometry(area.width(), area.height());
            if radius <= 0.0 {
                return;
            }
            let (x, y) = ((x - cx) / radius, (y - cy) / radius);
            let norm = x.hypot(y).max(1.0);
            point.set((x / norm, y / norm));
            update_hex();
            area.queue_draw();
        }
    };
    let start = set_position.clone();
    drag.connect_drag_begin(move |gesture, x, y| {
        if let Some(widget) = gesture.widget() {
            widget.grab_focus();
        }
        start(x, y);
    });
    drag.connect_drag_update(move |gesture, dx, dy| {
        if let Some((x, y)) = gesture.start_point() {
            set_position(x + dx, y + dy);
        }
    });
    area.add_controller(drag);
    let keys = gtk::EventControllerKey::new();
    let selected = point.clone();
    keys.connect_key_pressed(move |keys, key, _, _| {
        let (dx, dy) = match key {
            gdk::Key::Left => (-0.04, 0.0),
            gdk::Key::Right => (0.04, 0.0),
            gdk::Key::Up => (0.0, -0.04),
            gdk::Key::Down => (0.0, 0.04),
            _ => return glib::Propagation::Proceed,
        };
        let (x, y) = selected.get();
        let (x, y) = (x + dx, y + dy);
        let norm = x.hypot(y).max(1.0);
        selected.set((x / norm, y / norm));
        update_hex();
        if let Some(widget) = keys.widget() {
            widget.queue_draw();
        }
        glib::Propagation::Stop
    });
    area.add_controller(keys);

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 4);
    panel.add_css_class("picker-card");
    panel.set_hexpand(true);
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    panel.append(&heading);
    if let Some(slots) = slots {
        let middle = crate::adaptive::centered_pair(12);
        area.set_valign(gtk::Align::Center);
        middle.append(&area);
        middle.append(slots);
        panel.append(&middle);
    } else {
        panel.append(&area);
    }
    for (text, field) in [
        ("HEX", hex.upcast_ref::<gtk::Widget>()),
        (tr("Brightness", "Luminosité"), brightness.upcast_ref()),
    ] {
        let label = gtk::Label::new(Some(text));
        label.set_xalign(0.0);
        panel.append(&label);
        panel.append(field);
    }
    (panel, hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_keeps_a_centered_circle_as_widgets_scale() {
        assert_eq!(geometry(152, 152), (76.0, 76.0, 68.0));
        assert_eq!(geometry(304, 228), (152.0, 114.0, 102.0));
    }

    #[test]
    fn parses_only_six_hex_digits_with_optional_hash() {
        assert_eq!(parse_hex(" #ff8000 "), Some([255, 128, 0]));
        assert_eq!(parse_hex("00AaFF"), Some([0, 170, 255]));
        for invalid in ["", "#123", "12345678", "#12345g", "+12345", "é1234"] {
            assert_eq!(parse_hex(invalid), None);
        }
    }

    #[test]
    fn hex_colors_round_trip_through_wheel_coordinates() {
        for rgb in [
            [0, 0, 0],
            [255, 255, 255],
            [18, 173, 239],
            [255, 0, 10],
            [35, 35, 35],
        ] {
            let ((x, y), brightness) = wheel_position(rgb);
            assert_eq!(rgb_at(x, y, brightness), rgb);
        }
    }

    #[test]
    fn wheel_maps_hue_saturation_and_brightness() {
        assert_eq!(rgb_at(1.0, 0.0, 1.0), [255, 0, 0]);
        assert_eq!(rgb_at(-0.5, 3.0_f64.sqrt() / 2.0, 1.0), [0, 255, 0]);
        assert_eq!(rgb_at(0.0, 0.0, 0.5), [128; 3]);
        assert_eq!(rgb_at(0.0, -1.0, 0.0), [0; 3]);
    }
}
