use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

mod dashboard_page;

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_core::{
    CameraBackend, LegPoseAngles, LegSideViewPose, LegTopViewPose, RobotConfig, SemanticPoseKind,
    now_ms, resolve_config_path, smoothstep,
};
use arachno_feetech_sts::{
    RealStsBus, set_verified_torque_limit_on_current_position_for_ids,
    validate_servo_eeprom_profile as validate_bus_servo_eeprom_profile,
};
use arachno_hal::{CameraSource, ImuSource, ServoBus, read_current_pose};
use arachno_imu_host::{DeviceInfoProbe, SensorKind, UsbImuBridge};
use arachno_msg::{ImuTelemetry, JointCommand, ServoTelemetry};
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, process::Command};
use tokio_util::io::ReaderStream;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*, registry};

const IMU_BRIDGE_BAUD_RATE: u32 = 115_200;
const IMU_PROBE_TIMEOUT_MS: u64 = 1_000;
const TELEMETRY_LOOP_MS: u64 = 250;
const LOW_VOLTAGE_STRIKES_TO_TRIP: u8 = 6;
const BODY_ATTITUDE_STRIKES_TO_TRIP: u8 = 3;
const BODY_ATTITUDE_ACCEL_NORM_TOLERANCE_MPS2: f32 = 2.5;
const STAND_UP_FEMUR_PREP_RATIO: f32 = 0.20;
const STAND_UP_TIBIA_PLANT_RATIO: f32 = 0.20;
const STAND_UP_BODY_RISE_RATIO: f32 = 0.45;
const MANUAL_COXA_LIMIT_DEG: f32 = 180.0;
const MANUAL_FEMUR_LIMIT_DEG: f32 = 180.0;
const MANUAL_TIBIA_LIMIT_DEG: f32 = 180.0;
const MANUAL_TORQUE_LIMIT_MAX: u16 = 1000;
const TILTED_STAND_PITCH_LIMIT_DEG: f32 = 20.0;
const TILTED_STAND_ROLL_LIMIT_DEG: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BrainMode {
    Telemetry,
    Manual,
    TiltedStand,
    LayDown,
    SitDown,
    StandUp,
    StandUpHigh,
    Stand,
    StandHigh,
    SlowWalk,
    SlowWalkHigh,
    BackwardWalk,
    BackwardWalkHigh,
    RotateLeft,
    RotateRight,
    SidewalkLeft,
    SidewalkLeftHigh,
    SidewalkRight,
    SidewalkRightHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TripodLiftMode {
    Standard,
    HighStep,
}

impl TripodLiftMode {
    fn target_step_height_cm(self, config: &RobotConfig, leg: &arachno_core::LegConfig) -> f32 {
        let default_step_height_cm = (leg.tibia_length_cm() * 0.14).clamp(1.8, 4.0);
        match self {
            Self::Standard => default_step_height_cm,
            Self::HighStep => config
                .locomotion
                .tripod
                .high_step_height_cm
                .max(default_step_height_cm)
                .max(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TripodMotionKind {
    Forward,
    Backward,
    RotateLeft,
    RotateRight,
    SidewalkLeft,
    SidewalkRight,
}

impl TripodMotionKind {
    fn summary_label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::RotateLeft => "left rotation",
            Self::RotateRight => "right rotation",
            Self::SidewalkLeft => "sidewalk left",
            Self::SidewalkRight => "sidewalk right",
        }
    }

    fn coxa_gait_direction_for_leg(self, is_left_side: bool, coxa_zero_heading_deg: f32) -> f32 {
        match self {
            Self::Forward => 1.0,
            Self::Backward => -1.0,
            Self::RotateLeft => {
                if is_left_side {
                    -1.0
                } else {
                    1.0
                }
            }
            Self::RotateRight => {
                if is_left_side {
                    1.0
                } else {
                    -1.0
                }
            }
            Self::SidewalkRight | Self::SidewalkLeft => {
                // Middle legs (heading ≈ 0°) produce no sideward contribution;
                // suppress their stride to avoid longitudinal drift.
                if coxa_zero_heading_deg.abs() < 1.0 {
                    return 0.0;
                }
                // Front legs (heading > 0) and rear legs (heading < 0) must
                // swing in opposing directions so each tripod produces pure
                // lateral body movement.  See walk_pose_from_base for the full
                // geometric derivation.
                let base = if is_left_side == (coxa_zero_heading_deg > 0.0) {
                    1.0
                } else {
                    -1.0
                };
                if matches!(self, Self::SidewalkLeft) {
                    -base
                } else {
                    base
                }
            }
        }
    }
}

impl BrainMode {
    fn as_state_label(self) -> &'static str {
        match self {
            Self::Telemetry => "telemetry",
            Self::Manual => "manual",
            Self::TiltedStand => "tilted_stand",
            Self::LayDown => "lay_down",
            Self::SitDown => "sit_down",
            Self::StandUp => "stand_up",
            Self::StandUpHigh => "stand_up_high",
            Self::Stand => "stand",
            Self::StandHigh => "stand_high",
            Self::SlowWalk => "slow_walk",
            Self::SlowWalkHigh => "slow_walk_high",
            Self::BackwardWalk => "backward_walk",
            Self::BackwardWalkHigh => "backward_walk_high",
            Self::RotateLeft => "rotate_left",
            Self::RotateRight => "rotate_right",
            Self::SidewalkLeft => "sidewalk_left",
            Self::SidewalkLeftHigh => "sidewalk_left_high",
            Self::SidewalkRight => "sidewalk_right",
            Self::SidewalkRightHigh => "sidewalk_right_high",
        }
    }

    fn requires_torque(self) -> bool {
        !matches!(self, Self::Telemetry)
    }

    fn enforces_body_attitude_limits(self) -> bool {
        !matches!(self, Self::Telemetry | Self::Manual)
    }

    fn stand_transition_target(self) -> Option<SemanticPoseKind> {
        match self {
            Self::StandUp => Some(SemanticPoseKind::StandReference),
            Self::StandUpHigh => Some(SemanticPoseKind::StandHigh),
            _ => None,
        }
    }

    fn stand_settle_target(self) -> Option<SemanticPoseKind> {
        match self {
            Self::Stand => Some(SemanticPoseKind::StandReference),
            Self::StandHigh => Some(SemanticPoseKind::StandHigh),
            _ => None,
        }
    }

    fn tripod_motion_kind(self) -> Option<TripodMotionKind> {
        match self {
            Self::SlowWalk | Self::SlowWalkHigh => Some(TripodMotionKind::Forward),
            Self::BackwardWalk | Self::BackwardWalkHigh => Some(TripodMotionKind::Backward),
            Self::RotateLeft => Some(TripodMotionKind::RotateLeft),
            Self::RotateRight => Some(TripodMotionKind::RotateRight),
            Self::SidewalkLeft | Self::SidewalkLeftHigh => Some(TripodMotionKind::SidewalkLeft),
            Self::SidewalkRight | Self::SidewalkRightHigh => Some(TripodMotionKind::SidewalkRight),
            _ => None,
        }
    }

    fn tripod_lift_mode(self) -> Option<TripodLiftMode> {
        match self {
            Self::SlowWalk
            | Self::BackwardWalk
            | Self::RotateLeft
            | Self::RotateRight
            | Self::SidewalkLeft
            | Self::SidewalkRight => Some(TripodLiftMode::Standard),
            Self::SlowWalkHigh
            | Self::BackwardWalkHigh
            | Self::SidewalkLeftHigh
            | Self::SidewalkRightHigh => Some(TripodLiftMode::HighStep),
            _ => None,
        }
    }

    fn is_tripod_gait(self) -> bool {
        self.tripod_motion_kind().is_some()
    }

    fn tripod_motion_summary_label(self) -> Option<String> {
        let motion = self.tripod_motion_kind()?;
        let label = motion.summary_label();
        Some(match self.tripod_lift_mode()? {
            TripodLiftMode::Standard => label.to_owned(),
            TripodLiftMode::HighStep => format!("high-step {label}"),
        })
    }

    /// Direction sign applied to the coxa swing for each leg.
    ///
    /// For the sideward gaits the middle legs receive 0.0 so they lift in place
    /// without striding. Their forward/backward contribution would otherwise
    /// not cancel and would produce unwanted longitudinal drift.  The front and
    /// rear angled legs are assigned opposing signs so that, within each tripod
    /// stance, the forward force from one cancels the backward force from the
    /// other, leaving only a net sideward force on the body.
    fn coxa_gait_direction_for_leg(self, is_left_side: bool, coxa_zero_heading_deg: f32) -> f32 {
        self.tripod_motion_kind()
            .map(|motion| motion.coxa_gait_direction_for_leg(is_left_side, coxa_zero_heading_deg))
            .unwrap_or(0.0)
    }
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "config/robot/default.toml")]
    config: PathBuf,
    #[arg(long, default_value = "127.0.0.1:4000")]
    listen: SocketAddr,
    #[arg(long, default_value_t = false)]
    dashboard: bool,
    #[arg(long, value_enum, default_value_t = BrainMode::Telemetry)]
    mode: BrainMode,
    #[arg(long)]
    walk_seconds: Option<f32>,
    #[arg(long, default_value_t = 0.0)]
    tilted_stand_pitch_deg: f32,
    #[arg(long, default_value_t = 0.0)]
    tilted_stand_roll_deg: f32,
}

#[derive(Clone)]
struct AppState {
    config: RobotConfig,
    shared: Arc<RwLock<TelemetryState>>,
    manual: Arc<RwLock<ManualControlState>>,
    tilted_stand: Arc<RwLock<TiltedStandState>>,
    calibration: Arc<RwLock<SemanticCalibrationState>>,
    pending_mode: Arc<RwLock<Option<BrainMode>>>,
    dashboard_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryState {
    robot_name: String,
    deployment_profile: String,
    compute_target: String,
    serial_port: String,
    camera_backend: CameraBackend,
    camera_device: Option<String>,
    camera_pipeline: String,
    motion_mode: String,
    motion_summary: String,
    safety_status: String,
    motion_fault: Option<String>,
    updated_at_ms: u64,
    online_servo_count: usize,
    last_poll_error: Option<String>,
    imu: Option<TelemetryImuState>,
    manual: TelemetryManualState,
    tilted_stand: TelemetryTiltedStandState,
    calibration: TelemetryCalibrationState,
    leg_previews: Vec<TelemetryLegPreviewState>,
    servos: Vec<TelemetryServoState>,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryServoState {
    servo_id: u8,
    label: String,
    online: bool,
    error: Option<String>,
    telemetry: Option<ServoTelemetry>,
    position_deg: Option<f32>,
    semantic_angle_deg: Option<f32>,
    position_percent: Option<f32>,
    speed_rpm: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryImuState {
    enabled: bool,
    mode: String,
    device: Option<String>,
    sensor_kind: Option<String>,
    sample_hz: Option<u16>,
    spi_mode: Option<u8>,
    observed_who_am_i: Option<u8>,
    description: Option<String>,
    last_error: Option<String>,
    telemetry: Option<ImuTelemetry>,
    roll_deg: Option<f32>,
    pitch_deg: Option<f32>,
    accel_norm_mps2: Option<f32>,
    gyro_norm_deg_s: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryManualState {
    enabled: bool,
    ready: bool,
    base_pose_captured: bool,
    summary: String,
    groups: Vec<ManualGroupInfo>,
    group_values: Vec<ManualGroupValue>,
    joints: Vec<ManualJointInfo>,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryTiltedStandState {
    enabled: bool,
    ready: bool,
    pitch_deg: f32,
    roll_deg: f32,
    pitch_limit_deg: f32,
    roll_limit_deg: f32,
    summary: String,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryCalibrationState {
    enabled: bool,
    summary: String,
    store_path: Option<String>,
    legs: Vec<CalibrationLegInfo>,
    joints: Vec<CalibrationJointInfo>,
    entries: Vec<CalibrationEntryView>,
}

#[derive(Debug, Clone, Serialize)]
struct TelemetryLegPreviewState {
    leg_key: String,
    top_view: Option<LegTopViewPose>,
    side_view: Option<LegSideViewPose>,
}

#[derive(Debug, Clone, Serialize)]
struct ManualGroupInfo {
    key: String,
    label: String,
    legs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ManualGroupValue {
    key: String,
    coxa_deg: f32,
    femur_deg: f32,
    tibia_deg: f32,
}

#[derive(Debug, Clone, Serialize)]
struct ManualJointInfo {
    key: String,
    label: String,
    negative_label: String,
    positive_label: String,
    min_deg: f32,
    max_deg: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationLegInfo {
    key: String,
    label: String,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationJointInfo {
    key: String,
    label: String,
    negative_label: String,
    zero_label: String,
    positive_label: String,
    negative_deg: f32,
    zero_deg: f32,
    positive_deg: f32,
}

#[derive(Debug, Clone, Serialize)]
struct CalibrationEntryView {
    servo_id: u8,
    leg_key: String,
    joint_key: String,
    negative_ticks: Option<u16>,
    zero_ticks: Option<u16>,
    positive_ticks: Option<u16>,
    reference_count: usize,
    zero_reference_ticks: Option<f32>,
    max_reference_error_ticks: Option<f32>,
}

#[derive(Debug, Clone)]
struct MotionRuntime {
    mode: BrainMode,
    walk_seconds: Option<f32>,
    armed_at: Option<Instant>,
    initial_pose: Option<BTreeMap<u8, u16>>,
    hold_pose: Option<BTreeMap<u8, u16>>,
    body_attitude_strikes: u8,
    low_voltage_strikes: BTreeMap<u8, u8>,
    summary: String,
    fault: Option<String>,
}

#[derive(Debug, Clone)]
struct ManualControlState {
    enabled: bool,
    base_pose: Option<BTreeMap<u8, u16>>,
    target_pose: Option<BTreeMap<u8, u16>>,
    summary: String,
    pending_actions: VecDeque<ManualHardwareAction>,
}

#[derive(Debug, Clone)]
struct TiltedStandState {
    enabled: bool,
    pitch_deg: f32,
    roll_deg: f32,
    summary: String,
}

#[derive(Debug, Clone)]
enum ManualHardwareAction {
    SetTorqueLimit {
        group_key: String,
        target: ManualTorqueTarget,
        torque_limit: u16,
    },
    SyncTargetToCurrent {
        group_key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ManualTorqueTarget {
    All,
    Coxa,
    Femur,
    Tibia,
}

impl ManualTorqueTarget {
    fn as_label(self) -> &'static str {
        match self {
            Self::All => "all joints",
            Self::Coxa => "coxa only",
            Self::Femur => "femur only",
            Self::Tibia => "tibia only",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SemanticCalibrationState {
    path: Option<PathBuf>,
    entries: BTreeMap<u8, ServoSemanticCalibrationEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SemanticCalibrationFile {
    #[serde(default)]
    servos: Vec<ServoSemanticCalibrationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServoSemanticCalibrationEntry {
    servo_id: u8,
    #[serde(default)]
    leg_name: Option<String>,
    #[serde(default)]
    joint_key: Option<String>,
    #[serde(default)]
    negative_ticks: Option<u16>,
    #[serde(default)]
    zero_ticks: Option<u16>,
    #[serde(default)]
    positive_ticks: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct ManualApplyRequest {
    group_key: String,
    coxa_deg: f32,
    femur_deg: f32,
    tibia_deg: f32,
}

#[derive(Debug, Deserialize)]
struct ManualResetRequest {
    group_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManualTorqueLimitRequest {
    group_key: String,
    target: ManualTorqueTarget,
    torque_limit: u16,
}

#[derive(Debug, Deserialize)]
struct ManualSyncTargetRequest {
    group_key: String,
}

#[derive(Debug, Deserialize)]
struct ManualJumpRequest {
    group_key: String,
    joint_key: String,
    delta_deg: f32,
}

#[derive(Debug, Deserialize)]
struct TiltedStandApplyRequest {
    pitch_deg: f32,
    roll_deg: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CalibrationReferenceKey {
    Negative,
    Zero,
    Positive,
}

#[derive(Debug, Deserialize)]
struct CalibrationCaptureRequest {
    leg_key: String,
    joint_key: String,
    reference_key: CalibrationReferenceKey,
}

#[derive(Debug, Deserialize)]
struct CalibrationClearRequest {
    leg_key: String,
    joint_key: String,
}

#[derive(Debug, Serialize)]
struct ManualCommandResponse {
    summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MotionCommand {
    Manual,
    TiltedStand,
    StandUp,
    StandUpHigh,
    LayDown,
    SitDown,
    Stand,
    StandHigh,
    WalkForward,
    WalkForwardHigh,
    WalkBackward,
    WalkBackwardHigh,
    RotateLeft,
    RotateRight,
    SidewalkLeft,
    SidewalkLeftHigh,
    SidewalkRight,
    SidewalkRightHigh,
    Stop,
    Telemetry,
}

impl MotionCommand {
    fn as_brain_mode(self) -> BrainMode {
        match self {
            Self::Manual => BrainMode::Manual,
            Self::TiltedStand => BrainMode::TiltedStand,
            Self::StandUp => BrainMode::StandUp,
            Self::StandUpHigh => BrainMode::StandUpHigh,
            Self::LayDown => BrainMode::LayDown,
            Self::SitDown => BrainMode::SitDown,
            Self::StandHigh => BrainMode::StandHigh,
            Self::Stand | Self::Stop => BrainMode::Stand,
            Self::WalkForward => BrainMode::SlowWalk,
            Self::WalkForwardHigh => BrainMode::SlowWalkHigh,
            Self::WalkBackward => BrainMode::BackwardWalk,
            Self::WalkBackwardHigh => BrainMode::BackwardWalkHigh,
            Self::RotateLeft => BrainMode::RotateLeft,
            Self::RotateRight => BrainMode::RotateRight,
            Self::SidewalkLeft => BrainMode::SidewalkLeft,
            Self::SidewalkLeftHigh => BrainMode::SidewalkLeftHigh,
            Self::SidewalkRight => BrainMode::SidewalkRight,
            Self::SidewalkRightHigh => BrainMode::SidewalkRightHigh,
            Self::Telemetry => BrainMode::Telemetry,
        }
    }
}

#[derive(Debug, Deserialize)]
struct MotionCommandRequest {
    command: MotionCommand,
}

#[derive(Debug, Serialize)]
struct MotionCommandResponse {
    summary: String,
    mode: String,
}

struct ServoPollOutcome {
    should_reopen_bus: bool,
}

impl TelemetryState {
    fn from_config(
        config: &RobotConfig,
        mode: BrainMode,
        manual: &ManualControlState,
        tilted_stand: &TiltedStandState,
        calibration: &SemanticCalibrationState,
    ) -> Self {
        let labels = servo_labels(config);
        let camera = RobotCamera::new(config.camera.clone());

        Self {
            robot_name: config.robot.name.clone(),
            deployment_profile: config.deployment.profile.clone(),
            compute_target: config.deployment.compute.clone(),
            serial_port: config.bus.feetech.port.clone(),
            camera_backend: config.camera.backend,
            camera_device: config.camera.device.clone(),
            camera_pipeline: camera.pipeline_description().to_owned(),
            motion_mode: mode.as_state_label().to_owned(),
            motion_summary: if mode == BrainMode::Telemetry {
                "observation only; no motion commands are being sent".to_owned()
            } else {
                "waiting for the control worker to arm motion".to_owned()
            },
            safety_status: if mode == BrainMode::Telemetry {
                "observation only".to_owned()
            } else {
                "waiting for servo and IMU state".to_owned()
            },
            motion_fault: None,
            updated_at_ms: 0,
            online_servo_count: 0,
            last_poll_error: Some("control worker not started yet".to_owned()),
            imu: telemetry_imu_from_config(config),
            manual: build_manual_telemetry(config, manual, calibration, None),
            tilted_stand: build_tilted_stand_telemetry(tilted_stand, false),
            calibration: build_calibration_telemetry(config, calibration),
            leg_previews: config
                .legs
                .iter()
                .map(|leg| TelemetryLegPreviewState {
                    leg_key: leg.name.clone(),
                    top_view: None,
                    side_view: None,
                })
                .collect(),
            servos: config
                .all_servo_ids()
                .into_iter()
                .map(|servo_id| {
                    let label = labels
                        .get(&servo_id)
                        .cloned()
                        .unwrap_or_else(|| format!("servo-{servo_id}"));
                    TelemetryServoState::offline(servo_id, label, "waiting for first poll")
                })
                .collect(),
        }
    }
}

impl ManualControlState {
    fn for_mode(mode: BrainMode) -> Self {
        let enabled = mode == BrainMode::Manual;
        let summary = if enabled {
            "waiting for the current robot pose before manual angle control becomes ready"
        } else {
            "manual control is disabled; switch the motion mode to manual to enable dashboard sliders"
        };

        Self {
            enabled,
            base_pose: None,
            target_pose: None,
            summary: summary.to_owned(),
            pending_actions: VecDeque::new(),
        }
    }
}

fn sync_manual_mode_state(manual: &Arc<RwLock<ManualControlState>>, mode: BrainMode) {
    if let Ok(mut control) = manual.write() {
        *control = ManualControlState::for_mode(mode);
    }
}

impl TiltedStandState {
    fn for_mode(mode: BrainMode, pitch_deg: f32, roll_deg: f32) -> Self {
        let enabled = mode == BrainMode::TiltedStand;
        let pitch_deg = clamp_tilted_stand_pitch_deg(pitch_deg);
        let roll_deg = clamp_tilted_stand_roll_deg(roll_deg);
        let summary = if enabled {
            "waiting for the current robot stance before tilted stand parameters take effect"
        } else {
            "tilted stand is disabled; switch the motion mode to tilted-stand to enable pitch and roll sliders"
        };

        Self {
            enabled,
            pitch_deg,
            roll_deg,
            summary: summary.to_owned(),
        }
    }
}

fn sync_tilted_stand_mode_state(
    tilted_stand: &Arc<RwLock<TiltedStandState>>,
    mode: BrainMode,
    pitch_deg: f32,
    roll_deg: f32,
) {
    if let Ok(mut control) = tilted_stand.write() {
        *control = TiltedStandState::for_mode(mode, pitch_deg, roll_deg);
    }
}

impl SemanticCalibrationState {
    fn load(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let Some(path) = path else {
            info!("semantic calibration store disabled");
            return Ok(Self::default());
        };

        let file_exists = path.exists();
        let entries = Self::load_entries_from_path(&path)?;
        if file_exists {
            info!(
                path = %path.display(),
                entry_count = entries.len(),
                "loaded semantic calibration file"
            );
        } else {
            info!(
                path = %path.display(),
                "semantic calibration file not found; starting with empty calibration state"
            );
        }

        Ok(Self {
            path: Some(path),
            entries,
        })
    }

    fn load_entries_from_path(
        path: &Path,
    ) -> anyhow::Result<BTreeMap<u8, ServoSemanticCalibrationEntry>> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file: SemanticCalibrationFile = toml::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(file
            .servos
            .into_iter()
            .map(|entry| (entry.servo_id, entry))
            .collect())
    }

    fn is_enabled(&self) -> bool {
        self.path.is_some()
    }

    fn store_path_display(&self) -> Option<String> {
        self.path.as_ref().map(|path| path.display().to_string())
    }

    fn entry(&self, servo_id: u8) -> Option<&ServoSemanticCalibrationEntry> {
        self.entries.get(&servo_id)
    }

    fn set_reference(
        &mut self,
        servo_id: u8,
        leg_name: &str,
        joint_key: &str,
        reference_key: CalibrationReferenceKey,
        ticks: u16,
    ) {
        let entry = self
            .entries
            .entry(servo_id)
            .or_insert_with(|| ServoSemanticCalibrationEntry {
                servo_id,
                leg_name: Some(leg_name.to_owned()),
                joint_key: Some(joint_key.to_owned()),
                negative_ticks: None,
                zero_ticks: None,
                positive_ticks: None,
            });
        entry.leg_name = Some(leg_name.to_owned());
        entry.joint_key = Some(joint_key.to_owned());
        match reference_key {
            CalibrationReferenceKey::Negative => entry.negative_ticks = Some(ticks),
            CalibrationReferenceKey::Zero => entry.zero_ticks = Some(ticks),
            CalibrationReferenceKey::Positive => entry.positive_ticks = Some(ticks),
        }
    }

    fn clear_servo(&mut self, servo_id: u8) {
        self.entries.remove(&servo_id);
    }

    fn reload(&mut self) -> anyhow::Result<usize> {
        let Some(path) = self.path.clone() else {
            info!("semantic calibration reload skipped because no store is configured");
            self.entries.clear();
            return Ok(0);
        };

        let file_exists = path.exists();
        self.entries = Self::load_entries_from_path(&path)?;
        if file_exists {
            info!(
                path = %path.display(),
                entry_count = self.entries.len(),
                "reloaded semantic calibration file"
            );
        } else {
            info!(
                path = %path.display(),
                "semantic calibration reload found no file; keeping empty calibration state"
            );
        }
        Ok(self.entries.len())
    }

    fn save(&self) -> anyhow::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create calibration directory {}",
                    parent.display()
                )
            })?;
        }

        let file = SemanticCalibrationFile {
            servos: self.entries.values().cloned().collect(),
        };
        let content = toml::to_string_pretty(&file)
            .context("failed to serialize semantic calibration file")?;
        fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

impl TelemetryServoState {
    fn offline(servo_id: u8, label: String, message: impl Into<String>) -> Self {
        Self {
            servo_id,
            label,
            online: false,
            error: Some(message.into()),
            telemetry: None,
            position_deg: None,
            semantic_angle_deg: None,
            position_percent: None,
            speed_rpm: None,
        }
    }

    fn online(label: String, telemetry: ServoTelemetry) -> Self {
        let position_deg = Some(ticks_to_deg(telemetry.present_position_ticks));
        let position_percent = Some(telemetry.present_position_ticks as f32 / 4095.0 * 100.0);
        let speed_rpm = Some(speed_ticks_to_rpm(telemetry.present_speed_ticks));
        let error = if telemetry.faults.is_empty() {
            None
        } else {
            Some(telemetry.faults.join(", "))
        };

        Self {
            servo_id: telemetry.servo_id,
            label,
            online: true,
            error,
            telemetry: Some(telemetry),
            position_deg,
            semantic_angle_deg: None,
            position_percent,
            speed_rpm,
        }
    }
}

impl MotionRuntime {
    fn new(mode: BrainMode, walk_seconds: Option<f32>) -> Self {
        let summary = if mode.is_tripod_gait() {
            let gait = mode
                .tripod_motion_summary_label()
                .expect("tripod gait modes should define a summary label");
            format!("waiting for all servo feedback before starting the {gait} gait")
        } else {
            match mode {
                BrainMode::Telemetry => "observation only; no motion commands are being sent",
                BrainMode::Manual => "waiting for all servo feedback before arming manual control",
                BrainMode::TiltedStand => {
                    "waiting for all servo feedback before holding tilted stand"
                }
                BrainMode::LayDown => "waiting for all servo feedback before laying down",
                BrainMode::SitDown => "waiting for all servo feedback before sitting down",
                BrainMode::StandUp => "waiting for all servo feedback before standing up",
                BrainMode::StandUpHigh => "waiting for all servo feedback before standing up high",
                BrainMode::Stand => "waiting for all servo feedback before holding stand",
                BrainMode::StandHigh => "waiting for all servo feedback before holding high stand",
                _ => unreachable!("non-tripod modes should be handled above"),
            }
            .to_owned()
        };

        Self {
            mode,
            walk_seconds,
            armed_at: None,
            initial_pose: None,
            hold_pose: None,
            body_attitude_strikes: 0,
            low_voltage_strikes: BTreeMap::new(),
            summary,
            fault: None,
        }
    }

    fn arm(&mut self, pose: BTreeMap<u8, u16>) {
        if self.armed_at.is_some() {
            return;
        }

        let servo_count = pose.len();
        self.armed_at = Some(Instant::now());
        self.initial_pose = Some(pose.clone());
        self.hold_pose = Some(pose);
        self.body_attitude_strikes = 0;
        self.summary = match self.mode {
            BrainMode::Manual => "manual control armed at the measured robot pose".to_owned(),
            BrainMode::TiltedStand => "tilted stand armed at the measured robot pose".to_owned(),
            BrainMode::LayDown => "starting lay-down transition".to_owned(),
            BrainMode::SitDown => "starting sit-down transition".to_owned(),
            BrainMode::StandUp => "starting stand-up transition".to_owned(),
            BrainMode::StandUpHigh => "starting high stand-up transition".to_owned(),
            BrainMode::Stand => "holding the configured stand-reference pose".to_owned(),
            BrainMode::StandHigh => "holding the configured stand-high pose".to_owned(),
            BrainMode::Telemetry => {
                "observation only; no motion commands are being sent".to_owned()
            }
            mode => {
                debug_assert!(mode.is_tripod_gait());
                format!(
                    "holding the measured stand pose before {} gait",
                    mode.tripod_motion_summary_label()
                        .expect("tripod gait modes should define a summary label")
                )
            }
        };
        info!(
            mode = %self.mode.as_state_label(),
            servo_count,
            summary = %self.summary,
            "motion armed"
        );
    }

    fn disarm(&mut self, message: impl Into<String>) {
        self.armed_at = None;
        self.initial_pose = None;
        self.body_attitude_strikes = 0;
        self.summary = message.into();
    }

    fn trip_fault(&mut self, reason: impl Into<String>, hold_pose: Option<BTreeMap<u8, u16>>) {
        if self.fault.is_some() {
            return;
        }

        let reason = reason.into();
        if let Some(pose) = hold_pose {
            self.hold_pose = Some(pose);
        }
        self.fault = Some(reason.clone());
        self.summary = format!("motion halted: {reason}");
        warn!(mode = %self.mode.as_state_label(), reason = %reason, "motion fault tripped");
    }

    fn safety_status(&self, imu_enabled: bool) -> String {
        if let Some(fault) = &self.fault {
            return format!("tripped: {fault}");
        }

        match self.mode {
            BrainMode::Telemetry => "observation only".to_owned(),
            BrainMode::Manual => {
                let _ = imu_enabled;
                "manual control active; monitoring bus voltage and temperature".to_owned()
            }
            BrainMode::TiltedStand
            | BrainMode::LayDown
            | BrainMode::SitDown
            | BrainMode::StandUp
            | BrainMode::StandUpHigh
            | BrainMode::Stand
            | BrainMode::StandHigh => {
                if imu_enabled {
                    "monitoring roll, pitch, bus voltage, and temperature".to_owned()
                } else {
                    "monitoring bus voltage and temperature".to_owned()
                }
            }
            mode => {
                debug_assert!(mode.is_tripod_gait());
                if imu_enabled {
                    "monitoring roll, pitch, bus voltage, and temperature".to_owned()
                } else {
                    "monitoring bus voltage and temperature".to_owned()
                }
            }
        }
    }

    fn check_safety(
        &mut self,
        config: &RobotConfig,
        servo_ids: &[u8],
        servo_states: &BTreeMap<u8, TelemetryServoState>,
        imu_state: Option<&TelemetryImuState>,
    ) -> Option<String> {
        if let Some(reason) = body_attitude_fault_reason(
            self.mode,
            &config.safety,
            imu_state,
            &mut self.body_attitude_strikes,
        ) {
            return Some(reason);
        }

        for servo_id in servo_ids {
            let Some(telemetry) = servo_states
                .get(servo_id)
                .and_then(|state| state.telemetry.as_ref())
            else {
                continue;
            };

            if config.safety.min_bus_voltage_v > 0.0 {
                if telemetry.present_voltage_v < config.safety.min_bus_voltage_v {
                    let strikes = self.low_voltage_strikes.entry(*servo_id).or_default();
                    *strikes = strikes.saturating_add(1);
                    if *strikes >= LOW_VOLTAGE_STRIKES_TO_TRIP {
                        return Some(format!(
                            "servo {} voltage {:.1} V stayed below {:.1} V for {} samples",
                            telemetry.servo_id,
                            telemetry.present_voltage_v,
                            config.safety.min_bus_voltage_v,
                            LOW_VOLTAGE_STRIKES_TO_TRIP
                        ));
                    }
                } else {
                    self.low_voltage_strikes.remove(servo_id);
                }
            } else {
                self.low_voltage_strikes.clear();
            }

            if let Some(temp_c) = telemetry.present_temperature_c
                && temp_c > config.safety.max_servo_temp_c
            {
                return Some(format!(
                    "servo {} temperature {} C exceeded {} C",
                    telemetry.servo_id, temp_c, config.safety.max_servo_temp_c
                ));
            }
        }

        None
    }

    fn commands(
        &mut self,
        config: &RobotConfig,
        calibration: &SemanticCalibrationState,
        manual: Option<&Arc<RwLock<ManualControlState>>>,
        tilted_stand: Option<&Arc<RwLock<TiltedStandState>>>,
    ) -> Option<Vec<JointCommand>> {
        if self.mode == BrainMode::Telemetry {
            return None;
        }

        let base_pose = self
            .initial_pose
            .clone()
            .or_else(|| self.hold_pose.clone())
            .unwrap_or_else(|| self.fallback_pose(config, calibration));

        let target_pose = if self.fault.is_some() {
            self.hold_pose.clone().unwrap_or_else(|| base_pose.clone())
        } else {
            let armed_at = self.armed_at?;
            let elapsed = armed_at.elapsed().as_secs_f32();
            match self.mode {
                BrainMode::Manual => {
                    if let Some(shared) = manual {
                        match shared.read() {
                            Ok(control) => {
                                self.summary = control.summary.clone();
                                control
                                    .target_pose
                                    .clone()
                                    .or_else(|| control.base_pose.clone())
                                    .unwrap_or_else(|| base_pose.clone())
                            }
                            Err(_) => {
                                self.summary =
                                    "manual control state is unavailable; holding the current pose"
                                        .to_owned();
                                base_pose.clone()
                            }
                        }
                    } else {
                        self.summary =
                            "manual control channel is unavailable; holding the current pose"
                                .to_owned();
                        base_pose.clone()
                    }
                }
                BrainMode::TiltedStand => {
                    if let Some(shared) = tilted_stand {
                        match shared.read() {
                            Ok(control) => {
                                let (pose, summary) = tilted_stand_pose(
                                    config,
                                    calibration,
                                    &base_pose,
                                    control.pitch_deg,
                                    control.roll_deg,
                                );
                                self.summary = summary;
                                pose
                            }
                            Err(_) => {
                                self.summary =
                                    "tilted stand state is unavailable; holding the current pose"
                                        .to_owned();
                                base_pose.clone()
                            }
                        }
                    } else {
                        self.summary =
                            "tilted stand channel is unavailable; holding the current pose"
                                .to_owned();
                        base_pose.clone()
                    }
                }
                BrainMode::LayDown => {
                    let (pose, summary) = lay_down_pose(config, calibration, &base_pose, elapsed);
                    self.summary = summary;
                    pose
                }
                BrainMode::SitDown => {
                    let (pose, summary) = sit_down_pose(config, calibration, &base_pose, elapsed);
                    self.summary = summary;
                    pose
                }
                BrainMode::StandUp | BrainMode::StandUpHigh => {
                    let target_kind = self
                        .mode
                        .stand_transition_target()
                        .expect("stand-up modes should define a target pose");
                    let (pose, summary) =
                        staged_stand_up_pose(config, calibration, &base_pose, elapsed, target_kind);
                    self.summary = summary;
                    pose
                }
                BrainMode::Stand | BrainMode::StandHigh => {
                    let target_kind = self
                        .mode
                        .stand_settle_target()
                        .expect("stand modes should define a settle target pose");
                    let (pose, summary) =
                        stand_settle_pose(config, calibration, &base_pose, elapsed, target_kind);
                    self.summary = summary;
                    pose
                }
                BrainMode::Telemetry => unreachable!(),
                mode => {
                    debug_assert!(mode.is_tripod_gait());
                    let (pose, summary) = tripod_gait_pose(
                        config,
                        calibration,
                        &base_pose,
                        elapsed,
                        mode,
                        self.walk_seconds,
                    );
                    self.summary = summary;
                    pose
                }
            }
        };

        self.hold_pose = Some(target_pose.clone());
        Some(pose_to_commands(&target_pose))
    }

    fn fallback_pose(
        &self,
        config: &RobotConfig,
        calibration: &SemanticCalibrationState,
    ) -> BTreeMap<u8, u16> {
        match self.mode {
            BrainMode::Manual => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandReference)
            }
            BrainMode::TiltedStand => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandReference)
            }
            BrainMode::LayDown | BrainMode::StandUp | BrainMode::StandUpHigh => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::LayDown)
            }
            BrainMode::SitDown => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::SitDown)
            }
            BrainMode::Stand => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandReference)
            }
            BrainMode::StandHigh => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandHigh)
            }
            BrainMode::Telemetry => {
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandReference)
            }
            mode => {
                debug_assert!(mode.is_tripod_gait());
                named_pose_with_calibration(config, calibration, SemanticPoseKind::StandReference)
            }
        }
    }
}

fn body_attitude_fault_reason(
    mode: BrainMode,
    safety: &arachno_core::SafetyConfig,
    imu_state: Option<&TelemetryImuState>,
    body_attitude_strikes: &mut u8,
) -> Option<String> {
    if !mode.enforces_body_attitude_limits() {
        *body_attitude_strikes = 0;
        return None;
    }

    let Some(imu) = imu_state else {
        return None;
    };

    let attitude_reason = if let Some(roll_deg) = imu.roll_deg {
        if roll_deg.abs() > safety.max_body_roll_deg {
            Some(format!(
                "body roll {:.1} deg exceeded limit {:.1} deg",
                roll_deg, safety.max_body_roll_deg
            ))
        } else {
            None
        }
    } else {
        None
    }
    .or_else(|| {
        let pitch_deg = imu.pitch_deg?;
        (pitch_deg.abs() > safety.max_body_pitch_deg).then(|| {
            format!(
                "body pitch {:.1} deg exceeded limit {:.1} deg",
                pitch_deg, safety.max_body_pitch_deg
            )
        })
    });

    if let Some(reason) = attitude_reason {
        let accel_near_gravity = imu
            .accel_norm_mps2
            .map(|norm| (norm - 9.81).abs() <= BODY_ATTITUDE_ACCEL_NORM_TOLERANCE_MPS2)
            .unwrap_or(true);
        if accel_near_gravity {
            *body_attitude_strikes = body_attitude_strikes.saturating_add(1);
            if *body_attitude_strikes >= BODY_ATTITUDE_STRIKES_TO_TRIP {
                return Some(format!(
                    "{} for {} consecutive samples",
                    reason, BODY_ATTITUDE_STRIKES_TO_TRIP
                ));
            }
        } else {
            *body_attitude_strikes = 0;
        }
    } else {
        *body_attitude_strikes = 0;
    }

    None
}

fn validate_servo_eeprom_profile(config: &RobotConfig) -> anyhow::Result<()> {
    if config.servo_eeprom.entries.is_empty() {
        return Ok(());
    }

    let servo_ids = config.all_servo_ids();
    let mut bus = RealStsBus::open(
        config.bus.feetech.port.clone(),
        config.bus.feetech.baud_rate,
        servo_ids.clone(),
    )
    .with_context(|| {
        format!(
            "failed to open legs servo bus {} for EEPROM validation",
            config.bus.feetech.port
        )
    })?;

    validate_bus_servo_eeprom_profile(&mut bus, &servo_ids, &config.servo_eeprom.entries)
        .context("persistent servo EEPROM profile validation failed")?;

    Ok(())
}

fn init_logging() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,arachno_feetech_sts=warn,hyper=warn,tower_http=warn")
    });

    registry()
        .with(env_filter)
        .with(fmt::layer().compact().with_target(true))
        .try_init()
        .map_err(|err| anyhow::anyhow!("failed to initialize tracing subscriber: {err}"))?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_logging()?;
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;
    validate_servo_eeprom_profile(&config)?;
    if !config.servo_eeprom.entries.is_empty() {
        info!(
            entry_count = config.servo_eeprom.entries.len(),
            "servo EEPROM profile validated"
        );
    }

    let manual = Arc::new(RwLock::new(ManualControlState::for_mode(args.mode)));
    let tilted_stand = Arc::new(RwLock::new(TiltedStandState::for_mode(
        args.mode,
        args.tilted_stand_pitch_deg,
        args.tilted_stand_roll_deg,
    )));
    let calibration_path = config
        .semantic_calibration_store
        .as_ref()
        .map(|store| resolve_config_path(&args.config, &store.path));
    let calibration = Arc::new(RwLock::new(
        SemanticCalibrationState::load(calibration_path)
            .context("failed to load semantic calibration state")?,
    ));
    let initial_manual = manual
        .read()
        .map_err(|_| anyhow::anyhow!("failed to initialize manual control state"))?
        .clone();
    let initial_tilted_stand = tilted_stand
        .read()
        .map_err(|_| anyhow::anyhow!("failed to initialize tilted stand control state"))?
        .clone();
    let initial_calibration = calibration
        .read()
        .map_err(|_| anyhow::anyhow!("failed to initialize semantic calibration state"))?
        .clone();
    let shared = Arc::new(RwLock::new(TelemetryState::from_config(
        &config,
        args.mode,
        &initial_manual,
        &initial_tilted_stand,
        &initial_calibration,
    )));
    let pending_mode: Arc<RwLock<Option<BrainMode>>> = Arc::new(RwLock::new(None));
    spawn_control_worker(
        shared.clone(),
        manual.clone(),
        tilted_stand.clone(),
        calibration.clone(),
        pending_mode.clone(),
        config.clone(),
        args.mode,
        args.walk_seconds,
        clamp_tilted_stand_pitch_deg(args.tilted_stand_pitch_deg),
        clamp_tilted_stand_roll_deg(args.tilted_stand_roll_deg),
    );

    let app = Router::new()
        .route("/", get(index))
        .route("/dashboard", get(dashboard))
        .route("/api/state", get(api_state))
        .route("/api/motion/command", post(api_motion_command))
        .route("/api/manual/capture", post(api_manual_capture))
        .route("/api/manual/apply", post(api_manual_apply))
        .route("/api/manual/reset", post(api_manual_reset))
        .route("/api/manual/torque-limit", post(api_manual_torque_limit))
        .route("/api/manual/sync-current", post(api_manual_sync_current))
        .route("/api/manual/jump", post(api_manual_jump))
        .route("/api/tilted-stand/apply", post(api_tilted_stand_apply))
        .route("/api/tilted-stand/reset", post(api_tilted_stand_reset))
        .route("/api/calibration/capture", post(api_calibration_capture))
        .route("/api/calibration/clear", post(api_calibration_clear))
        .route("/api/calibration/reload", post(api_calibration_reload))
        .route("/camera.mjpg", get(camera_stream))
        .layer(CorsLayer::new().allow_origin(Any))
        .with_state(AppState {
            config: config.clone(),
            shared,
            manual,
            tilted_stand,
            calibration,
            pending_mode,
            dashboard_enabled: args.dashboard,
        });

    let listener = TcpListener::bind(args.listen).await?;
    info!(url = %format!("http://{}", args.listen), "arachno-brain API listening");
    info!(deployment_profile = %config.deployment.profile, "deployment profile");
    info!(compute_target = %config.deployment.compute, "compute target");
    info!(
        legs_servo_port = %config.bus.feetech.port,
        configured_servo_ports = ?config.bus.feetech.configured_ports(),
        "legs servo bus"
    );
    info!(mode = %args.mode.as_state_label(), "brain mode");
    if args.mode == BrainMode::Manual {
        info!("manual control enabled via /api/manual/* and dashboard sliders");
    }
    if args.mode == BrainMode::TiltedStand {
        info!(
            pitch_deg = clamp_tilted_stand_pitch_deg(args.tilted_stand_pitch_deg),
            roll_deg = clamp_tilted_stand_roll_deg(args.tilted_stand_roll_deg),
            "tilted stand enabled via /api/tilted-stand/* and dashboard sliders"
        );
    }
    if let Some(limit) = args.walk_seconds {
        info!(walk_seconds = limit, "walk duration limit configured");
    }
    if let Some(imu) = &config.imu {
        info!(
            enabled = imu.enabled,
            mode = %imu.mode,
            device = %imu.device.as_deref().unwrap_or("n/a"),
            "IMU bridge"
        );
    } else {
        info!("IMU bridge disabled");
    }
    if args.dashboard {
        info!(url = %format!("http://{}", args.listen), "dashboard UI enabled");
    } else {
        info!("dashboard UI disabled; start with --dashboard to serve it here");
    }
    axum::serve(listener, app).await?;
    Ok(())
}

fn motion_loop_period(mode: BrainMode, config: &RobotConfig) -> Duration {
    if mode == BrainMode::Telemetry {
        Duration::from_millis(TELEMETRY_LOOP_MS)
    } else {
        Duration::from_secs_f32(1.0 / config.locomotion.command_hz.max(1) as f32)
    }
}

fn spawn_control_worker(
    shared: Arc<RwLock<TelemetryState>>,
    manual: Arc<RwLock<ManualControlState>>,
    tilted_stand: Arc<RwLock<TiltedStandState>>,
    calibration: Arc<RwLock<SemanticCalibrationState>>,
    pending_mode: Arc<RwLock<Option<BrainMode>>>,
    config: RobotConfig,
    mode: BrainMode,
    walk_seconds: Option<f32>,
    tilted_stand_pitch_deg: f32,
    tilted_stand_roll_deg: f32,
) {
    thread::spawn(move || {
        let labels = servo_labels(&config);
        let servo_ids = config.all_servo_ids();
        let mut bus = None::<RealStsBus>;
        let mut torque_enabled = false;
        let mut imu_bridge = None::<UsbImuBridge>;
        let mut imu_state = telemetry_imu_from_config(&config);
        let mut mode = mode;
        let mut motion = MotionRuntime::new(mode, walk_seconds);
        let mut servo_states = initial_servo_states(&servo_ids, &labels);
        let mut telemetry_cursor = 0usize;
        let mut loop_period = motion_loop_period(mode, &config);

        loop {
            let tick_started = Instant::now();

            if let Ok(mut pm) = pending_mode.write()
                && let Some(new_mode) = pm.take()
                && new_mode != mode
            {
                let old_label = mode.as_state_label();
                mode = new_mode;
                motion = MotionRuntime::new(new_mode, walk_seconds);
                loop_period = motion_loop_period(new_mode, &config);
                sync_manual_mode_state(&manual, new_mode);
                sync_tilted_stand_mode_state(
                    &tilted_stand,
                    new_mode,
                    tilted_stand_pitch_deg,
                    tilted_stand_roll_deg,
                );
                if !new_mode.requires_torque() && torque_enabled {
                    if let Some(b) = bus.as_mut()
                        && let Err(e) = b.enable_torque(false)
                    {
                        warn!(error = %e, "failed to disable torque on mode switch to telemetry");
                    }
                    torque_enabled = false;
                }
                info!(
                    old = old_label,
                    new = new_mode.as_state_label(),
                    "motion mode changed via dashboard command"
                );
            }

            poll_imu(&config, &mut imu_bridge, &mut imu_state);

            let calibration_snapshot = calibration
                .read()
                .map(|state| state.clone())
                .unwrap_or_default();

            if bus.is_none() {
                match RealStsBus::open(
                    config.bus.feetech.port.clone(),
                    config.bus.feetech.baud_rate,
                    servo_ids.clone(),
                ) {
                    Ok(real_bus) => {
                        info!(
                            port = %config.bus.feetech.port,
                            baud_rate = config.bus.feetech.baud_rate,
                            servo_count = servo_ids.len(),
                            "legs servo bus opened"
                        );
                        bus = Some(real_bus);
                        torque_enabled = false;
                        if mode.requires_torque() {
                            motion.disarm("legs servo bus opened; waiting to arm motion");
                        }
                    }
                    Err(err) => {
                        motion.disarm(format!("waiting for legs servo bus: {err}"));
                        write_state(
                            &shared,
                            build_state_snapshot(
                                &config,
                                &servo_ids,
                                &servo_states,
                                imu_state.clone(),
                                &motion,
                                &manual,
                                &tilted_stand,
                                &calibration_snapshot,
                                Some(format!("failed to open legs servo bus: {err}")),
                            ),
                        );
                        sleep_remaining(tick_started, loop_period);
                        continue;
                    }
                }
            }

            let Some(real_bus) = bus.as_mut() else {
                sleep_remaining(tick_started, loop_period);
                continue;
            };

            if mode.requires_torque() && !torque_enabled {
                if let Err(err) = real_bus.enable_torque(true) {
                    warn!(error = %err, "failed to enable servo torque");
                    motion.disarm(format!("failed to enable torque: {err}"));
                    write_state(
                        &shared,
                        build_state_snapshot(
                            &config,
                            &servo_ids,
                            &servo_states,
                            imu_state.clone(),
                            &motion,
                            &manual,
                            &tilted_stand,
                            &calibration_snapshot,
                            Some(format!("failed to enable torque: {err}")),
                        ),
                    );
                    bus = None;
                    sleep_remaining(tick_started, loop_period);
                    continue;
                }
                torque_enabled = true;
            }

            let read_budget = if mode == BrainMode::Telemetry || motion.armed_at.is_none() {
                servo_ids.len()
            } else {
                config
                    .bus
                    .feetech
                    .telemetry_stride
                    .clamp(1, servo_ids.len())
            };

            let poll_outcome = poll_servo_window(
                real_bus,
                &servo_ids,
                &labels,
                &mut servo_states,
                &mut telemetry_cursor,
                read_budget,
            );

            if poll_outcome.should_reopen_bus {
                warn!("legs servo bus needs reopen after communication failure");
                motion.trip_fault(
                    "legs servo bus link dropped; motion paused",
                    current_pose(&servo_ids, &servo_states),
                );
                bus = None;
                torque_enabled = false;
                write_state(
                    &shared,
                    build_state_snapshot(
                        &config,
                        &servo_ids,
                        &servo_states,
                        imu_state.clone(),
                        &motion,
                        &manual,
                        &tilted_stand,
                        &calibration_snapshot,
                        Some("legs servo bus needs to be reopened".to_owned()),
                    ),
                );
                sleep_remaining(tick_started, loop_period);
                continue;
            }

            if mode.requires_torque() && motion.armed_at.is_none() {
                if let Some(start_pose) = current_pose(&servo_ids, &servo_states) {
                    ensure_manual_reference_pose(&manual, mode, &start_pose);
                    motion.arm(start_pose);
                } else {
                    motion.summary = format!(
                        "waiting for all {} servo feedback replies before motion",
                        servo_ids.len()
                    );
                }
            }

            if mode.requires_torque()
                && motion.fault.is_none()
                && let Some(reason) =
                    motion.check_safety(&config, &servo_ids, &servo_states, imu_state.as_ref())
            {
                motion.trip_fault(reason, current_pose(&servo_ids, &servo_states));
            }

            if mode == BrainMode::Manual
                && motion.fault.is_none()
                && let Err(err) = process_pending_manual_action(real_bus, &config, &manual)
            {
                if let Ok(mut control) = manual.write() {
                    control.summary = format!("manual utility failed: {err}");
                }
                warn!(error = %err, "manual utility failed");
                motion.trip_fault(
                    format!("manual utility failed: {err}"),
                    current_pose(&servo_ids, &servo_states),
                );
                bus = None;
                torque_enabled = false;
                write_state(
                    &shared,
                    build_state_snapshot(
                        &config,
                        &servo_ids,
                        &servo_states,
                        imu_state.clone(),
                        &motion,
                        &manual,
                        &tilted_stand,
                        &calibration_snapshot,
                        Some(format!("manual utility failed: {err}")),
                    ),
                );
                sleep_remaining(tick_started, loop_period);
                continue;
            }

            if let Some(commands) = motion.commands(
                &config,
                &calibration_snapshot,
                Some(&manual),
                Some(&tilted_stand),
            ) && let Err(err) = real_bus.sync_write_positions(&commands)
            {
                warn!(error = %err, command_count = commands.len(), "failed to send motion commands");
                motion.trip_fault(
                    format!("failed to send motion commands: {err}"),
                    current_pose(&servo_ids, &servo_states),
                );
                bus = None;
                torque_enabled = false;
                write_state(
                    &shared,
                    build_state_snapshot(
                        &config,
                        &servo_ids,
                        &servo_states,
                        imu_state.clone(),
                        &motion,
                        &manual,
                        &tilted_stand,
                        &calibration_snapshot,
                        Some(format!("sync write failed: {err}")),
                    ),
                );
                sleep_remaining(tick_started, loop_period);
                continue;
            }

            write_state(
                &shared,
                build_state_snapshot(
                    &config,
                    &servo_ids,
                    &servo_states,
                    imu_state.clone(),
                    &motion,
                    &manual,
                    &tilted_stand,
                    &calibration_snapshot,
                    None,
                ),
            );
            sleep_remaining(tick_started, loop_period);
        }
    });
}

fn poll_imu(
    config: &RobotConfig,
    imu_bridge: &mut Option<UsbImuBridge>,
    imu_state: &mut Option<TelemetryImuState>,
) {
    let Some(state) = imu_state.as_mut().filter(|state| state.enabled) else {
        return;
    };

    if imu_bridge.is_none() {
        match open_imu_bridge(state) {
            Ok(bridge) => *imu_bridge = Some(bridge),
            Err(err) => {
                state.last_error = Some(format!("failed to open IMU bridge: {err}"));
                return;
            }
        }
    }

    if let Some(bridge) = imu_bridge.as_mut()
        && let Err(err) = drain_imu_bridge(bridge, state)
    {
        state.last_error = Some(format!("IMU read failed: {err}"));
        *imu_bridge = None;
        if config.imu.as_ref().is_some_and(|imu| imu.enabled) {
            state.sensor_kind = None;
        }
    }
}

fn initial_servo_states(
    servo_ids: &[u8],
    labels: &BTreeMap<u8, String>,
) -> BTreeMap<u8, TelemetryServoState> {
    servo_ids
        .iter()
        .map(|servo_id| {
            let label = labels
                .get(servo_id)
                .cloned()
                .unwrap_or_else(|| format!("servo-{servo_id}"));
            (
                *servo_id,
                TelemetryServoState::offline(*servo_id, label, "waiting for first poll"),
            )
        })
        .collect()
}

fn poll_servo_window(
    bus: &mut RealStsBus,
    servo_ids: &[u8],
    labels: &BTreeMap<u8, String>,
    servo_states: &mut BTreeMap<u8, TelemetryServoState>,
    cursor: &mut usize,
    read_budget: usize,
) -> ServoPollOutcome {
    let mut should_reopen_bus = false;

    for _ in 0..read_budget.max(1) {
        let servo_id = servo_ids[*cursor % servo_ids.len()];
        *cursor = (*cursor + 1) % servo_ids.len();

        let label = labels
            .get(&servo_id)
            .cloned()
            .unwrap_or_else(|| format!("servo-{servo_id}"));

        let next = match bus.read_feedback(servo_id) {
            Ok(telemetry) => TelemetryServoState::online(label, telemetry),
            Err(err) => {
                let message = err.to_string();
                if is_reopen_error(&message) {
                    should_reopen_bus = true;
                }
                TelemetryServoState::offline(servo_id, label, message)
            }
        };
        servo_states.insert(servo_id, next);
    }

    ServoPollOutcome { should_reopen_bus }
}

#[allow(clippy::too_many_arguments)]
fn build_state_snapshot(
    config: &RobotConfig,
    servo_ids: &[u8],
    servo_states: &BTreeMap<u8, TelemetryServoState>,
    imu: Option<TelemetryImuState>,
    motion: &MotionRuntime,
    manual: &Arc<RwLock<ManualControlState>>,
    tilted_stand: &Arc<RwLock<TiltedStandState>>,
    calibration_snapshot: &SemanticCalibrationState,
    transport_error: Option<String>,
) -> TelemetryState {
    let pose = current_pose(servo_ids, servo_states);
    let leg_previews = build_leg_previews(config, servo_states, calibration_snapshot);
    let servos = servo_ids
        .iter()
        .map(|servo_id| {
            let mut servo = servo_states.get(servo_id).cloned().unwrap_or_else(|| {
                TelemetryServoState::offline(
                    *servo_id,
                    format!("servo-{servo_id}"),
                    "missing state",
                )
            });
            servo.semantic_angle_deg = servo.telemetry.as_ref().and_then(|telemetry| {
                servo_semantic_angle_deg(
                    config,
                    calibration_snapshot,
                    telemetry.servo_id,
                    telemetry.present_position_ticks,
                )
            });
            servo
        })
        .collect::<Vec<_>>();

    let online_servo_count = servos.iter().filter(|servo| servo.online).count();
    let last_poll_error = transport_error.or_else(|| {
        if online_servo_count == servo_ids.len() {
            None
        } else {
            Some(format!(
                "{} of {} configured servos replied on the latest sweep",
                online_servo_count,
                servo_ids.len()
            ))
        }
    });

    TelemetryState {
        robot_name: config.robot.name.clone(),
        deployment_profile: config.deployment.profile.clone(),
        compute_target: config.deployment.compute.clone(),
        serial_port: config.bus.feetech.port.clone(),
        camera_backend: config.camera.backend,
        camera_device: config.camera.device.clone(),
        camera_pipeline: RobotCamera::new(config.camera.clone())
            .pipeline_description()
            .to_owned(),
        motion_mode: motion.mode.as_state_label().to_owned(),
        motion_summary: motion.summary.clone(),
        safety_status: motion.safety_status(config.imu.as_ref().is_some_and(|imu| imu.enabled)),
        motion_fault: motion.fault.clone(),
        updated_at_ms: now_ms(),
        online_servo_count,
        last_poll_error,
        imu,
        manual: manual_snapshot(config, manual, calibration_snapshot, pose.as_ref()),
        tilted_stand: tilted_stand_snapshot(
            tilted_stand,
            motion.mode == BrainMode::TiltedStand && motion.armed_at.is_some(),
        ),
        calibration: build_calibration_telemetry(config, calibration_snapshot),
        leg_previews,
        servos,
    }
}

fn build_leg_previews(
    config: &RobotConfig,
    servo_states: &BTreeMap<u8, TelemetryServoState>,
    calibration: &SemanticCalibrationState,
) -> Vec<TelemetryLegPreviewState> {
    config
        .legs
        .iter()
        .map(|leg| {
            let semantic = |servo_id| {
                let telemetry = servo_states.get(&servo_id)?.telemetry.as_ref()?;
                servo_semantic_angle_deg(
                    config,
                    calibration,
                    servo_id,
                    telemetry.present_position_ticks,
                )
            };
            let coxa = semantic(leg.coxa_servo_id);
            let femur = semantic(leg.femur_servo_id);
            let tibia = semantic(leg.tibia_servo_id);

            TelemetryLegPreviewState {
                leg_key: leg.name.clone(),
                top_view: match (coxa, femur, tibia) {
                    (Some(coxa), Some(femur), Some(tibia)) => {
                        Some(leg.top_view_pose(coxa, femur, tibia))
                    }
                    _ => None,
                },
                side_view: match (femur, tibia) {
                    (Some(femur), Some(tibia)) => Some(leg.side_view_pose(femur, tibia)),
                    _ => None,
                },
            }
        })
        .collect()
}

fn build_manual_telemetry(
    config: &RobotConfig,
    control: &ManualControlState,
    calibration: &SemanticCalibrationState,
    current_pose: Option<&BTreeMap<u8, u16>>,
) -> TelemetryManualState {
    TelemetryManualState {
        enabled: control.enabled,
        ready: current_pose.is_some(),
        base_pose_captured: control.base_pose.is_some(),
        summary: control.summary.clone(),
        groups: manual_group_infos(config),
        group_values: manual_group_values(config, calibration, current_pose),
        joints: manual_joint_infos(),
    }
}

fn build_tilted_stand_telemetry(
    control: &TiltedStandState,
    ready: bool,
) -> TelemetryTiltedStandState {
    let summary = if control.enabled && ready {
        if control
            .summary
            .contains("waiting for the current robot stance")
        {
            "tilted stand is ready; holding the captured stance until pitch or roll changes"
                .to_owned()
        } else {
            control.summary.clone()
        }
    } else {
        control.summary.clone()
    };

    TelemetryTiltedStandState {
        enabled: control.enabled,
        ready,
        pitch_deg: control.pitch_deg,
        roll_deg: control.roll_deg,
        pitch_limit_deg: TILTED_STAND_PITCH_LIMIT_DEG,
        roll_limit_deg: TILTED_STAND_ROLL_LIMIT_DEG,
        summary,
    }
}

fn manual_snapshot(
    config: &RobotConfig,
    manual: &Arc<RwLock<ManualControlState>>,
    calibration: &SemanticCalibrationState,
    current_pose: Option<&BTreeMap<u8, u16>>,
) -> TelemetryManualState {
    match manual.read() {
        Ok(control) => build_manual_telemetry(config, &control, calibration, current_pose),
        Err(_) => TelemetryManualState {
            enabled: false,
            ready: false,
            base_pose_captured: false,
            summary: "manual control state is unavailable".to_owned(),
            groups: manual_group_infos(config),
            group_values: Vec::new(),
            joints: manual_joint_infos(),
        },
    }
}

fn tilted_stand_snapshot(
    tilted_stand: &Arc<RwLock<TiltedStandState>>,
    ready: bool,
) -> TelemetryTiltedStandState {
    match tilted_stand.read() {
        Ok(control) => build_tilted_stand_telemetry(&control, ready),
        Err(_) => TelemetryTiltedStandState {
            enabled: false,
            ready: false,
            pitch_deg: 0.0,
            roll_deg: 0.0,
            pitch_limit_deg: TILTED_STAND_PITCH_LIMIT_DEG,
            roll_limit_deg: TILTED_STAND_ROLL_LIMIT_DEG,
            summary: "tilted stand state is unavailable".to_owned(),
        },
    }
}

fn build_calibration_telemetry(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
) -> TelemetryCalibrationState {
    let entries = config
        .legs
        .iter()
        .flat_map(|leg| {
            [
                (leg.coxa_servo_id, "coxa"),
                (leg.femur_servo_id, "femur"),
                (leg.tibia_servo_id, "tibia"),
            ]
            .into_iter()
            .filter_map(move |(servo_id, joint_key)| {
                let entry = calibration.entry(servo_id)?;
                let reference_count =
                    [entry.negative_ticks, entry.zero_ticks, entry.positive_ticks]
                        .into_iter()
                        .flatten()
                        .count();
                Some(CalibrationEntryView {
                    servo_id,
                    leg_key: leg.name.clone(),
                    joint_key: joint_key.to_owned(),
                    negative_ticks: entry.negative_ticks,
                    zero_ticks: entry.zero_ticks,
                    positive_ticks: entry.positive_ticks,
                    reference_count,
                    zero_reference_ticks: servo_zero_reference_tick(config, calibration, servo_id),
                    max_reference_error_ticks: servo_calibration_reference_error_ticks(
                        config,
                        calibration,
                        servo_id,
                    ),
                })
            })
        })
        .collect::<Vec<_>>();

    let summary = if calibration.is_enabled() {
        format!("{} joint calibration profile(s) saved", entries.len())
    } else {
        "semantic calibration store disabled in this profile".to_owned()
    };

    TelemetryCalibrationState {
        enabled: calibration.is_enabled(),
        summary,
        store_path: calibration.store_path_display(),
        legs: std::iter::once(CalibrationLegInfo {
            key: "all".to_owned(),
            label: "All legs".to_owned(),
        })
        .chain(config.legs.iter().map(|leg| CalibrationLegInfo {
            key: leg.name.clone(),
            label: humanize_leg_name(&leg.name),
        }))
        .collect(),
        joints: calibration_joint_infos(),
        entries,
    }
}

fn manual_group_infos(config: &RobotConfig) -> Vec<ManualGroupInfo> {
    let mut groups = vec![
        manual_group_info(
            "all_legs",
            "All legs",
            config.legs.iter().map(|leg| leg.name.as_str()).collect(),
        ),
        manual_group_info(
            "left_side",
            "Left side",
            config
                .legs
                .iter()
                .filter(|leg| leg.name.contains("left"))
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "right_side",
            "Right side",
            config
                .legs
                .iter()
                .filter(|leg| leg.name.contains("right"))
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "front_pair",
            "Front pair",
            config
                .legs
                .iter()
                .filter(|leg| leg.name.starts_with("front_"))
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "middle_pair",
            "Middle pair",
            config
                .legs
                .iter()
                .filter(|leg| leg.name.starts_with("middle_"))
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "rear_pair",
            "Rear pair",
            config
                .legs
                .iter()
                .filter(|leg| leg.name.starts_with("rear_"))
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "tripod_a",
            "Tripod A",
            config
                .legs
                .iter()
                .filter(|leg| leg.is_tripod_a())
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
        manual_group_info(
            "tripod_b",
            "Tripod B",
            config
                .legs
                .iter()
                .filter(|leg| !leg.is_tripod_a())
                .map(|leg| leg.name.as_str())
                .collect(),
        ),
    ];

    groups.extend(config.legs.iter().map(|leg| ManualGroupInfo {
        key: format!("leg:{}", leg.name),
        label: humanize_leg_name(&leg.name),
        legs: vec![humanize_leg_name(&leg.name)],
    }));
    groups
}

fn manual_group_info(key: &str, label: &str, legs: Vec<&str>) -> ManualGroupInfo {
    ManualGroupInfo {
        key: key.to_owned(),
        label: label.to_owned(),
        legs: legs.into_iter().map(humanize_leg_name).collect(),
    }
}

fn manual_group_values(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    current_pose: Option<&BTreeMap<u8, u16>>,
) -> Vec<ManualGroupValue> {
    let Some(current_pose) = current_pose else {
        return Vec::new();
    };

    let mut groups = vec![
        manual_group_value(config, calibration, "all_legs", current_pose),
        manual_group_value_for_filter(config, calibration, "left_side", current_pose, |leg| {
            leg.name.contains("left")
        }),
        manual_group_value_for_filter(config, calibration, "right_side", current_pose, |leg| {
            leg.name.contains("right")
        }),
        manual_group_value_for_filter(config, calibration, "front_pair", current_pose, |leg| {
            leg.name.starts_with("front_")
        }),
        manual_group_value_for_filter(config, calibration, "middle_pair", current_pose, |leg| {
            leg.name.starts_with("middle_")
        }),
        manual_group_value_for_filter(config, calibration, "rear_pair", current_pose, |leg| {
            leg.name.starts_with("rear_")
        }),
        manual_group_value_for_filter(config, calibration, "tripod_a", current_pose, |leg| {
            leg.is_tripod_a()
        }),
        manual_group_value_for_filter(config, calibration, "tripod_b", current_pose, |leg| {
            !leg.is_tripod_a()
        }),
    ];

    groups.extend(config.legs.iter().map(|leg| {
        manual_group_value_for_legs(
            format!("leg:{}", leg.name),
            config,
            calibration,
            current_pose,
            vec![leg],
        )
    }));

    groups
}

fn manual_group_value(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    key: &str,
    current_pose: &BTreeMap<u8, u16>,
) -> ManualGroupValue {
    manual_group_value_for_legs(
        key.to_owned(),
        config,
        calibration,
        current_pose,
        config.legs.iter().collect(),
    )
}

fn manual_group_value_for_filter<F>(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    key: &str,
    current_pose: &BTreeMap<u8, u16>,
    predicate: F,
) -> ManualGroupValue
where
    F: Fn(&arachno_core::LegConfig) -> bool,
{
    manual_group_value_for_legs(
        key.to_owned(),
        config,
        calibration,
        current_pose,
        config.legs.iter().filter(|leg| predicate(leg)).collect(),
    )
}

fn manual_group_value_for_legs(
    key: String,
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    current_pose: &BTreeMap<u8, u16>,
    legs: Vec<&arachno_core::LegConfig>,
) -> ManualGroupValue {
    ManualGroupValue {
        key,
        coxa_deg: manual_joint_group_average_deg(config, calibration, current_pose, &legs, "coxa"),
        femur_deg: manual_joint_group_average_deg(
            config,
            calibration,
            current_pose,
            &legs,
            "femur",
        ),
        tibia_deg: manual_joint_group_average_deg(
            config,
            calibration,
            current_pose,
            &legs,
            "tibia",
        ),
    }
}

fn manual_joint_group_average_deg(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    current_pose: &BTreeMap<u8, u16>,
    legs: &[&arachno_core::LegConfig],
    joint_key: &str,
) -> f32 {
    let values = legs
        .iter()
        .filter_map(|leg| {
            let servo_id = manual_joint_servo_and_sign(leg, joint_key)?.0;
            let current_ticks = current_pose.get(&servo_id).copied()?;
            servo_semantic_angle_deg(config, calibration, servo_id, current_ticks)
        })
        .collect::<Vec<_>>();

    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn manual_joint_infos() -> Vec<ManualJointInfo> {
    vec![
        ManualJointInfo {
            key: "coxa".to_owned(),
            label: "Coxa".to_owned(),
            negative_label: "back".to_owned(),
            positive_label: "forward".to_owned(),
            min_deg: -MANUAL_COXA_LIMIT_DEG,
            max_deg: MANUAL_COXA_LIMIT_DEG,
        },
        ManualJointInfo {
            key: "femur".to_owned(),
            label: "Femur".to_owned(),
            negative_label: "down".to_owned(),
            positive_label: "up".to_owned(),
            min_deg: -MANUAL_FEMUR_LIMIT_DEG,
            max_deg: MANUAL_FEMUR_LIMIT_DEG,
        },
        ManualJointInfo {
            key: "tibia".to_owned(),
            label: "Tibia".to_owned(),
            negative_label: "down".to_owned(),
            positive_label: "up".to_owned(),
            min_deg: -MANUAL_TIBIA_LIMIT_DEG,
            max_deg: MANUAL_TIBIA_LIMIT_DEG,
        },
    ]
}

fn calibration_joint_infos() -> Vec<CalibrationJointInfo> {
    vec![
        CalibrationJointInfo {
            key: "coxa".to_owned(),
            label: "Coxa".to_owned(),
            negative_label: "back".to_owned(),
            zero_label: "center".to_owned(),
            positive_label: "forward".to_owned(),
            negative_deg: -45.0,
            zero_deg: 0.0,
            positive_deg: 45.0,
        },
        CalibrationJointInfo {
            key: "femur".to_owned(),
            label: "Femur".to_owned(),
            negative_label: "down".to_owned(),
            zero_label: "neutral".to_owned(),
            positive_label: "up".to_owned(),
            negative_deg: -45.0,
            zero_deg: 0.0,
            positive_deg: 45.0,
        },
        CalibrationJointInfo {
            key: "tibia".to_owned(),
            label: "Tibia".to_owned(),
            negative_label: "down".to_owned(),
            zero_label: "neutral".to_owned(),
            positive_label: "up".to_owned(),
            negative_deg: -60.0,
            zero_deg: 0.0,
            positive_deg: 60.0,
        },
    ]
}

fn humanize_leg_name(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ensure_manual_reference_pose(
    manual: &Arc<RwLock<ManualControlState>>,
    mode: BrainMode,
    pose: &BTreeMap<u8, u16>,
) {
    if mode != BrainMode::Manual {
        return;
    }

    if let Ok(mut control) = manual.write() {
        if !control.enabled || control.target_pose.is_some() {
            return;
        }

        control.target_pose = Some(pose.clone());
        control.summary =
            "manual control is ready; sliders now reflect the current robot pose as absolute semantic joint angles"
                .to_owned();
    }
}

fn current_pose(
    servo_ids: &[u8],
    servo_states: &BTreeMap<u8, TelemetryServoState>,
) -> Option<BTreeMap<u8, u16>> {
    let mut pose = BTreeMap::new();

    for servo_id in servo_ids {
        let state = servo_states.get(servo_id)?;
        let telemetry = state.telemetry.as_ref()?;
        pose.insert(*servo_id, telemetry.present_position_ticks);
    }

    Some(pose)
}

fn current_pose_from_snapshot_servos(servos: &[TelemetryServoState]) -> Option<BTreeMap<u8, u16>> {
    let mut pose = BTreeMap::new();
    for servo in servos {
        let telemetry = servo.telemetry.as_ref()?;
        pose.insert(servo.servo_id, telemetry.present_position_ticks);
    }
    Some(pose)
}

fn current_pose_from_shared_snapshot(
    state: &AppState,
) -> Result<BTreeMap<u8, u16>, (StatusCode, String)> {
    let snapshot = state.shared.read().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "telemetry state lock poisoned",
        )
    })?;
    current_pose_from_snapshot_servos(&snapshot.servos).ok_or_else(|| {
        manual_api_error(
            StatusCode::CONFLICT,
            "fresh feedback from all configured servos is required before capture or apply actions can run",
        )
    })
}

fn ensure_manual_enabled(state: &AppState) -> Result<(), (StatusCode, String)> {
    let enabled = state
        .manual
        .read()
        .map_err(|_| {
            manual_api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "manual control lock poisoned",
            )
        })?
        .enabled;
    if enabled {
        Ok(())
    } else {
        Err(manual_api_error(
            StatusCode::CONFLICT,
            "manual control is disabled; switch the motion mode to manual first",
        ))
    }
}

fn ensure_tilted_stand_enabled(state: &AppState) -> Result<(), (StatusCode, String)> {
    let enabled = state
        .tilted_stand
        .read()
        .map_err(|_| {
            manual_api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "tilted stand lock poisoned",
            )
        })?
        .enabled;
    if enabled {
        Ok(())
    } else {
        Err(manual_api_error(
            StatusCode::CONFLICT,
            "tilted stand is disabled; switch the motion mode to tilted-stand first",
        ))
    }
}

fn ensure_calibration_enabled(state: &AppState) -> Result<(), (StatusCode, String)> {
    let enabled = state
        .calibration
        .read()
        .map_err(|_| {
            manual_api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "semantic calibration lock poisoned",
            )
        })?
        .is_enabled();
    if enabled {
        Ok(())
    } else {
        Err(manual_api_error(
            StatusCode::CONFLICT,
            "semantic calibration store is disabled for this profile",
        ))
    }
}

fn manual_api_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, String) {
    (status, message.into())
}

fn resolve_leg<'a>(config: &'a RobotConfig, leg_key: &str) -> Option<&'a arachno_core::LegConfig> {
    config.legs.iter().find(|leg| leg.name == leg_key)
}

fn resolve_calibration_legs<'a>(
    config: &'a RobotConfig,
    leg_key: &str,
) -> Option<Vec<&'a arachno_core::LegConfig>> {
    if leg_key == "all" {
        return Some(config.legs.iter().collect());
    }
    resolve_leg(config, leg_key).map(|leg| vec![leg])
}

fn resolve_servo_id_for_joint(leg: &arachno_core::LegConfig, joint_key: &str) -> Option<u8> {
    match joint_key {
        "coxa" => Some(leg.coxa_servo_id),
        "femur" => Some(leg.femur_servo_id),
        "tibia" => Some(leg.tibia_servo_id),
        _ => None,
    }
}

fn resolve_manual_group<'a>(
    config: &'a RobotConfig,
    key: &str,
) -> Option<(String, Vec<&'a arachno_core::LegConfig>)> {
    let select = |predicate: &dyn Fn(&arachno_core::LegConfig) -> bool,
                  label: &str|
     -> Option<(String, Vec<&'a arachno_core::LegConfig>)> {
        let legs = config
            .legs
            .iter()
            .filter(|leg| predicate(leg))
            .collect::<Vec<_>>();
        (!legs.is_empty()).then(|| (label.to_owned(), legs))
    };

    match key {
        "all_legs" => select(&|_| true, "All legs"),
        "left_side" => select(&|leg| leg.name.contains("left"), "Left side"),
        "right_side" => select(&|leg| leg.name.contains("right"), "Right side"),
        "front_pair" => select(&|leg| leg.name.starts_with("front_"), "Front pair"),
        "middle_pair" => select(&|leg| leg.name.starts_with("middle_"), "Middle pair"),
        "rear_pair" => select(&|leg| leg.name.starts_with("rear_"), "Rear pair"),
        "tripod_a" => select(&|leg| leg.is_tripod_a(), "Tripod A"),
        "tripod_b" => select(&|leg| !leg.is_tripod_a(), "Tripod B"),
        _ => key.strip_prefix("leg:").and_then(|name| {
            config
                .legs
                .iter()
                .find(|leg| leg.name == name)
                .map(|leg| (humanize_leg_name(&leg.name), vec![leg]))
        }),
    }
}

fn manual_group_servo_ids(
    legs: &[&arachno_core::LegConfig],
    target: ManualTorqueTarget,
) -> Vec<u8> {
    let mut servo_ids = Vec::with_capacity(legs.len() * 3);
    for leg in legs {
        match target {
            ManualTorqueTarget::All => {
                servo_ids.push(leg.coxa_servo_id);
                servo_ids.push(leg.femur_servo_id);
                servo_ids.push(leg.tibia_servo_id);
            }
            ManualTorqueTarget::Coxa => servo_ids.push(leg.coxa_servo_id),
            ManualTorqueTarget::Femur => servo_ids.push(leg.femur_servo_id),
            ManualTorqueTarget::Tibia => servo_ids.push(leg.tibia_servo_id),
        }
    }
    servo_ids
}

fn process_pending_manual_action(
    bus: &mut RealStsBus,
    config: &RobotConfig,
    manual: &Arc<RwLock<ManualControlState>>,
) -> anyhow::Result<Option<String>> {
    let (action, target_seed) = {
        let mut control = manual
            .write()
            .map_err(|_| anyhow::anyhow!("manual control lock poisoned"))?;
        (
            control.pending_actions.pop_front(),
            control
                .target_pose
                .clone()
                .or_else(|| control.base_pose.clone()),
        )
    };
    let Some(action) = action else {
        return Ok(None);
    };

    let (group_key, torque_target) = match &action {
        ManualHardwareAction::SetTorqueLimit {
            group_key, target, ..
        } => (group_key.as_str(), *target),
        ManualHardwareAction::SyncTargetToCurrent { group_key } => {
            (group_key.as_str(), ManualTorqueTarget::All)
        }
    };
    let (group_label, legs) = resolve_manual_group(config, group_key)
        .ok_or_else(|| anyhow::anyhow!("unknown manual control group {group_key}"))?;
    let servo_ids = manual_group_servo_ids(&legs, torque_target);
    let current_group_pose = read_current_pose(bus, &servo_ids)
        .map_err(|err| anyhow::anyhow!("failed to read current pose for {group_label}: {err}"))?;

    let mut next_target_pose = if let Some(seed) = target_seed {
        seed
    } else {
        let all_servo_ids = bus.servo_ids().to_vec();
        read_current_pose(bus, &all_servo_ids).map_err(|err| {
            anyhow::anyhow!("failed to read current pose while seeding manual target: {err}")
        })?
    };
    for (&servo_id, &ticks) in &current_group_pose {
        next_target_pose.insert(servo_id, ticks);
    }

    let summary = match action {
        ManualHardwareAction::SetTorqueLimit {
            torque_limit,
            target,
            ..
        } => {
            set_verified_torque_limit_on_current_position_for_ids(bus, &servo_ids, torque_limit)
                .map_err(|err| {
                    anyhow::anyhow!(
                        "failed to apply verified torque limit {} to {} ({}): {}",
                        torque_limit,
                        group_label,
                        target.as_label(),
                        err
                    )
                })?;
            format!(
                "manual utility: synced {} {} to the live pose and applied torque limit {}",
                group_label,
                target.as_label(),
                torque_limit
            )
        }
        ManualHardwareAction::SyncTargetToCurrent { .. } => {
            format!(
                "manual utility: synced {} target to the live pose",
                group_label
            )
        }
    };

    let mut control = manual
        .write()
        .map_err(|_| anyhow::anyhow!("manual control lock poisoned"))?;
    control.target_pose = Some(next_target_pose);
    control.summary = summary.clone();
    Ok(Some(summary))
}

fn set_manual_joint_target(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    target_pose: &mut BTreeMap<u8, u16>,
    leg: &arachno_core::LegConfig,
    joint_key: &str,
    degrees: f32,
) {
    let Some((servo_id, _sign)) = manual_joint_servo_and_sign(leg, joint_key) else {
        return;
    };
    let Some(next_ticks) =
        servo_ticks_for_semantic_angle_deg(config, calibration, servo_id, degrees)
    else {
        return;
    };
    target_pose.insert(servo_id, next_ticks);
}

fn reset_manual_leg_to_base(
    target_pose: &mut BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    leg: &arachno_core::LegConfig,
) {
    for servo_id in [leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id] {
        if let Some(base_ticks) = base_pose.get(&servo_id).copied() {
            target_pose.insert(servo_id, base_ticks);
        }
    }
}

fn manual_joint_servo_and_sign(
    leg: &arachno_core::LegConfig,
    joint_key: &str,
) -> Option<(u8, i16)> {
    match joint_key {
        "coxa" => Some((leg.coxa_servo_id, leg.coxa_forward_sign())),
        "femur" => Some((leg.femur_servo_id, leg.femur_lift_sign())),
        "tibia" => Some((leg.tibia_servo_id, leg.tibia_lift_sign())),
        _ => None,
    }
}

fn named_pose_with_calibration(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    kind: SemanticPoseKind,
) -> BTreeMap<u8, u16> {
    let mut pose = BTreeMap::new();

    for leg in &config.legs {
        let resolved = resolved_leg_pose_with_calibration(config, calibration, leg, kind);
        pose.insert(leg.coxa_servo_id, resolved.coxa_ticks);
        pose.insert(leg.femur_servo_id, resolved.femur_ticks);
        pose.insert(leg.tibia_servo_id, resolved.tibia_ticks);
    }

    pose
}

fn resolved_leg_pose_with_calibration(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    leg: &arachno_core::LegConfig,
    kind: SemanticPoseKind,
) -> ResolvedLegPose {
    let semantic = configured_pose_angles(config, calibration, leg, kind);

    ResolvedLegPose {
        coxa_ticks: servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.coxa_servo_id,
            semantic.coxa_deg,
        )
        .unwrap_or_else(|| leg.coxa_zero_reference_ticks()),
        femur_ticks: servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.femur_servo_id,
            semantic.femur_deg,
        )
        .unwrap_or_else(|| leg.femur_zero_reference_ticks()),
        tibia_ticks: servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.tibia_servo_id,
            semantic.tibia_deg,
        )
        .unwrap_or_else(|| leg.tibia_zero_reference_ticks()),
    }
}

fn configured_pose_angles(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    leg: &arachno_core::LegConfig,
    kind: SemanticPoseKind,
) -> LegPoseAngles {
    config
        .pose_for_leg(kind, &leg.name)
        .unwrap_or_else(|| legacy_pose_angles(config, calibration, leg, kind))
}

fn legacy_pose_angles(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    leg: &arachno_core::LegConfig,
    kind: SemanticPoseKind,
) -> LegPoseAngles {
    let Some((coxa_ticks, femur_ticks, tibia_ticks)) = leg.legacy_pose_ticks(kind) else {
        return LegPoseAngles::default();
    };

    LegPoseAngles {
        coxa_deg: servo_semantic_angle_deg(config, calibration, leg.coxa_servo_id, coxa_ticks)
            .unwrap_or(0.0),
        femur_deg: servo_semantic_angle_deg(config, calibration, leg.femur_servo_id, femur_ticks)
            .unwrap_or(0.0),
        tibia_deg: servo_semantic_angle_deg(config, calibration, leg.tibia_servo_id, tibia_ticks)
            .unwrap_or(0.0),
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedLegPose {
    coxa_ticks: u16,
    femur_ticks: u16,
    tibia_ticks: u16,
}

#[derive(Debug, Clone, Copy)]
struct DerivedTripodProfile {
    coxa_swing_deg: f32,
    femur_lift_deg: f32,
    tibia_lift_deg: f32,
}

#[derive(Debug, Clone, Copy)]
struct TiltedStandLegGeometry {
    semantic: LegPoseAngles,
    foot_x_cm: f32,
    foot_y_cm: f32,
    reach_cm: f32,
    height_cm: f32,
}

fn semantic_pose_from_base_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    leg: &arachno_core::LegConfig,
    fallback_kind: SemanticPoseKind,
) -> LegPoseAngles {
    let fallback = configured_pose_angles(config, calibration, leg, fallback_kind);
    LegPoseAngles {
        coxa_deg: base_pose
            .get(&leg.coxa_servo_id)
            .and_then(|ticks| {
                servo_semantic_angle_deg(config, calibration, leg.coxa_servo_id, *ticks)
            })
            .unwrap_or(fallback.coxa_deg),
        femur_deg: base_pose
            .get(&leg.femur_servo_id)
            .and_then(|ticks| {
                servo_semantic_angle_deg(config, calibration, leg.femur_servo_id, *ticks)
            })
            .unwrap_or(fallback.femur_deg),
        tibia_deg: base_pose
            .get(&leg.tibia_servo_id)
            .and_then(|ticks| {
                servo_semantic_angle_deg(config, calibration, leg.tibia_servo_id, *ticks)
            })
            .unwrap_or(fallback.tibia_deg),
    }
}

fn tilted_stand_leg_geometry(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    leg: &arachno_core::LegConfig,
) -> TiltedStandLegGeometry {
    let semantic = semantic_pose_from_base_pose(
        config,
        calibration,
        base_pose,
        leg,
        SemanticPoseKind::StandReference,
    );
    let top_view = leg.top_view_pose(semantic.coxa_deg, semantic.femur_deg, semantic.tibia_deg);
    let side_view = leg.side_view_pose(semantic.femur_deg, semantic.tibia_deg);
    TiltedStandLegGeometry {
        semantic,
        foot_x_cm: top_view.tibia_end.x,
        foot_y_cm: top_view.tibia_end.y,
        reach_cm: (side_view.tibia_end.x - side_view.coxa_end.x).abs(),
        height_cm: side_view.tibia_end.y - side_view.coxa_end.y,
    }
}

fn derive_tripod_profile(
    config: &RobotConfig,
    leg: &arachno_core::LegConfig,
    stance_pose: LegPoseAngles,
    lift_mode: TripodLiftMode,
) -> DerivedTripodProfile {
    let gait = &config.locomotion.tripod;
    let stand_side = leg.side_view_pose(stance_pose.femur_deg, stance_pose.tibia_deg);
    let horizontal_reach_cm = (stand_side.tibia_end.x - stand_side.coxa_end.x)
        .abs()
        .max(1.0);
    let coxa_radius_cm = (leg.coxa_length_cm() + horizontal_reach_cm).max(1.0);
    let derived_stride_cm = (horizontal_reach_cm * 0.28).clamp(4.0, 8.0);
    let stride_cm = derived_stride_cm.max(legacy_semantic_delta_deg(gait.stride_ticks));
    let coxa_ratio = ((stride_cm * 0.5) / coxa_radius_cm).clamp(0.0, 0.45);
    let derived_coxa_swing_deg = coxa_ratio.asin().to_degrees().clamp(6.0, 18.0);

    let desired_step_height_cm = lift_mode.target_step_height_cm(config, leg);
    let (derived_femur_lift_deg, derived_tibia_lift_deg, _achieved_step_height_cm) =
        derive_leg_lift_deltas(leg, stance_pose, desired_step_height_cm);

    DerivedTripodProfile {
        coxa_swing_deg: derived_coxa_swing_deg.max(legacy_semantic_delta_deg(gait.stride_ticks)),
        femur_lift_deg: derived_femur_lift_deg
            .max(legacy_semantic_delta_deg(gait.femur_lift_ticks)),
        tibia_lift_deg: derived_tibia_lift_deg
            .max(legacy_semantic_delta_deg(gait.tibia_lift_ticks)),
    }
}

fn derive_leg_lift_deltas(
    leg: &arachno_core::LegConfig,
    stance_pose: LegPoseAngles,
    target_step_height_cm: f32,
) -> (f32, f32, f32) {
    let stand_pose = leg.side_view_pose(stance_pose.femur_deg, stance_pose.tibia_deg);
    let stand_tip = stand_pose.tibia_end;

    if let Some(solution) = direct_leg_lift_deltas(leg, stance_pose, target_step_height_cm) {
        return solution;
    }

    let mut best = None::<(f32, f32, f32, f32)>;

    for femur_lift_deg in (4..=56).map(|value| value as f32) {
        for tibia_lift_deg in (6..=88).map(|value| value as f32) {
            let lifted_pose = leg.side_view_pose(
                stance_pose.femur_deg + femur_lift_deg,
                stance_pose.tibia_deg + tibia_lift_deg,
            );
            let step_height_cm = (stand_tip.y - lifted_pose.tibia_end.y).max(0.0);
            let horizontal_shift_cm = (lifted_pose.tibia_end.x - stand_tip.x).abs();
            let shape_bias = (tibia_lift_deg - femur_lift_deg * 1.6).abs();
            let shortfall = (target_step_height_cm - step_height_cm).max(0.0);
            let overshoot = (step_height_cm - target_step_height_cm).max(0.0);
            let cost =
                shortfall * 4.0 + overshoot * 1.5 + horizontal_shift_cm * 0.7 + shape_bias * 0.08;
            let candidate = (cost, femur_lift_deg, tibia_lift_deg, step_height_cm);
            if best.map(|current| cost < current.0).unwrap_or(true) {
                best = Some(candidate);
            }
        }
    }

    best.map(|(_, femur_lift_deg, tibia_lift_deg, achieved_height_cm)| {
        (femur_lift_deg, tibia_lift_deg, achieved_height_cm)
    })
    .unwrap_or((12.0, 18.0, 0.0))
}

fn direct_leg_lift_deltas(
    leg: &arachno_core::LegConfig,
    stance_pose: LegPoseAngles,
    target_step_height_cm: f32,
) -> Option<(f32, f32, f32)> {
    let stand_pose = leg.side_view_pose(stance_pose.femur_deg, stance_pose.tibia_deg);
    let reach_cm = (stand_pose.tibia_end.x - stand_pose.coxa_end.x).abs();
    let height_cm = stand_pose.tibia_end.y - stand_pose.coxa_end.y;
    let target_height_cm = height_cm - target_step_height_cm.max(0.0);
    let lifted_pose = leg
        .foot_to_angles_2d(stance_pose.coxa_deg, reach_cm, target_height_cm)
        .ok()?;
    let lifted_side = leg.side_view_pose(lifted_pose.femur_deg, lifted_pose.tibia_deg);
    let achieved_height_cm = (stand_pose.tibia_end.y - lifted_side.tibia_end.y).max(0.0);
    Some((
        lifted_pose.femur_deg - stance_pose.femur_deg,
        lifted_pose.tibia_deg - stance_pose.tibia_deg,
        achieved_height_cm,
    ))
}

fn legacy_semantic_delta_deg(delta_ticks: i16) -> f32 {
    delta_ticks.abs() as f32 * 360.0 / 4096.0
}

fn semantic_ticks_to_degrees(delta_ticks: i32, sign: i16) -> f32 {
    delta_ticks as f32 * 360.0 / 4096.0 / sign as f32
}

fn leg_cycle_shape_deg(phase: f32, coxa_swing_deg: f32) -> (f32, f32) {
    if phase < 0.5 {
        let t = phase / 0.5;
        let eased = smoothstep(t);
        let coxa = lerp_f32(-coxa_swing_deg, coxa_swing_deg, eased);
        let lift = (std::f32::consts::PI * t).sin().max(0.0);
        (coxa, lift)
    } else {
        let t = (phase - 0.5) / 0.5;
        let coxa = lerp_f32(coxa_swing_deg, -coxa_swing_deg, t);
        (coxa, 0.0)
    }
}

fn calibration_ticks_per_degree(sign: i16) -> f32 {
    4096.0 / 360.0 * sign as f32
}

fn calibration_reference_degrees(
    joint_key: &str,
    reference_key: CalibrationReferenceKey,
) -> Option<f32> {
    let joint = calibration_joint_infos()
        .into_iter()
        .find(|joint| joint.key == joint_key)?;
    Some(match reference_key {
        CalibrationReferenceKey::Negative => joint.negative_deg,
        CalibrationReferenceKey::Zero => joint.zero_deg,
        CalibrationReferenceKey::Positive => joint.positive_deg,
    })
}

fn servo_semantic_metadata(
    config: &RobotConfig,
    servo_id: u8,
) -> Option<(&arachno_core::LegConfig, u16, i16, &'static str)> {
    let leg = config.legs.iter().find(|leg| {
        leg.coxa_servo_id == servo_id
            || leg.femur_servo_id == servo_id
            || leg.tibia_servo_id == servo_id
    })?;

    let (reference_ticks, sign, joint_key) = if leg.coxa_servo_id == servo_id {
        (
            leg.coxa_zero_reference_ticks(),
            leg.coxa_forward_sign(),
            "coxa",
        )
    } else if leg.femur_servo_id == servo_id {
        (
            leg.femur_zero_reference_ticks(),
            leg.femur_lift_sign(),
            "femur",
        )
    } else {
        (
            leg.tibia_zero_reference_ticks(),
            leg.tibia_lift_sign(),
            "tibia",
        )
    };

    Some((leg, reference_ticks, sign, joint_key))
}

fn servo_calibration_implied_zeroes(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    servo_id: u8,
) -> Option<Vec<f32>> {
    let (_leg, _reference_ticks, sign, joint_key) = servo_semantic_metadata(config, servo_id)?;
    let entry = calibration.entry(servo_id)?;
    let slope = calibration_ticks_per_degree(sign);
    let mut zeroes = Vec::new();
    for (reference_key, ticks) in [
        (CalibrationReferenceKey::Negative, entry.negative_ticks),
        (CalibrationReferenceKey::Zero, entry.zero_ticks),
        (CalibrationReferenceKey::Positive, entry.positive_ticks),
    ] {
        if let (Some(ticks), Some(degrees)) = (
            ticks,
            calibration_reference_degrees(joint_key, reference_key),
        ) {
            zeroes.push(ticks as f32 - degrees * slope);
        }
    }
    (!zeroes.is_empty()).then_some(zeroes)
}

fn servo_zero_reference_tick(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    servo_id: u8,
) -> Option<f32> {
    let (_leg, reference_ticks, _sign, _joint_key) = servo_semantic_metadata(config, servo_id)?;
    let Some(zeroes) = servo_calibration_implied_zeroes(config, calibration, servo_id) else {
        return Some(reference_ticks as f32);
    };
    if zeroes.is_empty() {
        return Some(reference_ticks as f32);
    }
    Some(zeroes.iter().sum::<f32>() / zeroes.len() as f32)
}

fn servo_calibration_reference_error_ticks(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    servo_id: u8,
) -> Option<f32> {
    let zeroes = servo_calibration_implied_zeroes(config, calibration, servo_id)?;
    if zeroes.is_empty() {
        return None;
    }
    let average = zeroes.iter().sum::<f32>() / zeroes.len() as f32;
    Some(
        zeroes
            .into_iter()
            .map(|value| (value - average).abs())
            .fold(0.0, f32::max),
    )
}

fn servo_semantic_angle_deg(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    servo_id: u8,
    present_ticks: u16,
) -> Option<f32> {
    let (_leg, _reference_ticks, sign, _joint_key) = servo_semantic_metadata(config, servo_id)?;
    let zero_tick = servo_zero_reference_tick(config, calibration, servo_id)?;
    Some(semantic_ticks_to_degrees(
        (present_ticks as f32 - zero_tick).round() as i32,
        sign,
    ))
}

fn servo_ticks_for_semantic_angle_deg(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    servo_id: u8,
    degrees: f32,
) -> Option<u16> {
    let (_leg, _reference_ticks, sign, _joint_key) = servo_semantic_metadata(config, servo_id)?;
    let zero_tick = servo_zero_reference_tick(config, calibration, servo_id)?;
    Some(
        (zero_tick + degrees * calibration_ticks_per_degree(sign))
            .round()
            .clamp(0.0, 4095.0) as u16,
    )
}

fn pose_to_commands(pose: &BTreeMap<u8, u16>) -> Vec<JointCommand> {
    pose.iter()
        .map(|(&servo_id, &position_ticks)| JointCommand {
            servo_id,
            position_ticks,
            speed_ticks: 200,
            acceleration: 10,
        })
        .collect()
}

fn tilted_stand_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    pitch_deg: f32,
    roll_deg: f32,
) -> (BTreeMap<u8, u16>, String) {
    let pitch_deg = clamp_tilted_stand_pitch_deg(pitch_deg);
    let roll_deg = clamp_tilted_stand_roll_deg(roll_deg);
    let pitch_tan = pitch_deg.to_radians().tan();
    let roll_tan = roll_deg.to_radians().tan();
    let geometries = config
        .legs
        .iter()
        .map(|leg| {
            (
                leg,
                tilted_stand_leg_geometry(config, calibration, base_pose, leg),
            )
        })
        .collect::<Vec<_>>();
    let foot_count = geometries.len().max(1) as f32;
    let centroid_x = geometries
        .iter()
        .map(|(_, geometry)| geometry.foot_x_cm)
        .sum::<f32>()
        / foot_count;
    let centroid_y = geometries
        .iter()
        .map(|(_, geometry)| geometry.foot_y_cm)
        .sum::<f32>()
        / foot_count;

    let mut pose = base_pose.clone();
    let mut constrained_legs = Vec::new();

    for (leg, geometry) in geometries {
        let forward_cm = -(geometry.foot_y_cm - centroid_y);
        let right_cm = geometry.foot_x_cm - centroid_x;
        let body_height_offset_cm = forward_cm * pitch_tan + right_cm * roll_tan;
        let target_height_cm = geometry.height_cm - body_height_offset_cm;

        let Some(target_semantic) = leg
            .foot_to_angles_2d(
                geometry.semantic.coxa_deg,
                geometry.reach_cm,
                target_height_cm,
            )
            .ok()
        else {
            constrained_legs.push(leg.name.as_str());
            continue;
        };

        for (servo_id, degrees) in [
            (leg.coxa_servo_id, target_semantic.coxa_deg),
            (leg.femur_servo_id, target_semantic.femur_deg),
            (leg.tibia_servo_id, target_semantic.tibia_deg),
        ] {
            if let Some(ticks) =
                servo_ticks_for_semantic_angle_deg(config, calibration, servo_id, degrees)
            {
                pose.insert(servo_id, ticks);
            }
        }
    }

    let summary = if constrained_legs.is_empty() {
        format!("holding tilted stand at pitch {pitch_deg:+.1}° and roll {roll_deg:+.1}°")
    } else {
        format!(
            "holding tilted stand at pitch {pitch_deg:+.1}° and roll {roll_deg:+.1}°; {} leg(s) stayed at the captured stance due to IK limits",
            constrained_legs.len()
        )
    };

    (pose, summary)
}

fn walk_pose_from_base(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    phase: f32,
    mode: BrainMode,
    amplitude_scale: f32,
) -> BTreeMap<u8, u16> {
    let mut commanded = BTreeMap::new();

    for leg in &config.legs {
        let base_semantic = semantic_pose_from_base_pose(
            config,
            calibration,
            base_pose,
            leg,
            SemanticPoseKind::StandReference,
        );

        let leg_phase = if leg.is_tripod_a() {
            phase
        } else {
            (phase + 0.5).fract()
        };
        let profile = derive_tripod_profile(
            config,
            leg,
            base_semantic,
            mode.tripod_lift_mode().unwrap_or(TripodLiftMode::Standard),
        );
        let (coxa_delta_deg, lift_ratio) = leg_cycle_shape_deg(leg_phase, profile.coxa_swing_deg);
        let coxa_direction_sign =
            mode.coxa_gait_direction_for_leg(leg.is_left_side(), leg.coxa_zero_heading_deg());
        let gait_delta = LegPoseAngles {
            // Ramp in horizontal stride first; keep vertical lift fully active so the feet
            // still unload and clear the ground during walk startup.
            coxa_deg: coxa_delta_deg * coxa_direction_sign * amplitude_scale,
            femur_deg: profile.femur_lift_deg * lift_ratio,
            tibia_deg: profile.tibia_lift_deg * lift_ratio,
        };
        let target_semantic = LegPoseAngles {
            coxa_deg: base_semantic.coxa_deg + gait_delta.coxa_deg,
            femur_deg: base_semantic.femur_deg + gait_delta.femur_deg,
            tibia_deg: base_semantic.tibia_deg + gait_delta.tibia_deg,
        };

        if let Some(ticks) = servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.coxa_servo_id,
            target_semantic.coxa_deg,
        ) {
            commanded.insert(leg.coxa_servo_id, ticks);
        }
        if let Some(ticks) = servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.femur_servo_id,
            target_semantic.femur_deg,
        ) {
            commanded.insert(leg.femur_servo_id, ticks);
        }
        if let Some(ticks) = servo_ticks_for_semantic_angle_deg(
            config,
            calibration,
            leg.tibia_servo_id,
            target_semantic.tibia_deg,
        ) {
            commanded.insert(leg.tibia_servo_id, ticks);
        }
    }

    commanded
}

fn staged_stand_up_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
    target_kind: SemanticPoseKind,
) -> (BTreeMap<u8, u16>, String) {
    let target_pose = named_pose_with_calibration(config, calibration, target_kind);
    let (hold_label, align_label) = stand_pose_labels(target_kind);
    let duration = config.locomotion.stand_up.duration_seconds.max(0.5);
    let progress = (elapsed / duration).clamp(0.0, 1.0);

    if progress >= 1.0 {
        return (
            target_pose,
            format!("holding the configured {hold_label} pose"),
        );
    }

    let lay_down_pose = named_pose_with_calibration(config, calibration, SemanticPoseKind::LayDown);
    let femur_lift_pose = femur_lift_pose(config, &lay_down_pose, base_pose);
    let foot_plant_pose = foot_plant_pose(config, &lay_down_pose, base_pose, &femur_lift_pose);
    let body_raise_pose = body_raise_pose(config, &lay_down_pose, base_pose, &target_pose);

    let femur_phase = (duration * STAND_UP_FEMUR_PREP_RATIO).max(0.1);
    let tibia_phase = (duration * STAND_UP_TIBIA_PLANT_RATIO).max(0.1);
    let body_phase = (duration * STAND_UP_BODY_RISE_RATIO).max(0.1);
    let coxa_phase = (duration - femur_phase - tibia_phase - body_phase).max(0.1);

    if elapsed < femur_phase {
        let phase_progress = smoothstep((elapsed / femur_phase).clamp(0.0, 1.0));
        (
            interpolate_pose(base_pose, &femur_lift_pose, phase_progress),
            format!(
                "raising femurs to lift tibia joints ({:.0}%)",
                phase_progress * 100.0
            ),
        )
    } else if elapsed < femur_phase + tibia_phase {
        let phase_elapsed = elapsed - femur_phase;
        let phase_progress = smoothstep((phase_elapsed / tibia_phase).clamp(0.0, 1.0));
        (
            interpolate_pose(&femur_lift_pose, &foot_plant_pose, phase_progress),
            format!(
                "lowering tibias to plant feet ({:.0}%)",
                phase_progress * 100.0
            ),
        )
    } else if elapsed < femur_phase + tibia_phase + body_phase {
        let phase_elapsed = elapsed - femur_phase - tibia_phase;
        let phase_progress = smoothstep((phase_elapsed / body_phase).clamp(0.0, 1.0));
        (
            interpolate_pose(&foot_plant_pose, &body_raise_pose, phase_progress),
            format!(
                "raising body with planted feet ({:.0}%)",
                phase_progress * 100.0
            ),
        )
    } else {
        let phase_elapsed = elapsed - femur_phase - tibia_phase - body_phase;
        let phase_progress = smoothstep((phase_elapsed / coxa_phase).clamp(0.0, 1.0));
        (
            interpolate_pose(&body_raise_pose, &target_pose, phase_progress),
            format!(
                "aligning coxae into {align_label} ({:.0}%)",
                phase_progress * 100.0
            ),
        )
    }
}

fn stand_pose_labels(kind: SemanticPoseKind) -> (&'static str, &'static str) {
    match kind {
        SemanticPoseKind::StandReference => ("stand-reference", "stand"),
        SemanticPoseKind::StandHigh => ("stand-high", "high stand"),
        SemanticPoseKind::LayDown => ("lay-down", "lay-down"),
        SemanticPoseKind::ZeroPose => ("zero", "zero"),
        SemanticPoseKind::SitDown => ("sit-down", "sit-down"),
    }
}

fn lay_down_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
) -> (BTreeMap<u8, u16>, String) {
    let target = named_pose_with_calibration(config, calibration, SemanticPoseKind::LayDown);
    let duration = config.locomotion.lay_down.duration_seconds.max(0.5);
    let progress = (elapsed / duration).clamp(0.0, 1.0);
    let summary = if progress < 1.0 {
        format!("laying down ({:.0}%)", progress * 100.0)
    } else {
        "holding the configured lay-down pose".to_owned()
    };
    (
        interpolate_pose(base_pose, &target, smoothstep(progress)),
        summary,
    )
}

fn sit_down_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
) -> (BTreeMap<u8, u16>, String) {
    let target = named_pose_with_calibration(config, calibration, SemanticPoseKind::SitDown);
    let duration = config.locomotion.sit_down.duration_seconds.max(0.5);
    let progress = (elapsed / duration).clamp(0.0, 1.0);
    let summary = if progress < 1.0 {
        format!("sitting down ({:.0}%)", progress * 100.0)
    } else {
        "holding the configured sit-down pose".to_owned()
    };
    (
        interpolate_pose(base_pose, &target, smoothstep(progress)),
        summary,
    )
}

fn stand_settle_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
    target_kind: SemanticPoseKind,
) -> (BTreeMap<u8, u16>, String) {
    let (hold_label, _) = stand_pose_labels(target_kind);
    let settle = config.locomotion.stand.settle_seconds.max(0.25);
    let progress = (elapsed / settle).clamp(0.0, 1.0);
    let summary = if progress < 1.0 {
        format!(
            "settling into the configured {hold_label} pose ({:.0}%)",
            progress * 100.0
        )
    } else {
        format!("holding the configured {hold_label} pose")
    };
    let pose = interpolate_pose(
        base_pose,
        &named_pose_with_calibration(config, calibration, target_kind),
        smoothstep(progress),
    );
    (pose, summary)
}

fn tripod_gait_pose(
    config: &RobotConfig,
    calibration: &SemanticCalibrationState,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
    mode: BrainMode,
    walk_seconds: Option<f32>,
) -> (BTreeMap<u8, u16>, String) {
    let settle = config.locomotion.tripod.settle_seconds.max(0.25);
    let gait_label = mode
        .tripod_motion_summary_label()
        .unwrap_or_else(|| "tripod".to_owned());

    if elapsed < settle {
        let progress = (elapsed / settle).clamp(0.0, 1.0);
        let summary = format!(
            "holding the measured stand pose before {} gait ({:.0}%)",
            gait_label,
            progress * 100.0
        );
        (base_pose.clone(), summary)
    } else if walk_seconds.is_some_and(|limit| elapsed - settle >= limit.max(0.0)) {
        let gait_elapsed = (elapsed - settle).max(0.0);
        let limit = walk_seconds.unwrap_or_default();
        let summary = format!(
            "{} gait duration reached after {:.1}s / {:.1}s; holding the measured stand pose",
            gait_label, gait_elapsed, limit
        );
        (base_pose.clone(), summary)
    } else {
        let gait_elapsed = elapsed - settle;
        let cycle_seconds = config.locomotion.tripod.cycle_seconds.max(0.5);
        let startup_blend = config.locomotion.tripod.startup_blend_seconds.max(0.0);
        let phase = (gait_elapsed / cycle_seconds).fract();
        let amplitude_scale = if startup_blend <= f32::EPSILON {
            1.0
        } else {
            smoothstep((gait_elapsed / startup_blend).clamp(0.0, 1.0))
        };
        let summary = format!(
            "slow tripod {} gait active; phase {:.2} / cycle {:.1}s / blend {:.0}%",
            gait_label,
            phase,
            cycle_seconds,
            amplitude_scale * 100.0,
        );
        let pose =
            walk_pose_from_base(config, calibration, base_pose, phase, mode, amplitude_scale);
        (pose, summary)
    }
}

fn femur_lift_pose(
    config: &RobotConfig,
    lay_down_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
) -> BTreeMap<u8, u16> {
    let femur_ticks = stand_up_femur_prep_ticks(config);
    let mut pose = base_pose.clone();

    for leg in &config.legs {
        let base_femur = base_pose
            .get(&leg.femur_servo_id)
            .copied()
            .or_else(|| lay_down_pose.get(&leg.femur_servo_id).copied())
            .unwrap_or_else(|| leg.femur_zero_reference_ticks());
        pose.insert(
            leg.femur_servo_id,
            offset_ticks(base_femur, leg.femur_lift_sign() * femur_ticks),
        );
    }

    pose
}

fn foot_plant_pose(
    config: &RobotConfig,
    lay_down_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    femur_lift_pose: &BTreeMap<u8, u16>,
) -> BTreeMap<u8, u16> {
    let tibia_ticks = stand_up_tibia_plant_ticks(config);
    let mut pose = femur_lift_pose.clone();

    for leg in &config.legs {
        let base_tibia = base_pose
            .get(&leg.tibia_servo_id)
            .copied()
            .or_else(|| lay_down_pose.get(&leg.tibia_servo_id).copied())
            .unwrap_or_else(|| leg.tibia_zero_reference_ticks());
        pose.insert(
            leg.tibia_servo_id,
            offset_ticks(base_tibia, -leg.tibia_lift_sign() * tibia_ticks),
        );
    }

    pose
}

fn body_raise_pose(
    config: &RobotConfig,
    lay_down_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    stand_reference_pose: &BTreeMap<u8, u16>,
) -> BTreeMap<u8, u16> {
    let mut pose = stand_reference_pose.clone();

    for leg in &config.legs {
        let base_coxa = base_pose
            .get(&leg.coxa_servo_id)
            .copied()
            .or_else(|| lay_down_pose.get(&leg.coxa_servo_id).copied())
            .unwrap_or_else(|| leg.coxa_zero_reference_ticks());
        pose.insert(leg.coxa_servo_id, base_coxa);
    }

    pose
}

fn stand_up_femur_prep_ticks(config: &RobotConfig) -> i16 {
    (config.locomotion.tripod.femur_lift_ticks.abs().max(12)) * 6
}

fn stand_up_tibia_plant_ticks(config: &RobotConfig) -> i16 {
    (config.locomotion.tripod.tibia_lift_ticks.abs().max(18)) * 5
}

fn interpolate_pose(
    start: &BTreeMap<u8, u16>,
    end: &BTreeMap<u8, u16>,
    t: f32,
) -> BTreeMap<u8, u16> {
    let t = t.clamp(0.0, 1.0);
    let mut pose = BTreeMap::new();
    let all_ids: std::collections::BTreeSet<u8> = start.keys().chain(end.keys()).copied().collect();
    for servo_id in all_ids {
        let start_ticks = start.get(&servo_id).copied().unwrap_or(0);
        let end_ticks = end.get(&servo_id).copied().unwrap_or(start_ticks);
        let interpolated = start_ticks as f32 + (end_ticks as f32 - start_ticks as f32) * t;
        pose.insert(servo_id, interpolated.round().clamp(0.0, 4095.0) as u16);
    }
    pose
}

fn offset_ticks(start_ticks: u16, delta_ticks: i16) -> u16 {
    (i32::from(start_ticks) + i32::from(delta_ticks)).clamp(0, 4095) as u16
}

fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t
}

fn clamp_tilted_stand_pitch_deg(pitch_deg: f32) -> f32 {
    pitch_deg.clamp(-TILTED_STAND_PITCH_LIMIT_DEG, TILTED_STAND_PITCH_LIMIT_DEG)
}

fn clamp_tilted_stand_roll_deg(roll_deg: f32) -> f32 {
    roll_deg.clamp(-TILTED_STAND_ROLL_LIMIT_DEG, TILTED_STAND_ROLL_LIMIT_DEG)
}

fn is_reopen_error(message: &str) -> bool {
    message.contains("failed to open")
        || message.contains("No such file")
        || message.contains("Input/output error")
        || message.contains("Broken pipe")
}

fn sleep_remaining(started_at: Instant, period: Duration) {
    if let Some(remaining) = period.checked_sub(started_at.elapsed()) {
        thread::sleep(remaining);
    }
}

fn write_state(shared: &Arc<RwLock<TelemetryState>>, next: TelemetryState) {
    if let Ok(mut state) = shared.write() {
        *state = next;
    }
}

async fn index(State(state): State<AppState>) -> Html<String> {
    if state.dashboard_enabled {
        return Html(dashboard_page::DASHBOARD_HTML.to_owned());
    }

    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>arachno-brain</title><body style=\"font-family: sans-serif; max-width: 48rem; margin: 2rem auto; line-height: 1.5;\"><h1>arachno-brain</h1><p>The hardware-owning brain process is running for <strong>{}</strong>.</p><ul><li><a href=\"/api/state\">/api/state</a> returns live robot telemetry as JSON.</li><li><a href=\"/camera.mjpg\">/camera.mjpg</a> exposes the live camera stream for the host USB profile.</li><li><a href=\"/dashboard\">/dashboard</a> serves the built-in UI when the process is started with <code>--dashboard</code>.</li></ul></body>",
        state.config.robot.name
    );
    Html(body)
}

async fn dashboard(State(state): State<AppState>) -> Result<Html<&'static str>, StatusCode> {
    if state.dashboard_enabled {
        Ok(Html(dashboard_page::DASHBOARD_HTML))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn api_state(State(state): State<AppState>) -> Result<Json<TelemetryState>, StatusCode> {
    state
        .shared
        .read()
        .map(|snapshot| Json(snapshot.clone()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn api_motion_command(
    State(state): State<AppState>,
    Json(req): Json<MotionCommandRequest>,
) -> Result<Json<MotionCommandResponse>, (StatusCode, String)> {
    let new_mode = req.command.as_brain_mode();
    state
        .pending_mode
        .write()
        .map(|mut pm| {
            *pm = Some(new_mode);
            Json(MotionCommandResponse {
                summary: format!("switching to {}", new_mode.as_state_label()),
                mode: new_mode.as_state_label().to_owned(),
            })
        })
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to acquire mode lock".to_owned(),
            )
        })
}

async fn api_manual_capture(
    State(state): State<AppState>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let pose = current_pose_from_shared_snapshot(&state)?;
    let summary = "captured the current robot pose as the manual control zero".to_owned();

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    manual.base_pose = Some(pose.clone());
    manual.target_pose = Some(pose);
    manual.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_manual_apply(
    State(state): State<AppState>,
    Json(request): Json<ManualApplyRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let (group_label, legs) = resolve_manual_group(&state.config, &request.group_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown manual control group"))?;
    let fallback_pose = current_pose_from_shared_snapshot(&state)?;
    let coxa_deg = request
        .coxa_deg
        .clamp(-MANUAL_COXA_LIMIT_DEG, MANUAL_COXA_LIMIT_DEG);
    let femur_deg = request
        .femur_deg
        .clamp(-MANUAL_FEMUR_LIMIT_DEG, MANUAL_FEMUR_LIMIT_DEG);
    let tibia_deg = request
        .tibia_deg
        .clamp(-MANUAL_TIBIA_LIMIT_DEG, MANUAL_TIBIA_LIMIT_DEG);

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    let mut target_pose = manual
        .target_pose
        .clone()
        .unwrap_or_else(|| fallback_pose.clone());
    let calibration = state.calibration.read().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "semantic calibration lock poisoned",
        )
    })?;

    for leg in legs {
        set_manual_joint_target(
            &state.config,
            &calibration,
            &mut target_pose,
            leg,
            "coxa",
            coxa_deg,
        );
        set_manual_joint_target(
            &state.config,
            &calibration,
            &mut target_pose,
            leg,
            "femur",
            femur_deg,
        );
        set_manual_joint_target(
            &state.config,
            &calibration,
            &mut target_pose,
            leg,
            "tibia",
            tibia_deg,
        );
    }

    let summary = format!(
        "manual target updated for {group_label}: coxa {coxa_deg:+.1}°, femur {femur_deg:+.1}°, tibia {tibia_deg:+.1}° absolute semantic"
    );
    manual.target_pose = Some(target_pose);
    manual.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_manual_reset(
    State(state): State<AppState>,
    payload: Option<Json<ManualResetRequest>>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let fallback_pose = current_pose_from_shared_snapshot(&state)?;
    let maybe_group_key = payload.and_then(|Json(request)| request.group_key);

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    let base_pose = manual
        .base_pose
        .clone()
        .unwrap_or_else(|| fallback_pose.clone());
    manual.base_pose = Some(base_pose.clone());

    let summary = if let Some(group_key) = maybe_group_key {
        let (group_label, legs) =
            resolve_manual_group(&state.config, &group_key).ok_or_else(|| {
                manual_api_error(StatusCode::BAD_REQUEST, "unknown manual control group")
            })?;
        let mut target_pose = manual
            .target_pose
            .clone()
            .unwrap_or_else(|| base_pose.clone());
        for leg in legs {
            reset_manual_leg_to_base(&mut target_pose, &base_pose, leg);
        }
        manual.target_pose = Some(target_pose);
        let summary = format!("manual target reset to zero for {group_label}");
        manual.summary = summary.clone();
        summary
    } else {
        manual.target_pose = Some(base_pose.clone());
        let summary = "manual target reset to zero for all legs".to_owned();
        manual.summary = summary.clone();
        summary
    };

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_manual_torque_limit(
    State(state): State<AppState>,
    Json(request): Json<ManualTorqueLimitRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let (group_label, _) = resolve_manual_group(&state.config, &request.group_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown manual control group"))?;
    let torque_limit = request.torque_limit.min(MANUAL_TORQUE_LIMIT_MAX);

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    manual
        .pending_actions
        .push_back(ManualHardwareAction::SetTorqueLimit {
            group_key: request.group_key.clone(),
            target: request.target,
            torque_limit,
        });
    let summary = format!(
        "queued torque limit {} for {} ({}); the control worker will sync current pose first",
        torque_limit,
        group_label,
        request.target.as_label()
    );
    manual.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_manual_jump(
    State(state): State<AppState>,
    Json(request): Json<ManualJumpRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let (group_label, legs) = resolve_manual_group(&state.config, &request.group_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown manual control group"))?;
    let current_pose = current_pose_from_shared_snapshot(&state)?;

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    let mut target_pose = manual
        .target_pose
        .clone()
        .unwrap_or_else(|| current_pose.clone());
    let calibration = state.calibration.read().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "semantic calibration lock poisoned",
        )
    })?;

    for leg in legs {
        let Some((servo_id, _sign)) = manual_joint_servo_and_sign(leg, &request.joint_key) else {
            return Err(manual_api_error(
                StatusCode::BAD_REQUEST,
                "unknown manual joint",
            ));
        };
        let current_ticks = current_pose.get(&servo_id).copied().ok_or_else(|| {
            manual_api_error(
                StatusCode::CONFLICT,
                "selected joint has no fresh live feedback to jump from",
            )
        })?;
        let current_deg =
            servo_semantic_angle_deg(&state.config, &calibration, servo_id, current_ticks)
                .ok_or_else(|| {
                    manual_api_error(
                        StatusCode::CONFLICT,
                        "selected joint has no semantic calibration mapping available",
                    )
                })?;
        let next_ticks = servo_ticks_for_semantic_angle_deg(
            &state.config,
            &calibration,
            servo_id,
            current_deg + request.delta_deg,
        )
        .ok_or_else(|| {
            manual_api_error(
                StatusCode::CONFLICT,
                "selected joint target could not be converted back into ticks",
            )
        })?;
        target_pose.insert(servo_id, next_ticks);
    }

    let summary = format!(
        "manual relative jump for {group_label}: {} {:+.1}° from each servo's live pose",
        request.joint_key, request.delta_deg
    );
    manual.target_pose = Some(target_pose);
    manual.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_manual_sync_current(
    State(state): State<AppState>,
    Json(request): Json<ManualSyncTargetRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_manual_enabled(&state)?;
    let (group_label, _) = resolve_manual_group(&state.config, &request.group_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown manual control group"))?;

    let mut manual = state.manual.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "manual control lock poisoned",
        )
    })?;
    manual
        .pending_actions
        .push_back(ManualHardwareAction::SyncTargetToCurrent {
            group_key: request.group_key.clone(),
        });
    let summary = format!(
        "queued live target sync for {}; the control worker will update the manual target to the current pose",
        group_label
    );
    manual.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_tilted_stand_apply(
    State(state): State<AppState>,
    Json(request): Json<TiltedStandApplyRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_tilted_stand_enabled(&state)?;
    let pitch_deg = clamp_tilted_stand_pitch_deg(request.pitch_deg);
    let roll_deg = clamp_tilted_stand_roll_deg(request.roll_deg);
    let summary =
        format!("tilted stand target updated: pitch {pitch_deg:+.1}°, roll {roll_deg:+.1}°");

    let mut tilted_stand = state.tilted_stand.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "tilted stand lock poisoned",
        )
    })?;
    tilted_stand.pitch_deg = pitch_deg;
    tilted_stand.roll_deg = roll_deg;
    tilted_stand.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_tilted_stand_reset(
    State(state): State<AppState>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_tilted_stand_enabled(&state)?;
    let summary = "tilted stand target reset to level (pitch +0.0°, roll +0.0°)".to_owned();

    let mut tilted_stand = state.tilted_stand.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "tilted stand lock poisoned",
        )
    })?;
    tilted_stand.pitch_deg = 0.0;
    tilted_stand.roll_deg = 0.0;
    tilted_stand.summary = summary.clone();

    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_calibration_capture(
    State(state): State<AppState>,
    Json(request): Json<CalibrationCaptureRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_calibration_enabled(&state)?;
    let pose = current_pose_from_shared_snapshot(&state)?;
    let legs = resolve_calibration_legs(&state.config, &request.leg_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown calibration leg"))?;

    let mut calibration = state.calibration.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "semantic calibration lock poisoned",
        )
    })?;
    for leg in &legs {
        let servo_id = resolve_servo_id_for_joint(leg, &request.joint_key).ok_or_else(|| {
            manual_api_error(StatusCode::BAD_REQUEST, "unknown calibration joint")
        })?;
        let ticks = pose.get(&servo_id).copied().ok_or_else(|| {
            manual_api_error(
                StatusCode::CONFLICT,
                "selected joint has no fresh live feedback to capture",
            )
        })?;
        calibration.set_reference(
            servo_id,
            &leg.name,
            &request.joint_key,
            request.reference_key,
            ticks,
        );
    }
    calibration.save().map_err(|err| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist semantic calibration: {err}"),
        )
    })?;

    let reference_label = match request.reference_key {
        CalibrationReferenceKey::Negative => "negative",
        CalibrationReferenceKey::Zero => "zero",
        CalibrationReferenceKey::Positive => "positive",
    };
    let summary = if request.leg_key == "all" {
        format!(
            "captured {reference_label} reference for all legs {} across {} servo(s)",
            request.joint_key,
            legs.len()
        )
    } else {
        let leg = legs[0];
        let servo_id = resolve_servo_id_for_joint(leg, &request.joint_key).ok_or_else(|| {
            manual_api_error(StatusCode::BAD_REQUEST, "unknown calibration joint")
        })?;
        let ticks = pose.get(&servo_id).copied().ok_or_else(|| {
            manual_api_error(
                StatusCode::CONFLICT,
                "selected joint has no fresh live feedback to capture",
            )
        })?;
        format!(
            "captured {reference_label} reference for {} {} at {} ticks",
            humanize_leg_name(&leg.name),
            request.joint_key,
            ticks
        )
    };
    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_calibration_clear(
    State(state): State<AppState>,
    Json(request): Json<CalibrationClearRequest>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_calibration_enabled(&state)?;
    let legs = resolve_calibration_legs(&state.config, &request.leg_key)
        .ok_or_else(|| manual_api_error(StatusCode::BAD_REQUEST, "unknown calibration leg"))?;

    let mut calibration = state.calibration.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "semantic calibration lock poisoned",
        )
    })?;
    for leg in &legs {
        let servo_id = resolve_servo_id_for_joint(leg, &request.joint_key).ok_or_else(|| {
            manual_api_error(StatusCode::BAD_REQUEST, "unknown calibration joint")
        })?;
        calibration.clear_servo(servo_id);
    }
    calibration.save().map_err(|err| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist semantic calibration: {err}"),
        )
    })?;

    let summary = if request.leg_key == "all" {
        format!(
            "cleared saved calibration for all legs {} across {} servo(s)",
            request.joint_key,
            legs.len()
        )
    } else {
        format!(
            "cleared saved calibration for {} {}",
            humanize_leg_name(&legs[0].name),
            request.joint_key
        )
    };
    Ok(Json(ManualCommandResponse { summary }))
}

async fn api_calibration_reload(
    State(state): State<AppState>,
) -> Result<Json<ManualCommandResponse>, (StatusCode, String)> {
    ensure_calibration_enabled(&state)?;

    let mut calibration = state.calibration.write().map_err(|_| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "semantic calibration lock poisoned",
        )
    })?;
    let entry_count = calibration.reload().map_err(|err| {
        manual_api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to reload semantic calibration: {err}"),
        )
    })?;
    let path = calibration
        .store_path_display()
        .unwrap_or_else(|| "<disabled>".to_owned());
    let summary = format!(
        "reloaded semantic calibration from {path} with {entry_count} servo entr{}",
        if entry_count == 1 { "y" } else { "ies" }
    );
    Ok(Json(ManualCommandResponse { summary }))
}

async fn camera_stream(State(state): State<AppState>) -> Response {
    if state.config.camera.backend != CameraBackend::V4l2 {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "camera streaming is currently implemented for the host-usb v4l2 backend",
        )
            .into_response();
    }

    let Some(device) = state.config.camera.device.as_deref() else {
        return (StatusCode::BAD_REQUEST, "camera device missing from config").into_response();
    };

    let mut command = Command::new("ffmpeg");
    command
        .args(ffmpeg_camera_args(&state.config))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to start ffmpeg for {}: {err}", device),
            )
                .into_response();
        }
    };

    let Some(stdout) = child.stdout.take() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "ffmpeg did not provide a stdout stream",
        )
            .into_response();
    };

    let stream = ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=ffmpeg",
        )
        .body(body)
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to build response",
            )
                .into_response()
        })
}

fn ffmpeg_camera_args(config: &RobotConfig) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-f".to_owned(),
        "video4linux2".to_owned(),
    ];

    let pixel_format = config.camera.pixel_format.to_ascii_lowercase();
    if pixel_format == "mjpg" || pixel_format == "mjpeg" {
        args.push("-input_format".to_owned());
        args.push("mjpeg".to_owned());
    } else {
        args.push("-input_format".to_owned());
        args.push(pixel_format);
    }

    args.push("-video_size".to_owned());
    args.push(format!("{}x{}", config.camera.width, config.camera.height));
    args.push("-framerate".to_owned());
    args.push(config.camera.fps.to_string());
    args.push("-i".to_owned());
    args.push(
        config
            .camera
            .device
            .clone()
            .unwrap_or_else(|| "/dev/video0".to_owned()),
    );
    args.push("-vf".to_owned());
    args.push(format!("fps={}", config.camera.fps.min(10)));
    args.push("-q:v".to_owned());
    args.push("7".to_owned());
    args.push("-f".to_owned());
    args.push("mpjpeg".to_owned());
    args.push("pipe:1".to_owned());
    args
}

fn servo_labels(config: &RobotConfig) -> BTreeMap<u8, String> {
    let mut labels = BTreeMap::new();
    for leg in &config.legs {
        labels.insert(leg.coxa_servo_id, format!("{} / coxa", leg.name));
        labels.insert(leg.femur_servo_id, format!("{} / femur", leg.name));
        labels.insert(leg.tibia_servo_id, format!("{} / tibia", leg.name));
    }
    labels
}

fn telemetry_imu_from_config(config: &RobotConfig) -> Option<TelemetryImuState> {
    let imu = config.imu.as_ref()?;
    Some(TelemetryImuState {
        enabled: imu.enabled,
        mode: imu.mode.clone(),
        device: imu.device.clone(),
        sensor_kind: None,
        sample_hz: Some(imu.sample_hz),
        spi_mode: None,
        observed_who_am_i: None,
        description: None,
        last_error: if imu.enabled {
            Some("waiting for IMU bridge".to_owned())
        } else {
            Some("disabled in config".to_owned())
        },
        telemetry: None,
        roll_deg: None,
        pitch_deg: None,
        accel_norm_mps2: None,
        gyro_norm_deg_s: None,
    })
}

fn open_imu_bridge(state: &mut TelemetryImuState) -> anyhow::Result<UsbImuBridge> {
    let device = state
        .device
        .clone()
        .context("IMU is enabled, but no device is configured")?;
    let mut bridge = UsbImuBridge::open(&device, IMU_BRIDGE_BAUD_RATE)
        .with_context(|| format!("failed to open {device}"))?;

    state.description = Some(bridge.description().to_owned());
    match bridge.probe_device_info(Duration::from_millis(IMU_PROBE_TIMEOUT_MS))? {
        DeviceInfoProbe::Info(info) => {
            state.sensor_kind = Some(sensor_kind_label(info.sensor_kind).to_owned());
            state.sample_hz = Some(info.sample_hz);
            state.spi_mode =
                (info.spi_mode != arachno_imu_host::SPI_MODE_UNKNOWN).then_some(info.spi_mode);
            state.observed_who_am_i =
                (info.observed_who_am_i != 0).then_some(info.observed_who_am_i);
            state.last_error = (info.fault_code != arachno_imu_host::SENSOR_FAULT_NONE)
                .then(|| format!("backend fault: {}", imu_fault_label(info.fault_code)));
        }
        DeviceInfoProbe::StreamingWithoutInfo => {
            state.last_error =
                Some("IMU samples are streaming, but firmware info was not seen".to_owned());
        }
        DeviceInfoProbe::Silent => {
            state.last_error = Some("timed out waiting for IMU firmware info".to_owned());
        }
    }
    bridge.start()?;
    Ok(bridge)
}

fn drain_imu_bridge(
    bridge: &mut UsbImuBridge,
    state: &mut TelemetryImuState,
) -> anyhow::Result<()> {
    let mut latest = state.telemetry.clone();
    let mut received_sample = false;

    for _ in 0..16 {
        match bridge.next_sample()? {
            Some(sample) => {
                received_sample = true;
                latest = Some(sample);
            }
            None => break,
        }
    }

    if let Some(sample) = latest {
        let (roll_deg, pitch_deg) = estimate_roll_pitch_deg(sample.accel_mps2);
        state.accel_norm_mps2 = Some(vector_norm3(sample.accel_mps2));
        state.gyro_norm_deg_s =
            Some(vector_norm3(sample.gyro_rad_s) * 180.0 / std::f32::consts::PI);
        state.roll_deg = Some(roll_deg);
        state.pitch_deg = Some(pitch_deg);
        state.telemetry = Some(sample);
        if received_sample {
            state.last_error = None;
        }
    }

    Ok(())
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

fn imu_fault_label(code: u8) -> &'static str {
    match code {
        arachno_imu_host::SENSOR_FAULT_NONE => "none",
        arachno_imu_host::SENSOR_FAULT_PROBE_NO_RESPONSE => "probe_no_response",
        arachno_imu_host::SENSOR_FAULT_UNEXPECTED_WHO_AM_I => "unexpected_who_am_i",
        arachno_imu_host::SENSOR_FAULT_READ => "read_fault",
        _ => "unknown",
    }
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

fn ticks_to_deg(ticks: u16) -> f32 {
    ticks as f32 * 360.0 / 4096.0
}

fn speed_ticks_to_rpm(speed_ticks: i16) -> f32 {
    speed_ticks as f32 * 60.0 / 4096.0
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    use super::{
        BODY_ATTITUDE_STRIKES_TO_TRIP, BrainMode, LegPoseAngles, ManualControlState, MotionCommand,
        MotionRuntime, SemanticCalibrationState, TelemetryImuState, body_attitude_fault_reason,
        clamp_tilted_stand_pitch_deg, clamp_tilted_stand_roll_deg, derive_leg_lift_deltas,
        named_pose_with_calibration, stand_pose_labels, sync_manual_mode_state,
        tilted_stand_leg_geometry, tilted_stand_pose,
    };
    use arachno_core::{LegConfig, RobotConfig, SafetyConfig, SemanticPoseKind};

    fn make_tripod_test_leg() -> LegConfig {
        LegConfig {
            name: "front_right".to_owned(),
            coxa_servo_id: 41,
            femur_servo_id: 42,
            tibia_servo_id: 43,
            coxa_stand_reference_ticks: None,
            femur_stand_reference_ticks: None,
            tibia_stand_reference_ticks: None,
            coxa_lay_down_ticks: None,
            femur_lay_down_ticks: None,
            tibia_lay_down_ticks: None,
            coxa_zero_reference_ticks: None,
            femur_zero_reference_ticks: None,
            tibia_zero_reference_ticks: None,
            coxa_forward_sign: 1,
            femur_lift_sign: 1,
            tibia_lift_sign: -1,
            coxa_zero_heading_deg: Some(45.0),
            coxa_length_cm: Some(3.0),
            femur_length_cm: Some(8.5),
            tibia_length_cm: Some(14.5),
        }
    }

    fn make_test_imu_state(roll_deg: Option<f32>, pitch_deg: Option<f32>) -> TelemetryImuState {
        TelemetryImuState {
            enabled: true,
            mode: "test".to_owned(),
            device: None,
            sensor_kind: None,
            sample_hz: None,
            spi_mode: None,
            observed_who_am_i: None,
            description: None,
            last_error: None,
            telemetry: None,
            roll_deg,
            pitch_deg,
            accel_norm_mps2: Some(9.81),
            gyro_norm_deg_s: None,
        }
    }

    fn load_test_robot_config() -> RobotConfig {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/robot/default.toml");
        RobotConfig::load_from_path(&config_path).expect("robot config should load for tests")
    }

    #[test]
    fn manual_motion_command_maps_to_manual_mode() {
        assert_eq!(MotionCommand::Manual.as_brain_mode(), BrainMode::Manual);
    }

    #[test]
    fn syncing_manual_mode_state_reinitializes_manual_control() {
        let manual = Arc::new(RwLock::new(ManualControlState {
            enabled: false,
            base_pose: Some(BTreeMap::from([(11, 2048u16)])),
            target_pose: Some(BTreeMap::from([(11, 2200u16)])),
            summary: "stale".to_owned(),
            pending_actions: Default::default(),
        }));

        sync_manual_mode_state(&manual, BrainMode::Manual);

        let control = manual.read().expect("manual control should lock");
        assert!(control.enabled);
        assert!(control.base_pose.is_none());
        assert!(control.target_pose.is_none());
        assert!(
            control
                .summary
                .contains("waiting for the current robot pose")
        );
    }

    #[test]
    fn stand_up_high_mode_targets_the_high_stand_pose() {
        assert_eq!(
            BrainMode::StandUpHigh.stand_transition_target(),
            Some(SemanticPoseKind::StandHigh)
        );
        assert_eq!(BrainMode::StandUpHigh.as_state_label(), "stand_up_high");
    }

    #[test]
    fn stand_up_high_motion_command_maps_to_the_new_mode() {
        assert_eq!(
            MotionCommand::StandUpHigh.as_brain_mode(),
            BrainMode::StandUpHigh
        );
    }

    #[test]
    fn stand_high_mode_targets_the_high_stand_pose() {
        assert_eq!(
            BrainMode::StandHigh.stand_settle_target(),
            Some(SemanticPoseKind::StandHigh)
        );
        assert_eq!(BrainMode::StandHigh.as_state_label(), "stand_high");
    }

    #[test]
    fn stand_high_motion_command_maps_to_the_new_mode() {
        assert_eq!(
            MotionCommand::StandHigh.as_brain_mode(),
            BrainMode::StandHigh
        );
    }

    #[test]
    fn stand_high_pose_labels_match_transition_copy() {
        assert_eq!(
            stand_pose_labels(SemanticPoseKind::StandHigh),
            ("stand-high", "high stand")
        );
    }

    #[test]
    fn high_step_walk_motion_commands_map_to_new_modes() {
        assert_eq!(
            MotionCommand::WalkForwardHigh.as_brain_mode(),
            BrainMode::SlowWalkHigh
        );
        assert_eq!(
            MotionCommand::WalkBackwardHigh.as_brain_mode(),
            BrainMode::BackwardWalkHigh
        );
        assert_eq!(
            MotionCommand::SidewalkLeftHigh.as_brain_mode(),
            BrainMode::SidewalkLeftHigh
        );
        assert_eq!(
            MotionCommand::SidewalkRightHigh.as_brain_mode(),
            BrainMode::SidewalkRightHigh
        );
        assert_eq!(
            MotionCommand::TiltedStand.as_brain_mode(),
            BrainMode::TiltedStand
        );
    }

    #[test]
    fn high_step_walk_modes_report_high_step_state_labels() {
        assert_eq!(BrainMode::SlowWalkHigh.as_state_label(), "slow_walk_high");
        assert_eq!(
            BrainMode::SlowWalkHigh
                .tripod_motion_summary_label()
                .as_deref(),
            Some("high-step forward")
        );
        assert_eq!(
            BrainMode::BackwardWalkHigh
                .tripod_motion_summary_label()
                .as_deref(),
            Some("high-step backward")
        );
        assert_eq!(
            BrainMode::SidewalkLeftHigh
                .tripod_motion_summary_label()
                .as_deref(),
            Some("high-step sidewalk left")
        );
        assert_eq!(
            BrainMode::SidewalkRightHigh
                .tripod_motion_summary_label()
                .as_deref(),
            Some("high-step sidewalk right")
        );
    }

    #[test]
    fn derive_leg_lift_deltas_reaches_the_requested_high_step_height() {
        let leg = make_tripod_test_leg();
        let stand_reference = LegPoseAngles {
            coxa_deg: 0.0,
            femur_deg: 50.8,
            tibia_deg: -121.6,
        };

        let (femur_delta_deg, tibia_delta_deg, achieved_height_cm) =
            derive_leg_lift_deltas(&leg, stand_reference, 10.0);

        assert!(
            femur_delta_deg.is_finite() && tibia_delta_deg.is_finite(),
            "expected finite lift deltas, got femur={femur_delta_deg}, tibia={tibia_delta_deg}"
        );
        assert!(
            achieved_height_cm >= 9.5,
            "expected about 10 cm of lift, got {achieved_height_cm:.2} cm"
        );
    }

    #[test]
    fn manual_mode_does_not_trip_body_attitude_faults() {
        let safety = SafetyConfig::default();
        let imu = make_test_imu_state(Some(20.4), None);
        let mut strikes = 0;

        for _ in 0..BODY_ATTITUDE_STRIKES_TO_TRIP {
            assert_eq!(
                body_attitude_fault_reason(BrainMode::Manual, &safety, Some(&imu), &mut strikes),
                None
            );
        }

        assert_eq!(strikes, 0);
    }

    #[test]
    fn automatic_modes_still_trip_body_attitude_faults() {
        let safety = SafetyConfig::default();
        let imu = make_test_imu_state(Some(20.4), None);
        let mut strikes = 0;

        for _ in 0..BODY_ATTITUDE_STRIKES_TO_TRIP - 1 {
            assert_eq!(
                body_attitude_fault_reason(BrainMode::Stand, &safety, Some(&imu), &mut strikes),
                None
            );
        }

        let reason =
            body_attitude_fault_reason(BrainMode::Stand, &safety, Some(&imu), &mut strikes)
                .expect("stand mode should still trip after consecutive roll strikes");
        assert!(reason.contains("body roll 20.4 deg exceeded limit 20.0 deg"));
        assert!(reason.contains("3 consecutive samples"));
    }

    #[test]
    fn manual_safety_status_reports_only_bus_and_temperature_monitoring() {
        let motion = MotionRuntime::new(BrainMode::Manual, None);
        assert_eq!(
            motion.safety_status(true),
            "manual control active; monitoring bus voltage and temperature"
        );
    }

    #[test]
    fn tilted_stand_limits_clamp_requested_angles() {
        assert_eq!(clamp_tilted_stand_pitch_deg(24.0), 20.0);
        assert_eq!(clamp_tilted_stand_pitch_deg(-24.0), -20.0);
        assert_eq!(clamp_tilted_stand_roll_deg(21.0), 20.0);
        assert_eq!(clamp_tilted_stand_roll_deg(-21.0), -20.0);
    }

    #[test]
    fn tilted_stand_pitch_lifts_front_relative_to_rear() {
        let config = load_test_robot_config();
        let calibration = SemanticCalibrationState::default();
        let base_pose =
            named_pose_with_calibration(&config, &calibration, SemanticPoseKind::StandReference);
        let (tilted_pose, _summary) =
            tilted_stand_pose(&config, &calibration, &base_pose, 8.0, 0.0);

        let mut front_delta = 0.0;
        let mut rear_delta = 0.0;
        let mut front_count = 0.0;
        let mut rear_count = 0.0;

        for leg in &config.legs {
            let base = tilted_stand_leg_geometry(&config, &calibration, &base_pose, leg);
            let tilted = tilted_stand_leg_geometry(&config, &calibration, &tilted_pose, leg);
            let delta = tilted.height_cm - base.height_cm;
            if leg.name.starts_with("front_") {
                front_delta += delta;
                front_count += 1.0;
            } else if leg.name.starts_with("rear_") {
                rear_delta += delta;
                rear_count += 1.0;
            }
        }

        assert!(front_count > 0.0 && rear_count > 0.0);
        assert!(front_delta / front_count < rear_delta / rear_count);
    }

    #[test]
    fn tilted_stand_roll_lifts_right_relative_to_left() {
        let config = load_test_robot_config();
        let calibration = SemanticCalibrationState::default();
        let base_pose =
            named_pose_with_calibration(&config, &calibration, SemanticPoseKind::StandReference);
        let (tilted_pose, _summary) =
            tilted_stand_pose(&config, &calibration, &base_pose, 0.0, 8.0);

        let mut left_delta = 0.0;
        let mut right_delta = 0.0;
        let mut left_count = 0.0;
        let mut right_count = 0.0;

        for leg in &config.legs {
            let base = tilted_stand_leg_geometry(&config, &calibration, &base_pose, leg);
            let tilted = tilted_stand_leg_geometry(&config, &calibration, &tilted_pose, leg);
            let delta = tilted.height_cm - base.height_cm;
            if leg.name.ends_with("_left") {
                left_delta += delta;
                left_count += 1.0;
            } else if leg.name.ends_with("_right") {
                right_delta += delta;
                right_count += 1.0;
            }
        }

        assert!(left_count > 0.0 && right_count > 0.0);
        assert!(right_delta / right_count < left_delta / left_count);
    }
}
