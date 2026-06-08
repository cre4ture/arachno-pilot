use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JointCommand {
    pub servo_id: u8,
    pub position_ticks: u16,
    pub speed_ticks: u16,
    pub acceleration: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CameraFrameMeta {
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ImuTelemetry {
    pub timestamp_ms: u64,
    pub accel_mps2: [f32; 3],
    pub gyro_rad_s: [f32; 3],
    pub temperature_c: Option<f32>,
    pub status_bits: Option<u16>,
    pub faults: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RobotSnapshot {
    pub timestamp_ms: u64,
    pub body_mode: String,
    pub telemetry: Vec<ServoTelemetry>,
    pub camera: Option<CameraFrameMeta>,
    pub imu: Option<ImuTelemetry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    >(
        value: &T,
    ) {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*value, back);
    }

    // JointCommand -------------------------------------------------------

    #[test]
    fn joint_command_default_roundtrip() {
        roundtrip(&JointCommand::default());
    }

    #[test]
    fn joint_command_populated_roundtrip() {
        roundtrip(&JointCommand {
            servo_id: 3,
            position_ticks: 2048,
            speed_ticks: 512,
            acceleration: 10,
        });
    }

    // ServoTelemetry -----------------------------------------------------

    #[test]
    fn servo_telemetry_default_roundtrip() {
        roundtrip(&ServoTelemetry::default());
    }

    #[test]
    fn servo_telemetry_populated_roundtrip() {
        roundtrip(&ServoTelemetry {
            servo_id: 5,
            present_position_ticks: 1500,
            present_speed_ticks: -100,
            present_load_pct: 42.5,
            present_voltage_v: 7.4,
            present_current_ma: Some(350),
            present_temperature_c: Some(38),
            status_bits: Some(0b0000_0011),
            faults: vec!["overload".to_string()],
            moving: true,
        });
    }

    // CameraFrameMeta ----------------------------------------------------

    #[test]
    fn camera_frame_meta_default_roundtrip() {
        roundtrip(&CameraFrameMeta::default());
    }

    #[test]
    fn camera_frame_meta_populated_roundtrip() {
        roundtrip(&CameraFrameMeta {
            frame_index: 99,
            width: 1280,
            height: 720,
            format: "RGB8".to_string(),
        });
    }

    // ImuTelemetry -------------------------------------------------------

    #[test]
    fn imu_telemetry_default_roundtrip() {
        roundtrip(&ImuTelemetry::default());
    }

    #[test]
    fn imu_telemetry_populated_roundtrip() {
        roundtrip(&ImuTelemetry {
            timestamp_ms: 123_456_789,
            accel_mps2: [0.1, -9.81, 0.05],
            gyro_rad_s: [0.01, 0.0, -0.02],
            temperature_c: Some(25.3),
            status_bits: Some(0x00FF),
            faults: vec!["gyro_saturation".to_string(), "temp_warning".to_string()],
        });
    }

    // RobotSnapshot ------------------------------------------------------

    #[test]
    fn robot_snapshot_default_roundtrip() {
        roundtrip(&RobotSnapshot::default());
    }

    #[test]
    fn robot_snapshot_populated_roundtrip() {
        let servo = ServoTelemetry {
            servo_id: 1,
            present_position_ticks: 2000,
            present_speed_ticks: 0,
            present_load_pct: 10.0,
            present_voltage_v: 7.2,
            present_current_ma: None,
            present_temperature_c: Some(30),
            status_bits: None,
            faults: vec![],
            moving: false,
        };
        let imu = ImuTelemetry {
            timestamp_ms: 1000,
            accel_mps2: [0.0, 0.0, 9.81],
            gyro_rad_s: [0.0, 0.0, 0.0],
            temperature_c: None,
            status_bits: None,
            faults: vec![],
        };
        let cam = CameraFrameMeta {
            frame_index: 7,
            width: 640,
            height: 480,
            format: "MJPEG".to_string(),
        };
        roundtrip(&RobotSnapshot {
            timestamp_ms: 999_999,
            body_mode: "walk_forward".to_string(),
            telemetry: vec![servo],
            camera: Some(cam),
            imu: Some(imu),
        });
    }
}
