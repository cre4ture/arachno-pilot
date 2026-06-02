use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use arachno_core::{LegConfig, RobotConfig, TripodGait};
use arachno_feetech_sts::RealStsBus;
use arachno_hal::ServoBus;
use arachno_msg::{JointCommand, ServoTelemetry};
use clap::{Parser, ValueEnum};
use serde::Serialize;

const RANGE_TARGET_TOLERANCE_TICKS: u16 = 18;
const RANGE_MOVE_TIMEOUT_MS: u64 = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CalibrateMode {
    Plan,
    SenseRanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum JointKind {
    Coxa,
    Femur,
    Tibia,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
    #[arg(long, value_enum, default_value_t = CalibrateMode::Plan)]
    mode: CalibrateMode,
    #[arg(long, default_value = "config/robot/servo-ranges.toml")]
    output: PathBuf,
    #[arg(long, default_value_t = 10)]
    step_ticks: i16,
    #[arg(long, default_value_t = 160)]
    settle_ms: u64,
    #[arg(long, default_value_t = 18.0)]
    resistance_load_pct: f32,
    #[arg(long, default_value_t = 120)]
    resistance_current_ma: u16,
    #[arg(long, default_value_t = 2)]
    min_progress_ticks: u16,
    #[arg(long, default_value_t = 10)]
    min_error_ticks: u16,
    #[arg(long, default_value_t = 120)]
    min_travel_before_detection_ticks: u16,
    #[arg(long, default_value_t = 3)]
    confirm_resistance_samples: u8,
    #[arg(long, default_value_t = 180)]
    max_steps_per_direction: u16,
}

#[derive(Debug, Clone, Serialize)]
struct SenseParams {
    step_ticks: i16,
    settle_ms: u64,
    resistance_load_pct: f32,
    resistance_current_ma: u16,
    min_progress_ticks: u16,
    min_error_ticks: u16,
    min_travel_before_detection_ticks: u16,
    confirm_resistance_samples: u8,
    max_steps_per_direction: u16,
}

#[derive(Debug, Clone)]
struct JointProbe {
    leg_name: String,
    joint: JointKind,
    servo_id: u8,
    logical_positive_sign: i16,
}

#[derive(Debug, Clone)]
struct BoundProbeState {
    probe: JointProbe,
    last_actual_ticks: u16,
    start_actual_ticks: u16,
    baseline_load_pct: f32,
    baseline_current_ma: u16,
    consecutive_hits: u8,
}

#[derive(Debug, Clone, Serialize)]
struct ResistanceObservation {
    position_ticks: u16,
    detection: String,
    load_pct: f32,
    current_ma: Option<u16>,
    moving: bool,
    status_bits: Option<u8>,
    faults: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct JointRangeMeasurement {
    joint: String,
    servo_id: u8,
    logical_positive_label: String,
    logical_negative_end_ticks: u16,
    logical_positive_end_ticks: u16,
    midpoint_ticks: u16,
    upper_seventy_percent_ticks: u16,
    span_ticks: u16,
    positive_limit: ResistanceObservation,
    negative_limit: ResistanceObservation,
}

#[derive(Debug, Clone)]
struct ServoRangeMeasurement {
    leg_name: String,
    joint: JointKind,
    servo_id: u8,
    positive_end_ticks: u16,
    negative_end_ticks: u16,
    positive_limit: ResistanceObservation,
    negative_limit: ResistanceObservation,
}

#[derive(Debug, Clone, Serialize)]
struct LegRangeReport {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coxa: Option<JointRangeMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    femur: Option<JointRangeMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tibia: Option<JointRangeMeasurement>,
}

#[derive(Debug, Clone, Serialize)]
struct RangeScanReport {
    robot_name: String,
    deployment_profile: String,
    generated_at_ms: u64,
    output_path: String,
    notes: String,
    params: SenseParams,
    legs: Vec<LegRangeReport>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;

    match args.mode {
        CalibrateMode::Plan => print_plan(&config),
        CalibrateMode::SenseRanges => sense_ranges(&config, &args)?,
    }

    Ok(())
}

fn print_plan(config: &RobotConfig) {
    println!("calibration plan for {}", config.robot.name);
    println!("deployment profile: {}", config.deployment.profile);
    println!("servo bus: {}", config.bus.feetech.port);

    for leg in &config.legs {
        println!(
            "{:>12}: coxa={} femur={} tibia={}",
            leg.name, leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id
        );
    }
}

fn sense_ranges(config: &RobotConfig, args: &Args) -> anyhow::Result<()> {
    let params = SenseParams {
        step_ticks: args.step_ticks.abs().max(1),
        settle_ms: args.settle_ms.max(40),
        resistance_load_pct: args.resistance_load_pct.max(0.0),
        resistance_current_ma: args.resistance_current_ma,
        min_progress_ticks: args.min_progress_ticks,
        min_error_ticks: args.min_error_ticks.max(1),
        min_travel_before_detection_ticks: args.min_travel_before_detection_ticks,
        confirm_resistance_samples: args.confirm_resistance_samples.max(1),
        max_steps_per_direction: args.max_steps_per_direction.max(4),
    };
    let output_path = resolve_output_path(&args.output);

    println!("range sensing for {}", config.robot.name);
    println!("deployment profile: {}", config.deployment.profile);
    println!("servo bus: {}", config.bus.feetech.port);
    println!("output: {}", output_path.display());

    let gait = TripodGait;
    let lay_pose = gait.lay_down_pose(config);
    let servo_ids = config.all_servo_ids();
    let mut bus = RealStsBus::open(
        config.bus.feetech.port.clone(),
        config.bus.feetech.baud_rate,
        servo_ids.clone(),
    )
    .with_context(|| format!("failed to open servo bus {}", config.bus.feetech.port))?;
    bus.enable_torque(true)
        .context("failed to enable torque for range sensing")?;

    let result = (|| -> anyhow::Result<RangeScanReport> {
        move_pose_until_close(
            &mut bus,
            &lay_pose,
            params.settle_ms,
            "moving into configured lay-down pose",
        )?;
        let mut pose = read_pose(&mut bus, &servo_ids)?;

        let tibia_measurements = sense_joint_group_range(
            &mut bus,
            &mut pose,
            &joint_probes(config, JointKind::Tibia),
            &params,
            "tibia",
        )?;
        move_joint_group_to_ratio(
            &mut bus,
            &mut pose,
            &tibia_measurements,
            0.70,
            "placing tibias at 70% toward the upper end",
            params.settle_ms,
        )?;

        let femur_measurements = sense_joint_group_range(
            &mut bus,
            &mut pose,
            &joint_probes(config, JointKind::Femur),
            &params,
            "femur",
        )?;
        move_joint_group_to_ratio(
            &mut bus,
            &mut pose,
            &femur_measurements,
            0.50,
            "placing femurs at 50% of the measured range",
            params.settle_ms,
        )?;
        move_joint_group_to_ratio(
            &mut bus,
            &mut pose,
            &tibia_measurements,
            0.50,
            "placing tibias at 50% of the measured range",
            params.settle_ms,
        )?;

        let coxa_measurements = sense_joint_group_range(
            &mut bus,
            &mut pose,
            &joint_probes(config, JointKind::Coxa),
            &params,
            "coxa",
        )?;

        let report = build_range_report(
            config,
            &output_path,
            params.clone(),
            &tibia_measurements,
            &femur_measurements,
            &coxa_measurements,
        );
        write_range_report(&output_path, &report)?;
        Ok(report)
    })();

    let cleanup = move_pose_until_close(
        &mut bus,
        &lay_pose,
        params.settle_ms,
        "returning to configured lay-down pose",
    );

    if let Err(err) = cleanup {
        eprintln!("cleanup warning: {err:#}");
    }

    let report = result?;
    println!("range sensing complete; wrote {}", output_path.display());
    println!("legs measured: {}", report.legs.len());

    Ok(())
}

fn joint_probes(config: &RobotConfig, joint: JointKind) -> Vec<JointProbe> {
    config
        .legs
        .iter()
        .map(|leg| JointProbe {
            leg_name: leg.name.clone(),
            joint,
            servo_id: servo_id_for_joint(leg, joint),
            logical_positive_sign: sign_for_joint(leg, joint),
        })
        .collect()
}

fn sense_joint_group_range(
    bus: &mut RealStsBus,
    pose: &mut BTreeMap<u8, u16>,
    probes: &[JointProbe],
    params: &SenseParams,
    label: &str,
) -> anyhow::Result<Vec<ServoRangeMeasurement>> {
    println!("sensing {label} ranges in parallel");
    let positive_hits = sense_group_bound(
        bus,
        pose,
        probes,
        params,
        1,
        &format!("{label}: positive sweep"),
    )?;
    let negative_hits = sense_group_bound(
        bus,
        pose,
        probes,
        params,
        -1,
        &format!("{label}: negative sweep"),
    )?;

    let positive_map = positive_hits
        .into_iter()
        .map(|(servo_id, hit)| (servo_id, hit))
        .collect::<BTreeMap<_, _>>();
    let negative_map = negative_hits
        .into_iter()
        .map(|(servo_id, hit)| (servo_id, hit))
        .collect::<BTreeMap<_, _>>();

    let mut measurements = Vec::with_capacity(probes.len());
    for probe in probes {
        let positive_limit = positive_map
            .get(&probe.servo_id)
            .cloned()
            .with_context(|| format!("missing positive bound for servo {}", probe.servo_id))?;
        let negative_limit = negative_map
            .get(&probe.servo_id)
            .cloned()
            .with_context(|| format!("missing negative bound for servo {}", probe.servo_id))?;
        measurements.push(ServoRangeMeasurement {
            leg_name: probe.leg_name.clone(),
            joint: probe.joint,
            servo_id: probe.servo_id,
            positive_end_ticks: positive_limit.position_ticks,
            negative_end_ticks: negative_limit.position_ticks,
            positive_limit,
            negative_limit,
        });
    }

    Ok(measurements)
}

fn sense_group_bound(
    bus: &mut RealStsBus,
    pose: &mut BTreeMap<u8, u16>,
    probes: &[JointProbe],
    params: &SenseParams,
    logical_direction: i16,
    phase_label: &str,
) -> anyhow::Result<Vec<(u8, ResistanceObservation)>> {
    let initial_feedback = read_feedback_map(
        bus,
        &probes
            .iter()
            .map(|probe| probe.servo_id)
            .collect::<Vec<_>>(),
    )?;
    let mut states = probes
        .iter()
        .map(|probe| {
            let feedback = initial_feedback
                .get(&probe.servo_id)
                .cloned()
                .unwrap_or_default();
            (
                probe.servo_id,
                BoundProbeState {
                    probe: probe.clone(),
                    last_actual_ticks: feedback.present_position_ticks,
                    start_actual_ticks: feedback.present_position_ticks,
                    baseline_load_pct: feedback.present_load_pct,
                    baseline_current_ma: feedback.present_current_ma.unwrap_or(0),
                    consecutive_hits: 0,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut finished = Vec::new();

    for step in 0..params.max_steps_per_direction {
        if states.is_empty() {
            break;
        }

        for state in states.values() {
            let raw_direction = logical_direction * state.probe.logical_positive_sign;
            let target_ticks =
                offset_ticks(state.last_actual_ticks, raw_direction * params.step_ticks);
            pose.insert(state.probe.servo_id, target_ticks);
        }

        sync_full_pose(bus, pose)?;
        thread::sleep(Duration::from_millis(params.settle_ms));

        let feedback = read_feedback_map(bus, &states.keys().copied().collect::<Vec<_>>())?;

        let mut newly_finished = Vec::new();
        for (&servo_id, state) in &mut states {
            let telemetry = feedback.get(&servo_id).with_context(|| {
                format!("missing feedback for servo {servo_id} during {phase_label}")
            })?;
            let target_ticks = pose
                .get(&servo_id)
                .copied()
                .unwrap_or(telemetry.present_position_ticks);
            let raw_direction = logical_direction * state.probe.logical_positive_sign;
            let progress_ticks = signed_distance(
                state.last_actual_ticks,
                telemetry.present_position_ticks,
                raw_direction,
            );
            let error_ticks = signed_distance(
                telemetry.present_position_ticks,
                target_ticks,
                raw_direction,
            );
            let resistance =
                resistance_detected(telemetry, state, progress_ticks, error_ticks, params);

            if resistance {
                state.consecutive_hits = state.consecutive_hits.saturating_add(1);
            } else {
                state.consecutive_hits = 0;
            }

            state.last_actual_ticks = telemetry.present_position_ticks;

            if state.consecutive_hits >= params.confirm_resistance_samples {
                pose.insert(servo_id, telemetry.present_position_ticks);
                newly_finished.push((
                    servo_id,
                    ResistanceObservation {
                        position_ticks: telemetry.present_position_ticks,
                        detection: "resistance".to_owned(),
                        load_pct: telemetry.present_load_pct,
                        current_ma: telemetry.present_current_ma,
                        moving: telemetry.moving,
                        status_bits: telemetry.status_bits,
                        faults: telemetry.faults.clone(),
                    },
                ));
            }
        }

        for (servo_id, observation) in newly_finished {
            states.remove(&servo_id);
            finished.push((servo_id, observation));
        }

        println!(
            "{phase_label}: step {}/{} active={} finished={}",
            step + 1,
            params.max_steps_per_direction,
            states.len(),
            finished.len()
        );
    }

    if !states.is_empty() {
        println!(
            "{phase_label}: no clear resistance on {} joint(s); recording final swept positions as max_sweep bounds",
            states.len()
        );
        let trailing_feedback =
            read_feedback_map(bus, &states.keys().copied().collect::<Vec<_>>())?;
        for (&servo_id, state) in &states {
            let telemetry = trailing_feedback.get(&servo_id).with_context(|| {
                format!("missing trailing feedback for servo {servo_id} during {phase_label}")
            })?;
            finished.push((
                servo_id,
                ResistanceObservation {
                    position_ticks: telemetry.present_position_ticks,
                    detection: format!(
                        "max_sweep:{}:{}",
                        state.probe.leg_name,
                        joint_label(state.probe.joint)
                    ),
                    load_pct: telemetry.present_load_pct,
                    current_ma: telemetry.present_current_ma,
                    moving: telemetry.moving,
                    status_bits: telemetry.status_bits,
                    faults: telemetry.faults.clone(),
                },
            ));
            pose.insert(servo_id, telemetry.present_position_ticks);
        }
    }

    sync_full_pose(bus, pose)?;
    thread::sleep(Duration::from_millis(params.settle_ms));
    Ok(finished)
}

fn resistance_detected(
    telemetry: &ServoTelemetry,
    state: &BoundProbeState,
    progress_ticks: i32,
    error_ticks: i32,
    params: &SenseParams,
) -> bool {
    let stalled = progress_ticks <= i32::from(params.min_progress_ticks)
        && error_ticks >= i32::from(params.min_error_ticks);
    let traveled_ticks = signed_distance(
        state.start_actual_ticks,
        telemetry.present_position_ticks,
        state.probe.logical_positive_sign,
    )
    .unsigned_abs() as u16;
    let load_triggered =
        telemetry.present_load_pct >= state.baseline_load_pct + params.resistance_load_pct;
    let current_triggered = telemetry.present_current_ma.is_some_and(|current| {
        current
            >= state
                .baseline_current_ma
                .saturating_add(params.resistance_current_ma)
    });
    let servo_reported_load = telemetry.faults.iter().any(|fault| fault == "load");
    let servo_stopped = !telemetry.moving || stalled;

    traveled_ticks >= min_travel_before_detection_ticks(state.probe.joint, params)
        && (load_triggered || current_triggered || servo_reported_load)
        && servo_stopped
}

fn move_joint_group_to_ratio(
    bus: &mut RealStsBus,
    pose: &mut BTreeMap<u8, u16>,
    measurements: &[ServoRangeMeasurement],
    ratio: f32,
    label: &str,
    settle_ms: u64,
) -> anyhow::Result<()> {
    println!("{label}");
    for measurement in measurements {
        pose.insert(
            measurement.servo_id,
            interpolate_ticks(
                measurement.negative_end_ticks,
                measurement.positive_end_ticks,
                ratio,
            ),
        );
    }
    move_pose_until_close(bus, pose, settle_ms, label)
}

fn move_pose_until_close(
    bus: &mut RealStsBus,
    target_pose: &BTreeMap<u8, u16>,
    settle_ms: u64,
    label: &str,
) -> anyhow::Result<()> {
    sync_full_pose(bus, target_pose)?;
    let started = SystemTime::now();

    loop {
        thread::sleep(Duration::from_millis(settle_ms));
        let feedback = read_feedback_map(bus, &target_pose.keys().copied().collect::<Vec<_>>())?;
        let mut all_close = true;

        for (&servo_id, &target_ticks) in target_pose {
            let actual_ticks = feedback
                .get(&servo_id)
                .map(|telemetry| telemetry.present_position_ticks)
                .unwrap_or(target_ticks);
            if actual_ticks.abs_diff(target_ticks) > RANGE_TARGET_TOLERANCE_TICKS {
                all_close = false;
            }
        }

        if all_close {
            return Ok(());
        }

        if started.elapsed().unwrap_or_default().as_millis() > u128::from(RANGE_MOVE_TIMEOUT_MS) {
            bail!("{label} did not settle within the timeout");
        }
    }
}

fn build_range_report(
    config: &RobotConfig,
    output_path: &Path,
    params: SenseParams,
    tibias: &[ServoRangeMeasurement],
    femurs: &[ServoRangeMeasurement],
    coxae: &[ServoRangeMeasurement],
) -> RangeScanReport {
    let tibia_map = tibias
        .iter()
        .map(|measurement| (measurement.leg_name.clone(), measurement))
        .collect::<BTreeMap<_, _>>();
    let femur_map = femurs
        .iter()
        .map(|measurement| (measurement.leg_name.clone(), measurement))
        .collect::<BTreeMap<_, _>>();
    let coxa_map = coxae
        .iter()
        .map(|measurement| (measurement.leg_name.clone(), measurement))
        .collect::<BTreeMap<_, _>>();

    let legs = config
        .legs
        .iter()
        .map(|leg| LegRangeReport {
            name: leg.name.clone(),
            coxa: coxa_map
                .get(&leg.name)
                .map(|measurement| measurement_report(leg, measurement)),
            femur: femur_map
                .get(&leg.name)
                .map(|measurement| measurement_report(leg, measurement)),
            tibia: tibia_map
                .get(&leg.name)
                .map(|measurement| measurement_report(leg, measurement)),
        })
        .collect();

    RangeScanReport {
        robot_name: config.robot.name.clone(),
        deployment_profile: config.deployment.profile.clone(),
        generated_at_ms: now_ms(),
        output_path: output_path.display().to_string(),
        notes: "Ranges were measured in the configured lay-down posture using slow resistance probing. Positive direction is lift for femur/tibia and forward for coxa.".to_owned(),
        params,
        legs,
    }
}

fn measurement_report(
    leg: &LegConfig,
    measurement: &ServoRangeMeasurement,
) -> JointRangeMeasurement {
    let (joint_name, positive_label) = match measurement.joint {
        JointKind::Coxa => ("coxa", "forward"),
        JointKind::Femur => ("femur", "up"),
        JointKind::Tibia => ("tibia", "up"),
    };
    let midpoint_ticks = interpolate_ticks(
        measurement.negative_end_ticks,
        measurement.positive_end_ticks,
        0.50,
    );
    let upper_seventy_percent_ticks = interpolate_ticks(
        measurement.negative_end_ticks,
        measurement.positive_end_ticks,
        0.70,
    );

    JointRangeMeasurement {
        joint: joint_name.to_owned(),
        servo_id: servo_id_for_joint(leg, measurement.joint),
        logical_positive_label: positive_label.to_owned(),
        logical_negative_end_ticks: measurement.negative_end_ticks,
        logical_positive_end_ticks: measurement.positive_end_ticks,
        midpoint_ticks,
        upper_seventy_percent_ticks,
        span_ticks: measurement
            .positive_end_ticks
            .abs_diff(measurement.negative_end_ticks),
        positive_limit: measurement.positive_limit.clone(),
        negative_limit: measurement.negative_limit.clone(),
    }
}

fn write_range_report(path: &Path, report: &RangeScanReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(report).context("failed to serialize range report")?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

fn read_pose(bus: &mut RealStsBus, servo_ids: &[u8]) -> anyhow::Result<BTreeMap<u8, u16>> {
    let feedback = read_feedback_map(bus, servo_ids)?;
    let mut pose = BTreeMap::new();
    for servo_id in servo_ids {
        let telemetry = feedback
            .get(servo_id)
            .with_context(|| format!("missing telemetry for servo {}", servo_id))?;
        pose.insert(*servo_id, telemetry.present_position_ticks);
    }
    Ok(pose)
}

fn read_feedback_map(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
) -> anyhow::Result<BTreeMap<u8, ServoTelemetry>> {
    let mut feedback = BTreeMap::new();
    for servo_id in servo_ids {
        let telemetry = bus
            .read_feedback(*servo_id)
            .with_context(|| format!("failed to read feedback from servo {}", servo_id))?;
        feedback.insert(*servo_id, telemetry);
    }
    Ok(feedback)
}

fn sync_full_pose(bus: &mut RealStsBus, pose: &BTreeMap<u8, u16>) -> anyhow::Result<()> {
    let commands = pose
        .iter()
        .map(|(&servo_id, &position_ticks)| JointCommand {
            servo_id,
            position_ticks,
            speed_ticks: 0,
            acceleration: 0,
        })
        .collect::<Vec<_>>();
    bus.sync_write_positions(&commands)
        .context("failed to sync-write servo pose")
}

fn servo_id_for_joint(leg: &LegConfig, joint: JointKind) -> u8 {
    match joint {
        JointKind::Coxa => leg.coxa_servo_id,
        JointKind::Femur => leg.femur_servo_id,
        JointKind::Tibia => leg.tibia_servo_id,
    }
}

fn sign_for_joint(leg: &LegConfig, joint: JointKind) -> i16 {
    match joint {
        JointKind::Coxa => leg.coxa_forward_sign(),
        JointKind::Femur => leg.femur_lift_sign(),
        JointKind::Tibia => leg.tibia_lift_sign(),
    }
}

fn signed_distance(from_ticks: u16, to_ticks: u16, raw_direction: i16) -> i32 {
    (i32::from(to_ticks) - i32::from(from_ticks)) * i32::from(raw_direction.signum())
}

fn offset_ticks(start_ticks: u16, delta_ticks: i16) -> u16 {
    (i32::from(start_ticks) + i32::from(delta_ticks)).clamp(0, 4095) as u16
}

fn interpolate_ticks(start_ticks: u16, end_ticks: u16, ratio: f32) -> u16 {
    let ratio = ratio.clamp(0.0, 1.0);
    let interpolated = start_ticks as f32 + (end_ticks as f32 - start_ticks as f32) * ratio;
    interpolated.round().clamp(0.0, 4095.0) as u16
}

fn min_travel_before_detection_ticks(joint: JointKind, params: &SenseParams) -> u16 {
    match joint {
        JointKind::Tibia => params.min_travel_before_detection_ticks.max(360),
        JointKind::Femur => params.min_travel_before_detection_ticks.max(160),
        JointKind::Coxa => params.min_travel_before_detection_ticks,
    }
}

fn joint_label(joint: JointKind) -> &'static str {
    match joint {
        JointKind::Coxa => "coxa",
        JointKind::Femur => "femur",
        JointKind::Tibia => "tibia",
    }
}

fn resolve_output_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
