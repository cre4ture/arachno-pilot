use std::{
    f32::consts::PI,
    io::{self, Read},
    time::Duration,
};

use arachno_hal::{HalError, HalResult, ImuSource};
use arachno_imu_proto::{Frame, FrameParser, ImuSample};
use arachno_msg::ImuTelemetry;
use serialport::SerialPort;

const DEFAULT_TIMEOUT_MS: u64 = 20;
const ACCEL_MG_TO_MPS2: f32 = 9.80665 / 1000.0;
const MDPS_TO_RAD_S: f32 = PI / 180_000.0;

pub struct UsbImuBridge {
    port_path: String,
    baud_rate: u32,
    description: String,
    port: Box<dyn SerialPort>,
    parser: FrameParser,
    read_buf: [u8; 64],
}

impl UsbImuBridge {
    pub fn open(port_path: impl Into<String>, baud_rate: u32) -> HalResult<Self> {
        let port_path = port_path.into();
        let port = serialport::new(&port_path, baud_rate)
            .timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .open()
            .map_err(|err| {
                HalError::Communication(format!("failed to open IMU bridge {}: {err}", port_path))
            })?;

        Ok(Self {
            description: format!("RP2040 USB IMU bridge on {port_path}"),
            port_path,
            baud_rate,
            port,
            parser: FrameParser::new(),
            read_buf: [0; 64],
        })
    }

    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    fn read_frame(&mut self) -> HalResult<Option<Frame>> {
        match self.port.read(&mut self.read_buf) {
            Ok(0) => Ok(None),
            Ok(read) => {
                for &byte in &self.read_buf[..read] {
                    match self.parser.push(byte) {
                        Ok(Some(frame)) => return Ok(Some(frame)),
                        Ok(None) => {}
                        Err(_) => {
                            // Stay tolerant during bring-up and resync on the next frame.
                        }
                    }
                }
                Ok(None)
            }
            Err(err) if err.kind() == io::ErrorKind::TimedOut => Ok(None),
            Err(err) => Err(HalError::Communication(format!(
                "failed reading IMU bridge {}: {err}",
                self.port_path
            ))),
        }
    }
}

impl ImuSource for UsbImuBridge {
    fn start(&mut self) -> HalResult<()> {
        self.parser.reset();
        Ok(())
    }

    fn next_sample(&mut self) -> HalResult<Option<ImuTelemetry>> {
        let Some(frame) = self.read_frame()? else {
            return Ok(None);
        };

        match frame {
            Frame::ImuSample { sample, .. } => Ok(Some(convert_sample(sample))),
        }
    }

    fn description(&self) -> &str {
        &self.description
    }
}

fn convert_sample(sample: ImuSample) -> ImuTelemetry {
    ImuTelemetry {
        timestamp_ms: (sample.timestamp_us / 1_000) as u64,
        accel_mps2: sample.accel_mg.map(|value| value as f32 * ACCEL_MG_TO_MPS2),
        gyro_rad_s: sample.gyro_mdps.map(|value| value as f32 * MDPS_TO_RAD_S),
        temperature_c: Some(sample.temperature_centi_c as f32 / 100.0),
        status_bits: Some(sample.status),
        faults: decode_status_bits(sample.status),
    }
}

fn decode_status_bits(status: u16) -> Vec<String> {
    let mut faults = Vec::new();

    if status & 0x0001 != 0 {
        faults.push("sensor_fault".to_owned());
    }
    if status & 0x0002 != 0 {
        faults.push("accel_clipped".to_owned());
    }
    if status & 0x0004 != 0 {
        faults.push("gyro_clipped".to_owned());
    }
    if status & 0x0008 != 0 {
        faults.push("mag_invalid".to_owned());
    }
    if status & 0x0010 != 0 {
        faults.push("clock_sync_lost".to_owned());
    }
    if status & 0x0020 != 0 {
        faults.push("calibrating".to_owned());
    }
    if status & 0x0040 != 0 {
        faults.push("bridge_overrun".to_owned());
    }

    faults
}
