use std::{path::PathBuf, time::Duration};

use anyhow::{Context, bail};
use arachno_core::RobotConfig;
use arachno_imu_host::{
    CAP_ACCEL, CAP_GYRO, CAP_MAG, CAP_POWER_MONITOR_REGISTERS, CAP_TEMP, CAP_USB_BOOT,
    DeviceInfoProbe, PowerMonitorRegisterRead, SENSOR_FAULT_NONE, SENSOR_FAULT_PROBE_NO_RESPONSE,
    SENSOR_FAULT_READ, SENSOR_FAULT_UNEXPECTED_WHO_AM_I, SPI_MODE_UNKNOWN, SensorKind,
    UsbImuBridge,
};
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
    #[arg(long)]
    device: Option<String>,
    #[arg(long, default_value_t = 1_000)]
    timeout_ms: u64,
    /// Ask compatible firmware to restart into the RP2040 ROM USB bootloader.
    #[arg(long)]
    enter_usb_boot: bool,
    /// Read one raw 16-bit INA power-monitor register (hex, e.g. 0x01). May be repeated.
    #[arg(long = "power-register", value_parser = parse_register)]
    power_registers: Vec<u8>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;

    let device = args
        .device
        .or_else(|| config.imu.as_ref().and_then(|imu| imu.device.clone()))
        .context("no IMU device configured; pass --device or set [imu].device in config")?;

    let mut bridge = UsbImuBridge::open(&device, 115_200)
        .with_context(|| format!("failed to open IMU bridge on {device}"))?;
    let info = match bridge.probe_device_info(Duration::from_millis(args.timeout_ms))? {
        DeviceInfoProbe::Info(info) => info,
        DeviceInfoProbe::StreamingWithoutInfo => bail!(
            "IMU frames are streaming on {}, but no firmware-info frame was seen. This usually means the board is running older firmware and needs the latest UF2 reflashed.",
            device
        ),
        DeviceInfoProbe::Silent => bail!(
            "timed out after {} ms waiting for firmware info on {}",
            args.timeout_ms,
            device
        ),
    };

    println!("device: {}", device);
    println!(
        "firmware_version: {}.{}.{}",
        info.firmware_version[0], info.firmware_version[1], info.firmware_version[2]
    );
    println!("sensor_kind: {}", sensor_kind_label(info.sensor_kind));
    println!("sample_hz: {}", info.sample_hz);
    println!("capabilities: {}", capability_labels(info.capabilities));
    if info.spi_mode != SPI_MODE_UNKNOWN {
        println!("spi_mode: {}", info.spi_mode);
    }
    if info.observed_who_am_i != 0 {
        println!("observed_who_am_i: 0x{:02x}", info.observed_who_am_i);
    }
    if info.fault_code != SENSOR_FAULT_NONE {
        println!("backend_fault: {}", fault_code_label(info.fault_code));
    }

    if info.sensor_kind == SensorKind::Faulted {
        bail!(
            "firmware is running, but the IMU backend is faulted ({})",
            fault_code_label(info.fault_code)
        );
    }

    if !args.power_registers.is_empty() {
        if info.capabilities & CAP_POWER_MONITOR_REGISTERS == 0 {
            bail!(
                "firmware on {} does not support power-monitor register reads; flash the latest UF2",
                device
            );
        }

        for register in args.power_registers {
            match bridge
                .read_power_monitor_register(register, Duration::from_millis(args.timeout_ms))?
            {
                PowerMonitorRegisterRead::Value {
                    address,
                    register,
                    value,
                } => println!(
                    "power_monitor[0x{address:02X}].register[0x{register:02X}] = 0x{value:04X}"
                ),
                PowerMonitorRegisterRead::NotFound { register } => {
                    println!("power_monitor.register[0x{register:02X}] = no device")
                }
                PowerMonitorRegisterRead::I2cError { address, register } => println!(
                    "power_monitor[0x{address:02X}].register[0x{register:02X}] = I2C error"
                ),
            }
        }
    }

    if args.enter_usb_boot {
        if info.capabilities & CAP_USB_BOOT == 0 {
            bail!(
                "firmware on {} does not support USB boot requests; flash firmware 0.1.2 or later once using BOOTSEL",
                device
            );
        }

        println!("requesting RP2040 USB bootloader; the serial device will disconnect");
        bridge.request_usb_boot()?;
    }

    Ok(())
}

fn parse_register(raw: &str) -> Result<u8, String> {
    let number = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X"));
    let (radix, digits) = match number {
        Some(digits) => (16, digits),
        None => (10, raw),
    };

    u8::from_str_radix(digits, radix)
        .map_err(|_| format!("{raw:?} is not a valid 8-bit register index"))
}

fn sensor_kind_label(kind: SensorKind) -> &'static str {
    match kind {
        SensorKind::Unknown => "unknown",
        SensorKind::Mock => "mock",
        SensorKind::Mpu9250 => "mpu9250",
        SensorKind::Mpu6500 => "mpu6500-compatible",
        SensorKind::Mpu6050 => "mpu6050",
        SensorKind::Faulted => "faulted",
    }
}

fn fault_code_label(code: u8) -> &'static str {
    match code {
        SENSOR_FAULT_NONE => "none",
        SENSOR_FAULT_PROBE_NO_RESPONSE => "probe_no_response",
        SENSOR_FAULT_UNEXPECTED_WHO_AM_I => "unexpected_who_am_i",
        SENSOR_FAULT_READ => "read_fault",
        _ => "unknown",
    }
}

fn capability_labels(bits: u16) -> String {
    let mut labels = Vec::new();

    if bits & CAP_ACCEL != 0 {
        labels.push("accel");
    }
    if bits & CAP_GYRO != 0 {
        labels.push("gyro");
    }
    if bits & CAP_TEMP != 0 {
        labels.push("temp");
    }
    if bits & CAP_MAG != 0 {
        labels.push("mag");
    }
    if bits & CAP_USB_BOOT != 0 {
        labels.push("usb_boot");
    }
    if bits & CAP_POWER_MONITOR_REGISTERS != 0 {
        labels.push("power_monitor_registers");
    }

    if labels.is_empty() {
        "none".to_owned()
    } else {
        labels.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::parse_register;

    #[test]
    fn parses_decimal_and_hex_power_monitor_registers() {
        assert_eq!(parse_register("1").unwrap(), 0x01);
        assert_eq!(parse_register("0xFE").unwrap(), 0xFE);
        assert!(parse_register("0x100").is_err());
    }
}
