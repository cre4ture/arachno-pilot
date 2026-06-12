#![no_std]

#[cfg(test)]
extern crate std;

use arachno_imu_proto::{
    ImuSample, SENSOR_FAULT_PROBE_NO_RESPONSE, SENSOR_FAULT_UNEXPECTED_WHO_AM_I, SensorKind,
};

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
}
