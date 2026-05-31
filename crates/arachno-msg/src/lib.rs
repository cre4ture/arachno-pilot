use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JointCommand {
    pub servo_id: u8,
    pub position_ticks: u16,
    pub speed_ticks: u16,
    pub acceleration: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServoTelemetry {
    pub servo_id: u8,
    pub present_position_ticks: u16,
    pub present_speed_ticks: i16,
    pub present_load_pct: f32,
    pub present_voltage_v: f32,
    pub present_current_ma: Option<u16>,
    pub present_temperature_c: Option<u8>,
    pub status_bits: Option<u8>,
    pub faults: Vec<String>,
    pub moving: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CameraFrameMeta {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RobotSnapshot {
    pub timestamp_ms: u64,
    pub body_mode: String,
    pub telemetry: Vec<ServoTelemetry>,
    pub camera: Option<CameraFrameMeta>,
}
