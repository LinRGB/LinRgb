use crate::{effects::Effect, i18n::tr};
use gtk::{cairo, glib, prelude::*};
use std::{cell::Cell, rc::Rc};

pub fn panel(
    mode: &gtk::DropDown,
    first: &gtk::Entry,
    second: &gtk::Entry,
    period: &gtk::SpinButton,
    alternate: &gtk::CheckButton,
    saturation: &gtk::SpinButton,
) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.add_css_class("preview-card");
    content.append(&crate::label(
        tr("Effect preview", "Aperçu de l’effet"),
        "title-2",
    ));
    let simulated = tr(
        "Simulated lighting. Your devices change only when you click Apply.",
        "Éclairage simulé. Cliquez sur Appliquer pour modifier les LED.",
    );
    let caption = crate::label(simulated, "dim-label");
    content.append(&caption);
    let models = crate::adaptive::pair(10, 50);
    let color = Rc::new(Cell::new([0; 3]));
    let gpu = model(
        &models,
        tr("Graphics card", "Carte graphique"),
        true,
        color.clone(),
    );
    let ram = model(&models, tr("Memory", "Mémoire"), false, color.clone());
    content.append(&models);
    let model_state = Rc::new(Cell::new(None::<(Effect, i64)>));
    let ticking = Rc::new(Cell::new(false));
    let refresh = {
        let (mode, first, second, period, alternate, saturation) = (
            mode.downgrade(),
            first.downgrade(),
            second.downgrade(),
            period.downgrade(),
            alternate.downgrade(),
            saturation.downgrade(),
        );
        let (models, caption, gpu, ram) = (
            models.downgrade(),
            caption.downgrade(),
            gpu.downgrade(),
            ram.downgrade(),
        );
        move || {
            let (
                Some(mode),
                Some(first),
                Some(second),
                Some(period),
                Some(alternate),
                Some(saturation),
            ) = (
                mode.upgrade(),
                first.upgrade(),
                second.upgrade(),
                period.upgrade(),
                alternate.upgrade(),
                saturation.upgrade(),
            )
            else {
                return;
            };
            let (Some(models), Some(caption), Some(gpu), Some(ram)) = (
                models.upgrade(),
                caption.upgrade(),
                gpu.upgrade(),
                ram.upgrade(),
            ) else {
                return;
            };
            let state = Effect::from_inputs(
                mode.selected(),
                &first.text(),
                &second.text(),
                alternate.is_active(),
                saturation.value(),
                period.value(),
            );
            model_state.set(state.map(|preview| (preview, glib::monotonic_time())));
            caption.set_text(if state.is_some() {
                simulated
            } else {
                tr(
                    "Enter valid HEX colors to preview the effect.",
                    "Saisissez des couleurs HEX valides pour voir l’aperçu.",
                )
            });
            let rgb = state.map_or([0; 3], |effect| effect.sample(0.0));
            if color.replace(rgb) != rgb {
                gpu.queue_draw();
                ram.queue_draw();
            }
            if state.is_some_and(Effect::animated) && !ticking.replace(true) {
                let (state, ticking, color) = (model_state.clone(), ticking.clone(), color.clone());
                let (gpu, ram) = (gpu.downgrade(), ram.downgrade());
                models.add_tick_callback(move |models, clock| {
                    let Some((preview, start)) =
                        state.get().filter(|(effect, _)| effect.animated())
                    else {
                        ticking.set(false);
                        return glib::ControlFlow::Break;
                    };
                    if let Some(viewport) = models.ancestor(gtk::Viewport::static_type())
                        && models.compute_bounds(&viewport).is_some_and(|bounds| {
                            bounds.y() >= viewport.height() as f32
                                || bounds.y() + bounds.height() <= 0.0
                        })
                    {
                        return glib::ControlFlow::Continue;
                    }
                    let elapsed = (clock.frame_time() - start).max(0) as f64 / 1_000_000.0;
                    let rgb = preview.sample(elapsed);
                    if color.replace(rgb) != rgb {
                        if let Some(gpu) = gpu.upgrade() {
                            gpu.queue_draw();
                        }
                        if let Some(ram) = ram.upgrade() {
                            ram.queue_draw();
                        }
                    }
                    glib::ControlFlow::Continue
                });
            }
        }
    };
    refresh();
    let update = refresh.clone();
    mode.connect_selected_notify(move |_| update());
    let update = refresh.clone();
    first.connect_changed(move |_| update());
    let update = refresh.clone();
    second.connect_changed(move |_| update());
    let update = refresh.clone();
    period.connect_value_changed(move |_| update());
    let update = refresh.clone();
    alternate.connect_toggled(move |_| update());
    saturation.connect_value_changed(move |_| refresh());
    content
}

fn model(parent: &gtk::Box, title: &str, gpu: bool, color: Rc<Cell<[u8; 3]>>) -> gtk::DrawingArea {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("picker-card");
    card.append(&crate::label(title, "heading"));
    let art = gtk::DrawingArea::new();
    art.set_content_width(180);
    art.set_content_height(116);
    art.set_draw_func(move |_, cr, width, height| {
        cr.set_antialias(cairo::Antialias::Best);
        let (width, height) = (f64::from(width), f64::from(height));
        let scale = (width / 260.0).min(height / 88.0);
        cr.translate((width - 260.0 * scale) / 2.0, (height - 88.0 * scale) / 2.0);
        cr.scale(scale, scale);
        let rgb = color.get().map(|channel| f64::from(channel) / 255.0);
        if gpu {
            block(cr, (8.0, 14.0, 248.0, 60.0), 10.0, [0.21, 0.23, 0.24]);
            block(cr, (15.0, 29.0, 234.0, 39.0), 8.0, [0.11, 0.12, 0.13]);
            block(cr, (22.0, 17.0, 220.0, 11.0), 5.0, [0.08, 0.09, 0.10]);
            block(cr, (25.0, 19.0, 214.0, 7.0), 3.5, rgb);
            for x in [63.0, 131.0, 199.0] {
                cr.arc(x, 49.0, 15.0, 0.0, std::f64::consts::TAU);
                cr.set_source_rgb(rgb[0] * 0.65, rgb[1] * 0.65, rgb[2] * 0.65);
                cr.set_line_width(2.5);
                let _ = cr.stroke();
                cr.arc(x, 49.0, 6.0, 0.0, std::f64::consts::TAU);
                cr.set_source_rgb(0.25, 0.27, 0.28);
                let _ = cr.fill();
            }
        } else {
            for y in [11.0, 52.0] {
                block(cr, (17.0, y - 2.0, 226.0, 33.0), 5.0, [0.19, 0.21, 0.22]);
                block(cr, (20.0, y, 220.0, 11.0), 4.0, [0.08, 0.09, 0.10]);
                block(cr, (23.0, y + 2.0, 214.0, 7.0), 3.5, rgb);
                for x in [37.0, 78.0, 119.0, 160.0, 201.0] {
                    block(cr, (x, y + 15.0, 22.0, 12.0), 2.0, [0.30, 0.32, 0.33]);
                    block(cr, (x + 2.0, y + 31.0, 18.0, 2.0), 1.0, [0.71, 0.57, 0.32]);
                }
            }
        }
    });
    card.append(&art);
    parent.append(&card);
    art
}

fn block(
    cr: &cairo::Context,
    (x, y, width, height): (f64, f64, f64, f64),
    radius: f64,
    [r, g, b]: [f64; 3],
) {
    use std::f64::consts::FRAC_PI_2;
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        FRAC_PI_2,
        2.0 * FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + radius,
        radius,
        2.0 * FRAC_PI_2,
        3.0 * FRAC_PI_2,
    );
    cr.close_path();
    cr.set_source_rgb(r, g, b);
    let _ = cr.fill();
}

#[cfg(test)]
mod tests {
    use super::Effect;

    #[test]
    fn preview_matches_solid_and_fade_settings() {
        let solid = Effect::from_inputs(0, "#FF0000", "invalid", false, 100.0, 6.0).unwrap();
        assert_eq!(solid.sample(3.0), [255, 0, 0]);
        let fade = Effect::from_inputs(1, "#FF0000", "#0000FF", false, 100.0, 6.0).unwrap();
        assert_eq!(fade.sample(1.5), [255, 0, 0]);
        assert_eq!(fade.sample(3.0), [0, 0, 255]);
        assert_eq!(fade.sample(6.0), [255, 0, 0]);
        assert!(Effect::from_inputs(1, "#FF0000", "invalid", false, 100.0, 6.0).is_none());
    }
}
