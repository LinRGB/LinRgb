use crate::i18n::tr;
use gtk::{glib, prelude::*};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs,
    process::{Command, Stdio},
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Gpu {
    load: Option<f64>,
    vram_gib: Option<f64>,
    temperature: Option<f64>,
}

fn memory(text: &str) -> Option<(f64, f64)> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key == name).then(|| value.split_whitespace().next()?.parse::<u64>().ok())?
        })
    };
    let total = field("MemTotal")?;
    let used = total.checked_sub(field("MemAvailable")?)?;
    (total != 0).then(|| {
        (
            used as f64 / 1_048_576.0,
            used as f64 * 100.0 / total as f64,
        )
    })
}

fn nvidia_fields(text: &str) -> Option<Gpu> {
    let mut fields = text.lines().next()?.split(',');
    let mut read = || {
        fields
            .next()?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
    };
    let result = Gpu {
        load: read().filter(|n| *n <= 100.0),
        vram_gib: read().map(|n| n / 1024.0),
        temperature: read(),
    };
    (result.load.is_some() || result.vram_gib.is_some() || result.temperature.is_some())
        .then_some(result)
}

fn nvidia() -> Option<Gpu> {
    let mut child = Command::new("nvidia-smi")
        .args([
            "--id=0",
            "--query-gpu=utilization.gpu,memory.used,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(900);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let output = child.wait_with_output().ok()?;
                return nvidia_fields(std::str::from_utf8(&output.stdout).ok()?);
            }
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn gpu() -> Gpu {
    if let Ok(cards) = fs::read_dir("/sys/class/drm") {
        for card in cards.flatten() {
            let name = card.file_name();
            if !name
                .to_str()
                .and_then(|name| name.strip_prefix("card"))
                .is_some_and(|suffix| suffix.parse::<u32>().is_ok())
            {
                continue;
            }
            let device = card.path().join("device");
            let read = |file: &str| {
                fs::read_to_string(device.join(file))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()
            };
            if let Some(load) = read("gpu_busy_percent").filter(|n| *n <= 100) {
                let temperature = fs::read_dir(device.join("hwmon"))
                    .ok()
                    .into_iter()
                    .flatten()
                    .flatten()
                    .find_map(|entry| {
                        fs::read_to_string(entry.path().join("temp1_input"))
                            .ok()?
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .map(|n| n / 1000.0)
                    });
                return Gpu {
                    load: Some(load as f64),
                    vram_gib: read("mem_info_vram_used")
                        .map(|bytes| bytes as f64 / 1_073_741_824.0),
                    temperature,
                };
            }
        }
    }
    nvidia().unwrap_or_default()
}

pub fn install(devices: &gtk::Box, workspace: &gtk::Box, window: &gtk::ApplicationWindow) {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 6);
    panel.set_vexpand(true);
    let toggle = gtk::ToggleButton::with_label(tr("System monitor  ▸", "Moniteur système  ▸"));
    panel.append(&toggle);
    let charts = crate::adaptive::pair(8, 50);
    charts.set_vexpand(true);
    crate::adaptive::set_equal_height(&charts, true);
    let (gpu_graph, gpu_label, gpu_history) = chart(&charts, "GPU");
    let (ram_graph, ram_label, ram_history) = chart(&charts, tr("Memory", "Mémoire"));
    panel.append(&charts);
    devices.append(&panel);
    panel.set_visible(false);
    charts.set_visible(false);

    let columns = Rc::new(std::cell::Cell::new(true));
    let refresh = {
        let (panel, charts, toggle, window) = (
            panel.downgrade(),
            charts.downgrade(),
            toggle.downgrade(),
            window.downgrade(),
        );
        let columns = columns.clone();
        move || {
            let (Some(panel), Some(charts), Some(toggle), Some(window)) = (
                panel.upgrade(),
                charts.upgrade(),
                toggle.upgrade(),
                window.upgrade(),
            ) else {
                return;
            };
            let stacked = !columns.get();
            let expanded = window.is_fullscreen() || window.is_maximized();
            panel.set_visible(stacked || expanded);
            toggle.set_visible(stacked);
            charts.set_visible(if stacked {
                toggle.is_active()
            } else {
                expanded
            });
            toggle.set_label(if toggle.is_active() {
                tr("System monitor  ▾", "Moniteur système  ▾")
            } else {
                tr("System monitor  ▸", "Moniteur système  ▸")
            });
        }
    };
    let sync = refresh.clone();
    crate::adaptive::on_columns_changed(workspace, move |side_by_side| {
        columns.set(side_by_side);
        sync();
    });
    let sync = refresh.clone();
    window.connect_maximized_notify(move |_| sync());
    let sync = refresh.clone();
    window.connect_fullscreened_notify(move |_| sync());
    toggle.connect_toggled(move |_| refresh());

    let charts = charts.downgrade();
    let mut pending = None;
    let mut next = Instant::now();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(charts) = charts.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if let Some(rx) = pending.as_ref() {
            match mpsc::Receiver::try_recv(rx) {
                Ok((gpu, ram)) => {
                    render_sample(gpu, ram, &gpu_label, &ram_label, &gpu_history, &ram_history);
                    gpu_graph.queue_draw();
                    ram_graph.queue_draw();
                    pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if charts.is_mapped() && pending.is_none() && Instant::now() >= next {
            let (tx, rx) = mpsc::channel();
            pending = Some(rx);
            next = Instant::now() + Duration::from_secs(2);
            std::thread::spawn(move || {
                let ram = fs::read_to_string("/proc/meminfo")
                    .ok()
                    .and_then(|text| memory(&text));
                let _ = tx.send((gpu(), ram));
            });
        }
        glib::ControlFlow::Continue
    });
}

type History = Rc<RefCell<VecDeque<f64>>>;

fn chart(panel: &gtk::Box, title: &str) -> (gtk::DrawingArea, gtk::Label, History) {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.add_css_class("picker-card");
    card.append(&crate::label(title, "heading"));
    let graph = gtk::DrawingArea::new();
    graph.set_content_width(88);
    graph.set_content_height(47);
    graph.set_vexpand(true);
    let history = Rc::new(RefCell::new(VecDeque::<f64>::with_capacity(30)));
    let series = history.clone();
    graph.set_draw_func(move |graph, cr, width, height| {
        let (w, h) = (f64::from(width), f64::from(height));
        let accent = graph.style_context().lookup_color("linrgb_accent");
        let [r, g, b] = accent.map_or([0.45, 0.89, 0.78], |color| {
            [
                color.red().into(),
                color.green().into(),
                color.blue().into(),
            ]
        });
        cr.set_source_rgba(r, g, b, 0.18);
        cr.set_line_width(1.0);
        cr.move_to(0.0, h - 1.0);
        cr.line_to(w, h - 1.0);
        let _ = cr.stroke();
        let values = series.borrow();
        let spacing = (w - 4.0) / 29.0;
        for (i, percent) in values.iter().enumerate() {
            let x = w - 2.0 - (values.len() - 1 - i) as f64 * spacing;
            let y = h - 2.0 - (h - 4.0) * (percent / 100.0).clamp(0.0, 1.0);
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
            if values.len() == 1 {
                cr.line_to(x - 3.0, y);
            }
        }
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(1.7);
        let _ = cr.stroke();
    });
    card.append(&graph);
    let status = crate::label(tr("Waiting for data…", "En attente de données…"), "eyebrow");
    status.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    status.set_max_width_chars(18);
    card.append(&status);
    panel.append(&card);
    (graph, status, history)
}

fn render_sample(
    gpu: Gpu,
    ram: Option<(f64, f64)>,
    gpu_label: &gtk::Label,
    ram_label: &gtk::Label,
    gpu_history: &History,
    ram_history: &History,
) {
    let fields = [
        gpu.load.map(|x| format!("{x:.0}%")),
        gpu.vram_gib.map(|x| format!("{x:.1} GiB VRAM")),
        gpu.temperature.map(|x| format!("{x:.0} °C")),
    ];
    let text = fields.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    gpu_label.set_text(if text.is_empty() {
        tr("Unavailable", "Indisponible")
    } else {
        &text
    });
    let text = ram.map_or_else(
        || tr("Unavailable", "Indisponible").to_owned(),
        |(gib, percent)| format!("{gib:.1} GiB · {percent:.0}%"),
    );
    ram_label.set_text(&text);
    for (series, value) in [
        (gpu_history, gpu.load),
        (ram_history, ram.map(|(_, percent)| percent)),
    ] {
        if let Some(value) = value {
            let mut series = series.borrow_mut();
            if series.len() == 30 {
                series.pop_front();
            }
            series.push_back(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ram_accounts_for_available_memory() {
        assert_eq!(
            memory("MemTotal: 2097152 kB\nMemAvailable: 1048576 kB\n"),
            Some((1.0, 50.0))
        );
        assert!(memory("MemTotal: 10 kB\nMemAvailable: 11 kB\n").is_none());
    }

    #[test]
    fn nvidia_accepts_partial_metrics() {
        let sample = nvidia_fields("12, 2048, 40.5\n").unwrap();
        assert_eq!(
            (sample.load, sample.vram_gib, sample.temperature),
            (Some(12.0), Some(2.0), Some(40.5))
        );
        let sample = nvidia_fields("N/A, 1024, N/A\n").unwrap();
        assert_eq!(
            (sample.load, sample.vram_gib, sample.temperature),
            (None, Some(1.0), None)
        );
    }
}
