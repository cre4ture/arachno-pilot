use std::{
    fs,
    path::PathBuf,
    thread,
    time::Duration,
};

use anyhow::{Context, bail};
use arachno_camera::RobotCamera;
use arachno_control::SpiderController;
use arachno_core::RobotConfig;
use arachno_feetech_sts::MockStsBus;
use arachno_hal::ImuSource;
use arachno_imu_host::{DeviceInfoProbe, SensorKind, UsbImuBridge};
use arachno_msg::{ImuTelemetry, RobotSnapshot};
use clap::Parser;

const IMU_BRIDGE_BAUD_RATE: u32 = 115_200;
const IMU_PROBE_TIMEOUT_MS: u64 = 1_000;
const SNAPSHOT_INTERVAL_MS: u64 = 500;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config_text = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config.display()))?;
    let config: RobotConfig = toml::from_str(&config_text)
        .with_context(|| format!("failed to parse {}", args.config.display()))?;

    let camera = RobotCamera::new(config.camera.clone());
    let servo_bus = MockStsBus::new(config.all_servo_ids());
    let (imu_source, imu_overview) = build_imu_source(&config)?;
    let mut controller = SpiderController::new(config.clone(), servo_bus, camera, imu_source);

    controller.initialize()?;
    let home_snapshot = controller.step_home_pose()?;

    println!("robot: {}", config.robot.name);
    println!("deployment_profile: {}", config.deployment.profile);
    println!("compute_target: {}", config.deployment.compute);
    println!("control_hz: {}", config.robot.control_hz);
    println!("camera_pipeline: {}", controller.camera_pipeline());
    println!(
        "servo_bus: mock ({})",
        MockStsBus::integration_notes()
    );
    match controller.imu_description() {
        Some(description) => println!("imu_bridge: {description}"),
        None => println!("imu_bridge: disabled"),
    }
    println!("imu_backend: {imu_overview}");
    println!("learning_mode: {}", config.learning.mode);
    println!(
        "home_snapshot: {}",
        snapshot_summary(&home_snapshot)
    );
    println!("brain_loop: live telemetry running, press Ctrl-C to stop");

    loop {
        let snapshot = controller.poll_snapshot("idle")?;
        println!("{}", snapshot_summary(&snapshot));
        thread::sleep(Duration::from_millis(SNAPSHOT_INTERVAL_MS));
    }
}

fn build_imu_source(config: &RobotConfig) -> anyhow::Result<(Option<Box<dyn ImuSource>>, String)> {
    let Some(imu) = &config.imu else {
        return Ok((None, "disabled in config".to_owned()));
    };

    if !imu.enabled {
        return Ok((None, "disabled in config".to_owned()));
    }

    if !imu.protocol.eq_ignore_ascii_case("arachno_imu_v1") {
        bail!(
            "unsupported IMU protocol {} in config; expected arachno_imu_v1",
            imu.protocol
        );
    }

    let device = imu
        .device
        .clone()
        .context("IMU is enabled, but no [imu].device is configured")?;
    let mut bridge = UsbImuBridge::open(&device, IMU_BRIDGE_BAUD_RATE)
        .with_context(|| format!("failed to open IMU bridge on {device}"))?;

    let overview = match bridge.probe_device_info(Duration::from_millis(IMU_PROBE_TIMEOUT_MS))? {
        DeviceInfoProbe::Info(info) => {
            let mut summary = format!(
                "{} at {} Hz",
                sensor_kind_label(info.sensor_kind),
                info.sample_hz
            );
            if info.spi_mode != arachno_imu_host::SPI_MODE_UNKNOWN {
                summary.push_str(&format!(", spi mode {}", info.spi_mode));
            }
            if info.observed_who_am_i != 0 {
                summary.push_str(&format!(", who_am_i=0x{:02x}", info.observed_who_am_i));
            }
            if info.fault_code != arachno_imu_host::SENSOR_FAULT_NONE {
                summary.push_str(&format!(", fault={}", imu_fault_label(info.fault_code)));
            }
            summary
        }
        DeviceInfoProbe::StreamingWithoutInfo => {
            "streaming samples, but firmware-info frame was not seen".to_owned()
        }
        DeviceInfoProbe::Silent => {
            "no device-info frame seen before timeout".to_owned()
        }
    };

    Ok((Some(Box::new(bridge)), overview))
}

fn snapshot_summary(snapshot: &RobotSnapshot) -> String {
    let camera_frame = snapshot
        .camera
        .as_ref()
        .map(|frame| frame.frame_index.to_string())
        .unwrap_or_else(|| "n/a".to_owned());
    let imu_summary = snapshot
        .imu
        .as_ref()
        .map(format_imu_sample)
        .unwrap_or_else(|| "imu=waiting".to_owned());

    format!(
        "t={}ms mode={} servos={} camera_frame={} {}",
        snapshot.timestamp_ms,
        snapshot.body_mode,
        snapshot.telemetry.len(),
        camera_frame,
        imu_summary
    )
}

fn format_imu_sample(sample: &ImuTelemetry) -> String {
    let (roll_deg, pitch_deg) = estimate_roll_pitch_deg(sample.accel_mps2);
    let accel_norm = vector_norm3(sample.accel_mps2);
    let gyro_norm_deg_s = vector_norm3(sample.gyro_rad_s) * 180.0 / std::f32::consts::PI;
    let temperature = sample
        .temperature_c
        .map(|temp| format!("{temp:.1}C"))
        .unwrap_or_else(|| "n/a".to_owned());
    let faults = if sample.faults.is_empty() {
        "ok".to_owned()
    } else {
        sample.faults.join("|")
    };

    format!(
        "imu roll={roll_deg:.1} pitch={pitch_deg:.1} accel={accel_norm:.2}m/s^2 gyro={gyro_norm_deg_s:.1}deg/s temp={} faults={}",
        temperature,
        faults
    )
}

fn estimate_roll_pitch_deg(accel_mps2: [f32; 3]) -> (f32, f32) {
    let ax = accel_mps2[0];
    let ay = accel_mps2[1];
    let az = accel_mps2[2];
    let roll = ay.atan2(az).to_degrees();
    let pitch = (-ax).atan2((ay * ay + az * az).sqrt()).to_degrees();
    (roll, pitch)
}

fn vector_norm3(values: [f32; 3]) -> f32 {
    (values[0] * values[0] + values[1] * values[1] + values[2] * values[2]).sqrt()
}

fn sensor_kind_label(kind: SensorKind) -> &'static str {
    match kind {
        SensorKind::Unknown => "unknown",
        SensorKind::Mock => "mock",
        SensorKind::Mpu9250 => "mpu9250",
        SensorKind::Mpu6500 => "mpu6500-compatible",
        SensorKind::Faulted => "faulted",
    }
}

fn imu_fault_label(code: u8) -> &'static str {
    match code {
        arachno_imu_host::SENSOR_FAULT_NONE => "none",
        arachno_imu_host::SENSOR_FAULT_PROBE_NO_RESPONSE => "probe_no_response",
        arachno_imu_host::SENSOR_FAULT_UNEXPECTED_WHO_AM_I => "unexpected_who_am_i",
        arachno_imu_host::SENSOR_FAULT_READ => "read_fault",
        _ => "unknown",
    }
}
