use std::{
    fs,
    io::{self, Seek, SeekFrom, Write},
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::{Path, PathBuf},
};

pub fn detect(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut devices = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.join("multi_index").is_file() {
            devices.push(path);
        }
    }
    devices.sort();
    Ok(devices)
}

fn intensities(index: &str, max: u32, rgb: [u8; 3]) -> Option<String> {
    if max == 0 || max > i32::MAX as u32 {
        return None;
    }
    let mut seen = [false; 4];
    let channels = [rgb[0], rgb[1], rgb[2], 0];
    let mut values = Vec::new();
    for name in index.split_whitespace() {
        let channel = match name {
            "red" => 0,
            "green" => 1,
            "blue" => 2,
            "white" => 3,
            _ => return None,
        };
        if std::mem::replace(&mut seen[channel], true) {
            return None;
        }
        values.push(((u64::from(channels[channel]) * u64::from(max) + 127) / 255).to_string());
    }
    seen[..3]
        .iter()
        .copied()
        .all(|present| present)
        .then(|| values.join(" ") + "\n")
}

pub fn channels(path: &Path) -> io::Result<(String, u32)> {
    let index = fs::read_to_string(path.join("multi_index"))?;
    let max = fs::read_to_string(path.join("max_brightness"))?;
    let max = max
        .trim()
        .parse()
        .map_err(|_| io::Error::other("Invalid maximum brightness"))?;
    if intensities(&index, max, [0; 3]).is_none() {
        return Err(io::Error::other("Unsupported RGB channels"));
    }
    Ok((index, max))
}

pub fn open(path: &Path, inode: u64) -> io::Result<impl FnMut([u8; 3]) -> io::Result<()> + use<>> {
    let dir = fs::File::open(path)?;
    if dir.metadata()?.ino() != inode {
        return Err(io::Error::other("LED attachment changed"));
    }
    // Resolve all attributes through the same open kernfs directory, never a retargeted alias.
    let path = PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()));
    let (index, max) = channels(&path)?;
    // Open both attributes before changing anything, so missing permissions leave the LED untouched.
    let mut brightness = fs::OpenOptions::new()
        .write(true)
        .open(path.join("brightness"))?;
    let mut intensity = fs::OpenOptions::new()
        .write(true)
        .open(path.join("multi_intensity"))?;
    let mut enabled = false;
    Ok(move |rgb| {
        let values = intensities(&index, max, rgb).expect("validated RGB channels");
        intensity.seek(SeekFrom::Start(0))?;
        intensity.write_all(values.as_bytes())?;
        if !enabled {
            brightness.write_all(format!("{max}\n").as_bytes())?;
            enabled = true;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_the_intensity_file_without_resetting_brightness() -> io::Result<()> {
        let dir =
            std::env::temp_dir().join(format!("linrgb-led-{}", gtk::glib::uuid_string_random()));
        fs::create_dir(&dir)?;
        let result = (|| -> io::Result<()> {
            fs::write(dir.join("multi_index"), "blue red green\n")?;
            fs::write(dir.join("max_brightness"), "255\n")?;
            fs::write(dir.join("brightness"), "000\n")?;
            fs::write(dir.join("multi_intensity"), "")?;
            let mut set = open(&dir, dir.metadata()?.ino())?;
            set([111, 222, 123])?;
            assert_eq!(fs::read_to_string(dir.join("brightness"))?, "255\n");
            fs::write(dir.join("brightness"), "127\n")?;
            set([222, 123, 111])?;
            assert_eq!(
                fs::read_to_string(dir.join("multi_intensity"))?,
                "111 222 123\n"
            );
            assert_eq!(fs::read_to_string(dir.join("brightness"))?, "127\n");
            Ok(())
        })();
        fs::remove_dir_all(dir)?;
        result
    }

    #[test]
    fn replacement_at_same_path_cannot_receive_a_stale_color() -> io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("linrgb-led-{}", gtk::glib::uuid_string_random()));
        let path = root.join("led");
        fs::create_dir_all(&path)?;
        let setup = || -> io::Result<()> {
            for (k, v) in [
                ("multi_index", "red green blue"),
                ("max_brightness", "31"),
                ("multi_intensity", "0 0 0"),
                ("brightness", "0"),
            ] {
                fs::write(path.join(k), v)?;
            }
            Ok(())
        };
        setup()?;
        let inode = path.metadata()?.ino();
        let mut before_remove = open(&path, inode)?;
        fs::rename(&path, root.join("removed"))?;
        fs::create_dir(&path)?;
        setup()?;
        assert!(open(&path, inode).is_err());
        before_remove([1, 2, 3])?; // fixture fds still refer to the old files, never the replacement
        assert_eq!(fs::read_to_string(path.join("multi_intensity"))?, "0 0 0");
        drop(before_remove);
        fs::remove_dir_all(root)
    }

    #[test]
    fn honors_kernel_channel_order_and_scale() {
        assert_eq!(
            intensities("blue red green\n", 31, [255, 0, 128]).as_deref(),
            Some("16 31 0\n")
        );
        assert_eq!(
            intensities("red green blue", 255, [0; 3]).as_deref(),
            Some("0 0 0\n")
        );
    }

    #[test]
    fn rgbw_keeps_white_off() {
        assert_eq!(
            intensities("white blue red green", 255, [8, 16, 32]).as_deref(),
            Some("0 32 8 16\n")
        );
    }

    #[test]
    fn rejects_unknown_or_incomplete_layouts() {
        for index in [
            "",
            "red green",
            "red blue blue",
            "red green blue white white",
            "red green blue amber",
        ] {
            assert!(intensities(index, 255, [0; 3]).is_none());
        }
    }
}
