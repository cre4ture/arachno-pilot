use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{Context, anyhow, bail};
use arachno_core::{LegConfig, RobotConfig, ServoEepromEntry, TripodGait, now_ms};
use arachno_feetech_sts::{
    LOCK_MARK, RealStsBus, WriteConfirmationMode,
    set_verified_torque_limit_on_current_position_for_ids as set_verified_bus_torque_limit_on_current_position_for_ids,
    validate_servo_eeprom_entry as validate_bus_servo_eeprom_entry,
    validate_servo_eeprom_entry_value as validate_bus_servo_eeprom_entry_value,
    validate_servo_eeprom_profile as validate_bus_servo_eeprom_profile,
};
use arachno_hal::{ServoBus, enable_torque_on_current_position};
use arachno_msg::{JointCommand, ServoTelemetry};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::{info, info_span, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
    registry,
};

const RANGE_TARGET_TOLERANCE_TICKS: u16 = 18;
const MOTION_START_TICKS: u16 = 24;
const FEEDBACK_READ_RETRIES: usize = 3;
const FEEDBACK_RETRY_SLEEP_MS: u64 = 15;
const EEPROM_WRITE_SETTLE_MS: u64 = 80;
const EEPROM_LOCK_SETTLE_MS: u64 = 50;
const POSITION_SETTLE_CONFIRM_CYCLES: u8 = 2;
const PHASE_SETTLE_SLEEP_MS: u64 = 250;
const POSE_EDGE_WARNING_MIN_TICKS: u16 = 20;
const COXA_SUSPICIOUS_SPAN_TICKS: u16 = 20;
const FEMUR_STAND_REFERENCE_RATIO: f32 = 0.40;
const TIBIA_STAND_REFERENCE_RATIO: f32 = 0.60;
const LAY_DOWN_INSET_RATIO: f32 = 0.08;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CalibrateMode {
    Plan,
    ApplyEeprom,
    VerifyEeprom,
    SenseRanges,
    CheckPoses,
    SuggestPoses,
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
    #[arg(long, default_value = "config/robot/servo-ranges.toml")]
    ranges: PathBuf,
    #[arg(long)]
    suggestions_output: Option<PathBuf>,
    #[arg(long, default_value_t = 100)]
    probe_torque_limit: u16,
    #[arg(long, default_value_t = 1000)]
    restore_torque_limit: u16,
    #[arg(long, default_value_t = 120)]
    poll_ms: u64,
    #[arg(long, default_value_t = 2)]
    stop_speed_ticks: u16,
    #[arg(long, default_value_t = 3)]
    confirm_stopped_samples: u8,
    #[arg(long, default_value_t = 15000)]
    move_timeout_ms: u64,
    #[arg(long, default_value_t = false)]
    skip_initial_lay_down: bool,
    #[arg(long)]
    trace_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SenseParams {
    probe_torque_limit: u16,
    restore_torque_limit: u16,
    poll_ms: u64,
    stop_speed_ticks: u16,
    confirm_stopped_samples: u8,
    move_timeout_ms: u64,
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
    start_position_ticks: u16,
    has_started_motion: bool,
    consecutive_stopped: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResistanceObservation {
    position_ticks: u16,
    detection: String,
    load_pct: f32,
    current_ma: Option<u16>,
    moving: bool,
    status_bits: Option<u8>,
    faults: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegRangeReport {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coxa: Option<JointRangeMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    femur: Option<JointRangeMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tibia: Option<JointRangeMeasurement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RangeScanReport {
    robot_name: String,
    deployment_profile: String,
    generated_at_ms: u64,
    output_path: String,
    notes: String,
    params: SenseParams,
    legs: Vec<LegRangeReport>,
}

#[derive(Debug, Clone, Serialize)]
struct SuggestedPoseReport {
    robot_name: String,
    deployment_profile: String,
    generated_at_ms: u64,
    ranges_path: String,
    notes: String,
    legs: Vec<SuggestedLegPose>,
}

#[derive(Debug, Clone, Serialize)]
struct SuggestedLegPose {
    name: String,
    coxa_stand_reference_ticks: u16,
    femur_stand_reference_ticks: u16,
    tibia_stand_reference_ticks: u16,
    coxa_lay_down_ticks: u16,
    femur_lay_down_ticks: u16,
    tibia_lay_down_ticks: u16,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;

    match args.mode {
        CalibrateMode::Plan => print_plan(&config),
        CalibrateMode::ApplyEeprom => apply_eeprom(&config)?,
        CalibrateMode::VerifyEeprom => verify_eeprom(&config)?,
        CalibrateMode::SenseRanges => sense_ranges(&config, &args)?,
        CalibrateMode::CheckPoses => check_poses(&config, &args)?,
        CalibrateMode::SuggestPoses => suggest_poses(&config, &args)?,
    }

    Ok(())
}

fn print_plan(config: &RobotConfig) {
    println!("calibration plan for {}", config.robot.name);
    println!("deployment profile: {}", config.deployment.profile);
    println!("servo bus: {}", config.bus.feetech.port);
    println!(
        "eeprom profile entries: {}",
        config.servo_eeprom.entries.len()
    );

    for leg in &config.legs {
        println!(
            "{:>12}: coxa={} femur={} tibia={}",
            leg.name, leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id
        );
    }
}

fn apply_eeprom(config: &RobotConfig) -> anyhow::Result<()> {
    if config.servo_eeprom.entries.is_empty() {
        bail!("config does not define any servo EEPROM entries");
    }

    let servo_ids = config.all_servo_ids();
    let mut bus = RealStsBus::open(
        config.bus.feetech.port.clone(),
        config.bus.feetech.baud_rate,
        servo_ids.clone(),
    )
    .with_context(|| format!("failed to open servo bus {}", config.bus.feetech.port))?;

    println!(
        "applying EEPROM profile with {} entries to {} servos via {}",
        config.servo_eeprom.entries.len(),
        servo_ids.len(),
        config.bus.feetech.port
    );
    bus.set_write_confirmation_mode(WriteConfirmationMode::Optional);
    info!(
        servo_count = servo_ids.len(),
        entry_count = config.servo_eeprom.entries.len(),
        "applying persistent servo EEPROM profile"
    );

    println!(
        "unlocking EEPROM writes via {} (0x{:X})",
        LOCK_MARK.name, LOCK_MARK.address
    );
    for &servo_id in &servo_ids {
        bus.set_eeprom_write_lock(servo_id, false)
            .with_context(|| format!("failed to unlock EEPROM writes on servo {}", servo_id))?;
    }
    thread::sleep(Duration::from_millis(EEPROM_LOCK_SETTLE_MS));

    let apply_result = (|| -> anyhow::Result<()> {
        for entry in &config.servo_eeprom.entries {
            println!(
                "writing EEPROM {} at address {} as {:?} = {}",
                entry.name, entry.address, entry.width, entry.value
            );
            info!(
                name = %entry.name,
                address = entry.address,
                width = ?entry.width,
                value = entry.value,
                "applying EEPROM entry"
            );
            for &servo_id in &servo_ids {
                write_and_verify_eeprom_entry(&mut bus, servo_id, entry).with_context(|| {
                    format!(
                        "failed to apply EEPROM entry {} to servo {}",
                        entry.name, servo_id
                    )
                })?;
            }
        }
        Ok(())
    })();

    println!(
        "relocking EEPROM writes via {} (0x{:X})",
        LOCK_MARK.name, LOCK_MARK.address
    );
    let relock_result = (|| -> anyhow::Result<()> {
        for &servo_id in &servo_ids {
            bus.set_eeprom_write_lock(servo_id, true).with_context(|| {
                format!("failed to restore EEPROM write lock on servo {}", servo_id)
            })?;
        }
        thread::sleep(Duration::from_millis(EEPROM_LOCK_SETTLE_MS));
        Ok(())
    })();

    if let Err(relock_err) = relock_result {
        return match apply_result {
            Ok(()) => Err(relock_err),
            Err(apply_err) => Err(apply_err.context(format!(
                "also failed to restore EEPROM write lock: {relock_err:#}"
            ))),
        };
    }

    apply_result?;

    println!("EEPROM profile applied successfully");
    info!("persistent servo EEPROM profile applied successfully");
    Ok(())
}

fn verify_eeprom(config: &RobotConfig) -> anyhow::Result<()> {
    if config.servo_eeprom.entries.is_empty() {
        bail!("config does not define any servo EEPROM entries");
    }

    let servo_ids = config.all_servo_ids();
    let mut bus = RealStsBus::open(
        config.bus.feetech.port.clone(),
        config.bus.feetech.baud_rate,
        servo_ids.clone(),
    )
    .with_context(|| format!("failed to open servo bus {}", config.bus.feetech.port))?;

    println!(
        "verifying EEPROM profile with {} entries on {} servos via {}",
        config.servo_eeprom.entries.len(),
        servo_ids.len(),
        config.bus.feetech.port
    );
    for entry in &config.servo_eeprom.entries {
        println!(
            "verifying EEPROM {} at address {} as {:?} = {}",
            entry.name, entry.address, entry.width, entry.value
        );
        for &servo_id in &servo_ids {
            let observed = validate_bus_servo_eeprom_entry_value(&mut bus, servo_id, entry)
                .with_context(|| {
                    format!(
                        "failed EEPROM validation for entry {} on servo {}",
                        entry.name, servo_id
                    )
                })?;
            println!(
                "  servo {}: verified {} (0x{:X}) = {}",
                servo_id, entry.name, entry.address, observed
            );
        }
    }

    println!("EEPROM profile verification passed");

    Ok(())
}

fn write_and_verify_eeprom_entry(
    bus: &mut RealStsBus,
    servo_id: u8,
    entry: &ServoEepromEntry,
) -> anyhow::Result<()> {
    match entry.width {
        arachno_core::ServoRegisterWidth::U8 => {
            let value = u8::try_from(entry.value).with_context(|| {
                format!(
                    "EEPROM entry {} value {} does not fit into u8",
                    entry.name, entry.value
                )
            })?;
            bus.write_persistent_register_u8(servo_id, entry.address, value)?;
        }
        arachno_core::ServoRegisterWidth::U16 => {
            bus.write_persistent_register_u16(servo_id, entry.address, entry.value)?;
        }
    }
    thread::sleep(Duration::from_millis(EEPROM_WRITE_SETTLE_MS));
    validate_bus_servo_eeprom_entry(bus, servo_id, entry).with_context(|| {
        format!(
            "failed to verify EEPROM entry {} on servo {}",
            entry.name, servo_id
        )
    })?;

    Ok(())
}

fn validate_servo_eeprom_profile(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
    entries: &[ServoEepromEntry],
) -> anyhow::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    println!(
        "validating EEPROM profile with {} entries on {} servos",
        entries.len(),
        servo_ids.len()
    );
    info!(
        servo_count = servo_ids.len(),
        entry_count = entries.len(),
        "validating persistent servo EEPROM profile"
    );

    validate_bus_servo_eeprom_profile(bus, servo_ids, entries)
        .context("persistent servo EEPROM profile validation failed")?;

    println!("EEPROM profile validation passed");
    info!("persistent servo EEPROM profile validation passed");
    Ok(())
}

fn check_poses(config: &RobotConfig, args: &Args) -> anyhow::Result<()> {
    let ranges_path = resolve_output_path(&args.ranges);
    let report = load_range_report(&ranges_path)?;
    let gait = TripodGait;
    let stand_reference_pose = gait.stand_reference_pose(config);
    let lay_down_pose = gait.lay_down_pose(config);

    println!("pose check for {}", config.robot.name);
    println!("deployment profile: {}", config.deployment.profile);
    println!("ranges: {}", ranges_path.display());

    let measured_legs = report
        .legs
        .iter()
        .map(|leg| (leg.name.as_str(), leg))
        .collect::<BTreeMap<_, _>>();

    let mut warnings = 0usize;

    for leg in &config.legs {
        println!("{}", leg.name);
        let measured = measured_legs
            .get(leg.name.as_str())
            .with_context(|| format!("missing measured ranges for leg {}", leg.name))?;

        warnings += print_joint_check(
            "coxa",
            *stand_reference_pose
                .get(&leg.coxa_servo_id)
                .with_context(|| format!("missing coxa stand pose for leg {}", leg.name))?,
            *lay_down_pose
                .get(&leg.coxa_servo_id)
                .with_context(|| format!("missing coxa lay pose for leg {}", leg.name))?,
            measured
                .coxa
                .as_ref()
                .with_context(|| format!("missing coxa range for leg {}", leg.name))?,
        );
        warnings += print_joint_check(
            "femur",
            *stand_reference_pose
                .get(&leg.femur_servo_id)
                .with_context(|| format!("missing femur stand pose for leg {}", leg.name))?,
            *lay_down_pose
                .get(&leg.femur_servo_id)
                .with_context(|| format!("missing femur lay pose for leg {}", leg.name))?,
            measured
                .femur
                .as_ref()
                .with_context(|| format!("missing femur range for leg {}", leg.name))?,
        );
        warnings += print_joint_check(
            "tibia",
            *stand_reference_pose
                .get(&leg.tibia_servo_id)
                .with_context(|| format!("missing tibia stand pose for leg {}", leg.name))?,
            *lay_down_pose
                .get(&leg.tibia_servo_id)
                .with_context(|| format!("missing tibia lay pose for leg {}", leg.name))?,
            measured
                .tibia
                .as_ref()
                .with_context(|| format!("missing tibia range for leg {}", leg.name))?,
        );
    }

    if warnings == 0 {
        println!("pose check: all configured poses are within measured bounds");
    } else {
        println!("pose check: {warnings} warning(s) detected");
    }

    Ok(())
}

fn suggest_poses(config: &RobotConfig, args: &Args) -> anyhow::Result<()> {
    let ranges_path = resolve_output_path(&args.ranges);
    let report = load_range_report(&ranges_path)?;
    let suggestions = build_suggested_pose_report(config, &ranges_path, &report)?;

    println!("suggested poses for {}", config.robot.name);
    println!("ranges: {}", ranges_path.display());
    for leg in &suggestions.legs {
        println!(
            "{:>12}: stand_reference[c={}, f={}, t={}] lay[c={}, f={}, t={}]",
            leg.name,
            leg.coxa_stand_reference_ticks,
            leg.femur_stand_reference_ticks,
            leg.tibia_stand_reference_ticks,
            leg.coxa_lay_down_ticks,
            leg.femur_lay_down_ticks,
            leg.tibia_lay_down_ticks
        );
    }

    if let Some(path) = &args.suggestions_output {
        let output_path = resolve_output_path(path);
        write_suggested_pose_report(&output_path, &suggestions)?;
        println!("wrote suggestions to {}", output_path.display());
    }

    Ok(())
}

fn sense_ranges(config: &RobotConfig, args: &Args) -> anyhow::Result<()> {
    let params = SenseParams {
        probe_torque_limit: args.probe_torque_limit.max(1),
        restore_torque_limit: args
            .restore_torque_limit
            .max(args.probe_torque_limit.max(1)),
        poll_ms: args.poll_ms.max(40),
        stop_speed_ticks: args.stop_speed_ticks,
        confirm_stopped_samples: args.confirm_stopped_samples.max(1),
        move_timeout_ms: args.move_timeout_ms.max(1_000),
    };
    let output_path = resolve_output_path(&args.output);
    let trace_path = resolve_trace_output_path(args, &output_path);
    let _trace_guard = init_trace_logging(&trace_path)?;

    info!(robot = %config.robot.name, "range sensing");
    info!(deployment_profile = %config.deployment.profile, "loaded deployment profile");
    info!(servo_bus = %config.bus.feetech.port, "servo bus");
    info!(output = %output_path.display(), "range report path");
    info!(trace_output = %trace_path.display(), "trace log path");

    let gait = TripodGait;
    let lay_pose = gait.lay_down_pose(config);
    let servo_ids = config.all_servo_ids();
    let mut bus = RealStsBus::open(
        config.bus.feetech.port.clone(),
        config.bus.feetech.baud_rate,
        servo_ids.clone(),
    )
    .with_context(|| format!("failed to open servo bus {}", config.bus.feetech.port))?;

    validate_servo_eeprom_profile(&mut bus, &servo_ids, &config.servo_eeprom.entries)
        .context("failed EEPROM validation before range sensing")?;

    bus.enable_torque(false)
        .context("failed to disable torque for range sensing")?;

    // sleep a moment to ensure all servos have processed the torque disable command and are ready to accept new position commands without resistance
    thread::sleep(Duration::from_millis(500));

    enable_torque_on_current_position(&mut bus)
        .context("failed to enable torque for range sensing")?;

    set_verified_torque_limit_on_current_position_for_ids(
        &mut bus,
        &servo_ids,
        params.restore_torque_limit,
        "restoring default torque limits before sensing",
    )?;

    let result = (|| -> anyhow::Result<RangeScanReport> {
        if args.skip_initial_lay_down {
            info!("skipping initial lay-down move and starting from the current pose");
        } else {
            move_pose_until_close(
                &mut bus,
                &lay_pose,
                &servo_ids,
                params.poll_ms,
                params.move_timeout_ms,
                "moving into configured lay-down pose",
            )?;
        }
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
            params.poll_ms,
            params.move_timeout_ms,
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
            params.poll_ms,
            params.move_timeout_ms,
        )?;
        move_joint_group_to_ratio(
            &mut bus,
            &mut pose,
            &tibia_measurements,
            0.50,
            "placing tibias at 50% of the measured range",
            params.poll_ms,
            params.move_timeout_ms,
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

    let restore = set_verified_torque_limit_on_current_position_for_ids(
        &mut bus,
        &servo_ids,
        params.restore_torque_limit,
        "restoring default torque limits after sensing",
    );
    let cleanup = move_pose_until_close(
        &mut bus,
        &lay_pose,
        &servo_ids,
        params.poll_ms,
        params.move_timeout_ms,
        "returning to configured lay-down pose",
    );

    if let Err(err) = restore {
        warn!("torque-limit restore warning: {err:#}");
    }
    if let Err(err) = cleanup {
        warn!("cleanup warning: {err:#}");
    }

    let report = result?;
    info!(output = %output_path.display(), "range sensing complete");
    info!(legs_measured = report.legs.len(), "range sensing summary");

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
    let _span = info_span!(
        "sense_joint_group",
        joint = label,
        servo_count = probes.len()
    )
    .entered();
    info!("sensing ranges in parallel");
    let active_ids = probes
        .iter()
        .map(|probe| probe.servo_id)
        .collect::<Vec<_>>();
    set_verified_torque_limit_on_current_position_for_ids(
        bus,
        &active_ids,
        params.probe_torque_limit,
        &format!("lowering torque limits for {label} sensing"),
    )?;

    let result = (|| -> anyhow::Result<_> {
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
        Ok((positive_hits, negative_hits))
    })();

    bus.enable_torque_on_ids(&active_ids, false)
        .with_context(|| format!("failed to disable torque after {label} sensing"))?;

    // sleep a moment to ensure all servos have processed the torque disable command and are ready to accept new position commands without resistance
    thread::sleep(Duration::from_millis(500));

    set_verified_torque_limit_on_current_position_for_ids(
        bus,
        &active_ids,
        params.restore_torque_limit,
        &format!("restoring torque limits after {label} sensing"),
    )?;

    bus.enable_torque_on_ids(&active_ids, true)
        .with_context(|| format!("failed to disable torque after {label} sensing"))?;

    let (positive_hits, negative_hits) = result?;

    let positive_map = positive_hits.into_iter().collect::<BTreeMap<_, _>>();
    let negative_map = negative_hits.into_iter().collect::<BTreeMap<_, _>>();

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

fn set_verified_torque_limit_on_current_position_for_ids(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
    torque_limit: u16,
    label: &str,
) -> anyhow::Result<()> {
    info!(
        servo_count = servo_ids.len(),
        torque_limit,
        "{label}: verifying torque limit {} on {} servo(s)",
        torque_limit,
        servo_ids.len()
    );
    set_verified_bus_torque_limit_on_current_position_for_ids(bus, servo_ids, torque_limit)
        .with_context(|| label.to_owned())
}

fn sense_group_bound(
    bus: &mut RealStsBus,
    pose: &mut BTreeMap<u8, u16>,
    probes: &[JointProbe],
    params: &SenseParams,
    logical_direction: i16,
    phase_label: &str,
) -> anyhow::Result<Vec<(u8, ResistanceObservation)>> {
    let _span = info_span!(
        "sense_group_bound",
        phase = phase_label,
        logical_direction,
        servo_count = probes.len()
    )
    .entered();
    let active_ids = probes
        .iter()
        .map(|probe| probe.servo_id)
        .collect::<Vec<_>>();
    let mut states = BTreeMap::new();
    for probe in probes {
        let start_position_ticks = *pose
            .get(&probe.servo_id)
            .with_context(|| format!("missing start pose for servo {}", probe.servo_id))?;
        states.insert(
            probe.servo_id,
            BoundProbeState {
                probe: probe.clone(),
                start_position_ticks,
                has_started_motion: false,
                consecutive_stopped: 0,
            },
        );
    }
    for state in states.values() {
        let raw_direction = logical_direction * state.probe.logical_positive_sign;
        pose.insert(state.probe.servo_id, full_range_target(raw_direction));
    }

    sync_full_pose(bus, pose)?;

    let observations = wait_for_group_stop(bus, &mut states, &active_ids, params, phase_label)?;
    for (servo_id, observation) in &observations {
        pose.insert(*servo_id, observation.position_ticks);
    }

    Ok(observations)
}

fn move_joint_group_to_ratio(
    bus: &mut RealStsBus,
    pose: &mut BTreeMap<u8, u16>,
    measurements: &[ServoRangeMeasurement],
    ratio: f32,
    label: &str,
    poll_ms: u64,
    move_timeout_ms: u64,
) -> anyhow::Result<()> {
    info!(ratio, servo_count = measurements.len(), "{label}");
    let tracked_servo_ids = measurements
        .iter()
        .map(|measurement| measurement.servo_id)
        .collect::<Vec<_>>();
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
    move_pose_until_close(
        bus,
        pose,
        &tracked_servo_ids,
        poll_ms,
        move_timeout_ms,
        label,
    )?;
    thread::sleep(Duration::from_millis(PHASE_SETTLE_SLEEP_MS));
    Ok(())
}

fn move_pose_until_close(
    bus: &mut RealStsBus,
    target_pose: &BTreeMap<u8, u16>,
    tracked_servo_ids: &[u8],
    poll_ms: u64,
    move_timeout_ms: u64,
    label: &str,
) -> anyhow::Result<()> {
    let _span = info_span!(
        "move_pose_until_close",
        label = label,
        tracked_servos = tracked_servo_ids.len()
    )
    .entered();
    info!("starting pose move");
    sync_full_pose(bus, target_pose)?;
    let started = SystemTime::now();
    let mut stable_cycles = 0u8;

    loop {
        thread::sleep(Duration::from_millis(poll_ms));
        let feedback = read_feedback_map_best_effort(bus, tracked_servo_ids);
        let mut all_close_this_poll = true;

        for &servo_id in tracked_servo_ids {
            let target_ticks = *target_pose
                .get(&servo_id)
                .with_context(|| format!("missing target pose for servo {servo_id}"))?;
            let Some(telemetry) = feedback.get(&servo_id) else {
                all_close_this_poll = false;
                continue;
            };

            if telemetry.present_position_ticks.abs_diff(target_ticks)
                > RANGE_TARGET_TOLERANCE_TICKS
            {
                all_close_this_poll = false;
            }
        }

        if all_close_this_poll {
            stable_cycles = stable_cycles.saturating_add(1);
        } else {
            stable_cycles = 0;
        }

        if stable_cycles >= POSITION_SETTLE_CONFIRM_CYCLES {
            info!("pose settled");
            return Ok(());
        }

        if started.elapsed().unwrap_or_default().as_millis() > u128::from(move_timeout_ms) {
            warn!("{label}: did not settle within the timeout");
            bail!("{label} did not settle within the timeout");
        }
    }
}

fn wait_for_group_stop(
    bus: &mut RealStsBus,
    states: &mut BTreeMap<u8, BoundProbeState>,
    active_ids: &[u8],
    params: &SenseParams,
    phase_label: &str,
) -> anyhow::Result<Vec<(u8, ResistanceObservation)>> {
    let started = SystemTime::now();

    loop {
        thread::sleep(Duration::from_millis(params.poll_ms));
        let feedback = read_feedback_map(bus, active_ids)?;
        let mut finished = Vec::new();

        for (&servo_id, state) in states.iter_mut() {
            let telemetry = feedback.get(&servo_id).with_context(|| {
                format!("missing feedback for servo {servo_id} during {phase_label}")
            })?;

            if !state.has_started_motion && servo_has_started_motion(state, telemetry, params) {
                state.has_started_motion = true;
            }

            if state.has_started_motion && servo_has_stopped(telemetry, params.stop_speed_ticks) {
                state.consecutive_stopped = state.consecutive_stopped.saturating_add(1);
            } else {
                state.consecutive_stopped = 0;
            }

            if state.consecutive_stopped >= params.confirm_stopped_samples {
                finished.push((
                    servo_id,
                    ResistanceObservation {
                        position_ticks: telemetry.present_position_ticks,
                        detection: "torque_limit_stop".to_owned(),
                        load_pct: telemetry.present_load_pct,
                        current_ma: telemetry.present_current_ma,
                        moving: telemetry.moving,
                        status_bits: telemetry.status_bits,
                        faults: telemetry.faults.clone(),
                    },
                ));
            }
        }

        if finished.len() == states.len() {
            info!(
                stopped = finished.len(),
                "{phase_label}: all servos stopped"
            );
            return Ok(finished);
        }

        info!(
            stopped = finished.len(),
            active = states.len(),
            "{phase_label}: progress"
        );

        if started.elapsed().unwrap_or_default().as_millis() > u128::from(params.move_timeout_ms) {
            let lingering = states
                .iter()
                .filter_map(|(&servo_id, state)| {
                    (state.consecutive_stopped < params.confirm_stopped_samples).then_some(servo_id)
                })
                .collect::<Vec<_>>();
            bail!(
                "{phase_label} did not reach a stable stop within {} ms; lingering servos: {:?}",
                params.move_timeout_ms,
                lingering
            );
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
        notes: "Ranges were measured in the configured lay-down posture by lowering RAM torque limit, commanding full-range travel, and recording where each servo self-stopped on resistance. Positive direction is lift for femur/tibia and forward for coxa.".to_owned(),
        params,
        legs,
    }
}

fn load_range_report(path: &Path) -> anyhow::Result<RangeScanReport> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn build_suggested_pose_report(
    config: &RobotConfig,
    ranges_path: &Path,
    report: &RangeScanReport,
) -> anyhow::Result<SuggestedPoseReport> {
    let gait = TripodGait;
    let lay_down_pose = gait.lay_down_pose(config);
    let measured_legs = report
        .legs
        .iter()
        .map(|leg| (leg.name.as_str(), leg))
        .collect::<BTreeMap<_, _>>();

    let mut legs = Vec::with_capacity(config.legs.len());
    for leg in &config.legs {
        let measured = measured_legs
            .get(leg.name.as_str())
            .with_context(|| format!("missing measured ranges for leg {}", leg.name))?;
        legs.push(build_suggested_leg_pose(leg, measured, &lay_down_pose)?);
    }

    Ok(SuggestedPoseReport {
        robot_name: config.robot.name.clone(),
        deployment_profile: config.deployment.profile.clone(),
        generated_at_ms: now_ms(),
        ranges_path: ranges_path.display().to_string(),
        notes: "Suggested poses derived from measured free movement ranges. Coxa stand_reference uses midpoint, femur stand_reference biases slightly downward, tibia stand_reference biases slightly upward, and lay-down stays slightly inset from the currently configured laying-side edge.".to_owned(),
        legs,
    })
}

fn build_suggested_leg_pose(
    leg: &LegConfig,
    measured: &LegRangeReport,
    lay_down_pose: &BTreeMap<u8, u16>,
) -> anyhow::Result<SuggestedLegPose> {
    let coxa = measured
        .coxa
        .as_ref()
        .with_context(|| format!("missing coxa range for leg {}", leg.name))?;
    let femur = measured
        .femur
        .as_ref()
        .with_context(|| format!("missing femur range for leg {}", leg.name))?;
    let tibia = measured
        .tibia
        .as_ref()
        .with_context(|| format!("missing tibia range for leg {}", leg.name))?;

    Ok(SuggestedLegPose {
        name: leg.name.clone(),
        coxa_stand_reference_ticks: coxa.midpoint_ticks,
        femur_stand_reference_ticks: interpolate_ticks(
            femur.logical_negative_end_ticks,
            femur.logical_positive_end_ticks,
            FEMUR_STAND_REFERENCE_RATIO,
        ),
        tibia_stand_reference_ticks: interpolate_ticks(
            tibia.logical_negative_end_ticks,
            tibia.logical_positive_end_ticks,
            TIBIA_STAND_REFERENCE_RATIO,
        ),
        coxa_lay_down_ticks: suggested_lay_down_ticks(
            coxa,
            *lay_down_pose
                .get(&leg.coxa_servo_id)
                .with_context(|| format!("missing coxa lay pose for leg {}", leg.name))?,
        ),
        femur_lay_down_ticks: suggested_lay_down_ticks(
            femur,
            *lay_down_pose
                .get(&leg.femur_servo_id)
                .with_context(|| format!("missing femur lay pose for leg {}", leg.name))?,
        ),
        tibia_lay_down_ticks: suggested_lay_down_ticks(
            tibia,
            *lay_down_pose
                .get(&leg.tibia_servo_id)
                .with_context(|| format!("missing tibia lay pose for leg {}", leg.name))?,
        ),
    })
}

fn print_joint_check(
    joint: &str,
    stand_reference_ticks: u16,
    lay_down_ticks: u16,
    measurement: &JointRangeMeasurement,
) -> usize {
    let mut warnings = 0usize;

    if joint == "coxa" && measurement.span_ticks <= COXA_SUSPICIOUS_SPAN_TICKS {
        println!(
            "  {joint}: warning span {} looks suspiciously small",
            measurement.span_ticks
        );
        warnings += 1;
    }

    warnings += print_pose_check_line(joint, "stand_reference", stand_reference_ticks, measurement);
    warnings += print_pose_check_line(joint, "lay", lay_down_ticks, measurement);
    warnings
}

fn print_pose_check_line(
    joint: &str,
    label: &str,
    ticks: u16,
    measurement: &JointRangeMeasurement,
) -> usize {
    let min_ticks = measurement
        .logical_negative_end_ticks
        .min(measurement.logical_positive_end_ticks);
    let max_ticks = measurement
        .logical_negative_end_ticks
        .max(measurement.logical_positive_end_ticks);

    if ticks < min_ticks {
        println!(
            "    {joint} {label}: {} outside range [{}, {}] by {} ticks",
            ticks,
            min_ticks,
            max_ticks,
            min_ticks - ticks
        );
        return 1;
    }
    if ticks > max_ticks {
        println!(
            "    {joint} {label}: {} outside range [{}, {}] by {} ticks",
            ticks,
            min_ticks,
            max_ticks,
            ticks - max_ticks
        );
        return 1;
    }

    let edge_distance = ticks.abs_diff(min_ticks).min(ticks.abs_diff(max_ticks));
    if edge_distance <= POSE_EDGE_WARNING_MIN_TICKS {
        println!(
            "    {joint} {label}: {} within bounds but only {} ticks from the measured edge",
            ticks, edge_distance
        );
        return 1;
    }

    println!(
        "    {joint} {label}: {} within range [{}, {}]",
        ticks, min_ticks, max_ticks
    );
    0
}

fn suggested_lay_down_ticks(measurement: &JointRangeMeasurement, current_ticks: u16) -> u16 {
    let current_distance_to_negative =
        current_ticks.abs_diff(measurement.logical_negative_end_ticks);
    let current_distance_to_positive =
        current_ticks.abs_diff(measurement.logical_positive_end_ticks);

    if current_distance_to_negative <= current_distance_to_positive {
        interpolate_ticks(
            measurement.logical_negative_end_ticks,
            measurement.logical_positive_end_ticks,
            LAY_DOWN_INSET_RATIO,
        )
    } else {
        interpolate_ticks(
            measurement.logical_negative_end_ticks,
            measurement.logical_positive_end_ticks,
            1.0 - LAY_DOWN_INSET_RATIO,
        )
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

fn write_suggested_pose_report(path: &Path, report: &SuggestedPoseReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(report).context("failed to serialize suggested poses")?;
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
        let telemetry = read_feedback_with_retries(bus, *servo_id)?;
        feedback.insert(*servo_id, telemetry);
    }
    Ok(feedback)
}

fn read_feedback_map_best_effort(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
) -> BTreeMap<u8, ServoTelemetry> {
    let mut feedback = BTreeMap::new();
    for servo_id in servo_ids {
        match read_feedback_with_retries(bus, *servo_id) {
            Ok(telemetry) => {
                feedback.insert(*servo_id, telemetry);
            }
            Err(err) => {
                warn!("feedback warning for servo {servo_id}: {err:#}");
            }
        }
    }
    feedback
}

fn read_feedback_with_retries(
    bus: &mut RealStsBus,
    servo_id: u8,
) -> anyhow::Result<ServoTelemetry> {
    let mut last_error = None;
    for attempt in 0..FEEDBACK_READ_RETRIES {
        match bus.read_feedback(servo_id) {
            Ok(telemetry) => return Ok(telemetry),
            Err(err) => {
                last_error = Some(err);
                if attempt + 1 < FEEDBACK_READ_RETRIES {
                    thread::sleep(Duration::from_millis(FEEDBACK_RETRY_SLEEP_MS));
                }
            }
        }
    }

    Err(last_error.expect("feedback retry loop must record an error"))
        .with_context(|| format!("failed to read feedback from servo {}", servo_id))
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

fn full_range_target(raw_direction: i16) -> u16 {
    if raw_direction >= 0 { 4095 } else { 0 }
}

fn servo_has_started_motion(
    state: &BoundProbeState,
    telemetry: &ServoTelemetry,
    params: &SenseParams,
) -> bool {
    let speed_abs = i32::from(telemetry.present_speed_ticks).abs();
    telemetry.moving
        || speed_abs > i32::from(params.stop_speed_ticks)
        || state
            .start_position_ticks
            .abs_diff(telemetry.present_position_ticks)
            >= MOTION_START_TICKS
}

fn servo_has_stopped(telemetry: &ServoTelemetry, stop_speed_ticks: u16) -> bool {
    let speed_abs = i32::from(telemetry.present_speed_ticks).abs();
    !telemetry.moving || speed_abs <= i32::from(stop_speed_ticks)
}

fn interpolate_ticks(start_ticks: u16, end_ticks: u16, ratio: f32) -> u16 {
    let ratio = ratio.clamp(0.0, 1.0);
    let interpolated = start_ticks as f32 + (end_ticks as f32 - start_ticks as f32) * ratio;
    interpolated.round().clamp(0.0, 4095.0) as u16
}

fn resolve_output_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

fn resolve_trace_output_path(args: &Args, output_path: &Path) -> PathBuf {
    if let Some(path) = &args.trace_output {
        return resolve_output_path(path);
    }

    let parent = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("servo-ranges");

    parent.join(format!("{stem}.trace.log"))
}

fn init_trace_logging(path: &Path) -> anyhow::Result<WorkerGuard> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("invalid trace log filename: {}", path.display()))?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));

    let file_appender = tracing_appender::rolling::never(directory, file_name);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let stdout_layer = fmt::layer()
        .compact()
        .with_target(true)
        .with_filter(LevelFilter::INFO);

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_timer(fmt::time::uptime())
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_writer(file_writer)
        .with_filter(LevelFilter::TRACE);

    registry()
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|err| anyhow!("failed to initialize tracing subscriber: {err}"))?;

    Ok(guard)
}
