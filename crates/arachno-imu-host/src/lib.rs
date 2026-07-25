use std::{
    collections::VecDeque,
    f32::consts::PI,
    io::{self, Read, Write},
    time::{Duration, Instant},
};

use arachno_hal::{HalError, HalResult, ImuSource};
pub use arachno_imu_proto::{
    CAP_ACCEL, CAP_GYRO, CAP_MAG, CAP_POWER_MONITOR_REGISTERS, CAP_TEMP, CAP_USB_BOOT, DeviceInfo,
    POWER_MONITOR_REGISTER_STATUS_I2C_ERROR, POWER_MONITOR_REGISTER_STATUS_NOT_FOUND,
    POWER_MONITOR_REGISTER_STATUS_OK, PowerMonitorRegisterValue, SENSOR_FAULT_NONE,
    SENSOR_FAULT_PROBE_NO_RESPONSE, SENSOR_FAULT_READ, SENSOR_FAULT_UNEXPECTED_WHO_AM_I,
    SPI_MODE_UNKNOWN, SensorKind,
};
use arachno_imu_proto::{
    Frame, FrameParser, ImuSample, encode_power_monitor_register_request_frame,
    encode_usb_boot_request_frame,
};
use arachno_msg::ImuTelemetry;
use serialport::SerialPort;

const DEFAULT_TIMEOUT_MS: u64 = 20;
const ACCEL_MG_TO_MPS2: f32 = 9.80665 / 1000.0;
const MDPS_TO_RAD_S: f32 = PI / 180_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceInfoProbe {
    Info(DeviceInfo),
    StreamingWithoutInfo,
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMonitorRegisterRead {
    Value {
        address: u8,
        register: u8,
        value: u16,
    },
    NotFound {
        register: u8,
    },
    I2cError {
        address: u8,
        register: u8,
    },
}

pub struct UsbImuBridge {
    port_path: String,
    baud_rate: u32,
    description: String,
    port: Box<dyn SerialPort>,
    parser: FrameParser,
    pending_frames: VecDeque<Frame>,
    read_buf: [u8; 64],
    control_sequence: u8,
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
        let mut port = port;
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);

        Ok(Self {
            description: format!("RP2040 USB IMU bridge on {port_path}"),
            port_path,
            baud_rate,
            port,
            parser: FrameParser::new(),
            pending_frames: VecDeque::new(),
            read_buf: [0; 64],
            control_sequence: 0,
        })
    }

    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub fn next_frame(&mut self) -> HalResult<Option<Frame>> {
        if let Some(frame) = self.pending_frames.pop_front() {
            return Ok(Some(frame));
        }

        match self.port.read(&mut self.read_buf) {
            Ok(0) => Ok(None),
            Ok(read) => {
                for &byte in &self.read_buf[..read] {
                    match self.parser.push(byte) {
                        Ok(Some(frame)) => self.pending_frames.push_back(frame),
                        Ok(None) => {}
                        Err(_) => {
                            // Stay tolerant during bring-up and resync on the next frame.
                        }
                    }
                }
                Ok(self.pending_frames.pop_front())
            }
            Err(err) if err.kind() == io::ErrorKind::TimedOut => Ok(None),
            Err(err) => Err(HalError::Communication(format!(
                "failed reading IMU bridge {}: {err}",
                self.port_path
            ))),
        }
    }

    pub fn probe_device_info(&mut self, timeout: Duration) -> HalResult<DeviceInfoProbe> {
        let deadline = Instant::now() + timeout;
        let mut saw_sample = false;

        while Instant::now() < deadline {
            match self.next_frame()? {
                Some(Frame::DeviceInfo { info, .. }) => return Ok(DeviceInfoProbe::Info(info)),
                Some(Frame::ImuSample { .. }) => saw_sample = true,
                Some(Frame::EnterUsbBoot { .. })
                | Some(Frame::ReadPowerMonitorRegister { .. })
                | Some(Frame::PowerMonitorRegister { .. }) => {}
                None => {}
            }
        }

        if saw_sample {
            Ok(DeviceInfoProbe::StreamingWithoutInfo)
        } else {
            Ok(DeviceInfoProbe::Silent)
        }
    }

    /// Requests that compatible firmware switch to the RP2040 ROM USB bootloader.
    ///
    /// The caller should wait for the serial port to disconnect and then copy a UF2 onto the
    /// `RPI-RP2` mass-storage device which appears in its place.
    pub fn request_usb_boot(&mut self) -> HalResult<()> {
        let mut frame = [0u8; 8];
        let frame_len = encode_usb_boot_request_frame(0, &mut frame)
            .expect("the fixed USB boot control frame fits in its buffer");

        self.port.write_all(&frame[..frame_len]).map_err(|err| {
            HalError::Communication(format!(
                "failed requesting USB boot from IMU bridge {}: {err}",
                self.port_path
            ))
        })?;
        self.port.flush().map_err(|err| {
            HalError::Communication(format!(
                "failed flushing USB boot request to IMU bridge {}: {err}",
                self.port_path
            ))
        })
    }

    /// Requests one raw 16-bit power-monitor register from compatible firmware.
    pub fn read_power_monitor_register(
        &mut self,
        register: u8,
        timeout: Duration,
    ) -> HalResult<PowerMonitorRegisterRead> {
        let sequence = self.control_sequence;
        self.control_sequence = self.control_sequence.wrapping_add(1);
        let mut frame = [0u8; 9];
        let frame_len = encode_power_monitor_register_request_frame(sequence, register, &mut frame)
            .expect("the fixed power-monitor request fits in its buffer");

        self.port.write_all(&frame[..frame_len]).map_err(|err| {
            HalError::Communication(format!(
                "failed requesting power-monitor register 0x{register:02x} from {}: {err}",
                self.port_path
            ))
        })?;
        self.port.flush().map_err(|err| {
            HalError::Communication(format!(
                "failed flushing power-monitor request to {}: {err}",
                self.port_path
            ))
        })?;

        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.next_frame()? {
                Some(Frame::PowerMonitorRegister {
                    sequence: response_sequence,
                    value,
                }) if response_sequence == sequence && value.register == register => {
                    return decode_power_monitor_register(value);
                }
                Some(_) | None => {}
            }
        }

        Err(HalError::Communication(format!(
            "timed out after {} ms waiting for power-monitor register 0x{register:02x} from {}",
            timeout.as_millis(),
            self.port_path
        )))
    }
}

fn decode_power_monitor_register(
    value: PowerMonitorRegisterValue,
) -> HalResult<PowerMonitorRegisterRead> {
    match value.status {
        POWER_MONITOR_REGISTER_STATUS_OK => Ok(PowerMonitorRegisterRead::Value {
            address: value.address,
            register: value.register,
            value: value.value,
        }),
        POWER_MONITOR_REGISTER_STATUS_NOT_FOUND => Ok(PowerMonitorRegisterRead::NotFound {
            register: value.register,
        }),
        POWER_MONITOR_REGISTER_STATUS_I2C_ERROR => Ok(PowerMonitorRegisterRead::I2cError {
            address: value.address,
            register: value.register,
        }),
        status => Err(HalError::Communication(format!(
            "power-monitor register 0x{:02x} returned unknown status {status}",
            value.register
        ))),
    }
}

impl ImuSource for UsbImuBridge {
    fn start(&mut self) -> HalResult<()> {
        self.parser.reset();
        self.pending_frames.clear();
        Ok(())
    }

    fn next_sample(&mut self) -> HalResult<Option<ImuTelemetry>> {
        loop {
            let Some(frame) = self.next_frame()? else {
                return Ok(None);
            };

            match frame {
                Frame::DeviceInfo { .. } => continue,
                Frame::ImuSample { sample, .. } => return Ok(Some(convert_sample(sample))),
                Frame::EnterUsbBoot { .. }
                | Frame::ReadPowerMonitorRegister { .. }
                | Frame::PowerMonitorRegister { .. } => continue,
            }
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
