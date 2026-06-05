use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod dashboard_page;

use anyhow::Context;
use arachno_camera::RobotCamera;
use arachno_core::{CameraBackend, RobotConfig, TripodGait};
use arachno_feetech_sts::{
    RealStsBus, validate_servo_eeprom_profile as validate_bus_servo_eeprom_profile,
};
use arachno_hal::{CameraSource, ImuSource, ServoBus};
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

const IMU_BRIDGE_BAUD_RATE: u32 = 115_200;
const IMU_PROBE_TIMEOUT_MS: u64 = 1_000;
const TELEMETRY_LOOP_MS: u64 = 250;
const LOW_VOLTAGE_STRIKES_TO_TRIP: u8 = 6;
const STAND_UP_FEMUR_PREP_RATIO: f32 = 0.20;
const STAND_UP_TIBIA_PLANT_RATIO: f32 = 0.20;
const STAND_UP_BODY_RISE_RATIO: f32 = 0.45;
const MANUAL_COXA_LIMIT_DEG: f32 = 180.0;
const MANUAL_FEMUR_LIMIT_DEG: f32 = 180.0;
const MANUAL_TIBIA_LIMIT_DEG: f32 = 180.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BrainMode {
    Telemetry,
    Manual,
    LayDown,
    StandUp,
    Stand,
    SlowWalk,
}

impl BrainMode {
    fn as_state_label(self) -> &'static str {
        match self {
            Self::Telemetry => "telemetry",
            Self::Manual => "manual",
            Self::LayDown => "lay_down",
            Self::StandUp => "stand_up",
            Self::Stand => "stand",
            Self::SlowWalk => "slow_walk",
        }
    }

    fn requires_torque(self) -> bool {
        !matches!(self, Self::Telemetry)
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
}

#[derive(Clone)]
struct AppState {
    config: RobotConfig,
    shared: Arc<RwLock<TelemetryState>>,
    manual: Arc<RwLock<ManualControlState>>,
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

#[derive(Debug, Clone)]
struct MotionRuntime {
    mode: BrainMode,
    walk_seconds: Option<f32>,
    armed_at: Option<Instant>,
    initial_pose: Option<BTreeMap<u8, u16>>,
    hold_pose: Option<BTreeMap<u8, u16>>,
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

#[derive(Debug, Serialize)]
struct ManualCommandResponse {
    summary: String,
}

struct ServoPollOutcome {
    should_reopen_bus: bool,
}

impl TelemetryState {
    fn from_config(config: &RobotConfig, mode: BrainMode, manual: &ManualControlState) -> Self {
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
            manual: build_manual_telemetry(config, manual, None),
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
            "manual control is disabled; start arachno-brain with --mode manual to enable dashboard sliders"
        };

        Self {
            enabled,
            base_pose: None,
            target_pose: None,
            summary: summary.to_owned(),
        }
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
        let summary = match mode {
            BrainMode::Telemetry => "observation only; no motion commands are being sent",
            BrainMode::Manual => "waiting for all servo feedback before arming manual control",
            BrainMode::LayDown => "waiting for all servo feedback before laying down",
            BrainMode::StandUp => "waiting for all servo feedback before standing up",
            BrainMode::Stand => "waiting for all servo feedback before holding stand",
            BrainMode::SlowWalk => "waiting for all servo feedback before starting the gait",
        };

        Self {
            mode,
            walk_seconds,
            armed_at: None,
            initial_pose: None,
            hold_pose: None,
            low_voltage_strikes: BTreeMap::new(),
            summary: summary.to_owned(),
            fault: None,
        }
    }

    fn arm(&mut self, pose: BTreeMap<u8, u16>) {
        if self.armed_at.is_some() {
            return;
        }

        self.armed_at = Some(Instant::now());
        self.initial_pose = Some(pose.clone());
        self.hold_pose = Some(pose);
        self.summary = match self.mode {
            BrainMode::Manual => "manual control armed at the measured robot pose".to_owned(),
            BrainMode::LayDown => "starting lay-down transition".to_owned(),
            BrainMode::StandUp => "starting stand-up transition".to_owned(),
            BrainMode::Stand => "holding the configured stand-reference pose".to_owned(),
            BrainMode::SlowWalk => "holding the measured stand pose before gait".to_owned(),
            BrainMode::Telemetry => {
                "observation only; no motion commands are being sent".to_owned()
            }
        };
    }

    fn disarm(&mut self, message: impl Into<String>) {
        self.armed_at = None;
        self.initial_pose = None;
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
    }

    fn safety_status(&self, imu_enabled: bool) -> String {
        if let Some(fault) = &self.fault {
            return format!("tripped: {fault}");
        }

        match self.mode {
            BrainMode::Telemetry => "observation only".to_owned(),
            BrainMode::Manual => {
                if imu_enabled {
                    "manual control active; monitoring roll, pitch, bus voltage, and temperature"
                        .to_owned()
                } else {
                    "manual control active; monitoring bus voltage and temperature".to_owned()
                }
            }
            BrainMode::LayDown | BrainMode::StandUp | BrainMode::Stand | BrainMode::SlowWalk => {
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
        if let Some(imu) = imu_state {
            if let Some(roll_deg) = imu.roll_deg {
                if roll_deg.abs() > config.safety.max_body_roll_deg {
                    return Some(format!(
                        "body roll {:.1} deg exceeded limit {:.1} deg",
                        roll_deg, config.safety.max_body_roll_deg
                    ));
                }
            }
            if let Some(pitch_deg) = imu.pitch_deg {
                if pitch_deg.abs() > config.safety.max_body_pitch_deg {
                    return Some(format!(
                        "body pitch {:.1} deg exceeded limit {:.1} deg",
                        pitch_deg, config.safety.max_body_pitch_deg
                    ));
                }
            }
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

            if let Some(temp_c) = telemetry.present_temperature_c {
                if temp_c > config.safety.max_servo_temp_c {
                    return Some(format!(
                        "servo {} temperature {} C exceeded {} C",
                        telemetry.servo_id, temp_c, config.safety.max_servo_temp_c
                    ));
                }
            }
        }

        None
    }

    fn commands(
        &mut self,
        config: &RobotConfig,
        gait: &TripodGait,
        manual: Option<&Arc<RwLock<ManualControlState>>>,
    ) -> Option<Vec<JointCommand>> {
        if self.mode == BrainMode::Telemetry {
            return None;
        }

        let base_pose = self
            .initial_pose
            .clone()
            .or_else(|| self.hold_pose.clone())
            .unwrap_or_else(|| self.fallback_pose(config, gait));

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
                BrainMode::LayDown => {
                    let target = gait.lay_down_pose(config);
                    let duration = config.locomotion.lay_down.duration_seconds.max(0.5);
                    let progress = (elapsed / duration).clamp(0.0, 1.0);
                    self.summary = if progress < 1.0 {
                        format!("laying down ({:.0}%)", progress * 100.0)
                    } else {
                        "holding the configured lay-down pose".to_owned()
                    };
                    interpolate_pose(&base_pose, &target, smoothstep(progress))
                }
                BrainMode::StandUp => {
                    let (pose, summary) = staged_stand_up_pose(config, gait, &base_pose, elapsed);
                    self.summary = summary;
                    pose
                }
                BrainMode::Stand => {
                    let settle = config.locomotion.stand.settle_seconds.max(0.25);
                    let progress = (elapsed / settle).clamp(0.0, 1.0);
                    self.summary = if progress < 1.0 {
                        format!(
                            "settling into the configured stand-reference pose ({:.0}%)",
                            progress * 100.0
                        )
                    } else {
                        "holding the configured stand-reference pose".to_owned()
                    };
                    interpolate_pose(
                        &base_pose,
                        &gait.stand_reference_pose(config),
                        smoothstep(progress),
                    )
                }
                BrainMode::SlowWalk => {
                    let settle = config.locomotion.tripod.settle_seconds.max(0.25);

                    if elapsed < settle {
                        let progress = (elapsed / settle).clamp(0.0, 1.0);
                        self.summary = format!(
                            "holding the measured stand pose before gait ({:.0}%)",
                            progress * 100.0
                        );
                        base_pose.clone()
                    } else if self
                        .walk_seconds
                        .is_some_and(|limit| elapsed - settle >= limit.max(0.0))
                    {
                        let gait_elapsed = (elapsed - settle).max(0.0);
                        let limit = self.walk_seconds.unwrap_or_default();
                        self.summary = format!(
                            "walk duration reached after {:.1}s / {:.1}s; holding the measured stand pose",
                            gait_elapsed, limit
                        );
                        base_pose.clone()
                    } else {
                        let gait_elapsed = elapsed - settle;
                        let cycle_seconds = config.locomotion.tripod.cycle_seconds.max(0.5);
                        let phase = (gait_elapsed / cycle_seconds).fract();
                        self.summary = format!(
                            "slow tripod gait active; phase {:.2} / cycle {:.1}s",
                            phase, cycle_seconds
                        );
                        walk_pose_from_base(config, gait, &base_pose, phase)
                    }
                }
                BrainMode::Telemetry => unreachable!(),
            }
        };

        self.hold_pose = Some(target_pose.clone());
        Some(pose_to_commands(&target_pose))
    }

    fn fallback_pose(&self, config: &RobotConfig, gait: &TripodGait) -> BTreeMap<u8, u16> {
        match self.mode {
            BrainMode::Manual => gait.stand_reference_pose(config),
            BrainMode::LayDown | BrainMode::StandUp => gait.lay_down_pose(config),
            BrainMode::Stand | BrainMode::SlowWalk | BrainMode::Telemetry => {
                gait.stand_reference_pose(config)
            }
        }
    }
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
            "failed to open servo bus {} for EEPROM validation",
            config.bus.feetech.port
        )
    })?;

    validate_bus_servo_eeprom_profile(&mut bus, &servo_ids, &config.servo_eeprom.entries)
        .context("persistent servo EEPROM profile validation failed")?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = RobotConfig::load_from_path(&args.config)
        .with_context(|| format!("failed to load {}", args.config.display()))?;
    validate_servo_eeprom_profile(&config)?;
    if !config.servo_eeprom.entries.is_empty() {
        println!(
            "servo_eeprom: validated {} configured persistent register values",
            config.servo_eeprom.entries.len()
        );
    }

    let manual = Arc::new(RwLock::new(ManualControlState::for_mode(args.mode)));
    let initial_manual = manual
        .read()
        .map_err(|_| anyhow::anyhow!("failed to initialize manual control state"))?
        .clone();
    let shared = Arc::new(RwLock::new(TelemetryState::from_config(
        &config,
        args.mode,
        &initial_manual,
    )));
    spawn_control_worker(
        shared.clone(),
        manual.clone(),
        config.clone(),
        args.mode,
        args.walk_seconds,
    );

    let app = Router::new()
        .route("/", get(index))
        .route("/dashboard", get(dashboard))
        .route("/api/state", get(api_state))
        .route("/api/manual/capture", post(api_manual_capture))
        .route("/api/manual/apply", post(api_manual_apply))
        .route("/api/manual/reset", post(api_manual_reset))
        .route("/camera.mjpg", get(camera_stream))
        .layer(CorsLayer::new().allow_origin(Any))
        .with_state(AppState {
            config: config.clone(),
            shared,
            manual,
            dashboard_enabled: args.dashboard,
        });

    let listener = TcpListener::bind(args.listen).await?;
    println!("arachno-brain api: http://{}", args.listen);
    println!("deployment_profile: {}", config.deployment.profile);
    println!("compute_target: {}", config.deployment.compute);
    println!("servo_port: {}", config.bus.feetech.port);
    println!("mode: {}", args.mode.as_state_label());
    if args.mode == BrainMode::Manual {
        println!("manual_control: enabled via /api/manual/* and dashboard sliders");
    }
    if let Some(limit) = args.walk_seconds {
        println!("walk_seconds: {limit:.1}");
    }
    if let Some(imu) = &config.imu {
        println!(
            "imu_bridge: enabled={} mode={} device={}",
            imu.enabled,
            imu.mode,
            imu.device.as_deref().unwrap_or("n/a")
        );
    } else {
        println!("imu_bridge: disabled");
    }
    if args.dashboard {
        println!("dashboard ui: http://{}", args.listen);
    } else {
        println!("dashboard ui: disabled (start with --dashboard to serve it here)");
    }
    axum::serve(listener, app).await?;
    Ok(())
}

fn spawn_control_worker(
    shared: Arc<RwLock<TelemetryState>>,
    manual: Arc<RwLock<ManualControlState>>,
    config: RobotConfig,
    mode: BrainMode,
    walk_seconds: Option<f32>,
) {
    thread::spawn(move || {
        let labels = servo_labels(&config);
        let servo_ids = config.all_servo_ids();
        let gait = TripodGait;
        let mut bus = None::<RealStsBus>;
        let mut torque_enabled = false;
        let mut imu_bridge = None::<UsbImuBridge>;
        let mut imu_state = telemetry_imu_from_config(&config);
        let mut motion = MotionRuntime::new(mode, walk_seconds);
        let mut servo_states = initial_servo_states(&servo_ids, &labels);
        let mut telemetry_cursor = 0usize;
        let loop_period = if mode == BrainMode::Telemetry {
            Duration::from_millis(TELEMETRY_LOOP_MS)
        } else {
            Duration::from_secs_f32(1.0 / config.locomotion.command_hz.max(1) as f32)
        };

        loop {
            let tick_started = Instant::now();

            poll_imu(&config, &mut imu_bridge, &mut imu_state);

            if bus.is_none() {
                match RealStsBus::open(
                    config.bus.feetech.port.clone(),
                    config.bus.feetech.baud_rate,
                    servo_ids.clone(),
                ) {
                    Ok(real_bus) => {
                        bus = Some(real_bus);
                        torque_enabled = false;
                        if mode.requires_torque() {
                            motion.disarm("servo bus opened; waiting to arm motion");
                        }
                    }
                    Err(err) => {
                        motion.disarm(format!("waiting for servo bus: {err}"));
                        write_state(
                            &shared,
                            build_state_snapshot(
                                &config,
                                &servo_ids,
                                &servo_states,
                                imu_state.clone(),
                                &motion,
                                &manual,
                                Some(format!("failed to open servo bus: {err}")),
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
                            Some(format!("failed to enable torque: {err}")),
                        ),
                    );
                    bus = None;
                    sleep_remaining(tick_started, loop_period);
                    continue;
                }
                torque_enabled = true;
            }

            let read_budget = if mode == BrainMode::Telemetry || !motion.armed_at.is_some() {
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
                motion.trip_fault(
                    "servo bus link dropped; motion paused",
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
                        Some("servo bus needs to be reopened".to_owned()),
                    ),
                );
                sleep_remaining(tick_started, loop_period);
                continue;
            }

            if mode.requires_torque() && motion.armed_at.is_none() {
                if let Some(start_pose) = current_pose(&servo_ids, &servo_states) {
                    ensure_manual_reference_pose(&manual, &config, mode, &start_pose);
                    motion.arm(start_pose);
                } else {
                    motion.summary = format!(
                        "waiting for all {} servo feedback replies before motion",
                        servo_ids.len()
                    );
                }
            }

            if mode.requires_torque() && motion.fault.is_none() {
                if let Some(reason) =
                    motion.check_safety(&config, &servo_ids, &servo_states, imu_state.as_ref())
                {
                    motion.trip_fault(reason, current_pose(&servo_ids, &servo_states));
                }
            }

            if let Some(commands) = motion.commands(&config, &gait, Some(&manual)) {
                if let Err(err) = real_bus.sync_write_positions(&commands) {
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
                            Some(format!("sync write failed: {err}")),
                        ),
                    );
                    sleep_remaining(tick_started, loop_period);
                    continue;
                }
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

    if let Some(bridge) = imu_bridge.as_mut() {
        if let Err(err) = drain_imu_bridge(bridge, state) {
            state.last_error = Some(format!("IMU read failed: {err}"));
            *imu_bridge = None;
            if config.imu.as_ref().is_some_and(|imu| imu.enabled) {
                state.sensor_kind = None;
            }
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

fn build_state_snapshot(
    config: &RobotConfig,
    servo_ids: &[u8],
    servo_states: &BTreeMap<u8, TelemetryServoState>,
    imu: Option<TelemetryImuState>,
    motion: &MotionRuntime,
    manual: &Arc<RwLock<ManualControlState>>,
    transport_error: Option<String>,
) -> TelemetryState {
    let pose = current_pose(servo_ids, servo_states);
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
        manual: manual_snapshot(config, manual, pose.as_ref()),
        servos,
    }
}

fn build_manual_telemetry(
    config: &RobotConfig,
    control: &ManualControlState,
    current_pose: Option<&BTreeMap<u8, u16>>,
) -> TelemetryManualState {
    TelemetryManualState {
        enabled: control.enabled,
        ready: control.base_pose.is_some() && current_pose.is_some(),
        base_pose_captured: control.base_pose.is_some(),
        summary: control.summary.clone(),
        groups: manual_group_infos(config),
        group_values: manual_group_values(config, current_pose, control.base_pose.as_ref()),
        joints: manual_joint_infos(),
    }
}

fn manual_snapshot(
    config: &RobotConfig,
    manual: &Arc<RwLock<ManualControlState>>,
    current_pose: Option<&BTreeMap<u8, u16>>,
) -> TelemetryManualState {
    match manual.read() {
        Ok(control) => build_manual_telemetry(config, &control, current_pose),
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
    current_pose: Option<&BTreeMap<u8, u16>>,
    base_pose: Option<&BTreeMap<u8, u16>>,
) -> Vec<ManualGroupValue> {
    let Some(current_pose) = current_pose else {
        return Vec::new();
    };
    let Some(base_pose) = base_pose else {
        return Vec::new();
    };

    let mut groups = vec![
        manual_group_value(config, "all_legs", current_pose, base_pose),
        manual_group_value_for_filter(config, "left_side", current_pose, base_pose, |leg| {
            leg.name.contains("left")
        }),
        manual_group_value_for_filter(config, "right_side", current_pose, base_pose, |leg| {
            leg.name.contains("right")
        }),
        manual_group_value_for_filter(config, "front_pair", current_pose, base_pose, |leg| {
            leg.name.starts_with("front_")
        }),
        manual_group_value_for_filter(config, "middle_pair", current_pose, base_pose, |leg| {
            leg.name.starts_with("middle_")
        }),
        manual_group_value_for_filter(config, "rear_pair", current_pose, base_pose, |leg| {
            leg.name.starts_with("rear_")
        }),
        manual_group_value_for_filter(config, "tripod_a", current_pose, base_pose, |leg| {
            leg.is_tripod_a()
        }),
        manual_group_value_for_filter(config, "tripod_b", current_pose, base_pose, |leg| {
            !leg.is_tripod_a()
        }),
    ];

    groups.extend(config.legs.iter().map(|leg| {
        manual_group_value_for_legs(
            format!("leg:{}", leg.name),
            current_pose,
            base_pose,
            vec![leg],
        )
    }));

    groups
}

fn manual_group_value(
    config: &RobotConfig,
    key: &str,
    current_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
) -> ManualGroupValue {
    manual_group_value_for_legs(
        key.to_owned(),
        current_pose,
        base_pose,
        config.legs.iter().collect(),
    )
}

fn manual_group_value_for_filter<F>(
    config: &RobotConfig,
    key: &str,
    current_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    predicate: F,
) -> ManualGroupValue
where
    F: Fn(&arachno_core::LegConfig) -> bool,
{
    manual_group_value_for_legs(
        key.to_owned(),
        current_pose,
        base_pose,
        config.legs.iter().filter(|leg| predicate(leg)).collect(),
    )
}

fn manual_group_value_for_legs(
    key: String,
    current_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    legs: Vec<&arachno_core::LegConfig>,
) -> ManualGroupValue {
    ManualGroupValue {
        key,
        coxa_deg: manual_joint_group_average_deg(current_pose, base_pose, &legs, "coxa"),
        femur_deg: manual_joint_group_average_deg(current_pose, base_pose, &legs, "femur"),
        tibia_deg: manual_joint_group_average_deg(current_pose, base_pose, &legs, "tibia"),
    }
}

fn manual_joint_group_average_deg(
    current_pose: &BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    legs: &[&arachno_core::LegConfig],
    joint_key: &str,
) -> f32 {
    let values = legs
        .iter()
        .filter_map(|leg| {
            let (servo_id, sign) = manual_joint_servo_and_sign(leg, joint_key)?;
            let current_ticks = current_pose.get(&servo_id).copied()?;
            let base_ticks = base_pose.get(&servo_id).copied()?;
            Some(semantic_ticks_to_degrees(
                i32::from(current_ticks) - i32::from(base_ticks),
                sign,
            ))
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
    config: &RobotConfig,
    mode: BrainMode,
    pose: &BTreeMap<u8, u16>,
) {
    if mode != BrainMode::Manual {
        return;
    }

    if let Ok(mut control) = manual.write() {
        if !control.enabled || control.base_pose.is_some() {
            return;
        }

        control.base_pose = Some(TripodGait.stand_reference_pose(config));
        control.target_pose = Some(pose.clone());
        control.summary =
            "manual control is ready; sliders now reflect the current robot pose relative to the configured stand-reference pose"
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
            "manual control needs fresh feedback from all configured servos before it can capture or apply angles",
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
            "manual control is disabled; restart arachno-brain with --mode manual",
        ))
    }
}

fn manual_api_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, String) {
    (status, message.into())
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

fn set_manual_joint_target(
    target_pose: &mut BTreeMap<u8, u16>,
    base_pose: &BTreeMap<u8, u16>,
    leg: &arachno_core::LegConfig,
    joint_key: &str,
    degrees: f32,
) {
    let Some((servo_id, sign)) = manual_joint_servo_and_sign(leg, joint_key) else {
        return;
    };
    let Some(base_ticks) = base_pose.get(&servo_id).copied() else {
        return;
    };
    let delta_ticks = semantic_degrees_to_ticks(degrees, sign);
    let next_ticks = (i32::from(base_ticks) + i32::from(delta_ticks)).clamp(0, 4095) as u16;
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

fn semantic_degrees_to_ticks(degrees: f32, sign: i16) -> i16 {
    let delta = degrees * (4096.0 / 360.0);
    (delta * sign as f32).round() as i16
}

fn semantic_ticks_to_degrees(delta_ticks: i32, sign: i16) -> f32 {
    delta_ticks as f32 * 360.0 / 4096.0 / sign as f32
}

fn servo_semantic_angle_deg(config: &RobotConfig, servo_id: u8, present_ticks: u16) -> Option<f32> {
    let leg = config.legs.iter().find(|leg| {
        leg.coxa_servo_id == servo_id
            || leg.femur_servo_id == servo_id
            || leg.tibia_servo_id == servo_id
    })?;

    let (reference_ticks, sign) = if leg.coxa_servo_id == servo_id {
        (leg.coxa_stand_reference_ticks, leg.coxa_forward_sign())
    } else if leg.femur_servo_id == servo_id {
        (leg.femur_stand_reference_ticks, leg.femur_lift_sign())
    } else {
        (leg.tibia_stand_reference_ticks, leg.tibia_lift_sign())
    };

    Some(semantic_ticks_to_degrees(
        i32::from(present_ticks) - i32::from(reference_ticks),
        sign,
    ))
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

fn walk_pose_from_base(
    config: &RobotConfig,
    gait: &TripodGait,
    base_pose: &BTreeMap<u8, u16>,
    phase: f32,
) -> BTreeMap<u8, u16> {
    let stand_reference_pose = gait.stand_reference_pose(config);
    let walk_pose = gait.slow_walk_pose(config, phase);
    let mut commanded = BTreeMap::new();

    for (&servo_id, &walk_ticks) in &walk_pose {
        let stand_ticks = stand_reference_pose
            .get(&servo_id)
            .copied()
            .unwrap_or(walk_ticks);
        let base_ticks = base_pose.get(&servo_id).copied().unwrap_or(stand_ticks);
        let delta_ticks = i32::from(walk_ticks) - i32::from(stand_ticks);
        let target_ticks = (i32::from(base_ticks) + delta_ticks).clamp(0, 4095) as u16;
        commanded.insert(servo_id, target_ticks);
    }

    commanded
}

fn staged_stand_up_pose(
    config: &RobotConfig,
    gait: &TripodGait,
    base_pose: &BTreeMap<u8, u16>,
    elapsed: f32,
) -> (BTreeMap<u8, u16>, String) {
    let stand_reference_pose = gait.stand_reference_pose(config);
    let duration = config.locomotion.stand_up.duration_seconds.max(0.5);
    let progress = (elapsed / duration).clamp(0.0, 1.0);

    if progress >= 1.0 {
        return (
            stand_reference_pose,
            "holding the configured stand-reference pose".to_owned(),
        );
    }

    let femur_lift_pose = femur_lift_pose(config, base_pose);
    let foot_plant_pose = foot_plant_pose(config, base_pose, &femur_lift_pose);
    let body_raise_pose = body_raise_pose(config, base_pose, &stand_reference_pose);

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
            interpolate_pose(&body_raise_pose, &stand_reference_pose, phase_progress),
            format!("aligning coxae into stand ({:.0}%)", phase_progress * 100.0),
        )
    }
}

fn femur_lift_pose(config: &RobotConfig, base_pose: &BTreeMap<u8, u16>) -> BTreeMap<u8, u16> {
    let femur_ticks = stand_up_femur_prep_ticks(config);
    let mut pose = base_pose.clone();

    for leg in &config.legs {
        let base_femur = base_pose.get(&leg.femur_servo_id).copied().unwrap_or(
            leg.femur_lay_down_ticks
                .unwrap_or(leg.femur_stand_reference_ticks),
        );
        pose.insert(
            leg.femur_servo_id,
            offset_ticks(base_femur, leg.femur_lift_sign() * femur_ticks),
        );
    }

    pose
}

fn foot_plant_pose(
    config: &RobotConfig,
    base_pose: &BTreeMap<u8, u16>,
    femur_lift_pose: &BTreeMap<u8, u16>,
) -> BTreeMap<u8, u16> {
    let tibia_ticks = stand_up_tibia_plant_ticks(config);
    let mut pose = femur_lift_pose.clone();

    for leg in &config.legs {
        let base_tibia = base_pose.get(&leg.tibia_servo_id).copied().unwrap_or(
            leg.tibia_lay_down_ticks
                .unwrap_or(leg.tibia_stand_reference_ticks),
        );
        pose.insert(
            leg.tibia_servo_id,
            offset_ticks(base_tibia, -leg.tibia_lift_sign() * tibia_ticks),
        );
    }

    pose
}

fn body_raise_pose(
    config: &RobotConfig,
    base_pose: &BTreeMap<u8, u16>,
    stand_reference_pose: &BTreeMap<u8, u16>,
) -> BTreeMap<u8, u16> {
    let mut pose = stand_reference_pose.clone();

    for leg in &config.legs {
        let base_coxa = base_pose.get(&leg.coxa_servo_id).copied().unwrap_or(
            leg.coxa_lay_down_ticks
                .unwrap_or(leg.coxa_stand_reference_ticks),
        );
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

    for (&servo_id, &end_ticks) in end {
        let start_ticks = start.get(&servo_id).copied().unwrap_or(end_ticks);
        let interpolated = start_ticks as f32 + (end_ticks as f32 - start_ticks as f32) * t;
        pose.insert(servo_id, interpolated.round().clamp(0.0, 4095.0) as u16);
    }

    pose
}

fn offset_ticks(start_ticks: u16, delta_ticks: i16) -> u16 {
    (i32::from(start_ticks) + i32::from(delta_ticks)).clamp(0, 4095) as u16
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
    let base_pose = manual
        .base_pose
        .clone()
        .unwrap_or_else(|| fallback_pose.clone());
    let mut target_pose = manual
        .target_pose
        .clone()
        .unwrap_or_else(|| base_pose.clone());
    manual.base_pose = Some(base_pose.clone());

    for leg in legs {
        set_manual_joint_target(&mut target_pose, &base_pose, leg, "coxa", coxa_deg);
        set_manual_joint_target(&mut target_pose, &base_pose, leg, "femur", femur_deg);
        set_manual_joint_target(&mut target_pose, &base_pose, leg, "tibia", tibia_deg);
    }

    let summary = format!(
        "manual target updated for {group_label}: coxa {coxa_deg:+.1}°, femur {femur_deg:+.1}°, tibia {tibia_deg:+.1}°"
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
