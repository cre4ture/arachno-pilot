use std::{
    collections::BTreeMap,
    f32::consts::PI,
    fs, io,
    path::{Path, PathBuf},
};

use arachno_msg::JointCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotConfig {
    pub deployment: DeploymentConfig,
    pub robot: RobotMeta,
    #[serde(default)]
    pub servo_store: Option<ServoStoreConfig>,
    #[serde(default)]
    pub pose_store: Option<PoseStoreConfig>,
    #[serde(default)]
    pub semantic_calibration_store: Option<SemanticCalibrationStoreConfig>,
    #[serde(default)]
    pub poses: SemanticPoseSet,
    #[serde(default)]
    pub servo_eeprom: ServoEepromConfig,
    #[serde(default)]
    pub bus: BusConfig,
    pub camera: CameraConfig,
    #[serde(default)]
    pub imu: Option<ImuConfig>,
    #[serde(default)]
    pub safety: SafetyConfig,
    pub learning: LearningConfig,
    #[serde(default)]
    pub locomotion: LocomotionConfig,
    #[serde(default)]
    pub legs: Vec<LegConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub profile: String,
    pub compute: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotMeta {
    pub name: String,
    pub control_hz: u16,
    pub perception_hz: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServoStoreConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseStoreConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCalibrationStoreConfig {
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticPoseSet {
    #[serde(default, alias = "home")]
    pub stand_reference: BTreeMap<String, LegPoseAngles>,
    #[serde(default)]
    pub lay_down: BTreeMap<String, LegPoseAngles>,
    #[serde(default, alias = "zero")]
    pub zero_pose: BTreeMap<String, LegPoseAngles>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LegPoseAngles {
    pub coxa_deg: f32,
    pub femur_deg: f32,
    pub tibia_deg: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticPoseKind {
    StandReference,
    LayDown,
    ZeroPose,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServoEepromConfig {
    #[serde(default)]
    pub entries: Vec<ServoEepromEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServoEepromEntry {
    pub name: String,
    pub address: u8,
    pub width: ServoRegisterWidth,
    pub value: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServoRegisterWidth {
    #[default]
    U8,
    U16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BusConfig {
    pub feetech: FeetechBusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeetechBusConfig {
    pub port: String,
    pub baud_rate: u32,
    pub telemetry_stride: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub backend: CameraBackend,
    pub device: Option<String>,
    pub sensor_id: Option<u8>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub fov_deg: f32,
    pub pixel_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImuConfig {
    pub enabled: bool,
    pub mode: String,
    pub device: Option<String>,
    pub sample_hz: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraBackend {
    Argus,
    V4l2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub max_body_pitch_deg: f32,
    pub max_body_roll_deg: f32,
    pub max_servo_temp_c: u8,
    pub min_bus_voltage_v: f32,
    pub max_servo_load_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    pub mode: String,
    pub policy_transport: String,
    pub policy_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocomotionConfig {
    #[serde(default = "default_command_hz")]
    pub command_hz: u16,
    #[serde(default)]
    pub stand_up: PoseTransitionConfig,
    #[serde(default)]
    pub lay_down: PoseTransitionConfig,
    #[serde(default)]
    pub stand: StandConfig,
    #[serde(default)]
    pub tripod: TripodWalkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseTransitionConfig {
    #[serde(default = "default_pose_transition_seconds")]
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandConfig {
    #[serde(default = "default_stand_settle_seconds")]
    pub settle_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripodWalkConfig {
    #[serde(default = "default_tripod_settle_seconds")]
    pub settle_seconds: f32,
    #[serde(default = "default_tripod_startup_blend_seconds")]
    pub startup_blend_seconds: f32,
    #[serde(default = "default_tripod_cycle_seconds")]
    pub cycle_seconds: f32,
    #[serde(default = "default_stride_ticks")]
    pub stride_ticks: i16,
    #[serde(default = "default_femur_lift_ticks")]
    pub femur_lift_ticks: i16,
    #[serde(default = "default_tibia_lift_ticks")]
    pub tibia_lift_ticks: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegConfig {
    pub name: String,
    pub coxa_servo_id: u8,
    pub femur_servo_id: u8,
    pub tibia_servo_id: u8,
    #[serde(default, alias = "coxa_home_ticks")]
    pub coxa_stand_reference_ticks: Option<u16>,
    #[serde(default, alias = "femur_home_ticks")]
    pub femur_stand_reference_ticks: Option<u16>,
    #[serde(default, alias = "tibia_home_ticks")]
    pub tibia_stand_reference_ticks: Option<u16>,
    #[serde(default)]
    pub coxa_lay_down_ticks: Option<u16>,
    #[serde(default)]
    pub femur_lay_down_ticks: Option<u16>,
    #[serde(default)]
    pub tibia_lay_down_ticks: Option<u16>,
    #[serde(default, alias = "coxa_zero_pose_ticks")]
    pub coxa_zero_reference_ticks: Option<u16>,
    #[serde(default, alias = "femur_zero_pose_ticks")]
    pub femur_zero_reference_ticks: Option<u16>,
    #[serde(default, alias = "tibia_zero_pose_ticks")]
    pub tibia_zero_reference_ticks: Option<u16>,
    #[serde(default)]
    pub coxa_forward_sign: i8,
    #[serde(default)]
    pub femur_lift_sign: i8,
    #[serde(default)]
    pub tibia_lift_sign: i8,
    #[serde(default)]
    pub coxa_zero_heading_deg: Option<f32>,
    #[serde(default, alias = "coxa_length")]
    pub coxa_length_cm: Option<f32>,
    #[serde(default, alias = "femur_length")]
    pub femur_length_cm: Option<f32>,
    #[serde(default, alias = "tibia_length")]
    pub tibia_length_cm: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LegPoint2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LegTopViewPose {
    pub anchor: LegPoint2,
    pub coxa_end: LegPoint2,
    pub femur_end: LegPoint2,
    pub tibia_end: LegPoint2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LegSideViewPose {
    pub anchor: LegPoint2,
    pub coxa_end: LegPoint2,
    pub femur_end: LegPoint2,
    pub tibia_end: LegPoint2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedServoConfigFile {
    #[serde(default)]
    pub servo_eeprom: Option<ServoEepromConfig>,
    #[serde(default)]
    pub bus: Option<BusConfig>,
    #[serde(default)]
    pub safety: Option<SafetyConfig>,
    #[serde(default)]
    pub locomotion: Option<LocomotionConfig>,
    pub legs: Vec<LegConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SharedPoseConfigFile {
    #[serde(default, alias = "home")]
    pub stand_reference: BTreeMap<String, LegPoseAngles>,
    #[serde(default)]
    pub lay_down: BTreeMap<String, LegPoseAngles>,
    #[serde(default, alias = "zero")]
    pub zero_pose: BTreeMap<String, LegPoseAngles>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("failed to read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("config {path} does not define any legs and has no [servo_store]")]
    MissingServoStore { path: PathBuf },
    #[error("servo store {path} does not define any legs")]
    EmptyServoStore { path: PathBuf },
}

impl Default for FeetechBusConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 1_000_000,
            telemetry_stride: 6,
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_body_pitch_deg: 20.0,
            max_body_roll_deg: 20.0,
            max_servo_temp_c: 65,
            min_bus_voltage_v: 5.8,
            max_servo_load_pct: 82.0,
        }
    }
}

impl RobotConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigLoadError> {
        let path = path.as_ref();
        let mut config: Self = read_toml(path)?;

        if let Some(servo_store) = &config.servo_store {
            let servo_path = resolve_config_path(path, &servo_store.path);
            let servo_file: SharedServoConfigFile = read_toml(&servo_path)?;
            if servo_file.legs.is_empty() {
                return Err(ConfigLoadError::EmptyServoStore { path: servo_path });
            }
            if let Some(bus) = servo_file.bus {
                config.bus = bus;
            }
            if let Some(servo_eeprom) = servo_file.servo_eeprom {
                config.servo_eeprom = servo_eeprom;
            }
            if let Some(safety) = servo_file.safety {
                config.safety = safety;
            }
            if let Some(locomotion) = servo_file.locomotion {
                config.locomotion = locomotion;
            }
            config.legs = servo_file.legs;
        }

        if let Some(pose_store) = &config.pose_store {
            let pose_path = resolve_config_path(path, &pose_store.path);
            let pose_file: SharedPoseConfigFile = read_toml(&pose_path)?;
            config.poses = SemanticPoseSet {
                stand_reference: pose_file.stand_reference,
                lay_down: pose_file.lay_down,
                zero_pose: pose_file.zero_pose,
            };
        }

        if config.legs.is_empty() {
            return Err(ConfigLoadError::MissingServoStore {
                path: path.to_path_buf(),
            });
        }

        Ok(config)
    }

    pub fn all_servo_ids(&self) -> Vec<u8> {
        self.legs
            .iter()
            .flat_map(|leg| [leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id])
            .collect()
    }

    pub fn pose_for_leg(&self, kind: SemanticPoseKind, leg_name: &str) -> Option<LegPoseAngles> {
        match kind {
            SemanticPoseKind::StandReference => self.poses.stand_reference.get(leg_name).copied(),
            SemanticPoseKind::LayDown => self
                .poses
                .lay_down
                .get(leg_name)
                .copied()
                .or_else(|| self.poses.zero_pose.get(leg_name).copied())
                .or(Some(LegPoseAngles::default())),
            SemanticPoseKind::ZeroPose => self
                .poses
                .zero_pose
                .get(leg_name)
                .copied()
                .or_else(|| self.poses.lay_down.get(leg_name).copied())
                .or(Some(LegPoseAngles::default())),
        }
    }
}

fn read_toml<T>(path: &Path) -> Result<T, ConfigLoadError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(path).map_err(|source| ConfigLoadError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigLoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_config_path(config_path: &Path, configured_path: &str) -> PathBuf {
    let path = PathBuf::from(configured_path);
    if path.is_absolute() {
        path
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

impl Default for LocomotionConfig {
    fn default() -> Self {
        Self {
            command_hz: default_command_hz(),
            stand_up: PoseTransitionConfig::default(),
            lay_down: PoseTransitionConfig::default(),
            stand: StandConfig::default(),
            tripod: TripodWalkConfig::default(),
        }
    }
}

impl Default for PoseTransitionConfig {
    fn default() -> Self {
        Self {
            duration_seconds: default_pose_transition_seconds(),
        }
    }
}

impl Default for StandConfig {
    fn default() -> Self {
        Self {
            settle_seconds: default_stand_settle_seconds(),
        }
    }
}

impl Default for TripodWalkConfig {
    fn default() -> Self {
        Self {
            settle_seconds: default_tripod_settle_seconds(),
            startup_blend_seconds: default_tripod_startup_blend_seconds(),
            cycle_seconds: default_tripod_cycle_seconds(),
            stride_ticks: default_stride_ticks(),
            femur_lift_ticks: default_femur_lift_ticks(),
            tibia_lift_ticks: default_tibia_lift_ticks(),
        }
    }
}

impl LegConfig {
    pub fn coxa_forward_sign(&self) -> i16 {
        resolve_sign(
            self.coxa_forward_sign,
            if self.name.contains("right") { -1 } else { 1 },
        )
    }

    pub fn femur_lift_sign(&self) -> i16 {
        resolve_sign(
            self.femur_lift_sign,
            toward_center_sign(
                self.femur_stand_reference_ticks
                    .unwrap_or(self.femur_zero_reference_ticks()),
            ),
        )
    }

    pub fn tibia_lift_sign(&self) -> i16 {
        resolve_sign(
            self.tibia_lift_sign,
            toward_center_sign(
                self.tibia_stand_reference_ticks
                    .unwrap_or(self.tibia_zero_reference_ticks()),
            ),
        )
    }

    pub fn is_tripod_a(&self) -> bool {
        matches!(
            self.name.as_str(),
            "front_left" | "middle_right" | "rear_left"
        )
    }

    pub fn is_left_side(&self) -> bool {
        self.name.contains("left")
    }

    pub fn coxa_zero_reference_ticks(&self) -> u16 {
        self.coxa_zero_reference_ticks
            .or(self.coxa_lay_down_ticks)
            .or(self.coxa_stand_reference_ticks)
            .unwrap_or(DEFAULT_REFERENCE_TICKS)
    }

    pub fn femur_zero_reference_ticks(&self) -> u16 {
        self.femur_zero_reference_ticks
            .or(self.femur_lay_down_ticks)
            .or(self.femur_stand_reference_ticks)
            .unwrap_or(DEFAULT_REFERENCE_TICKS)
    }

    pub fn tibia_zero_reference_ticks(&self) -> u16 {
        self.tibia_zero_reference_ticks
            .or(self.tibia_lay_down_ticks)
            .or(self.tibia_stand_reference_ticks)
            .unwrap_or(DEFAULT_REFERENCE_TICKS)
    }

    pub fn legacy_pose_ticks(&self, kind: SemanticPoseKind) -> Option<(u16, u16, u16)> {
        match kind {
            SemanticPoseKind::StandReference => Some((
                self.coxa_stand_reference_ticks?,
                self.femur_stand_reference_ticks?,
                self.tibia_stand_reference_ticks?,
            )),
            SemanticPoseKind::LayDown => {
                let (coxa_stand, femur_stand, tibia_stand) =
                    self.legacy_pose_ticks(SemanticPoseKind::StandReference)?;
                Some((
                    self.coxa_lay_down_ticks.unwrap_or(coxa_stand),
                    self.femur_lay_down_ticks.unwrap_or(femur_stand),
                    self.tibia_lay_down_ticks.unwrap_or(tibia_stand),
                ))
            }
            SemanticPoseKind::ZeroPose => Some((
                self.coxa_zero_reference_ticks(),
                self.femur_zero_reference_ticks(),
                self.tibia_zero_reference_ticks(),
            )),
        }
    }

    pub fn pose_ticks_from_angles(&self, pose: LegPoseAngles) -> (u16, u16, u16) {
        (
            semantic_degrees_to_ticks(
                self.coxa_zero_reference_ticks(),
                self.coxa_forward_sign(),
                pose.coxa_deg,
            ),
            semantic_degrees_to_ticks(
                self.femur_zero_reference_ticks(),
                self.femur_lift_sign(),
                pose.femur_deg,
            ),
            semantic_degrees_to_ticks(
                self.tibia_zero_reference_ticks(),
                self.tibia_lift_sign(),
                pose.tibia_deg,
            ),
        )
    }

    pub fn coxa_zero_heading_deg(&self) -> f32 {
        self.coxa_zero_heading_deg
            .unwrap_or_else(|| default_coxa_zero_heading_deg(&self.name))
    }

    pub fn coxa_length_cm(&self) -> f32 {
        self.coxa_length_cm.unwrap_or(DEFAULT_COXA_LENGTH_CM)
    }

    pub fn femur_length_cm(&self) -> f32 {
        self.femur_length_cm.unwrap_or(DEFAULT_FEMUR_LENGTH_CM)
    }

    pub fn tibia_length_cm(&self) -> f32 {
        self.tibia_length_cm.unwrap_or(DEFAULT_TIBIA_LENGTH_CM)
    }

    pub fn body_heading_deg_for_coxa(&self, semantic_coxa_deg: f32) -> f32 {
        let local_heading = self.coxa_zero_heading_deg() + semantic_coxa_deg;
        if self.is_left_side() {
            180.0 + local_heading
        } else {
            -local_heading
        }
        .rem_euclid(360.0)
    }

    pub fn top_view_pose(
        &self,
        semantic_coxa_deg: f32,
        semantic_femur_deg: f32,
        semantic_tibia_deg: f32,
    ) -> LegTopViewPose {
        let heading_rad = self
            .body_heading_deg_for_coxa(semantic_coxa_deg)
            .to_radians();
        let anchor = LegPoint2 { x: 0.0, y: 0.0 };
        let coxa_end = offset_point(anchor, heading_rad, self.coxa_length_cm());
        let femur_projection = self.femur_length_cm() * semantic_femur_deg.to_radians().cos();
        let tibia_projection =
            self.tibia_length_cm() * (semantic_femur_deg + semantic_tibia_deg).to_radians().cos();
        let femur_end = offset_point(coxa_end, heading_rad, femur_projection);
        let tibia_end = offset_point(femur_end, heading_rad, tibia_projection);
        LegTopViewPose {
            anchor,
            coxa_end,
            femur_end,
            tibia_end,
        }
    }

    pub fn side_view_pose(
        &self,
        semantic_femur_deg: f32,
        semantic_tibia_deg: f32,
    ) -> LegSideViewPose {
        let outward_sign = if self.is_left_side() { -1.0 } else { 1.0 };
        let anchor = LegPoint2 { x: 0.0, y: 0.0 };
        let coxa_end = LegPoint2 {
            x: outward_sign * self.coxa_length_cm(),
            y: 0.0,
        };
        let femur_end = offset_side_point(
            coxa_end,
            outward_sign,
            semantic_femur_deg,
            self.femur_length_cm(),
        );
        let tibia_end = offset_side_point(
            femur_end,
            outward_sign,
            semantic_femur_deg + semantic_tibia_deg,
            self.tibia_length_cm(),
        );
        LegSideViewPose {
            anchor,
            coxa_end,
            femur_end,
            tibia_end,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TripodGait;

impl TripodGait {
    pub fn stand_reference_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        named_pose_ticks(config, SemanticPoseKind::StandReference)
    }

    pub fn stand_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        self.stand_reference_pose(config)
    }

    pub fn lay_down_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        named_pose_ticks(config, SemanticPoseKind::LayDown)
    }

    pub fn zero_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        named_pose_ticks(config, SemanticPoseKind::ZeroPose)
    }

    pub fn stand_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        pose_to_commands(&self.stand_reference_pose(config))
    }

    pub fn lay_down_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        pose_to_commands(&self.lay_down_pose(config))
    }

    pub fn zero_pose_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        pose_to_commands(&self.zero_pose(config))
    }

    pub fn slow_walk_pose(&self, config: &RobotConfig, phase: f32) -> BTreeMap<u8, u16> {
        let mut pose = self.stand_reference_pose(config);
        let phase = phase.rem_euclid(1.0);
        let gait = &config.locomotion.tripod;

        for leg in &config.legs {
            let leg_phase = if leg.is_tripod_a() {
                phase
            } else {
                (phase + 0.5).fract()
            };
            let (coxa_offset, lift_ratio) = leg_cycle_shape(leg_phase, gait.stride_ticks);
            let femur_offset =
                (leg.femur_lift_sign() as f32 * gait.femur_lift_ticks as f32 * lift_ratio).round()
                    as i16;
            let tibia_offset =
                (leg.tibia_lift_sign() as f32 * gait.tibia_lift_ticks as f32 * lift_ratio).round()
                    as i16;

            pose.insert(
                leg.coxa_servo_id,
                offset_ticks(
                    pose.get(&leg.coxa_servo_id)
                        .copied()
                        .unwrap_or_else(|| leg.coxa_zero_reference_ticks()),
                    leg.coxa_forward_sign() * coxa_offset,
                ),
            );
            pose.insert(
                leg.femur_servo_id,
                offset_ticks(
                    pose.get(&leg.femur_servo_id)
                        .copied()
                        .unwrap_or_else(|| leg.femur_zero_reference_ticks()),
                    femur_offset,
                ),
            );
            pose.insert(
                leg.tibia_servo_id,
                offset_ticks(
                    pose.get(&leg.tibia_servo_id)
                        .copied()
                        .unwrap_or_else(|| leg.tibia_zero_reference_ticks()),
                    tibia_offset,
                ),
            );
        }

        pose
    }

    pub fn slow_walk_commands(&self, config: &RobotConfig, phase: f32) -> Vec<JointCommand> {
        pose_to_commands(&self.slow_walk_pose(config, phase))
    }

    pub fn stand_reference_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        self.stand_commands(config)
    }

    // Backward-compatible alias for older callers and serialized terminology.
    pub fn home_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        self.stand_reference_commands(config)
    }
}

fn default_command_hz() -> u16 {
    20
}

fn default_pose_transition_seconds() -> f32 {
    8.0
}

fn default_stand_settle_seconds() -> f32 {
    2.5
}

fn default_tripod_settle_seconds() -> f32 {
    2.5
}

fn default_tripod_startup_blend_seconds() -> f32 {
    1.5
}

fn default_tripod_cycle_seconds() -> f32 {
    5.0
}

fn default_stride_ticks() -> i16 {
    20
}

fn default_femur_lift_ticks() -> i16 {
    12
}

fn default_tibia_lift_ticks() -> i16 {
    18
}

const DEFAULT_COXA_LENGTH_CM: f32 = 18.0;
const DEFAULT_FEMUR_LENGTH_CM: f32 = 34.0;
const DEFAULT_TIBIA_LENGTH_CM: f32 = 38.0;
const DEFAULT_REFERENCE_TICKS: u16 = 2048;

fn named_pose_ticks(config: &RobotConfig, kind: SemanticPoseKind) -> BTreeMap<u8, u16> {
    let mut pose = BTreeMap::new();

    for leg in &config.legs {
        let (coxa_ticks, femur_ticks, tibia_ticks) = resolved_leg_pose_ticks(config, leg, kind);
        pose.insert(leg.coxa_servo_id, coxa_ticks);
        pose.insert(leg.femur_servo_id, femur_ticks);
        pose.insert(leg.tibia_servo_id, tibia_ticks);
    }

    pose
}

fn resolved_leg_pose_ticks(
    config: &RobotConfig,
    leg: &LegConfig,
    kind: SemanticPoseKind,
) -> (u16, u16, u16) {
    if let Some(pose) = config.pose_for_leg(kind, &leg.name) {
        return leg.pose_ticks_from_angles(pose);
    }

    leg.legacy_pose_ticks(kind).unwrap_or((
        leg.coxa_zero_reference_ticks(),
        leg.femur_zero_reference_ticks(),
        leg.tibia_zero_reference_ticks(),
    ))
}

fn semantic_degrees_to_ticks(reference_ticks: u16, sign: i16, degrees: f32) -> u16 {
    let delta_ticks = degrees * 4096.0 / 360.0 * sign as f32;
    (reference_ticks as f32 + delta_ticks)
        .round()
        .clamp(0.0, 4095.0) as u16
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

fn resolve_sign(configured: i8, fallback: i16) -> i16 {
    match configured.cmp(&0) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => fallback.signum(),
    }
}

fn toward_center_sign(stand_reference_ticks: u16) -> i16 {
    if stand_reference_ticks >= 2048 { -1 } else { 1 }
}

fn leg_cycle_shape(phase: f32, stride_ticks: i16) -> (i16, f32) {
    if phase < 0.5 {
        let t = phase / 0.5;
        let eased = smoothstep(t);
        let coxa = lerp_i16(-stride_ticks, stride_ticks, eased);
        let lift = (PI * t).sin().max(0.0);
        (coxa, lift)
    } else {
        let t = (phase - 0.5) / 0.5;
        let coxa = lerp_i16(stride_ticks, -stride_ticks, t);
        (coxa, 0.0)
    }
}

fn lerp_i16(start: i16, end: i16, t: f32) -> i16 {
    (start as f32 + (end - start) as f32 * t).round() as i16
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn default_coxa_zero_heading_deg(name: &str) -> f32 {
    if name.starts_with("front_") {
        45.0
    } else if name.starts_with("rear_") {
        -45.0
    } else {
        0.0
    }
}

fn offset_point(start: LegPoint2, angle_rad: f32, length: f32) -> LegPoint2 {
    LegPoint2 {
        x: start.x + angle_rad.cos() * length,
        y: start.y + angle_rad.sin() * length,
    }
}

fn offset_side_point(
    start: LegPoint2,
    outward_sign: f32,
    semantic_deg: f32,
    length: f32,
) -> LegPoint2 {
    let angle_rad = (-semantic_deg).to_radians();
    LegPoint2 {
        x: start.x + outward_sign * angle_rad.cos() * length,
        y: start.y + angle_rad.sin() * length,
    }
}

fn offset_ticks(start_ticks: u16, delta: i16) -> u16 {
    (i32::from(start_ticks) + i32::from(delta)).clamp(0, 4095) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    // -----------------------------------------------------------------------
    // offset_ticks
    // -----------------------------------------------------------------------

    #[test]
    fn offset_ticks_zero_delta_returns_start() {
        assert_eq!(offset_ticks(2048, 0), 2048);
    }

    #[test]
    fn offset_ticks_positive_delta() {
        assert_eq!(offset_ticks(2048, 100), 2148);
    }

    #[test]
    fn offset_ticks_negative_delta() {
        assert_eq!(offset_ticks(2048, -100), 1948);
    }

    #[test]
    fn offset_ticks_clamps_to_zero_on_underflow() {
        // Large negative delta that would go below 0.
        assert_eq!(offset_ticks(10, -100), 0);
    }

    #[test]
    fn offset_ticks_clamps_to_4095_on_overflow() {
        // Large positive delta that would exceed 4095.
        assert_eq!(offset_ticks(4090, 100), 4095);
    }

    #[test]
    fn offset_ticks_exact_bounds() {
        assert_eq!(offset_ticks(0, 0), 0);
        assert_eq!(offset_ticks(4095, 0), 4095);
    }

    // -----------------------------------------------------------------------
    // smoothstep
    // -----------------------------------------------------------------------

    #[test]
    fn smoothstep_at_zero_is_zero() {
        assert_eq!(smoothstep(0.0), 0.0);
    }

    #[test]
    fn smoothstep_at_one_is_one() {
        let v = smoothstep(1.0);
        assert!((v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
    }

    #[test]
    fn smoothstep_at_half_is_half() {
        // smoothstep(0.5) = 0.5*0.5*(3 - 2*0.5) = 0.25 * 2.0 = 0.5
        let v = smoothstep(0.5);
        assert!((v - 0.5).abs() < 1e-6, "expected 0.5, got {v}");
    }

    #[test]
    fn smoothstep_clamps_below_zero() {
        assert_eq!(smoothstep(-1.0), 0.0);
    }

    #[test]
    fn smoothstep_clamps_above_one() {
        let v = smoothstep(2.0);
        assert!((v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
    }

    // -----------------------------------------------------------------------
    // lerp_i16
    // -----------------------------------------------------------------------

    #[test]
    fn lerp_i16_at_zero_returns_start() {
        assert_eq!(lerp_i16(-20, 20, 0.0), -20);
    }

    #[test]
    fn lerp_i16_at_one_returns_end() {
        assert_eq!(lerp_i16(-20, 20, 1.0), 20);
    }

    #[test]
    fn lerp_i16_at_half_returns_midpoint() {
        assert_eq!(lerp_i16(-20, 20, 0.5), 0);
    }

    // -----------------------------------------------------------------------
    // leg_cycle_shape
    // -----------------------------------------------------------------------

    #[test]
    fn leg_cycle_shape_phase_zero_starts_at_negative_stride() {
        // At phase=0 the swing starts: coxa should be at -stride_ticks.
        let (coxa, lift) = leg_cycle_shape(0.0, 20);
        assert_eq!(coxa, -20);
        assert!(
            (lift - 0.0).abs() < 1e-5,
            "lift at phase=0 should be 0, got {lift}"
        );
    }

    #[test]
    fn leg_cycle_shape_phase_quarter_has_positive_lift() {
        // Mid-swing (phase=0.25): lift should be near its peak (sin(PI*0.5) == 1).
        let (_coxa, lift) = leg_cycle_shape(0.25, 20);
        assert!(lift > 0.9, "expected lift near 1.0, got {lift}");
    }

    #[test]
    fn leg_cycle_shape_phase_half_transitions_to_stance() {
        // At phase=0.5 swing ends: coxa == +stride_ticks, lift returns to 0.
        let (coxa, lift) = leg_cycle_shape(0.5, 20);
        assert_eq!(coxa, 20);
        assert!(
            (lift - 0.0).abs() < 1e-5,
            "lift at phase=0.5 should be 0, got {lift}"
        );
    }

    #[test]
    fn leg_cycle_shape_phase_one_back_to_negative_stride() {
        // At phase=1.0 (same as 0.0) coxa completes the stance return.
        let (coxa, lift) = leg_cycle_shape(1.0, 20);
        // t=1.0 in stance branch: lerp(stride, -stride, 1.0) == -stride
        assert_eq!(coxa, -20);
        assert!(
            (lift - 0.0).abs() < 1e-5,
            "lift at phase=1.0 should be 0, got {lift}"
        );
    }

    // -----------------------------------------------------------------------
    // resolve_sign / toward_center_sign
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_sign_positive_config_returns_plus_one() {
        assert_eq!(resolve_sign(1, -1), 1);
    }

    #[test]
    fn resolve_sign_negative_config_returns_minus_one() {
        assert_eq!(resolve_sign(-1, 1), -1);
    }

    #[test]
    fn resolve_sign_zero_falls_back_to_fallback_sign() {
        assert_eq!(resolve_sign(0, -5), -1);
        assert_eq!(resolve_sign(0, 7), 1);
    }

    #[test]
    fn toward_center_sign_above_center_is_negative() {
        assert_eq!(toward_center_sign(2048), -1);
        assert_eq!(toward_center_sign(3000), -1);
    }

    #[test]
    fn toward_center_sign_below_center_is_positive() {
        assert_eq!(toward_center_sign(1000), 1);
        assert_eq!(toward_center_sign(0), 1);
    }

    // -----------------------------------------------------------------------
    // semantic_degrees_to_ticks
    // -----------------------------------------------------------------------

    #[test]
    fn semantic_degrees_to_ticks_zero_degrees_returns_reference() {
        assert_eq!(semantic_degrees_to_ticks(2048, 1, 0.0), 2048);
    }

    #[test]
    fn semantic_degrees_to_ticks_full_revolution_positive_sign() {
        // 360 deg with sign +1 should advance exactly 4096 ticks, clamped to 4095.
        assert_eq!(semantic_degrees_to_ticks(0, 1, 360.0), 4095);
    }

    #[test]
    fn semantic_degrees_to_ticks_negative_sign_goes_down() {
        // 90 deg, sign -1, reference 2048 -> 2048 - 1024 = 1024.
        assert_eq!(semantic_degrees_to_ticks(2048, -1, 90.0), 1024);
    }

    #[test]
    fn semantic_degrees_to_ticks_clamps_at_zero() {
        // Large negative angle should clamp to 0.
        assert_eq!(semantic_degrees_to_ticks(2048, 1, -360.0), 0);
    }

    // -----------------------------------------------------------------------
    // default_coxa_zero_heading_deg
    // -----------------------------------------------------------------------

    #[test]
    fn default_coxa_zero_heading_front_legs() {
        assert_eq!(default_coxa_zero_heading_deg("front_left"), 45.0);
        assert_eq!(default_coxa_zero_heading_deg("front_right"), 45.0);
    }

    #[test]
    fn default_coxa_zero_heading_rear_legs() {
        assert_eq!(default_coxa_zero_heading_deg("rear_left"), -45.0);
        assert_eq!(default_coxa_zero_heading_deg("rear_right"), -45.0);
    }

    #[test]
    fn default_coxa_zero_heading_middle_legs() {
        assert_eq!(default_coxa_zero_heading_deg("middle_left"), 0.0);
        assert_eq!(default_coxa_zero_heading_deg("middle_right"), 0.0);
    }

    // -----------------------------------------------------------------------
    // LegConfig helper methods
    // -----------------------------------------------------------------------

    fn make_leg(name: &str) -> LegConfig {
        LegConfig {
            name: name.to_string(),
            coxa_servo_id: 1,
            femur_servo_id: 2,
            tibia_servo_id: 3,
            coxa_stand_reference_ticks: None,
            femur_stand_reference_ticks: None,
            tibia_stand_reference_ticks: None,
            coxa_lay_down_ticks: None,
            femur_lay_down_ticks: None,
            tibia_lay_down_ticks: None,
            coxa_zero_reference_ticks: None,
            femur_zero_reference_ticks: None,
            tibia_zero_reference_ticks: None,
            coxa_forward_sign: 0,
            femur_lift_sign: 0,
            tibia_lift_sign: 0,
            coxa_zero_heading_deg: None,
            coxa_length_cm: None,
            femur_length_cm: None,
            tibia_length_cm: None,
        }
    }

    #[test]
    fn is_tripod_a_correct_legs() {
        assert!(make_leg("front_left").is_tripod_a());
        assert!(make_leg("middle_right").is_tripod_a());
        assert!(make_leg("rear_left").is_tripod_a());
    }

    #[test]
    fn is_tripod_a_not_tripod_b_legs() {
        assert!(!make_leg("front_right").is_tripod_a());
        assert!(!make_leg("middle_left").is_tripod_a());
        assert!(!make_leg("rear_right").is_tripod_a());
    }

    #[test]
    fn is_left_side_for_left_and_right() {
        assert!(make_leg("front_left").is_left_side());
        assert!(!make_leg("front_right").is_left_side());
    }

    #[test]
    fn zero_reference_ticks_fallback_chain() {
        // No fields set → DEFAULT_REFERENCE_TICKS (2048).
        let leg = make_leg("middle_left");
        assert_eq!(leg.coxa_zero_reference_ticks(), DEFAULT_REFERENCE_TICKS);
        assert_eq!(leg.femur_zero_reference_ticks(), DEFAULT_REFERENCE_TICKS);
        assert_eq!(leg.tibia_zero_reference_ticks(), DEFAULT_REFERENCE_TICKS);
    }

    #[test]
    fn zero_reference_ticks_prefers_explicit_zero_over_stand() {
        let mut leg = make_leg("middle_left");
        leg.coxa_stand_reference_ticks = Some(1800);
        leg.coxa_zero_reference_ticks = Some(2100);
        // Explicit zero overrides stand reference.
        assert_eq!(leg.coxa_zero_reference_ticks(), 2100);
    }

    #[test]
    fn zero_reference_ticks_falls_back_to_stand_reference() {
        let mut leg = make_leg("middle_left");
        leg.coxa_stand_reference_ticks = Some(1800);
        // No explicit zero set → falls back to stand_reference via lay_down → stand chain.
        assert_eq!(leg.coxa_zero_reference_ticks(), 1800);
    }

    // -----------------------------------------------------------------------
    // body_heading_deg_for_coxa
    // -----------------------------------------------------------------------

    #[test]
    fn body_heading_deg_for_coxa_right_side_zero_semantic() {
        let mut leg = make_leg("front_right");
        leg.coxa_zero_heading_deg = Some(45.0);
        // Right side: heading = -(45 + 0) = -45 mod 360 = 315.
        let h = leg.body_heading_deg_for_coxa(0.0);
        assert!((h - 315.0).abs() < 1e-4, "expected 315.0, got {h}");
    }

    #[test]
    fn body_heading_deg_for_coxa_left_side_zero_semantic() {
        let mut leg = make_leg("front_left");
        leg.coxa_zero_heading_deg = Some(45.0);
        // Left side: heading = 180 + (45 + 0) = 225.
        let h = leg.body_heading_deg_for_coxa(0.0);
        assert!((h - 225.0).abs() < 1e-4, "expected 225.0, got {h}");
    }

    // -----------------------------------------------------------------------
    // LocomotionConfig / TripodWalkConfig defaults
    // -----------------------------------------------------------------------

    #[test]
    fn locomotion_config_default_values() {
        let cfg = LocomotionConfig::default();
        assert_eq!(cfg.command_hz, 20);
        assert_eq!(cfg.stand_up.duration_seconds, 8.0);
        assert_eq!(cfg.tripod.stride_ticks, 20);
        assert_eq!(cfg.tripod.femur_lift_ticks, 12);
        assert_eq!(cfg.tripod.tibia_lift_ticks, 18);
        assert_eq!(cfg.tripod.cycle_seconds, 5.0);
    }

    #[test]
    fn safety_config_default_values() {
        let s = SafetyConfig::default();
        assert_eq!(s.max_body_pitch_deg, 20.0);
        assert_eq!(s.max_servo_temp_c, 65);
    }

    #[test]
    fn load_from_path_overlays_shared_servo_config() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("arachno-core-config-{unique}"));
        fs::create_dir_all(&temp_dir).expect("failed to create temp config dir");

        let main_config = temp_dir.join("host-usb.toml");
        let servo_config = temp_dir.join("servo-config.toml");
        let pose_config = temp_dir.join("servo-poses.toml");

        fs::write(
            &main_config,
            r#"
[deployment]
profile = "host-usb"
compute = "linux-pc"

[robot]
name = "test-spider"
control_hz = 150
perception_hz = 20

[servo_store]
path = "servo-config.toml"

[pose_store]
path = "servo-poses.toml"

[camera]
name = "usb camera"
backend = "v4l2"
device = "/dev/video-test"
width = 640
height = 480
fps = 30
fov_deg = 60.0
pixel_format = "MJPG"

[learning]
mode = "shadow"
policy_transport = "unix-socket"
policy_path = "artifacts/policies/latest.onnx"
"#,
        )
        .expect("failed to write main config");

        fs::write(
            &servo_config,
            r#"
[bus.feetech]
port = "/dev/ttyACM-test"
baud_rate = 1000000
telemetry_stride = 4

[[servo_eeprom.entries]]
name = "status_return_level"
address = 8
width = "u8"
value = 1

[[servo_eeprom.entries]]
name = "max_torque_limit"
address = 16
width = "u16"
value = 1000

[safety]
max_body_pitch_deg = 18.0
max_body_roll_deg = 17.0
max_servo_temp_c = 60
min_bus_voltage_v = 6.1
max_servo_load_pct = 75.0

[locomotion]
command_hz = 25

[locomotion.stand_up]
duration_seconds = 12.0

[[legs]]
name = "front_left"
coxa_servo_id = 11
femur_servo_id = 12
tibia_servo_id = 13
coxa_stand_reference_ticks = 2048
femur_stand_reference_ticks = 2150
tibia_stand_reference_ticks = 1850
coxa_lay_down_ticks = 2000
femur_lay_down_ticks = 2050
tibia_lay_down_ticks = 2040
coxa_forward_sign = 1
femur_lift_sign = -1
tibia_lift_sign = 1
"#,
        )
        .expect("failed to write shared servo config");

        fs::write(
            &pose_config,
            r#"
[stand_reference.front_left]
coxa_deg = 1.0
femur_deg = 52.5
tibia_deg = -120.0

[lay_down.front_left]
coxa_deg = 0.0
femur_deg = 0.0
tibia_deg = 0.0
"#,
        )
        .expect("failed to write shared pose config");

        let config = RobotConfig::load_from_path(&main_config).expect("config should load");

        assert_eq!(config.bus.feetech.port, "/dev/ttyACM-test");
        assert_eq!(config.bus.feetech.telemetry_stride, 4);
        assert_eq!(config.servo_eeprom.entries.len(), 2);
        assert_eq!(config.servo_eeprom.entries[0].address, 8);
        assert_eq!(config.servo_eeprom.entries[1].value, 1000);
        assert_eq!(config.safety.min_bus_voltage_v, 6.1);
        assert_eq!(config.locomotion.command_hz, 25);
        assert_eq!(config.locomotion.stand_up.duration_seconds, 12.0);
        assert_eq!(config.legs.len(), 1);
        assert_eq!(config.legs[0].name, "front_left");
        let stand_reference_pose = config
            .pose_for_leg(SemanticPoseKind::StandReference, "front_left")
            .expect("pose store should provide a stand-reference pose");
        assert_eq!(stand_reference_pose.coxa_deg, 1.0);
        assert_eq!(stand_reference_pose.femur_deg, 52.5);
        assert_eq!(stand_reference_pose.tibia_deg, -120.0);

        fs::remove_dir_all(&temp_dir).expect("failed to clean temp config dir");
    }
}
