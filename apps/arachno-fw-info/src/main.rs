use std::{fs, path::PathBuf, time::Duration};

use anyhow::{Context, bail};
use arachno_core::RobotConfig;
use arachno_imu_host::{
    CAP_ACCEL, CAP_GYRO, CAP_MAG, CAP_TEMP, DeviceInfoProbe, SensorKind, UsbImuBridge,
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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_text = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config.display()))?;
    let config: RobotConfig = toml::from_str(&config_text)
        .with_context(|| format!("failed to parse {}", args.config.display()))?;

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

    if info.sensor_kind == SensorKind::Faulted {
        bail!("firmware is running, but the IMU backend is faulted");
    }

    Ok(())
}

fn sensor_kind_label(kind: SensorKind) -> &'static str {
    match kind {
        SensorKind::Unknown => "unknown",
        SensorKind::Mock => "mock",
        SensorKind::Mpu9250 => "mpu9250",
        SensorKind::Faulted => "faulted",
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

    if labels.is_empty() {
        "none".to_owned()
    } else {
        labels.join(",")
    }
}
