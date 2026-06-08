use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use arachno_core::{RobotConfig, TripodGait};
use arachno_hal::{
    CameraSource, HalResult, ImuSource, ServoBus, enable_torque_on_current_position,
};
use arachno_msg::RobotSnapshot;

pub struct SpiderController<B, C> {
    config: RobotConfig,
    gait: TripodGait,
    servo_bus: B,
    camera: C,
    imu: Option<Box<dyn ImuSource>>,
}

impl<B, C> SpiderController<B, C>
where
    B: ServoBus,
    C: CameraSource,
{
    pub fn new(
        config: RobotConfig,
        servo_bus: B,
        camera: C,
        imu: Option<Box<dyn ImuSource>>,
    ) -> Self {
        Self {
            config,
            gait: TripodGait,
            servo_bus,
            camera,
            imu,
        }
    }

    pub fn initialize(&mut self) -> HalResult<()> {
        enable_torque_on_current_position(&mut self.servo_bus)?;
        self.camera.start()?;
        if let Some(imu) = self.imu.as_mut() {
            imu.start()?;
        }
        Ok(())
    }

    pub fn step_stand_reference_pose(&mut self) -> HalResult<RobotSnapshot> {
        let commands = self.gait.stand_reference_commands(&self.config);
        self.servo_bus.sync_write_positions(&commands)?;
        self.poll_snapshot("stand_reference")
    }

    // Backward-compatible alias for older callers.
    pub fn step_home_pose(&mut self) -> HalResult<RobotSnapshot> {
        self.step_stand_reference_pose()
    }

    pub fn poll_snapshot(&mut self, body_mode: &str) -> HalResult<RobotSnapshot> {
        let telemetry = self.poll_servo_telemetry()?;
        let camera = self.camera.next_frame()?;
        let imu = self.poll_imu_sample()?;

        Ok(RobotSnapshot {
            timestamp_ms: now_ms(),
            body_mode: body_mode.to_owned(),
            telemetry,
            camera,
            imu,
        })
    }

    pub fn camera_pipeline(&self) -> &str {
        self.camera.pipeline_description()
    }

    pub fn imu_description(&self) -> Option<&str> {
        self.imu.as_ref().map(|imu| imu.description())
    }

    fn poll_servo_telemetry(&mut self) -> HalResult<Vec<arachno_msg::ServoTelemetry>> {
        let servo_ids = self.servo_bus.servo_ids().to_vec();
        servo_ids
            .into_iter()
            .map(|servo_id| self.servo_bus.read_feedback(servo_id))
            .collect::<HalResult<Vec<_>>>()
    }

    fn poll_imu_sample(&mut self) -> HalResult<Option<arachno_msg::ImuTelemetry>> {
        let Some(imu) = self.imu.as_mut() else {
            return Ok(None);
        };

        for _ in 0..8 {
            if let Some(sample) = imu.next_sample()? {
                return Ok(Some(sample));
            }
            thread::sleep(Duration::from_millis(5));
        }

        Ok(None)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, env, fs};

    use arachno_hal::HalError;
    use arachno_msg::{CameraFrameMeta, ImuTelemetry, ServoTelemetry};

    #[derive(Default)]
    struct MockServoBus {
        ids: Vec<u8>,
        feedback: BTreeMap<u8, ServoTelemetry>,
        writes: Vec<Vec<arachno_msg::JointCommand>>,
        torque_enabled: Option<bool>,
    }

    impl ServoBus for MockServoBus {
        fn servo_ids(&self) -> &[u8] {
            &self.ids
        }

        fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
            self.torque_enabled = Some(enabled);
            Ok(())
        }

        fn sync_write_positions(
            &mut self,
            commands: &[arachno_msg::JointCommand],
        ) -> HalResult<()> {
            self.writes.push(commands.to_vec());
            Ok(())
        }

        fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
            self.feedback.get(&servo_id).cloned().ok_or_else(|| {
                HalError::Communication(format!("missing mock feedback for servo {servo_id}"))
            })
        }
    }

    #[derive(Default)]
    struct MockCamera {
        started: bool,
        frame: Option<CameraFrameMeta>,
    }

    impl CameraSource for MockCamera {
        fn start(&mut self) -> HalResult<()> {
            self.started = true;
            Ok(())
        }

        fn next_frame(&mut self) -> HalResult<Option<CameraFrameMeta>> {
            Ok(self.frame.clone())
        }

        fn pipeline_description(&self) -> &str {
            "mock-camera"
        }
    }

    #[derive(Default)]
    struct MockImu {
        started: bool,
        sample: Option<ImuTelemetry>,
    }

    impl ImuSource for MockImu {
        fn start(&mut self) -> HalResult<()> {
            self.started = true;
            Ok(())
        }

        fn next_sample(&mut self) -> HalResult<Option<ImuTelemetry>> {
            Ok(self.sample.clone())
        }

        fn description(&self) -> &str {
            "mock-imu"
        }
    }

    fn load_test_config() -> RobotConfig {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("arachno-control-config-{unique}"));
        fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

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

[[legs]]
name = "front_left"
coxa_servo_id = 11
femur_servo_id = 12
tibia_servo_id = 13
coxa_zero_reference_ticks = 2000
femur_zero_reference_ticks = 2000
tibia_zero_reference_ticks = 2000
coxa_forward_sign = 1
femur_lift_sign = -1
tibia_lift_sign = 1
"#,
        )
        .expect("failed to write servo config");

        fs::write(
            &pose_config,
            r#"
[stand_reference.front_left]
coxa_deg = 10.0
femur_deg = 20.0
tibia_deg = -30.0
"#,
        )
        .expect("failed to write pose config");

        let config = RobotConfig::load_from_path(&main_config).expect("config should load");
        fs::remove_dir_all(&temp_dir).expect("failed to clean temp dir");
        config
    }

    #[test]
    fn initialize_syncs_pose_and_starts_devices() {
        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2100,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2200,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2300,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };
        let camera = MockCamera::default();
        let imu = Some(Box::new(MockImu::default()) as Box<dyn ImuSource>);

        let mut controller = SpiderController::new(config, servo_bus, camera, imu);
        controller.initialize().expect("initialize should succeed");

        assert_eq!(controller.servo_bus.writes.len(), 1);
        assert_eq!(controller.servo_bus.torque_enabled, Some(true));
        assert!(controller.camera.started);
        assert!(controller.imu.as_ref().is_some());
    }

    #[test]
    fn step_stand_reference_pose_writes_named_pose_and_returns_snapshot() {
        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2100,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2200,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2300,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };
        let camera = MockCamera {
            frame: Some(CameraFrameMeta {
                frame_index: 1,
                width: 640,
                height: 480,
                format: "MJPG".to_owned(),
            }),
            ..MockCamera::default()
        };

        let mut controller = SpiderController::new(config, servo_bus, camera, None);
        let snapshot = controller
            .step_stand_reference_pose()
            .expect("step should succeed");

        assert_eq!(snapshot.body_mode, "stand_reference");
        assert_eq!(snapshot.telemetry.len(), 3);
        assert_eq!(snapshot.camera.as_ref().map(|frame| frame.frame_index), Some(1));
        assert_eq!(controller.servo_bus.writes.len(), 1);
        let commands = &controller.servo_bus.writes[0];
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].servo_id, 11);
        assert_eq!(commands[0].position_ticks, 2114);
        assert_eq!(commands[1].position_ticks, 1772);
        assert_eq!(commands[2].position_ticks, 1659);
    }
}
