#![no_std]

#[cfg(test)]
extern crate std;

use core::fmt::Write;

use arachno_imu_proto::{
    ImuSample, SENSOR_FAULT_PROBE_NO_RESPONSE, SENSOR_FAULT_UNEXPECTED_WHO_AM_I, SensorKind,
};
use heapless::String;

pub const SENSOR_STATUS_FAULT: u16 = 0x0001;
pub const SENSOR_STATUS_ACCEL_CLIPPED: u16 = 0x0002;
pub const SENSOR_STATUS_GYRO_CLIPPED: u16 = 0x0004;
pub const SENSOR_STATUS_CALIBRATING: u16 = 0x0020;

pub const MPU_I2C_ADDRESSES: [u8; 2] = [0x68, 0x69];
pub const MPU_MEASUREMENT_PAYLOAD_LEN: usize = 14;
pub const MPU_WHO_AM_I_MPU6050: u8 = 0x68;
pub const MPU_WHO_AM_I_MPU6500: u8 = 0x70;
pub const MPU_WHO_AM_I_MPU9250: u8 = 0x71;
pub const MPU_REG_SMPLRT_DIV: u8 = 0x19;
pub const MPU_REG_CONFIG: u8 = 0x1A;
pub const MPU_REG_GYRO_CONFIG: u8 = 0x1B;
pub const MPU_REG_ACCEL_CONFIG: u8 = 0x1C;
pub const MPU_REG_ACCEL_CONFIG2: u8 = 0x1D;
pub const MPU_REG_ACCEL_XOUT_H: u8 = 0x3B;
pub const MPU_REG_SIGNAL_PATH_RESET: u8 = 0x68;
pub const MPU_REG_USER_CTRL: u8 = 0x6A;
pub const MPU_REG_PWR_MGMT_1: u8 = 0x6B;
pub const MPU_REG_PWR_MGMT_2: u8 = 0x6C;
pub const MPU_REG_WHO_AM_I: u8 = 0x75;

pub const MPU_USER_CTRL_I2C_IF_DIS: u8 = 1 << 4;
pub const MPU_PWR_MGMT_1_H_RESET: u8 = 1 << 7;
pub const MPU_PWR_MGMT_1_CLKSEL_AUTO: u8 = 0x01;
pub const MPU_SIGNAL_PATH_RESET_ALL: u8 = 0x07;

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct FaultInfo {
    pub code: u8,
    pub observed_who_am_i: u8,
    pub spi_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeResult {
    pub sensor_kind: SensorKind,
    pub who_am_i: u8,
    pub spi_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitStep {
    pub register: u8,
    pub value: u8,
    pub delay_after_ms: u64,
}

const SPI_INIT_STEPS: [InitStep; 11] = [
    InitStep {
        register: MPU_REG_USER_CTRL,
        value: MPU_USER_CTRL_I2C_IF_DIS,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_PWR_MGMT_1,
        value: MPU_PWR_MGMT_1_H_RESET,
        delay_after_ms: 100,
    },
    InitStep {
        register: MPU_REG_USER_CTRL,
        value: MPU_USER_CTRL_I2C_IF_DIS,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_SIGNAL_PATH_RESET,
        value: MPU_SIGNAL_PATH_RESET_ALL,
        delay_after_ms: 10,
    },
    InitStep {
        register: MPU_REG_PWR_MGMT_1,
        value: MPU_PWR_MGMT_1_CLKSEL_AUTO,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_PWR_MGMT_2,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_CONFIG,
        value: 0x03,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_SMPLRT_DIV,
        value: 0x04,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_GYRO_CONFIG,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_ACCEL_CONFIG,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_ACCEL_CONFIG2,
        value: 0x03,
        delay_after_ms: 20,
    },
];

const I2C_INIT_STEPS: [InitStep; 9] = [
    InitStep {
        register: MPU_REG_PWR_MGMT_1,
        value: MPU_PWR_MGMT_1_H_RESET,
        delay_after_ms: 100,
    },
    InitStep {
        register: MPU_REG_SIGNAL_PATH_RESET,
        value: MPU_SIGNAL_PATH_RESET_ALL,
        delay_after_ms: 10,
    },
    InitStep {
        register: MPU_REG_PWR_MGMT_1,
        value: MPU_PWR_MGMT_1_CLKSEL_AUTO,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_PWR_MGMT_2,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_CONFIG,
        value: 0x03,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_SMPLRT_DIV,
        value: 0x04,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_GYRO_CONFIG,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_ACCEL_CONFIG,
        value: 0x00,
        delay_after_ms: 0,
    },
    InitStep {
        register: MPU_REG_ACCEL_CONFIG2,
        value: 0x03,
        delay_after_ms: 20,
    },
];

pub fn init_steps(disable_i2c_interface: bool) -> &'static [InitStep] {
    if disable_i2c_interface {
        &SPI_INIT_STEPS
    } else {
        &I2C_INIT_STEPS
    }
}

pub fn sensor_kind_from_who_am_i(who_am_i: u8) -> Option<SensorKind> {
    match who_am_i {
        MPU_WHO_AM_I_MPU6050 => Some(SensorKind::Mpu6050),
        MPU_WHO_AM_I_MPU9250 => Some(SensorKind::Mpu9250),
        MPU_WHO_AM_I_MPU6500 => Some(SensorKind::Mpu6500),
        _ => None,
    }
}

pub fn validate_who_am_i(observed_who_am_i: u8, probe: ProbeResult) -> Result<(), FaultInfo> {
    if observed_who_am_i == probe.who_am_i {
        return Ok(());
    }

    Err(FaultInfo {
        code: if observed_who_am_i == 0x00 || observed_who_am_i == 0xFF {
            SENSOR_FAULT_PROBE_NO_RESPONSE
        } else {
            SENSOR_FAULT_UNEXPECTED_WHO_AM_I
        },
        observed_who_am_i,
        spi_mode: probe.spi_mode,
    })
}

pub fn sample_from_payload(
    payload: [u8; MPU_MEASUREMENT_PAYLOAD_LEN],
    warmup_remaining: &mut u32,
    timestamp_us: u32,
) -> ImuSample {
    let accel_raw = [
        be_i16(payload[0], payload[1]),
        be_i16(payload[2], payload[3]),
        be_i16(payload[4], payload[5]),
    ];
    let temp_raw = be_i16(payload[6], payload[7]);
    let gyro_raw = [
        be_i16(payload[8], payload[9]),
        be_i16(payload[10], payload[11]),
        be_i16(payload[12], payload[13]),
    ];

    let mut status = 0u16;
    if *warmup_remaining > 0 {
        status |= SENSOR_STATUS_CALIBRATING;
        *warmup_remaining -= 1;
    }
    if near_limit(&accel_raw) {
        status |= SENSOR_STATUS_ACCEL_CLIPPED;
    }
    if near_limit(&gyro_raw) {
        status |= SENSOR_STATUS_GYRO_CLIPPED;
    }

    ImuSample {
        timestamp_us,
        accel_mg: accel_raw.map(raw_accel_to_mg),
        gyro_mdps: gyro_raw.map(raw_gyro_to_mdps),
        temperature_centi_c: raw_temp_to_centi_c(temp_raw),
        status,
    }
}

fn be_i16(high: u8, low: u8) -> i16 {
    i16::from_be_bytes([high, low])
}

fn raw_accel_to_mg(raw: i16) -> i16 {
    ((raw as i32 * 1000) / 16_384) as i16
}

fn raw_gyro_to_mdps(raw: i16) -> i32 {
    (raw as i32 * 1000) / 131
}

fn raw_temp_to_centi_c(raw: i16) -> i16 {
    (((raw as i32) * 10_000) / 33_387 + 2_100) as i16
}

fn near_limit(values: &[i16; 3]) -> bool {
    values
        .iter()
        .copied()
        .any(|value| value >= 32_000 || value <= -32_000)
}

pub const INA228_ADDRESS_MIN: u8 = 0x40;
pub const INA228_ADDRESS_MAX: u8 = 0x4F;
pub const INA228_MANUFACTURER_ID: u16 = 0x5449;
pub const INA228_DEVICE_ID_MASK: u16 = 0xFFF0;
pub const INA228_DEVICE_ID: u16 = 0x2280;
pub const INA226_MANUFACTURER_ID: u16 = 0x5449;
pub const INA226_DEVICE_ID: u16 = 0x2260;

pub const DISPLAY_STATUS_LINE_COUNT: usize = 10;
pub const DISPLAY_STATUS_COLUMNS: usize = 20;
pub type DisplayLine = String<24>;

/// A command and its parameter bytes for a write-only SPI LCD controller.
///
/// The profile is kept in this testable library crate because Waveshare's mechanically similar
/// 1.83-inch LCD revisions use incompatible controller initialisation sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LcdInitCommand {
    pub command: u8,
    pub data: &'static [u8],
}

/// Waveshare 1.83-inch LCD Module Rev2 (240×284, ST7789P) register profile.
///
/// The Rev1 240×280 NV3030B module is not compatible with this sequence.
pub const WAVESHARE_1IN83_REV2_INIT: [LcdInitCommand; 14] = [
    LcdInitCommand {
        command: 0x36,
        data: &[0x00],
    },
    LcdInitCommand {
        command: 0x3A,
        data: &[0x05],
    },
    LcdInitCommand {
        command: 0xB2,
        data: &[0x0C, 0x0C, 0x00, 0x33, 0x33],
    },
    LcdInitCommand {
        command: 0xB7,
        data: &[0x35],
    },
    LcdInitCommand {
        command: 0xBB,
        data: &[0x19],
    },
    LcdInitCommand {
        command: 0xC0,
        data: &[0x2C],
    },
    LcdInitCommand {
        command: 0xC2,
        data: &[0x01],
    },
    LcdInitCommand {
        command: 0xC3,
        data: &[0x12],
    },
    LcdInitCommand {
        command: 0xC4,
        data: &[0x20],
    },
    LcdInitCommand {
        command: 0xC6,
        data: &[0x0F],
    },
    LcdInitCommand {
        command: 0xD0,
        data: &[0xA4, 0xA1],
    },
    LcdInitCommand {
        command: 0xE0,
        data: &[
            0xD0, 0x04, 0x0D, 0x11, 0x13, 0x2B, 0x3F, 0x54, 0x4C, 0x18, 0x0D, 0x0B, 0x1F, 0x23,
        ],
    },
    LcdInitCommand {
        command: 0xE1,
        data: &[
            0xD0, 0x04, 0x0C, 0x11, 0x13, 0x2C, 0x3F, 0x44, 0x51, 0x2F, 0x1F, 0x1F, 0x20, 0x23,
        ],
    },
    LcdInitCommand {
        command: 0x21,
        data: &[],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerMonitorMeasurement {
    pub bus_millivolts: u32,
    pub shunt_microvolts: i32,
    pub current_milliamps: i32,
    pub power_milliwatts: i32,
    /// INA228 exposes a die temperature; INA226 does not.
    pub die_temperature_centi_c: Option<i16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMonitorKind {
    Ina226,
    Ina228,
}

impl PowerMonitorKind {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Ina226 => "INA226",
            Self::Ina228 => "INA228",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMonitorStatus {
    Online {
        kind: PowerMonitorKind,
        address: u8,
        measurement: PowerMonitorMeasurement,
    },
    /// No device on any standard INA226/INA228 address acknowledged its I2C address.
    NoResponse,
    /// An I2C device acknowledged but did not expose INA226 or INA228 manufacturer/device IDs.
    IdentityMismatch { address: u8 },
    /// A device address acknowledged, then a later I2C transaction failed.
    BusError { address: u8 },
}

impl PowerMonitorStatus {
    pub const fn online(
        kind: PowerMonitorKind,
        address: u8,
        measurement: PowerMonitorMeasurement,
    ) -> Self {
        Self::Online {
            kind,
            address,
            measurement,
        }
    }
}

pub struct DisplayStatus {
    pub lines: [DisplayLine; DISPLAY_STATUS_LINE_COUNT],
}

pub fn is_ina228_identity(manufacturer_id: u16, device_id: u16) -> bool {
    manufacturer_id == INA228_MANUFACTURER_ID
        && (device_id & INA228_DEVICE_ID_MASK) == INA228_DEVICE_ID
}

pub fn is_ina226_identity(manufacturer_id: u16, device_id: u16) -> bool {
    manufacturer_id == INA226_MANUFACTURER_ID && device_id == INA226_DEVICE_ID
}

/// Converts the INA228's left-aligned register values using its default ±163.84 mV shunt range.
///
/// `shunt_micro_ohms` must be the actual resistance of the sense shunt fitted to the board.
pub fn ina228_measurement_from_registers(
    shunt_register: [u8; 3],
    bus_register: [u8; 3],
    die_temperature_register: [u8; 2],
    shunt_micro_ohms: u32,
) -> PowerMonitorMeasurement {
    let shunt_raw = signed_20_bit(shunt_register);
    let bus_raw = unsigned_20_bit(bus_register);
    let die_temperature_raw = i16::from_be_bytes(die_temperature_register);

    // INA228 default ADCRANGE = 0: 312.5 nV/LSB, or 5/16 µV/LSB.
    let shunt_microvolts = shunt_raw.saturating_mul(5) / 16;
    // INA228 VBUS: 195.3125 µV/LSB, or 25/128 mV/LSB.
    let bus_millivolts = ((u64::from(bus_raw) * 25) / 128) as u32;
    // INA228 DIETEMP: 7.8125 m°C/LSB, or 25/32 centi°C/LSB.
    let die_temperature_centi_c = ((i32::from(die_temperature_raw) * 25) / 32) as i16;
    let current_milliamps = current_milliamps_from_shunt(shunt_microvolts, shunt_micro_ohms);
    let power_milliwatts =
        clamp_i64_to_i32((i64::from(bus_millivolts) * i64::from(current_milliamps)) / 1_000);

    PowerMonitorMeasurement {
        bus_millivolts,
        shunt_microvolts,
        current_milliamps,
        power_milliwatts,
        die_temperature_centi_c: Some(die_temperature_centi_c),
    }
}

/// Converts INA226 register values. Current and power are calculated directly from the measured
/// shunt voltage so this works without relying on the INA226 calibration register.
pub fn ina226_measurement_from_registers(
    shunt_register: [u8; 2],
    bus_register: [u8; 2],
    shunt_micro_ohms: u32,
) -> PowerMonitorMeasurement {
    let shunt_raw = i16::from_be_bytes(shunt_register);
    let bus_raw = u16::from_be_bytes(bus_register);

    // INA226 VSHUNT: 2.5 µV/LSB. Keep integer arithmetic with rounded-toward-zero halves.
    let shunt_microvolts = (i32::from(shunt_raw) * 5) / 2;
    // INA226 VBUS: 1.25 mV/LSB.
    let bus_millivolts = (u32::from(bus_raw) * 5) / 4;
    let current_milliamps = current_milliamps_from_shunt(shunt_microvolts, shunt_micro_ohms);
    let power_milliwatts =
        clamp_i64_to_i32((i64::from(bus_millivolts) * i64::from(current_milliamps)) / 1_000);

    PowerMonitorMeasurement {
        bus_millivolts,
        shunt_microvolts,
        current_milliamps,
        power_milliwatts,
        die_temperature_centi_c: None,
    }
}

pub fn format_display_status(sample: ImuSample, power: PowerMonitorStatus) -> DisplayStatus {
    let mut lines = core::array::from_fn(|_| DisplayLine::new());

    push_text(&mut lines[0], "ARACHNO");

    push_text(&mut lines[1], "A X");
    push_signed_milli(&mut lines[1], i32::from(sample.accel_mg[0]));
    push_text(&mut lines[1], " Y");
    push_signed_milli(&mut lines[1], i32::from(sample.accel_mg[1]));

    push_text(&mut lines[2], "  Z");
    push_signed_milli(&mut lines[2], i32::from(sample.accel_mg[2]));
    push_text(&mut lines[2], " G");

    push_text(&mut lines[3], "G X");
    push_signed_tenths(&mut lines[3], sample.gyro_mdps[0] / 100);
    push_text(&mut lines[3], " Y");
    push_signed_tenths(&mut lines[3], sample.gyro_mdps[1] / 100);

    push_text(&mut lines[4], "  Z");
    push_signed_tenths(&mut lines[4], sample.gyro_mdps[2] / 100);
    push_text(&mut lines[4], " D/S");

    push_text(&mut lines[5], "T ");
    push_signed_centi(&mut lines[5], i32::from(sample.temperature_centi_c));
    push_text(&mut lines[5], " C");

    match power {
        PowerMonitorStatus::Online {
            kind,
            address,
            measurement,
        } => {
            push_text(&mut lines[6], kind.display_name());
            push_text(&mut lines[6], " ");
            write!(&mut lines[6], "{address:02X}").expect("INA address fits display line");

            push_text(&mut lines[7], "V ");
            push_unsigned_milli(&mut lines[7], measurement.bus_millivolts);
            push_text(&mut lines[7], " I");
            push_signed_milli(&mut lines[7], measurement.current_milliamps);

            push_text(&mut lines[8], "P ");
            push_signed_tenths(&mut lines[8], measurement.power_milliwatts / 100);
            push_text(&mut lines[8], "W");

            push_text(&mut lines[9], "S");
            push_signed_milli(&mut lines[9], measurement.shunt_microvolts);
            push_text(&mut lines[9], "mV T");
            match measurement.die_temperature_centi_c {
                Some(temperature_centi_c) => {
                    push_signed_centi(&mut lines[9], i32::from(temperature_centi_c));
                }
                None => push_text(&mut lines[9], "--"),
            }
        }
        PowerMonitorStatus::NoResponse => {
            push_text(&mut lines[6], "INA NACK 40-4F");
            push_text(&mut lines[7], "CHECK SDA SCL PWR");
            push_text(&mut lines[8], "P --");
            push_text(&mut lines[9], "S -- T --");
        }
        PowerMonitorStatus::IdentityMismatch { address } => {
            push_text(&mut lines[6], "INA ID MISMATCH");
            write!(&mut lines[7], "ADDR {address:02X}").expect("INA address fits display line");
            push_text(&mut lines[8], "P --");
            push_text(&mut lines[9], "S -- T --");
        }
        PowerMonitorStatus::BusError { address } => {
            write!(&mut lines[6], "INA I2C ERR {address:02X}")
                .expect("INA address fits display line");
            push_text(&mut lines[7], "CHECK SDA SCL PWR");
            push_text(&mut lines[8], "P --");
            push_text(&mut lines[9], "S -- T --");
        }
    }

    DisplayStatus { lines }
}

fn unsigned_20_bit(register: [u8; 3]) -> u32 {
    u32::from_be_bytes([0, register[0], register[1], register[2]]) >> 4
}

fn signed_20_bit(register: [u8; 3]) -> i32 {
    let raw = unsigned_20_bit(register);
    if raw & (1 << 19) == 0 {
        raw as i32
    } else {
        raw as i32 - (1 << 20)
    }
}

fn current_milliamps_from_shunt(shunt_microvolts: i32, shunt_micro_ohms: u32) -> i32 {
    if shunt_micro_ohms == 0 {
        return 0;
    }

    clamp_i64_to_i32((i64::from(shunt_microvolts) * 1_000) / i64::from(shunt_micro_ohms))
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn push_text(line: &mut DisplayLine, text: &str) {
    line.push_str(text)
        .expect("display line has fixed capacity");
}

fn push_signed_milli(line: &mut DisplayLine, value: i32) {
    push_signed_fixed(line, i64::from(value), 1_000, 3);
}

fn push_unsigned_milli(line: &mut DisplayLine, value: u32) {
    let whole = value / 1_000;
    let fraction = value % 1_000;
    write!(line, "{whole}.{fraction:03}").expect("display line has fixed capacity");
}

fn push_signed_centi(line: &mut DisplayLine, value: i32) {
    push_signed_fixed(line, i64::from(value), 100, 2);
}

fn push_signed_tenths(line: &mut DisplayLine, value: i32) {
    push_signed_fixed(line, i64::from(value), 10, 1);
}

fn push_signed_fixed(line: &mut DisplayLine, value: i64, scale: u64, decimals: u8) {
    let magnitude = value.unsigned_abs();
    let whole = magnitude / scale;
    let fraction = magnitude % scale;
    let sign = if value < 0 { '-' } else { '+' };

    match decimals {
        1 => write!(line, "{sign}{whole}.{fraction:01}"),
        2 => write!(line, "{sign}{whole}.{fraction:02}"),
        3 => write!(line, "{sign}{whole}.{fraction:03}"),
        _ => unreachable!("display format only uses one to three decimal places"),
    }
    .expect("display line has fixed capacity");
}

#[cfg(test)]
mod tests {
    use super::*;
    use arachno_imu_proto::SPI_MODE_UNKNOWN;

    #[test]
    fn sensor_kind_from_who_am_i_supports_mpu6050() {
        assert_eq!(
            sensor_kind_from_who_am_i(MPU_WHO_AM_I_MPU6050),
            Some(SensorKind::Mpu6050)
        );
    }

    #[test]
    fn init_steps_include_i2c_disable_only_for_spi() {
        assert_eq!(init_steps(true)[0].register, MPU_REG_USER_CTRL);
        assert_eq!(init_steps(true)[0].value, MPU_USER_CTRL_I2C_IF_DIS);
        assert_eq!(init_steps(false)[0].register, MPU_REG_PWR_MGMT_1);
    }

    #[test]
    fn validate_who_am_i_reports_unexpected_id() {
        let probe = ProbeResult {
            sensor_kind: SensorKind::Mpu6050,
            who_am_i: MPU_WHO_AM_I_MPU6050,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        assert_eq!(
            validate_who_am_i(0x42, probe),
            Err(FaultInfo {
                code: SENSOR_FAULT_UNEXPECTED_WHO_AM_I,
                observed_who_am_i: 0x42,
                spi_mode: SPI_MODE_UNKNOWN,
            })
        );
    }

    #[test]
    fn sample_from_payload_converts_values_and_sets_status_bits() {
        let payload = [
            0x7D, 0x00, // accel x near positive limit
            0x00, 0x00, // accel y
            0x80, 0x00, // accel z near negative limit
            0x00, 0x00, // temp
            0x7D, 0x00, // gyro x near positive limit
            0x00, 0x83, // gyro y
            0x80, 0x00, // gyro z near negative limit
        ];
        let mut warmup_remaining = 1;

        let sample = sample_from_payload(payload, &mut warmup_remaining, 123);

        assert_eq!(sample.timestamp_us, 123);
        assert_eq!(sample.accel_mg[0], 1953);
        assert_eq!(sample.gyro_mdps[0], 244274);
        assert_eq!(
            sample.status,
            SENSOR_STATUS_CALIBRATING | SENSOR_STATUS_ACCEL_CLIPPED | SENSOR_STATUS_GYRO_CLIPPED
        );
        assert_eq!(warmup_remaining, 0);
    }

    #[test]
    fn ina228_register_conversion_uses_r002_two_milliohm_shunt_resistance() {
        let measurement = ina228_measurement_from_registers(
            [0x00, 0xA0, 0x00], // 2,560 LSB = 800 µV
            [0x0F, 0xA0, 0x00], // 64,000 LSB = 12.500 V
            [0x0C, 0x80],       // 3,200 LSB = 25.00 °C
            2_000,              // R002 = 2 mΩ
        );

        assert_eq!(
            measurement,
            PowerMonitorMeasurement {
                bus_millivolts: 12_500,
                shunt_microvolts: 800,
                current_milliamps: 400,
                power_milliwatts: 5_000,
                die_temperature_centi_c: Some(2_500),
            }
        );
    }

    #[test]
    fn ina228_identity_requires_ti_manufacturer_and_ina228_device_bits() {
        assert!(is_ina228_identity(0x5449, 0x2281));
        assert!(!is_ina228_identity(0x5449, 0x2291));
        assert!(!is_ina228_identity(0x0000, 0x2281));
    }

    #[test]
    fn ina226_identity_and_register_conversion_are_supported_with_r002() {
        assert!(is_ina226_identity(0x5449, 0x2260));
        assert!(!is_ina226_identity(0x5449, 0x2281));
        assert!(!is_ina226_identity(0x0000, 0x2260));

        let measurement = ina226_measurement_from_registers(
            [0x06, 0x40], // 1,600 LSB = 4,000 µV
            [0x27, 0x10], // 10,000 LSB = 12.500 V
            2_000,        // R002 = 2 mΩ
        );

        assert_eq!(
            measurement,
            PowerMonitorMeasurement {
                bus_millivolts: 12_500,
                shunt_microvolts: 4_000,
                current_milliamps: 2_000,
                power_milliwatts: 25_000,
                die_temperature_centi_c: None,
            }
        );
    }

    #[test]
    fn display_status_formats_imu_and_power_monitor_values_in_fixed_width_lines() {
        let sample = ImuSample {
            timestamp_us: 0,
            accel_mg: [1_000, -20, 980],
            gyro_mdps: [12_300, -400, 0],
            temperature_centi_c: 2_500,
            status: 0,
        };
        let measurement = PowerMonitorMeasurement {
            bus_millivolts: 12_500,
            shunt_microvolts: 800,
            current_milliamps: 80,
            power_milliwatts: 1_000,
            die_temperature_centi_c: Some(2_500),
        };

        let status = format_display_status(
            sample,
            PowerMonitorStatus::online(PowerMonitorKind::Ina228, 0x40, measurement),
        );

        assert_eq!(status.lines[1].as_str(), "A X+1.000 Y-0.020");
        assert_eq!(status.lines[3].as_str(), "G X+12.3 Y-0.4");
        assert_eq!(status.lines[7].as_str(), "V 12.500 I+0.080");
        assert_eq!(status.lines[8].as_str(), "P +1.0W");
        assert_eq!(status.lines[9].as_str(), "S+0.800mV T+25.00");
        assert!(
            status
                .lines
                .iter()
                .all(|line| line.len() <= DISPLAY_STATUS_COLUMNS)
        );
    }

    #[test]
    fn display_status_identifies_ina226_and_marks_its_temperature_unavailable() {
        let status = format_display_status(
            ImuSample::default(),
            PowerMonitorStatus::online(
                PowerMonitorKind::Ina226,
                0x45,
                PowerMonitorMeasurement {
                    bus_millivolts: 12_500,
                    shunt_microvolts: 800,
                    current_milliamps: 400,
                    power_milliwatts: 5_000,
                    die_temperature_centi_c: None,
                },
            ),
        );

        assert_eq!(status.lines[6].as_str(), "INA226 45");
        assert_eq!(status.lines[9].as_str(), "S+0.800mV T--");
    }

    #[test]
    fn display_status_explains_ina228_probe_failures() {
        let sample = ImuSample::default();

        let no_response = format_display_status(sample, PowerMonitorStatus::NoResponse);
        assert_eq!(no_response.lines[6].as_str(), "INA NACK 40-4F");
        assert_eq!(no_response.lines[7].as_str(), "CHECK SDA SCL PWR");

        let wrong_device = format_display_status(
            sample,
            PowerMonitorStatus::IdentityMismatch { address: 0x43 },
        );
        assert_eq!(wrong_device.lines[6].as_str(), "INA ID MISMATCH");
        assert_eq!(wrong_device.lines[7].as_str(), "ADDR 43");

        let bus_error =
            format_display_status(sample, PowerMonitorStatus::BusError { address: 0x40 });
        assert_eq!(bus_error.lines[6].as_str(), "INA I2C ERR 40");
        assert_eq!(bus_error.lines[7].as_str(), "CHECK SDA SCL PWR");
    }

    #[test]
    fn waveshare_rev2_lcd_profile_uses_the_documented_rgb565_setup() {
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[0].command, 0x36);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[0].data, &[0x00]);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[1].command, 0x3A);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[1].data, &[0x05]);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[2].command, 0xB2);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[10].command, 0xD0);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[11].command, 0xE0);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[12].command, 0xE1);
        assert_eq!(WAVESHARE_1IN83_REV2_INIT[13].command, 0x21);
    }
}
