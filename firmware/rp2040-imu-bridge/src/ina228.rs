use defmt::info;
use embassy_rp::Peri;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::peripherals::{I2C0, PIN_8, PIN_9};
use rp2040_imu_bridge::{
    INA228_ADDRESS_MAX, INA228_ADDRESS_MIN, Ina228Measurement, ina228_measurement_from_registers,
    is_ina228_identity,
};

const INA228_I2C_HZ: u32 = 400_000;
// R002 marking: 0.002 Ω = 2 mΩ.
pub const INA228_SHUNT_MICRO_OHMS: u32 = 2_000;

const REG_ADC_CONFIG: u8 = 0x01;
const REG_VSHUNT: u8 = 0x04;
const REG_VBUS: u8 = 0x05;
const REG_DIETEMP: u8 = 0x06;
const REG_MANUFACTURER_ID: u8 = 0x3E;
const REG_DEVICE_ID: u8 = 0x3F;

// Continuous VBUS, VSHUNT, and temperature conversion with the INA228 reset-time settings.
const ADC_CONFIG_CONTINUOUS_ALL: u16 = 0xFB68;

pub struct Ina228<'d> {
    i2c: I2c<'d, I2C0, i2c::Blocking>,
    address: Option<u8>,
    shunt_micro_ohms: u32,
}

impl<'d> Ina228<'d> {
    pub fn new(
        i2c0: Peri<'d, I2C0>,
        scl: Peri<'d, PIN_9>,
        sda: Peri<'d, PIN_8>,
        shunt_micro_ohms: u32,
    ) -> Self {
        let mut config = i2c::Config::default();
        config.frequency = INA228_I2C_HZ;

        Self {
            i2c: I2c::new_blocking(i2c0, scl, sda, config),
            address: None,
            shunt_micro_ohms,
        }
    }

    pub fn read_measurement(&mut self) -> Result<(u8, Ina228Measurement), Ina228Error> {
        let address = match self.address {
            Some(address) => address,
            None => self.probe()?,
        };

        let result = (|| {
            let shunt_register = self.read_u24(address, REG_VSHUNT)?;
            let bus_register = self.read_u24(address, REG_VBUS)?;
            let die_temperature_register = self.read_u16_bytes(address, REG_DIETEMP)?;

            Ok(ina228_measurement_from_registers(
                shunt_register,
                bus_register,
                die_temperature_register,
                self.shunt_micro_ohms,
            ))
        })();

        match result {
            Ok(measurement) => Ok((address, measurement)),
            Err(error) => {
                self.address = None;
                Err(error)
            }
        }
    }

    fn probe(&mut self) -> Result<u8, Ina228Error> {
        for address in INA228_ADDRESS_MIN..=INA228_ADDRESS_MAX {
            let Ok(manufacturer_id) = self.read_u16(address, REG_MANUFACTURER_ID) else {
                continue;
            };
            let Ok(device_id) = self.read_u16(address, REG_DEVICE_ID) else {
                continue;
            };

            if !is_ina228_identity(manufacturer_id, device_id) {
                continue;
            }

            self.write_u16(address, REG_ADC_CONFIG, ADC_CONFIG_CONTINUOUS_ALL)?;
            self.address = Some(address);
            info!("INA228 online at I2C address {=u8}", address);
            return Ok(address);
        }

        Err(Ina228Error::NotFound)
    }

    fn read_u16(&mut self, address: u8, register: u8) -> Result<u16, Ina228Error> {
        let bytes = self.read_u16_bytes(address, register)?;
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_u16_bytes(&mut self, address: u8, register: u8) -> Result<[u8; 2], Ina228Error> {
        let mut bytes = [0; 2];
        self.i2c
            .blocking_write_read(address, &[register], &mut bytes)
            .map_err(Ina228Error::I2c)?;
        Ok(bytes)
    }

    fn read_u24(&mut self, address: u8, register: u8) -> Result<[u8; 3], Ina228Error> {
        let mut bytes = [0; 3];
        self.i2c
            .blocking_write_read(address, &[register], &mut bytes)
            .map_err(Ina228Error::I2c)?;
        Ok(bytes)
    }

    fn write_u16(&mut self, address: u8, register: u8, value: u16) -> Result<(), Ina228Error> {
        let [high, low] = value.to_be_bytes();
        self.i2c
            .blocking_write(address, &[register, high, low])
            .map_err(Ina228Error::I2c)
    }
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum Ina228Error {
    I2c(i2c::Error),
    NotFound,
}
