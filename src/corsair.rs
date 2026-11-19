use i2cdev::{core::I2CDevice, linux::LinuxI2CDevice};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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

// Read-only signature used by the OpenRGB Corsair DRAM detector (see docs/discovery-audit.md).
fn identify_family(mut read: impl FnMut(u8) -> Result<u8>) -> Result<()> {
    if !matches!(read(0x43)?, 0x1a..=0x1c) || !matches!(read(0x44)?, 1 | 3 | 4) {
        return Err("Not a recognized Corsair DRAM controller.".into());
    }
    Ok(())
}

pub fn open(address: u16) -> Result<impl FnMut([u8; 3]) -> Result<()>> {
    if !crate::board::b550_present() || !matches!(address, 0x5a | 0x5b) {
        return Err("Unsupported Corsair RAM setup.".into());
    }
    let bus = crate::board::ram_bus()?;
    let mut dev = LinuxI2CDevice::new(format!("/dev/i2c-{bus}"), address)?;
    identify_family(|reg| Ok(dev.smbus_read_byte_data(reg)?))?;
    // The configuration-write busy flag does not gate device info or direct RGB.
    dev.smbus_write_byte_data(0x61, 0)?;
    dev.smbus_write_byte_data(0x21, 0)?;
    let mut info = [0; 32];
    for byte in &mut info {
        *byte = dev.smbus_read_byte_data(0x40)?;
    }
    if info[..2] != [0x1c, 0x1b]
        || !matches!(info[2..4], [0 | 1, 3])
        || info[28] < 4
        || crc(&info) != dev.smbus_read_byte_data(0x42)?
    {
        return Err("Unrecognized Corsair RGB controller.".into());
    }
    Ok(move |rgb| {
        dev.smbus_write_block_data(0x31, &frame(rgb))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identification_rejects_before_info_selectors() {
        assert!(
            identify_family(|r| {
                assert_eq!(r, 0x43);
                Ok(0xff)
            })
            .is_err()
        );
        assert!(identify_family(|r| Ok(if r == 0x43 { 0x1b } else { 4 })).is_ok());
        assert!(identify_family(|_| Err("unavailable".into())).is_err());
    }
    #[test]
    fn direct_frame() {
        assert_eq!(crc(b"123456789"), 0xf4);
        let packet = frame([1, 2, 3]);
        assert_eq!(packet[0], 10);
        assert_eq!(&packet[1..31], &[1, 2, 3].repeat(10));
        assert_eq!(crc(&packet), 0);
    }
}
