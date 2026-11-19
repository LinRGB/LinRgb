use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

pub struct Graphics {
    pub path: PathBuf,
    pub ids: [Option<u16>; 4],
    pub name: String,
    pub ene: bool,
}

fn pci_ids(path: &Path) -> [Option<u16>; 4] {
    ["vendor", "device", "subsystem_vendor", "subsystem_device"].map(|key| {
        let id = fs::read_to_string(path.join(key)).ok()?;
        let hex = id.trim().strip_prefix("0x")?;
        (hex.len() == 4)
            .then(|| u16::from_str_radix(hex, 16).ok())
            .flatten()
    })
}

fn display_controller(path: &Path) -> bool {
    fs::read_to_string(path.join("class")).is_ok_and(|s| {
        s.trim()
            .strip_prefix("0x")
            .filter(|s| s.len() == 6)
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .is_some_and(|class| class >> 16 == 3)
    })
}

// A catalogue match is not proof of compatibility; the driver must validate the controller.
fn pci_candidates(ids: [Option<u16>; 4]) -> impl Iterator<Item = (&'static str, &'static str)> {
    let ids = ids.map(|n| n.map(|v| format!("0x{v:04x}")));
    include_str!("../data/openrgb-pci.tsv")
        .lines()
        .filter_map(move |line| {
            let mut cols = line.split('\t');
            if !ids
                .iter()
                .all(|id| id.as_deref().is_some_and(|id| cols.next() == Some(id)))
            {
                return None;
            }
            Some((cols.next()?, cols.next()?))
        })
}

pub fn nvidia_ene_name(path: &Path) -> Option<&'static str> {
    if !display_controller(path) {
        return None;
    }
    let ids = pci_ids(path);
    if ids[0] != Some(0x10de) || ids[2] != Some(0x1043) {
        return None;
    }
    pci_candidates(ids).find_map(|(family, name)| (family == "ENESMBusController").then_some(name))
}

pub fn graphics(root: &Path) -> io::Result<Vec<Graphics>> {
    let mut cards = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if !display_controller(&path) {
            continue;
        }
        let ids = pci_ids(&path);
        let mut names = BTreeSet::new();
        let mut families = BTreeSet::new();
        for (family, name) in pci_candidates(ids) {
            names.insert(name);
            families.insert(family);
        }
        let ene = ids[0] == Some(0x10de)
            && ids[2] == Some(0x1043)
            && families.contains("ENESMBusController");
        let name = if ids == [0x1002, 0x7550, 0x1eae, 0x8811].map(Some) {
            "XFX Radeon RX 9070 XT Mercury".into()
        } else if names.is_empty() {
            match ids[0] {
                Some(0x1002) => "AMD GPU",
                Some(0x10de) => "NVIDIA GPU",
                Some(0x8086) => "Intel GPU",
                _ => "GPU",
            }
            .into()
        } else {
            let models = names.into_iter().collect::<Vec<_>>().join(" / ");
            if ene {
                models
            } else {
                format!(
                    "{models} · {}",
                    families.into_iter().collect::<Vec<_>>().join(" / ")
                )
            }
        };
        cards.push(Graphics {
            path,
            ids,
            name,
            ene,
        });
    }
    cards.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pci_catalog_matches_exact_ids_and_keeps_alternative_families() {
        assert!(
            pci_candidates([0x1002, 0x7550, 0x1043, 0x0613].map(Some))
                .any(|(family, name)| family == "ENESMBusController" && name.contains("9070 XT"))
        );
        assert_eq!(
            pci_candidates([0x1002, 0x7550, 0x1458, 0x0613].map(Some)).count(),
            0
        );
        assert!(pci_candidates([0x1002, 0x7550, 0x1458, 0x2426].map(Some)).count() >= 2);
    }
    #[test]
    fn matches_exact_subsystem() -> io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("linrgb-pci-{}", gtk::glib::uuid_string_random()));
        fs::create_dir(&root)?;
        let result = (|| -> io::Result<()> {
            for (slot, subsystem, class) in [
                ("01", "0x89de", "0x030000"),
                ("02", "0x0000", "0x030000"),
                ("03", "0x8a2b", "0x030200"),
                ("04", "0x89de", "0x0c0500"),
            ] {
                let path = root.join(slot);
                fs::create_dir(&path)?;
                for (k, v) in [
                    ("class", class),
                    ("vendor", "0x10de"),
                    ("device", "0x2c02"),
                    ("subsystem_vendor", "0x1043"),
                    ("subsystem_device", subsystem),
                ] {
                    fs::write(path.join(k), v)?;
                }
            }
            fs::remove_file(root.join("02/device"))?;
            let cards = graphics(&root)?;
            assert_eq!(cards.len(), 3);
            assert_eq!(
                cards.iter().map(|c| c.ene).collect::<Vec<_>>(),
                [true, false, true]
            );
            for c in cards {
                assert_eq!(c.ene, nvidia_ene_name(&c.path).is_some());
            }
            assert!(nvidia_ene_name(&root.join("04")).is_none());
            Ok(())
        })();
        fs::remove_dir_all(root)?;
        result
    }
}
