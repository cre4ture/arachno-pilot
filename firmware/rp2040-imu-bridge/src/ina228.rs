use defmt::info;
use embassy_rp::Peri;
use embassy_rp::i2c::{self, I2c};
use embassy_rp::peripherals::{I2C0, PIN_8, PIN_9};
use rp2040_imu_bridge::{
    INA228_ADDRESS_MAX, INA228_ADDRESS_MIN, PowerMonitorKind, PowerMonitorMeasurement,
    PowerMonitorStatus, ina226_measurement_from_registers, ina228_measurement_from_registers,
    is_ina226_identity, is_ina228_identity,
};

const INA228_I2C_HZ: u32 = 400_000;
// R002 marking: 0.002 Ω = 2 mΩ.
pub const INA228_SHUNT_MICRO_OHMS: u32 = 2_000;

const REG_INA228_ADC_CONFIG: u8 = 0x01;
const REG_INA228_VSHUNT: u8 = 0x04;
const REG_INA228_VBUS: u8 = 0x05;
const REG_INA228_DIETEMP: u8 = 0x06;
const REG_INA228_MANUFACTURER_ID: u8 = 0x3E;
const REG_INA228_DEVICE_ID: u8 = 0x3F;
const REG_INA226_VSHUNT: u8 = 0x01;
const REG_INA226_VBUS: u8 = 0x02;
const REG_INA226_MANUFACTURER_ID: u8 = 0xFE;
const REG_INA226_DIE_ID: u8 = 0xFF;

// Continuous VBUS, VSHUNT, and temperature conversion with the INA228 reset-time settings.
const ADC_CONFIG_CONTINUOUS_ALL: u16 = 0xFB68;

pub struct InaPowerMonitor<'d> {
    i2c: I2c<'d, I2C0, i2c::Blocking>,
    monitor: Option<DetectedPowerMonitor>,
    shunt_micro_ohms: u32,
}

#[derive(Debug, Clone, Copy)]
struct DetectedPowerMonitor {
    kind: PowerMonitorKind,
    address: u8,
}

impl<'d> InaPowerMonitor<'d> {
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
            monitor: None,
            shunt_micro_ohms,
        }
    }

    fn read_measurement(
        &mut self,
    ) -> Result<(DetectedPowerMonitor, PowerMonitorMeasurement), PowerMonitorError> {
        let monitor = match self.monitor {
            Some(monitor) => monitor,
            None => self.probe()?,
        };

        let result = match monitor.kind {
            PowerMonitorKind::Ina228 => self.read_ina228_measurement(monitor.address),
            PowerMonitorKind::Ina226 => self.read_ina226_measurement(monitor.address),
        };

        match result {
            Ok(measurement) => Ok((monitor, measurement)),
            Err(error) => {
                self.monitor = None;
                Err(error)
            }
        }
    }

    pub fn read_status(&mut self) -> PowerMonitorStatus {
        match self.read_measurement() {
            Ok((monitor, measurement)) => {
                PowerMonitorStatus::online(monitor.kind, monitor.address, measurement)
            }
            Err(PowerMonitorError::NotFound) => PowerMonitorStatus::NoResponse,
            Err(PowerMonitorError::UnexpectedIdentity { address }) => {
                PowerMonitorStatus::IdentityMismatch { address }
            }
            Err(PowerMonitorError::I2c { address, .. }) => PowerMonitorStatus::BusError { address },
        }
    }

    fn probe(&mut self) -> Result<DetectedPowerMonitor, PowerMonitorError> {
        let mut unexpected_identity_address = None;

        for address in INA228_ADDRESS_MIN..=INA228_ADDRESS_MAX {
            let kind = match self.probe_address(address) {
                Ok(ProbeOutcome::NoResponse) => continue,
                Ok(ProbeOutcome::UnexpectedIdentity) => {
                    unexpected_identity_address.get_or_insert(address);
                    continue;
                }
                Ok(ProbeOutcome::Found(kind)) => kind,
                Err(error) => return Err(PowerMonitorError::I2c { address, error }),
            };

            if kind == PowerMonitorKind::Ina228 {
                self.write_u16(address, REG_INA228_ADC_CONFIG, ADC_CONFIG_CONTINUOUS_ALL)
                    .map_err(|error| PowerMonitorError::I2c { address, error })?;
            }

            let monitor = DetectedPowerMonitor { kind, address };
            self.monitor = Some(monitor);
            match kind {
                PowerMonitorKind::Ina226 => info!("INA226 online at I2C address {=u8}", address),
                PowerMonitorKind::Ina228 => info!("INA228 online at I2C address {=u8}", address),
            }
            return Ok(monitor);
        }

        match unexpected_identity_address {
            Some(address) => Err(PowerMonitorError::UnexpectedIdentity { address }),
            None => Err(PowerMonitorError::NotFound),
        }
    }

    fn probe_address(&mut self, address: u8) -> Result<ProbeOutcome, i2c::Error> {
        let ina228_manufacturer_id = match self.read_u16(address, REG_INA228_MANUFACTURER_ID) {
            Ok(value) => value,
            Err(error) if is_no_acknowledge(error) => return Ok(ProbeOutcome::NoResponse),
            Err(error) => return Err(error),
        };
        let ina228_device_id = self.read_u16(address, REG_INA228_DEVICE_ID)?;
        if is_ina228_identity(ina228_manufacturer_id, ina228_device_id) {
            return Ok(ProbeOutcome::Found(PowerMonitorKind::Ina228));
        }

        let ina226_manufacturer_id = self.read_u16(address, REG_INA226_MANUFACTURER_ID)?;
        let ina226_device_id = self.read_u16(address, REG_INA226_DIE_ID)?;
        if is_ina226_identity(ina226_manufacturer_id, ina226_device_id) {
            return Ok(ProbeOutcome::Found(PowerMonitorKind::Ina226));
        }

        Ok(ProbeOutcome::UnexpectedIdentity)
    }

    fn read_ina228_measurement(
        &mut self,
        address: u8,
    ) -> Result<PowerMonitorMeasurement, PowerMonitorError> {
        let shunt_register = self
            .read_u24(address, REG_INA228_VSHUNT)
            .map_err(|error| PowerMonitorError::I2c { address, error })?;
        let bus_register = self
            .read_u24(address, REG_INA228_VBUS)
            .map_err(|error| PowerMonitorError::I2c { address, error })?;
        let die_temperature_register = self
            .read_u16_bytes(address, REG_INA228_DIETEMP)
            .map_err(|error| PowerMonitorError::I2c { address, error })?;

        Ok(ina228_measurement_from_registers(
            shunt_register,
            bus_register,
            die_temperature_register,
            self.shunt_micro_ohms,
        ))
    }

    fn read_ina226_measurement(
        &mut self,
        address: u8,
    ) -> Result<PowerMonitorMeasurement, PowerMonitorError> {
        let shunt_register = self
            .read_u16_bytes(address, REG_INA226_VSHUNT)
            .map_err(|error| PowerMonitorError::I2c { address, error })?;
        let bus_register = self
            .read_u16_bytes(address, REG_INA226_VBUS)
            .map_err(|error| PowerMonitorError::I2c { address, error })?;

        Ok(ina226_measurement_from_registers(
            shunt_register,
            bus_register,
            self.shunt_micro_ohms,
        ))
    }

    fn read_u16(&mut self, address: u8, register: u8) -> Result<u16, i2c::Error> {
        let bytes = self.read_u16_bytes(address, register)?;
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_u16_bytes(&mut self, address: u8, register: u8) -> Result<[u8; 2], i2c::Error> {
        let mut bytes = [0; 2];
        self.i2c
            .blocking_write_read(address, &[register], &mut bytes)?;
        Ok(bytes)
    }

    fn read_u24(&mut self, address: u8, register: u8) -> Result<[u8; 3], i2c::Error> {
        let mut bytes = [0; 3];
        self.i2c
            .blocking_write_read(address, &[register], &mut bytes)?;
        Ok(bytes)
    }

    fn write_u16(&mut self, address: u8, register: u8, value: u16) -> Result<(), i2c::Error> {
        let [high, low] = value.to_be_bytes();
        self.i2c.blocking_write(address, &[register, high, low])
    }
}

fn is_no_acknowledge(error: i2c::Error) -> bool {
    matches!(error, i2c::Error::Abort(i2c::AbortReason::NoAcknowledge))
}

#[derive(Debug, Clone, Copy)]
enum ProbeOutcome {
    NoResponse,
    UnexpectedIdentity,
    Found(PowerMonitorKind),
}

#[derive(Debug, Clone, Copy, defmt::Format)]
enum PowerMonitorError {
    I2c { address: u8, error: i2c::Error },
    NotFound,
    UnexpectedIdentity { address: u8 },
}
