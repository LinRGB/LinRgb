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

pub fn open(address: u16) -> Result<impl FnMut([u8; 3]) -> Result<()>> {
    if !crate::board::b550_present() || !matches!(address, 0x5a | 0x5b) {
        return Err("Unsupported Corsair RAM setup.".into());
    }
    let bus = crate::board::ram_bus()?;
    let mut dev = LinuxI2CDevice::new(format!("/dev/i2c-{bus}"), address)?;
    for _ in 0..20 {
        if dev.smbus_read_byte_data(0x30)? & 8 == 0 {
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
            return Ok(move |rgb| {
                dev.smbus_write_block_data(0x31, &frame(rgb))?;
                Ok(())
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Err("Corsair RGB controller is busy.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_frame() {
        assert_eq!(crc(b"123456789"), 0xf4);
        let packet = frame([1, 2, 3]);
        assert_eq!(packet[0], 10);
        assert_eq!(&packet[1..31], &[1, 2, 3].repeat(10));
        assert_eq!(crc(&packet), 0);
    }
}
