use std::{fs, path::PathBuf};

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_control::SpiderController;
use arachno_core::RobotConfig;
use arachno_feetech_sts::MockStsBus;
use clap::Parser;

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
    let mut controller = SpiderController::new(config.clone(), servo_bus, camera);

    controller.initialize()?;
    let snapshot = controller.step_home_pose()?;

    println!("robot: {}", config.robot.name);
    println!("deployment_profile: {}", config.deployment.profile);
    println!("compute_target: {}", config.deployment.compute);
    println!("control_hz: {}", config.robot.control_hz);
    println!("camera_pipeline: {}", controller.camera_pipeline());
    println!("telemetry_samples: {}", snapshot.telemetry.len());
    if let Some(imu) = &config.imu {
        println!(
            "imu: enabled={} mode={} device={}",
            imu.enabled,
            imu.mode,
            imu.device.as_deref().unwrap_or("n/a")
        );
    } else {
        println!("imu: disabled");
    }
    println!("learning_mode: {}", config.learning.mode);

    Ok(())
}
