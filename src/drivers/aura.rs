//! Two motherboard profiles, one verified output interface. No firmware queries.
use super::{Capabilities, Result, Sink};
use std::{
    cell::RefCell,
    ffi::OsString,
    fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

pub const CROSSHAIR: &str = "ROG CROSSHAIR VIII HERO (WI-FI)";
pub const B550: &str = "ROG STRIX B550-A GAMING";
const DESCRIPTOR: &[u8] = include_bytes!("aura-interface.hid");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Crosshair,
    Strix,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Fixed,
    Header1,
}
/// Packet addressing from the matched profile, not a measured physical lamp count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Fixed8,
    Fixed2,
    Header100,
}
impl Profile {
    pub fn identify(model: &str) -> Option<Self> {
        match model.trim() {
            CROSSHAIR => Some(Self::Crosshair),
            B550 => Some(Self::Strix),
            _ => None,
        }
    }
    fn hid(self) -> &'static str {
        match self {
            Self::Crosshair => "HID_ID=0003:00000B05:000018F3",
            Self::Strix => "HID_ID=0003:00000B05:00001939",
        }
    }
    fn layout(self, zone: Zone) -> io::Result<Layout> {
        match (self, zone) {
            (Self::Crosshair, Zone::Fixed) => Ok(Layout::Fixed8),
            (Self::Strix, Zone::Fixed) => Ok(Layout::Fixed2),
            (Self::Strix, Zone::Header1) => Ok(Layout::Header100),
            _ => Err(io::Error::other("Unsupported Aura zone")),
        }
    }
}
impl Layout {
    pub fn description(self) -> &'static str {
        match self {
            Self::Fixed8 => "Aura fixed · profile: 8 positions",
            Self::Fixed2 => "Aura fixed · profile: 2 positions",
            Self::Header100 => "Aura header 1 · profile: 100 positions",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub instance: PathBuf,
    inode: u64,
    profile: Profile,
}
fn matches_interface(instance: &Path, profile: Profile) -> io::Result<bool> {
    if !fs::read_to_string(instance.join("uevent"))?
        .lines()
        .any(|s| s == profile.hid())
    {
        return Ok(false);
    }
    let parent = instance
        .parent()
        .ok_or_else(|| io::Error::other("Missing USB interface"))?;
    Ok(
        fs::read_to_string(parent.join("bInterfaceNumber"))?.trim() == "02"
            && fs::read(instance.join("report_descriptor"))? == DESCRIPTOR,
    )
}
pub fn observe(root: &Path, profile: Profile) -> io::Result<(OsString, Attachment)> {
    let mut found = None;
    for entry in fs::read_dir(root)? {
        let e = entry?;
        let instance = e.path().join("device").canonicalize()?;
        if matches_interface(&instance, profile)? {
            if found.is_some() {
                return Err(io::Error::other("Ambiguous Aura interfaces"));
            }
            found = Some((
                e.file_name(),
                Attachment {
                    inode: instance.metadata()?.ino(),
                    instance,
                    profile,
                },
            ));
        }
    }
    found.ok_or_else(|| io::Error::other("No supported Aura HID interface"))
}
impl Attachment {
    fn verify(&self, model: &str) -> io::Result<()> {
        if Profile::identify(model) != Some(self.profile)
            || self.instance.metadata()?.ino() != self.inode
            || !matches_interface(&self.instance, self.profile)?
        {
            return Err(io::Error::other("Aura controller or interface changed"));
        }
        Ok(())
    }
    fn open(&self) -> Result<fs::File> {
        let file = crate::hid::open(&self.instance, self.profile.hid(), false)?;
        self.verify(&fs::read_to_string("/sys/class/dmi/id/board_name")?)?;
        crate::hid::verify_descriptor(&file, DESCRIPTOR)?;
        Ok(file)
    }
}

// Both zones run on one worker. Never lock a mutex or resolve a path per frame.
// Dropping the shared transport on failure revokes the sibling zone as well.
type Port<W> = Rc<RefCell<Option<W>>>;
#[derive(Default)]
pub struct Connections(Option<(Attachment, Port<fs::File>)>);
impl Connections {
    pub(super) fn bind<'a>(
        &mut self,
        a: Attachment,
        zone: Zone,
        stop: &'a AtomicBool,
    ) -> Result<(Capabilities, Sink<'a>)> {
        self.bind_with(a, zone, stop, Attachment::open)
    }
    fn bind_with<'a>(
        &mut self,
        a: Attachment,
        zone: Zone,
        stop: &'a AtomicBool,
        open: impl FnOnce(&Attachment) -> Result<fs::File>,
    ) -> Result<(Capabilities, Sink<'a>)> {
        let layout = a.profile.layout(zone)?;
        let port = match &self.0 {
            Some((key, p)) if key == &a && p.borrow().is_some() => p.clone(),
            Some(_) => return Err("Aura controller changed or closed".into()),
            None => {
                // Reserve this attachment even if verification or locking fails. No job retries.
                let (key, p) = self.0.insert((a, Rc::default()));
                let file = open(key)?;
                file.try_lock()?; // nonblocking advisory exclusion across cooperating opens
                *p.borrow_mut() = Some(file);
                p.clone()
            }
        };
        Ok((
            Capabilities::Aura(layout),
            Box::new(writer(port, layout, stop)),
        ))
    }
}
fn writer<W: Write>(
    port: Port<W>,
    layout: Layout,
    stop: &AtomicBool,
) -> impl FnMut([u8; 3]) -> Result<()> {
    let mut initialized = false;
    move |rgb| {
        let mut port = port.borrow_mut();
        let hid = port.as_mut().ok_or("Aura connection closed")?;
        let mut send = |packet: &[u8; 65]| -> io::Result<()> {
            if stop.load(Ordering::Acquire) {
                return Err(io::Error::other("RGB operation cancelled"));
            }
            if hid.write(packet)? != packet.len() {
                return Err(io::Error::other("Incomplete Aura report"));
            }
            Ok(())
        };
        let result = match layout {
            Layout::Fixed8 => send(&direct(rgb, 0x84, 0, 8)),
            Layout::Fixed2 => send(&direct(rgb, 0x84, 0, 2)),
            Layout::Header100 => (|| {
                if !initialized {
                    let mut mode = [0; 65];
                    mode[..5].copy_from_slice(&[0xec, 0x52, 0x53, 0, 1]);
                    send(&mode)?;
                    initialized = true;
                }
                for i in 0..5 {
                    send(&direct(rgb, if i == 4 { 0x80 } else { 0 }, 20 * i, 20))?;
                }
                Ok(())
            })(),
        };
        if result.is_err() {
            port.take();
        }
        Ok(result?)
    }
}
fn direct(rgb: [u8; 3], channel: u8, offset: u8, count: u8) -> [u8; 65] {
    let mut report = [0; 65];
    report[..5].copy_from_slice(&[0xec, 0x40, channel, offset, count]);
    for color in report[5..5 + usize::from(count) * 3].as_chunks_mut::<3>().0 {
        color.copy_from_slice(&rgb);
    }
    report
}
#[cfg(test)]
mod tests;
