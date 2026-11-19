use i2cdev::{core::I2CDevice, linux::LinuxI2CDevice};
use std::{error::Error, fs, path::Path};

fn color_bytes([red, green, blue]: [u8; 3], leds: usize) -> Vec<u8> {
    [red, blue, green].repeat(leds)
}

pub fn open(card: &Path) -> Result<Ene, Box<dyn Error>> {
    if crate::hardware::nvidia_ene_name(card).is_none() {
        return Err("GPU is not listed for this ASUS ENE driver.".into());
    }
    let mut adapter = None;
    for entry in fs::read_dir(card)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(number) = name
            .to_str()
            .and_then(|s| s.strip_prefix("i2c-"))
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if fs::read_to_string(entry.path().join("name"))?.starts_with("NVIDIA i2c adapter 1 at ")
            && adapter.replace(number).is_some()
        {
            return Err("Multiple NVIDIA buses match this card.".into());
        }
    }
    let adapter = adapter.ok_or("NVIDIA I²C bus not found.")?;
    // These ENE GPU firmware layouts share the direct-color register and LED count field.
    open_ene(
        adapter,
        0x67,
        &[
            b"AUMA0-E6K5-1113",
            b"AUMA0-E6K5-1114",
            b"AUMA0-E6K5-0107",
            b"AUMA0-E6K5-1107",
            b"AUMA0-E6K5-1110",
            b"AUMA0-E6K5-1111",
            b"AUMA0-E6K5-0008",
        ],
        0x8100,
        None,
    )
}

pub struct Ene {
    bus: LinuxI2CDevice,
    base: u16,
    leds: u8,
}

pub fn open_ene(
    adapter: u32,
    address: u16,
    identities: &[&[u8]],
    base: u16,
    fixed_leds: Option<u8>,
) -> Result<Ene, Box<dyn Error>> {
    let mut bus = LinuxI2CDevice::new(format!("/dev/i2c-{adapter}"), address)?;
    if fixed_leds.is_some() {
        // Match the ENE DRAM detection handshake before selecting a register.
        if bus.smbus_read_byte().is_err() {
            bus.smbus_read_byte_data(0x00).map_err(|error| {
                format!("ENE probe on /dev/i2c-{adapter} at {address:#04x}: {error}")
            })?;
        }
    }
    let mut read = |register: u16| {
        bus.smbus_write_word_data(0x00, register.swap_bytes())
            .map_err(|error| format!("ENE select register {register:#06x}: {error}"))?;
        bus.smbus_read_byte_data(0x81)
            .map_err(|error| format!("ENE read register {register:#06x}: {error}"))
    };
    let mut identity = [0; 15];
    let length = identities
        .first()
        .map(|known| known.len())
        .filter(|length| (1..=identity.len()).contains(length))
        .ok_or("Invalid ENE firmware list")?;
    for offset in 0..length {
        identity[offset] = read(0x1000 + offset as u16)?;
        if !identities
            .iter()
            .any(|known| known.get(..=offset) == Some(&identity[..=offset]))
        {
            return Err("Unknown ENE controller. No color applied.".into());
        }
    }
    let leds = match fixed_leds {
        Some(count) => count,
        None => read(0x1c03)?,
    };
    if !(1..=30).contains(&leds) {
        return Err("Unexpected LED count. No color applied.".into());
    }

    // ENE Aura direct mode. Do not issue the persistent save command.
    for register in [0x8020u16, 0x80a0] {
        bus.smbus_write_word_data(0x00, register.swap_bytes())?;
        bus.smbus_write_byte_data(0x01, 1)?;
    }
    Ok(Ene { bus, base, leds })
}

impl Ene {
    pub fn set(&mut self, rgb: [u8; 3]) -> Result<(), Box<dyn Error>> {
        for (index, block) in color_bytes(rgb, usize::from(self.leds))
            .chunks(31)
            .enumerate()
        {
            let register = self.base + (index * 31) as u16;
            self.bus
                .smbus_write_word_data(0x00, register.swap_bytes())?;
            self.bus.smbus_write_block_data(0x03, block)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn uses_rbg_order_for_every_led() {
        assert_eq!(super::color_bytes([1, 2, 3], 2), [1, 3, 2, 1, 3, 2]);
    }
}
