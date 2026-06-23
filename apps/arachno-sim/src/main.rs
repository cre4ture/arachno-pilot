use std::{
    fs::{self, File},
    io::BufWriter,
    path::PathBuf,
};

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_control::{SpiderController, TrajectoryLogWriter};
use arachno_core::{RobotConfig, SemanticPoseKind, TripodGait, now_ms};
use arachno_msg::{TrajectoryEvent, TrajectoryFrame, TrajectoryHeader};
use arachno_sim_hal::SimServoBus;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: SimCommand,
}

#[derive(Debug, Subcommand)]
enum SimCommand {
    ExportSpec {
        #[arg(long, default_value = "artifacts/sim/robot-spec.json")]
        output: PathBuf,
    },
    SilStand {
        #[arg(long, default_value = "artifacts/sim/stand-reference.jsonl")]
        trajectory_output: PathBuf,
        #[arg(long, default_value_t = 20)]
        steps: u32,
        #[arg(long, value_enum, default_value_t = SeedPose::LayDown)]
        seed_pose: SeedPose,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SeedPose {
    LayDown,
    StandReference,
    ZeroPose,
}

impl SeedPose {
    fn as_pose_kind(self) -> SemanticPoseKind {
        match self {
            Self::LayDown => SemanticPoseKind::LayDown,
            Self::StandReference => SemanticPoseKind::StandReference,
            Self::ZeroPose => SemanticPoseKind::ZeroPose,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load robot config {}", args.config.display()))?;

    match args.command {
        SimCommand::ExportSpec { output } => export_spec(&config, output),
        SimCommand::SilStand {
            trajectory_output,
            steps,
            seed_pose,
        } => run_sil_stand(&config, &args.config, trajectory_output, steps, seed_pose),
    }
}

fn export_spec(config: &RobotConfig, output: PathBuf) -> anyhow::Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file =
        File::create(&output).with_context(|| format!("failed to create {}", output.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(file), &config.simulation_spec())
        .with_context(|| format!("failed to write {}", output.display()))?;

    println!(
        "wrote simulation robot spec for {} to {}",
        config.robot.name,
        output.display()
    );
    Ok(())
}

fn run_sil_stand(
    config: &RobotConfig,
    config_path: &PathBuf,
    trajectory_output: PathBuf,
    steps: u32,
    seed_pose: SeedPose,
) -> anyhow::Result<()> {
    if let Some(parent) = trajectory_output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let trajectory_file = File::create(&trajectory_output)
        .with_context(|| format!("failed to create {}", trajectory_output.display()))?;
    let mut trajectory = TrajectoryLogWriter::new(BufWriter::new(trajectory_file));
    let gait = TripodGait;
    let commanded_pose = gait.stand_reference_commands(config);
    let header = TrajectoryHeader {
        format_version: 1,
        recorded_at_ms: now_ms(),
        robot_name: config.robot.name.clone(),
        deployment_profile: config.deployment.profile.clone(),
        control_hz: config.robot.control_hz,
        command_hz: config.locomotion.command_hz,
        config_path: Some(config_path.display().to_string()),
    };
    trajectory.write_header(&header)?;
    trajectory.write_event(&TrajectoryEvent {
        elapsed_ms: 0,
        kind: "seed_pose".to_owned(),
        message: format!("{seed_pose:?}"),
    })?;

    let servo_bus = SimServoBus::from_robot_config(config, seed_pose.as_pose_kind());
    let camera = RobotCamera::new(config.camera.clone());
    let mut controller = SpiderController::new(config.clone(), servo_bus, camera, None);
    controller
        .initialize()
        .context("failed to initialize simulated controller")?;

    let step_ms = (1000.0 / config.locomotion.command_hz.max(1) as f32).round() as u64;
    for step in 0..steps {
        let snapshot = controller
            .step_stand_reference_pose()
            .with_context(|| format!("failed during SIL step {step}"))?;
        trajectory.write_frame(&TrajectoryFrame {
            elapsed_ms: u64::from(step) * step_ms,
            snapshot,
            commands: commanded_pose.clone(),
            motion_fault: None,
        })?;
    }

    trajectory.write_event(&TrajectoryEvent {
        elapsed_ms: u64::from(steps) * step_ms,
        kind: "complete".to_owned(),
        message: format!("recorded {steps} simulated stand-reference steps"),
    })?;

    println!(
        "wrote {} simulated stand-reference steps to {}",
        steps,
        trajectory_output.display()
    );
    Ok(())
}
