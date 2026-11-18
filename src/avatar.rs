use gtk::{cairo, gdk::prelude::GdkCairoContextExt, gdk_pixbuf::Pixbuf, glib, prelude::*};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn noctalia_avatar(toml: &str, home: &Path) -> Option<PathBuf> {
    let config = toml.parse::<toml::Table>().ok()?;
    let path = config.get("shell")?.get("avatar_path")?.as_str()?;
    if let Some(relative) = path.strip_prefix("~/") {
        Some(home.join(relative))
    } else {
        let path = Path::new(path);
        path.is_absolute().then(|| path.to_owned())
    }
}

fn load(path: &Path) -> Option<Pixbuf> {
    let info = fs::metadata(path).ok()?;
    if !info.is_file() || info.len() > 16 * 1024 * 1024 {
        return None;
    }
    Pixbuf::from_file_at_scale(path, 128, 128, true).ok()
}

fn account_image() -> Option<Pixbuf> {
    let home = glib::home_dir();
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/state"));
    [
        state.join("noctalia/settings.toml"),
        glib::user_config_dir().join("noctalia/config.toml"),
    ]
    .into_iter()
    .filter_map(|path| fs::read_to_string(path).ok())
    .filter_map(|text| noctalia_avatar(&text, &home))
    .chain([
        Path::new("/var/lib/AccountsService/icons").join(glib::user_name()),
        home.join(".face"),
        home.join(".face.icon"),
    ])
    .find_map(|path| load(&path))
}

pub fn widget() -> gtk::Widget {
    let Some(photo) = account_image() else {
        let icon = gtk::Image::from_icon_name("avatar-default-symbolic");
        icon.add_css_class("sidebar-avatar");
        return icon.upcast();
    };
    let drawing = gtk::DrawingArea::new();
    drawing.add_css_class("sidebar-photo");
    drawing.set_draw_func(move |_, cr, width, height| {
        let (x, y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        let radius = x.min(y);
        let (w, h) = (f64::from(photo.width()), f64::from(photo.height()));
        let scale = 2.0 * radius / w.min(h);
        if scale <= 0.0 {
            return;
        }
        cr.set_antialias(cairo::Antialias::Best);
        cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
        cr.clip();
        cr.translate(x - w * scale / 2.0, y - h * scale / 2.0);
        cr.scale(scale, scale);
        cr.set_source_pixbuf(&photo, 0.0, 0.0);
        cr.source().set_filter(cairo::Filter::Bilinear);
        let _ = cr.paint();
    });
    drawing.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_the_shell_avatar_and_expands_home() {
        let home = Path::new("/home/test");
        assert_eq!(
            noctalia_avatar("[shell]\navatar_path = '~/Pictures/me.jpg'", home),
            Some(home.join("Pictures/me.jpg"))
        );
        assert_eq!(
            noctalia_avatar("[other]\navatar_path = '/tmp/image.jpg'", home),
            None
        );
        assert_eq!(
            noctalia_avatar("[shell]\navatar_path = 'https://example.com/me.jpg'", home),
            None
        );
    }
}
