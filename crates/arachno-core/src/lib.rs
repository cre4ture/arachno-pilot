use std::{collections::BTreeMap, f32::consts::PI};

use arachno_msg::JointCommand;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotConfig {
    pub deployment: DeploymentConfig,
    pub robot: RobotMeta,
    pub bus: BusConfig,
    pub camera: CameraConfig,
    #[serde(default)]
    pub imu: Option<ImuConfig>,
    pub safety: SafetyConfig,
    pub learning: LearningConfig,
    #[serde(default)]
    pub locomotion: LocomotionConfig,
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
    pub stand: StandConfig,
    #[serde(default)]
    pub tripod: TripodWalkConfig,
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
    pub coxa_home_ticks: u16,
    pub femur_home_ticks: u16,
    pub tibia_home_ticks: u16,
    #[serde(default)]
    pub coxa_forward_sign: i8,
    #[serde(default)]
    pub femur_lift_sign: i8,
    #[serde(default)]
    pub tibia_lift_sign: i8,
}

impl RobotConfig {
    pub fn all_servo_ids(&self) -> Vec<u8> {
        self.legs
            .iter()
            .flat_map(|leg| [leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id])
            .collect()
    }
}

impl Default for LocomotionConfig {
    fn default() -> Self {
        Self {
            command_hz: default_command_hz(),
            stand: StandConfig::default(),
            tripod: TripodWalkConfig::default(),
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
            toward_center_sign(self.femur_home_ticks),
        )
    }

    pub fn tibia_lift_sign(&self) -> i16 {
        resolve_sign(
            self.tibia_lift_sign,
            toward_center_sign(self.tibia_home_ticks),
        )
    }

    pub fn is_tripod_a(&self) -> bool {
        matches!(
            self.name.as_str(),
            "front_left" | "middle_right" | "rear_left"
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct TripodGait;

impl TripodGait {
    pub fn stand_pose(&self, config: &RobotConfig) -> BTreeMap<u8, u16> {
        let mut pose = BTreeMap::new();

        for leg in &config.legs {
            pose.insert(leg.coxa_servo_id, leg.coxa_home_ticks);
            pose.insert(leg.femur_servo_id, leg.femur_home_ticks);
            pose.insert(leg.tibia_servo_id, leg.tibia_home_ticks);
        }

        pose
    }

    pub fn stand_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        pose_to_commands(&self.stand_pose(config))
    }

    pub fn slow_walk_pose(&self, config: &RobotConfig, phase: f32) -> BTreeMap<u8, u16> {
        let mut pose = self.stand_pose(config);
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
                offset_ticks(leg.coxa_home_ticks, leg.coxa_forward_sign() * coxa_offset),
            );
            pose.insert(
                leg.femur_servo_id,
                offset_ticks(leg.femur_home_ticks, femur_offset),
            );
            pose.insert(
                leg.tibia_servo_id,
                offset_ticks(leg.tibia_home_ticks, tibia_offset),
            );
        }

        pose
    }

    pub fn slow_walk_commands(&self, config: &RobotConfig, phase: f32) -> Vec<JointCommand> {
        pose_to_commands(&self.slow_walk_pose(config, phase))
    }

    pub fn home_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        self.stand_commands(config)
    }
}

fn default_command_hz() -> u16 {
    20
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

fn toward_center_sign(home_ticks: u16) -> i16 {
    if home_ticks >= 2048 { -1 } else { 1 }
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

fn offset_ticks(home_ticks: u16, delta: i16) -> u16 {
    (i32::from(home_ticks) + i32::from(delta)).clamp(0, 4095) as u16
}
