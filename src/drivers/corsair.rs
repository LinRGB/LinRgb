//! B550-A Pro SL candidates. A family signature alone must never produce a writer.
use super::{Capabilities, Result, Sink, Ticket};
use crate::board;
use i2cdev::{core::I2CDevice, linux::LinuxI2CDevice};
use std::{
    fs, io,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Slot {
    A = 0x5a,
    B = 0x5b,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Adapter {
    instance: PathBuf,
    inode: u64,
    number: u32,
}
impl Adapter {
    pub fn observe(controller: &Path, number: u32) -> io::Result<Self> {
        let instance = controller.join(format!("i2c-{number}")).canonicalize()?;
        Ok(Self {
            inode: instance.metadata()?.ino(),
            instance,
            number,
        })
    }
    fn check_parent(&self, parent: &Path) -> io::Result<()> {
        if parent.canonicalize()? != self.instance || parent.metadata()?.ino() != self.inode {
            return Err(io::Error::other("SMBus adapter attachment changed"));
        }
        Ok(())
    }
    fn open(&self, slot: Slot) -> Result<LinuxI2CDevice> {
        if !board::b550_present() || board::ram_bus()? != self.number {
            return Err("Unsupported Corsair adapter".into());
        }
        let dev = LinuxI2CDevice::new(format!("/dev/i2c-{}", self.number), slot as u16)?; // I2C_SLAVE, never FORCE
        let fd = dev.as_raw_fd();
        let stat = fs::metadata(format!("/proc/self/fd/{fd}"))?;
        if !stat.file_type().is_char_device()
            || libc::major(stat.rdev()) != 89
            || libc::minor(stat.rdev()) != self.number
        {
            return Err("Unexpected i2c-dev device number".into());
        }
        self.check_parent(
            &Path::new("/sys/dev/char")
                .join(format!("89:{}", self.number))
                .join("device"),
        )?;
        lock_slot(&dev, slot)?;
        Ok(dev)
    }
}
// Linux open-file-description lock: independent address-bound fds may coexist, but
// another cooperating client cannot interleave commands for the same adapter/slot.
fn lock_slot(file: &impl AsRawFd, slot: Slot) -> io::Result<()> {
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = slot as _;
    lock.l_len = 1;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn crc(data: &[u8]) -> u8 {
    data.iter().fold(0, |crc, byte| {
        (0..8).fold(crc ^ byte, |c, _| {
            (c << 1) ^ if c & 0x80 != 0 { 7 } else { 0 }
        })
    })
}
fn frame(rgb: [u8; 3]) -> [u8; 32] {
    let mut data = [0; 32];
    data[0] = 10;
    for led in data[1..31].as_chunks_mut::<3>().0 {
        led.copy_from_slice(&rgb);
    }
    data[31] = crc(&data[..31]);
    data
}

trait Transport {
    fn read(&mut self, reg: u8) -> Result<u8>;
    fn write_zero(&mut self, reg: u8) -> Result<()>;
    fn direct(&mut self, frame: &[u8; 32]) -> Result<()>;
}
impl Transport for LinuxI2CDevice {
    fn read(&mut self, reg: u8) -> Result<u8> {
        Ok(self.smbus_read_byte_data(reg)?)
    }
    fn write_zero(&mut self, reg: u8) -> Result<()> {
        Ok(self.smbus_write_byte_data(reg, 0)?)
    }
    fn direct(&mut self, frame: &[u8; 32]) -> Result<()> {
        Ok(self.smbus_write_block_data(0x31, frame)?)
    }
}
fn bind<'a>(
    mut dev: impl Transport + 'a,
    stop: &AtomicBool,
    ticket: &Ticket,
) -> Result<(Capabilities, Sink<'a>)> {
    let check = || -> Result<()> {
        if stop.load(Ordering::Acquire) {
            return Err("RGB operation cancelled".into());
        }
        Ok(ticket.check()?)
    };
    check()?;
    if !matches!(dev.read(0x43)?, 0x1a..=0x1c) {
        return Err("Unrecognized Corsair controller family".into());
    }
    check()?;
    if !matches!(dev.read(0x44)?, 1 | 3 | 4) {
        return Err("Unrecognized Corsair controller family".into());
    }
    check()?;
    // Select device information, then start its binary transfer. Neither is a color command.
    dev.write_zero(0x61)?;
    check()?;
    dev.write_zero(0x21)?;
    let mut info = [0; 32];
    for byte in &mut info {
        check()?;
        *byte = dev.read(0x40)?;
    }
    check()?;
    let checksum = dev.read(0x42)?;
    if info[..2] != [0x1c, 0x1b]
        || !matches!(info[2..4], [0 | 1, 3])
        || info[28] != 4
        || crc(&info) != checksum
    {
        return Err("Unrecognized Corsair module information".into());
    }
    Ok((
        Capabilities::Corsair { protocol: info[28] },
        Box::new(move |rgb| dev.direct(&frame(rgb))),
    ))
}
pub(super) fn connect<'a>(
    adapter: &Adapter,
    slot: Slot,
    stop: &AtomicBool,
    ticket: &Ticket,
) -> Result<(Capabilities, Sink<'a>)> {
    bind(adapter.open(slot)?, stop, ticket)
}
#[cfg(test)]
mod tests;
