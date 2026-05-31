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
pub struct LegConfig {
    pub name: String,
    pub coxa_servo_id: u8,
    pub femur_servo_id: u8,
    pub tibia_servo_id: u8,
    pub coxa_home_ticks: u16,
    pub femur_home_ticks: u16,
    pub tibia_home_ticks: u16,
}

impl RobotConfig {
    pub fn all_servo_ids(&self) -> Vec<u8> {
        self.legs
            .iter()
            .flat_map(|leg| [leg.coxa_servo_id, leg.femur_servo_id, leg.tibia_servo_id])
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TripodGait;

impl TripodGait {
    pub fn home_commands(&self, config: &RobotConfig) -> Vec<JointCommand> {
        let mut commands = Vec::with_capacity(config.legs.len() * 3);

        for leg in &config.legs {
            commands.push(JointCommand {
                servo_id: leg.coxa_servo_id,
                position_ticks: leg.coxa_home_ticks,
                speed_ticks: 200,
                acceleration: 10,
            });
            commands.push(JointCommand {
                servo_id: leg.femur_servo_id,
                position_ticks: leg.femur_home_ticks,
                speed_ticks: 200,
                acceleration: 10,
            });
            commands.push(JointCommand {
                servo_id: leg.tibia_servo_id,
                position_ticks: leg.tibia_home_ticks,
                speed_ticks: 200,
                acceleration: 10,
            });
        }

        commands
    }
}
