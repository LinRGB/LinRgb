//! Static backend dispatch. Inventory holds candidates, never a writable handle.
pub(crate) mod kernel_led;
pub(crate) mod lamp_array;
#[cfg(test)]
mod tests;

use std::{
    error::Error,
    io,
    num::NonZeroU16,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq, Eq)]
pub enum Target {
    EneGpu(PathBuf),
    Board(PathBuf),
    BoardArgb(PathBuf),
    Ram(u16),
    Corsair(u16),
    Kernel(PathBuf, u64, kernel_led::Layout),
    LampArray(PathBuf),
}

/// A ticket authorizes a new attachment, not a persistent effect. A refresh invalidates
/// queued opens; already bound fds are reconciled separately (unplug, ACL, sleep).
#[derive(Default)]
pub struct Generation(Arc<AtomicU64>);
impl Generation {
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
    pub fn invalidate(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
    pub fn ticket(&self) -> Ticket {
        Ticket {
            value: self.current(),
            current: self.0.clone(),
        }
    }
}
pub struct Ticket {
    current: Arc<AtomicU64>,
    value: u64,
}
impl Ticket {
    fn check(&self) -> io::Result<()> {
        if self.current.load(Ordering::Acquire) == self.value {
            Ok(())
        } else {
            Err(io::Error::other("Inventory changed; apply again"))
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Sink<'a> = Box<dyn FnMut([u8; 3]) -> Result<()> + 'a>;

/// Backend contract: one uniform RGB zone. Effects are sampled by the host, not
/// hardware modes. No backend exposes color readback through this contract.
#[derive(Clone, Copy)]
pub enum Capabilities {
    Sysfs {
        layout: kernel_led::Layout,
    }, // read identity/layout, write two attributes
    HidRange {
        lamps: NonZeroU16,
        min_interval: Duration,
    }, // read/write hidraw features
    LegacyDirect, // protocol-specific details not exposed as verified capabilities
}
impl Capabilities {
    fn frame_due(self, last: Option<Instant>, now: Instant) -> bool {
        match (self, last) {
            (Self::HidRange { min_interval, .. }, Some(last)) => {
                now.saturating_duration_since(last) >= min_interval
            }
            _ => true,
        }
    }
    pub fn description(self) -> Option<String> {
        match self {
            Self::Sysfs { layout } => Some(layout.to_string()),
            Self::HidRange {
                lamps,
                min_interval,
            } => Some(format!("{lamps} LED · min {} µs", min_interval.as_micros())),
            Self::LegacyDirect => None,
        }
    }
}

/// Non-cloneable, owns a single checked connection. First fatal error drops its fds.
/// Neither a `Target` nor a selection key implements the RGB operation.
pub struct Session<'a> {
    sink: Option<Sink<'a>>,
    caps: Capabilities,
    stop: &'a AtomicBool,
    ticket: &'a Ticket,
    last: Option<Instant>,
}
impl<'a> Session<'a> {
    fn connect(
        stop: &'a AtomicBool,
        ticket: &'a Ticket,
        verify: impl FnOnce() -> Result<(Capabilities, Sink<'a>)>,
    ) -> Result<Self> {
        let check = || -> Result<()> {
            if stop.load(Ordering::Acquire) {
                return Err("RGB operation cancelled".into());
            }
            ticket.check()?;
            Ok(())
        };
        check()?;
        let (caps, sink) = verify()?;
        check()?;
        Ok(Self {
            sink: Some(sink),
            caps,
            stop,
            ticket,
            last: None,
        })
    }
    pub fn capabilities(&self) -> Capabilities {
        self.caps
    }
    /// False means the previous frame still occupies the device's update interval.
    /// Do not block other outputs or mark the skipped color as sent.
    pub fn set_rgb(&mut self, rgb: [u8; 3]) -> Result<bool> {
        let result = (|| {
            let sink = self.sink.as_mut().ok_or("RGB session closed")?;
            if self.stop.load(Ordering::Acquire) {
                return Err("RGB operation cancelled".into());
            }
            if !self.caps.frame_due(self.last, Instant::now()) {
                return Ok(false);
            }
            if self.last.is_none() {
                self.ticket.check()?;
            }
            sink(rgb)?;
            self.last = Some(Instant::now());
            Ok(true)
        })();
        if result.is_err() {
            self.sink.take();
        }
        result
    }
}

/// Access needed for operation; inventory never requests these permissions.
#[derive(Clone, Copy)]
pub enum Interface {
    SysfsAttributes,
    HidrawWrite,
    HidrawFeatures,
    Smbus,
}
impl Interface {
    pub fn description(self) -> &'static str {
        match self {
            Self::SysfsAttributes => "sysfs:read-layout/write-attributes",
            Self::HidrawWrite => "hidraw:write",
            Self::HidrawFeatures => "hidraw:read-write",
            Self::Smbus => "i2c-dev:read-write",
        }
    }
}

impl Target {
    pub fn interface(&self) -> Interface {
        match self {
            Self::Kernel(..) => Interface::SysfsAttributes,
            Self::LampArray(_) => Interface::HidrawFeatures,
            Self::Board(_) | Self::BoardArgb(_) => Interface::HidrawWrite,
            Self::EneGpu(_) | Self::Ram(_) | Self::Corsair(_) => Interface::Smbus,
        }
    }

    pub fn connect<'a>(self, stop: &'a AtomicBool, ticket: &'a Ticket) -> Result<Session<'a>> {
        Session::connect(stop, ticket, || {
            let sink: Sink<'a> = match self {
                Self::Kernel(path, inode, layout) => {
                    let mut write = kernel_led::open(&path, inode, layout)?;
                    return Ok((
                        Capabilities::Sysfs { layout },
                        Box::new(move |rgb| Ok(write(rgb)?)),
                    ));
                }
                Self::LampArray(path) => {
                    let (caps, mut write) = lamp_array::open(&path, stop)?;
                    return Ok((caps, Box::new(move |rgb| Ok(write(rgb)?))));
                }
                Self::EneGpu(card) => {
                    let mut device = crate::astral::open(&card)?;
                    Box::new(move |rgb| device.set(rgb))
                }
                Self::Ram(address) => {
                    let mut device = crate::board::open_ram(address)?;
                    Box::new(move |rgb| device.set(rgb))
                }
                Self::Corsair(address) => Box::new(crate::corsair::open(address)?),
                Self::Board(path) => Box::new(crate::board::open(&path)?),
                Self::BoardArgb(path) => Box::new(crate::board::open_argb(&path)?),
            };
            Ok((Capabilities::LegacyDirect, sink))
        })
    }
}
