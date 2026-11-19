//! Passive inventory. Only a user-requested Apply can open a hardware backend.
#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
use crate::{
    astral, board,
    drivers::{Target, kernel_led, lamp_array},
    hardware,
    i18n::tr,
};
use std::{
    collections::BTreeSet,
    ffi::CString,
    fs, io,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Available,
    Denied,
    Missing,
}

fn access(path: &Path, mode: i32) -> Access {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return Access::Missing;
    };
    // Check effective credentials/ACL without opening a device. The driver must still check at open.
    if unsafe { libc::faccessat(libc::AT_FDCWD, path.as_ptr(), mode, libc::AT_EACCESS) } == 0 {
        Access::Available
    } else if io::Error::last_os_error().kind() == io::ErrorKind::PermissionDenied {
        Access::Denied
    } else {
        Access::Missing
    }
}

pub struct Device {
    pub key: Target,
    pub target: Target,
    pub name: String,
    pub access: Access,
}

impl Device {
    pub fn can_restore_from(&self, previous: &Self) -> bool {
        self.access == Access::Available
            && previous.access == Access::Available
            && self.key == previous.key
            && self.target == previous.target
    }

    pub fn support_label(&self) -> &'static str {
        match &self.target {
            Target::BoardArgb(_) => tr("experimental layout", "zones expérimentales"),
            Target::Board(_) => tr("matched board layout", "modèle de carte reconnu"),
            Target::LampArray(_) => tr("matched HID layout", "descripteur HID reconnu"),
            Target::Kernel(..) => tr("kernel RGB interface", "interface RGB du noyau"),
            Target::EneGpu(_) | Target::Ram(_) | Target::Corsair(_) => "", // labels already state the pending check
        }
    }

    pub fn bulk_selectable(&self) -> bool {
        !matches!(self.target, Target::Ram(_) | Target::Corsair(_))
            && self.access == Access::Available
    }
}

#[derive(Default)]
pub struct Inventory {
    pub devices: Vec<Device>,
    pci_ids: Vec<[Option<u16>; 4]>,
    pub notes: Vec<String>,
}

impl Inventory {
    fn add(&mut self, target: Target, name: String, access: Access) {
        let key = match &target {
            Target::LampArray(path) => Target::LampArray(hid_key(path)),
            Target::Board(path) => Target::Board(hid_key(path)),
            Target::BoardArgb(path) => Target::BoardArgb(hid_key(path)),
            Target::Kernel(..) => target.clone(),
            _ => target.clone(),
        };
        if !self.devices.iter().any(|d| d.key == key) {
            self.devices.push(Device {
                key,
                target,
                name,
                access,
            });
        }
    }
}

/// Only export allowlisted fields. Sysfs names, paths, serials and error strings stay local.
impl Inventory {
    pub fn diagnostics(&self) -> serde_json::Value {
        let devices: Vec<_> = self.devices.iter().map(|d| {
            let (backend, matched, address) = match &d.target {
                Target::Ram(a) => ("ene-dram", "slot-candidate", Some(a)),
                Target::Corsair(a) => ("corsair-dram", "slot-candidate", Some(a)),
                Target::Board(_) => ("aura-fixed", "dmi-and-usb", None),
                Target::BoardArgb(_) => ("aura-argb", "experimental-board-layout", None),
                Target::EneGpu(_) => ("ene-gpu", "pci-candidate", None),
                Target::Kernel(..) => ("linux-multicolor", "kernel-interface", None),
                Target::LampArray(_) => ("hid-lamparray", "known-report-layout", None),
            };
            serde_json::json!({"backend":backend, "match":matched, "required_interface": d.target.interface().description(), "slot_address":address.map(|a|format!("0x{a:02x}")), "access":match d.access { Access::Available=>"available", Access::Denied=>"denied", Access::Missing=>"interface-unavailable" }})
        }).collect();
        let graphics: Vec<_> = self
            .pci_ids
            .iter()
            .map(|ids| ids.map(|n| n.map(|n| format!("{n:04x}"))))
            .collect();
        serde_json::json!({"schema":1, "active_probing":false, "devices":devices, "graphics_pci_ids":graphics, "inventory_notices":self.notes.len()})
    }
}

fn hid_key(path: &Path) -> PathBuf {
    let Ok(canonical) = fs::canonicalize(path) else {
        return path.into();
    };
    // USB interface topology + HID bus/vendor/product, not the per-connection .NNNN suffix.
    if let Some((id, _)) = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|s| s.rsplit_once('.'))
    {
        canonical.with_file_name(id)
    } else {
        canonical
    }
}

pub fn scan(sys: &Path, dev: &Path) -> Inventory {
    let mut out = Inventory::default();
    let mut owners = BTreeSet::new();
    let pci = sys.join("bus/pci/devices");
    let rw = libc::R_OK | libc::W_OK;
    let bus_access = |n| access(&dev.join(format!("i2c-{n}")), rw);
    match hardware::graphics(&pci) {
        Ok(cards) => {
            for hardware::Graphics {
                path,
                ids,
                name,
                ene,
            } in cards
            {
                out.pci_ids.push(ids);
                if !ene {
                    let slot = path.file_name().unwrap_or_default().to_string_lossy();
                    out.notes.push(format!(
                        "{name} · {slot} · {}",
                        tr("no direct RGB driver", "sans pilote RGB direct")
                    ));
                    continue;
                }
                let state = astral::adapter_number(&path)
                    .map(bus_access)
                    .unwrap_or(Access::Missing);
                if state == Access::Available
                    && let Ok(parent) = path.canonicalize()
                {
                    owners.insert(parent);
                }
                let slot = path.file_name().unwrap_or_default().to_string_lossy();
                let label = format!(
                    "{name} · ENE · {slot} · {}",
                    tr(
                        "firmware checked on Apply",
                        "firmware vérifié à l’application"
                    )
                );
                out.add(Target::EneGpu(path), label, state);
            }
        }
        Err(e) => out.notes.push(format!("PCI: {e}")),
    }
    if let Ok(model) = fs::read_to_string(sys.join("class/dmi/id/board_name")) {
        let model = model.trim();
        if let Some((hid, _)) = board::fixed_zone(model) {
            let raw = sys.join("class/hidraw");
            match board::hid_node(&raw, hid) {
                Ok(node) => {
                    let state = access(&dev.join(&node), libc::W_OK);
                    if let Ok(instance) = raw.join(node).join("device").canonicalize() {
                        if state == Access::Available {
                            owners.insert(instance.clone());
                        }
                        out.add(
                            Target::Board(instance.clone()),
                            format!("{model} · Aura RGB"),
                            state,
                        );
                        if model == board::B550 {
                            out.add(
                                Target::BoardArgb(instance),
                                format!("{model} · Addressable RGB Header 1"),
                                state,
                            );
                        }
                    }
                }
                Err(e) => out.notes.push(format!("{model} · Aura USB: {e}")),
            }
            if let Ok(bus) = board::ram_bus_at(&pci.join("0000:00:14.0")) {
                let candidates = if model == board::B550 {
                    [Target::Corsair(0x5a), Target::Corsair(0x5b)]
                } else {
                    [Target::Ram(0x72), Target::Ram(0x73)]
                };
                for target in candidates {
                    let (name, addr) = match target {
                        Target::Ram(a) => ("ENE RAM", a),
                        Target::Corsair(a) => ("Corsair Pro SL", a),
                        _ => unreachable!(),
                    };
                    out.add(
                        target,
                        format!(
                            "{name} · {addr:#04x} · {}",
                            tr("unverified slot", "emplacement non vérifié")
                        ),
                        bus_access(bus),
                    );
                }
            }
        } else if !model.is_empty() {
            out.notes.push(format!(
                "{model} · {}",
                tr("no motherboard driver", "sans pilote de carte mère")
            ));
        }
    }
    match lamp_array::detect(&sys.join("bus/hid/devices")) {
        Ok(devices) => {
            for (name, path) in devices {
                if let Some(path) = path {
                    let Ok(instance) = path.canonicalize() else {
                        continue;
                    };
                    let state = crate::hid::node(&instance)
                        .ok()
                        .and_then(|p| p.file_name().map(|n| access(&dev.join(n), rw)))
                        .unwrap_or(Access::Missing);
                    if state == Access::Available {
                        owners.insert(instance.clone());
                    }
                    out.add(Target::LampArray(instance), format!("{name} · WDL"), state);
                } else {
                    out.notes.push(format!(
                        "{name} · {}",
                        tr("LampArray detection only", "LampArray : détection seule")
                    ));
                }
            }
        }
        Err(e) if e.kind() != io::ErrorKind::NotFound => out.notes.push(format!("HID: {e}")),
        Err(_) => (),
    }
    match kernel_led::detect(&sys.join("class/leds")) {
        Ok(leds) => {
            for path in leds {
                // Don't offer a second writer for a class LED below a claimed native controller.
                let Ok(path) = path.canonicalize() else {
                    continue;
                };
                if owners.iter().any(|owner| path.starts_with(owner)) {
                    continue;
                }
                let Ok(meta) = path.metadata() else {
                    continue;
                };
                let layout = match kernel_led::channels(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        out.notes.push(format!(
                            "{} · LED: {e}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                        continue;
                    }
                };
                let mut state = access(&path.join("multi_intensity"), libc::W_OK);
                if state == Access::Available {
                    state = access(&path.join("brightness"), libc::W_OK);
                }
                let label = format!(
                    "{} · Linux LED · {layout}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                out.add(Target::Kernel(path, meta.ino(), layout), label, state);
            }
        }
        Err(e) if e.kind() != io::ErrorKind::NotFound => out.notes.push(format!("LED: {e}")),
        Err(_) => (),
    }
    out
}
