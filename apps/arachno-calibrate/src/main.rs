use std::path::PathBuf;

use anyhow::Context;
use arachno_core::RobotConfig;
use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;

    println!("calibration plan for {}", config.robot.name);
    println!("deployment profile: {}", config.deployment.profile);
    println!("servo bus: {}", config.bus.feetech.port);

    for leg in &config.legs {
        println!(
            "{:>12}: coxa={} femur={} tibia={}",
            leg.name, leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id
        );
    }

    Ok(())
}
