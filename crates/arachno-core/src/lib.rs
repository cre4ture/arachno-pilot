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
    pub semantic_calibration_store: Option<SemanticCalibrationStoreConfig>,
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
pub struct SemanticCalibrationStoreConfig {
    pub path: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServoRegisterWidth {
    U8,
    U16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(alias = "coxa_home_ticks")]
    pub coxa_stand_reference_ticks: u16,
    #[serde(alias = "femur_home_ticks")]
    pub femur_stand_reference_ticks: u16,
    #[serde(alias = "tibia_home_ticks")]
    pub tibia_stand_reference_ticks: u16,
    #[serde(default)]
    pub coxa_lay_down_ticks: Option<u16>,
    #[serde(default)]
    pub femur_lay_down_ticks: Option<u16>,
    #[serde(default)]
    pub tibia_lay_down_ticks: Option<u16>,
    #[serde(default)]
    pub coxa_zero_pose_ticks: Option<u16>,
    #[serde(default)]
    pub femur_zero_pose_ticks: Option<u16>,
    #[serde(default)]
    pub tibia_zero_pose_ticks: Option<u16>,
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

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            feetech: FeetechBusConfig::default(),
        }
    }
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

impl Default for ServoRegisterWidth {
    fn default() -> Self {
        Self::U8
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
            toward_center_sign(self.femur_stand_reference_ticks),
        )
    }

    pub fn tibia_lift_sign(&self) -> i16 {
        resolve_sign(
            self.tibia_lift_sign,
            toward_center_sign(self.tibia_stand_reference_ticks),
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

    pub fn coxa_zero_pose_ticks(&self) -> u16 {
        self.coxa_zero_pose_ticks
            .or(self.coxa_lay_down_ticks)
            .unwrap_or(self.coxa_stand_reference_ticks)
    }

    pub fn femur_zero_pose_ticks(&self) -> u16 {
        self.femur_zero_pose_ticks
            .or(self.femur_lay_down_ticks)
            .unwrap_or(self.femur_stand_reference_ticks)
    }

    pub fn tibia_zero_pose_ticks(&self) -> u16 {
        self.tibia_zero_pose_ticks
            .or(self.tibia_lay_down_ticks)
            .unwrap_or(self.tibia_stand_reference_ticks)
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
        let tibia_projection = self.tibia_length_cm()
            * (semantic_femur_deg + semantic_tibia_deg).to_radians().cos();
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
        let mut pose = BTreeMap::new();

        for leg in &config.legs {
            pose.insert(leg.coxa_servo_id, leg.coxa_stand_reference_ticks);
            pose.insert(leg.femur_servo_id, leg.femur_stand_reference_ticks);
            pose.insert(leg.tibia_servo_id, leg.tibia_stand_reference_ticks);
        }

        pose
    }

    pub fn stand_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        self.stand_reference_pose(config)
    }

    pub fn lay_down_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        let mut pose = BTreeMap::new();

        for leg in &config.legs {
            pose.insert(
                leg.coxa_servo_id,
                leg.coxa_lay_down_ticks
                    .unwrap_or(leg.coxa_stand_reference_ticks),
            );
            pose.insert(
                leg.femur_servo_id,
                leg.femur_lay_down_ticks
                    .unwrap_or(leg.femur_stand_reference_ticks),
            );
            pose.insert(
                leg.tibia_servo_id,
                leg.tibia_lay_down_ticks
                    .unwrap_or(leg.tibia_stand_reference_ticks),
            );
        }

        pose
    }

    pub fn zero_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        let mut pose = BTreeMap::new();

        for leg in &config.legs {
            pose.insert(leg.coxa_servo_id, leg.coxa_zero_pose_ticks());
            pose.insert(leg.femur_servo_id, leg.femur_zero_pose_ticks());
            pose.insert(leg.tibia_servo_id, leg.tibia_zero_pose_ticks());
        }

        pose
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
                    leg.coxa_stand_reference_ticks,
                    leg.coxa_forward_sign() * coxa_offset,
                ),
            );
            pose.insert(
                leg.femur_servo_id,
                offset_ticks(leg.femur_stand_reference_ticks, femur_offset),
            );
            pose.insert(
                leg.tibia_servo_id,
                offset_ticks(leg.tibia_stand_reference_ticks, tibia_offset),
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

        fs::remove_dir_all(&temp_dir).expect("failed to clean temp config dir");
    }
}
