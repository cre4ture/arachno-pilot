use std::{fs, path::PathBuf};

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
    let config_text = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config.display()))?;
    let config: RobotConfig = toml::from_str(&config_text)
        .with_context(|| format!("failed to parse {}", args.config.display()))?;

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
