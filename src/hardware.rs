use std::{fs, io, path::Path};

pub const ASTRAL: &str = "ASUS ROG Astral GeForce RTX 5080";

// ASUS RTX 5080 Astral variants using the ENE controller at 0x67, from OpenRGB's PCI registry.
const ASTRAL_SUBSYSTEMS: [&str; 4] = ["0x89de", "0x89df", "0x8a2b", "0x8a2c"];

pub fn detect(root: &Path) -> io::Result<Vec<String>> {
    let mut matches = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let ids = [
            ("vendor", "0x10de"),
            ("device", "0x2c02"),
            ("subsystem_vendor", "0x1043"),
        ];
        if ids.iter().all(|(key, expected)| {
            fs::read_to_string(entry.path().join(key)).is_ok_and(|id| id.trim() == *expected)
        }) && fs::read_to_string(entry.path().join("subsystem_device"))
            .is_ok_and(|id| ASTRAL_SUBSYSTEMS.contains(&id.trim()))
        {
            matches.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    matches.sort();
    Ok(matches)
}

// Inventory only: none of these IDs authorizes an RGB write.
fn pci_candidates(ids: [&str; 4]) -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("../data/openrgb-pci.tsv")
        .lines()
        .filter_map(move |line| {
            let mut cols = line.split('\t');
            if !ids.iter().all(|id| cols.next() == Some(*id)) {
                return None;
            }
            Some((cols.next()?, cols.next()?))
        })
}

pub fn other_graphics(root: &Path, supported: &[String]) -> io::Result<Vec<String>> {
    let mut cards = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let address = entry.file_name().to_string_lossy().into_owned();
        if supported.contains(&address)
            || !fs::read_to_string(path.join("class")).is_ok_and(|class| class.starts_with("0x03"))
        {
            continue;
        }
        let vendor = fs::read_to_string(path.join("vendor"))?;
        let device = fs::read_to_string(path.join("device"))?;
        let brand = match vendor.trim() {
            "0x1002" => "AMD",
            "0x10de" => "NVIDIA",
            "0x8086" => "Intel",
            _ => "GPU",
        };
        let subvendor = fs::read_to_string(path.join("subsystem_vendor")).unwrap_or_default();
        let subsystem = fs::read_to_string(path.join("subsystem_device")).unwrap_or_default();
        let mut names = std::collections::BTreeSet::new();
        let mut families = std::collections::BTreeSet::new();
        for (family, name) in pci_candidates([
            vendor.trim(),
            device.trim(),
            subvendor.trim(),
            subsystem.trim(),
        ]) {
            names.insert(name);
            families.insert(family);
        }
        let name = if names.is_empty() {
            brand.to_owned()
        } else {
            format!(
                "{} · Catalogue : {}",
                names.into_iter().collect::<Vec<_>>().join(" / "),
                families.into_iter().collect::<Vec<_>>().join(" / ")
            )
        };
        cards.push(format!(
            "{name} {}:{} · {address}",
            vendor.trim(),
            device.trim()
        ));
    }
    cards.sort();
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_catalog_matches_exact_ids_and_keeps_alternative_families() {
        assert!(
            pci_candidates(["0x1002", "0x7550", "0x1043", "0x0613"])
                .any(|(family, name)| family == "ENESMBusController" && name.contains("9070 XT"))
        );
        assert_eq!(
            pci_candidates(["0x1002", "0x7550", "0x1458", "0x0613"]).count(),
            0
        );
        assert!(pci_candidates(["0x1002", "0x7550", "0x1458", "0x2426"]).count() >= 2);
    }

    #[test]
    fn matches_exact_subsystem() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("linrgb-test-{}", std::process::id()));
        fs::create_dir(&root)?;
        let result = (|| {
            for (address, subsystem) in [
                ("0000:01:00.0", "0x89de"),
                ("0000:02:00.0", "0x0000"),
                ("0000:03:00.0", "0x8a2b"),
            ] {
                let path = root.join(address);
                fs::create_dir(&path)?;
                for (key, value) in [
                    ("vendor", "0x10de"),
                    ("device", "0x2c02"),
                    ("subsystem_vendor", "0x1043"),
                    ("subsystem_device", subsystem),
                ] {
                    fs::write(path.join(key), format!("{value}\n"))?;
                }
            }
            detect(&root)
        })();
        fs::remove_dir_all(root)?;
        assert_eq!(result?, ["0000:01:00.0", "0000:03:00.0"]);
        Ok(())
    }
}
