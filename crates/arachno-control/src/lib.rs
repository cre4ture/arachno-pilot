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
        assert_eq!(
            snapshot.camera.as_ref().map(|frame| frame.frame_index),
            Some(1)
        );
        assert_eq!(controller.servo_bus.writes.len(), 1);
        let commands = &controller.servo_bus.writes[0];
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].servo_id, 11);
        assert_eq!(commands[0].position_ticks, 2114);
        assert_eq!(commands[1].position_ticks, 1772);
        assert_eq!(commands[2].position_ticks, 1659);
    }

    // -----------------------------------------------------------------------
    // step_home_pose is a backward-compat alias for step_stand_reference_pose
    // -----------------------------------------------------------------------

    #[test]
    fn step_home_pose_alias_produces_same_result_as_step_stand_reference_pose() {
        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };

        let mut controller = SpiderController::new(config, servo_bus, MockCamera::default(), None);
        let snapshot = controller
            .step_home_pose()
            .expect("step_home_pose should succeed");

        // The alias must produce the same body_mode label as the canonical method.
        assert_eq!(snapshot.body_mode, "stand_reference");
        // Exactly one sync_write_positions call must have been issued.
        assert_eq!(controller.servo_bus.writes.len(), 1);
    }

    // -----------------------------------------------------------------------
    // poll_snapshot carries the caller-supplied body_mode label
    // -----------------------------------------------------------------------

    #[test]
    fn poll_snapshot_uses_supplied_body_mode_label() {
        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };

        let mut controller = SpiderController::new(config, servo_bus, MockCamera::default(), None);
        let snapshot = controller
            .poll_snapshot("walking")
            .expect("poll_snapshot should succeed");

        assert_eq!(snapshot.body_mode, "walking");
        // No servo write should have been issued — poll_snapshot only reads.
        assert_eq!(controller.servo_bus.writes.len(), 0);
        // IMU field must be None because no IMU was injected.
        assert!(snapshot.imu.is_none());
    }

    // -----------------------------------------------------------------------
    // camera_pipeline and imu_description accessors
    // -----------------------------------------------------------------------

    #[test]
    fn camera_pipeline_returns_mock_description() {
        let config = load_test_config();
        let controller =
            SpiderController::new(config, MockServoBus::default(), MockCamera::default(), None);

        assert_eq!(controller.camera_pipeline(), "mock-camera");
    }

    #[test]
    fn imu_description_none_when_no_imu_injected() {
        let config = load_test_config();
        let controller =
            SpiderController::new(config, MockServoBus::default(), MockCamera::default(), None);

        assert!(controller.imu_description().is_none());
    }

    #[test]
    fn imu_description_some_when_imu_injected() {
        let config = load_test_config();
        let imu = Some(Box::new(MockImu::default()) as Box<dyn ImuSource>);
        let controller =
            SpiderController::new(config, MockServoBus::default(), MockCamera::default(), imu);

        assert_eq!(controller.imu_description(), Some("mock-imu"));
    }

    // -----------------------------------------------------------------------
    // initialize without IMU must not crash and must not start an absent IMU
    // -----------------------------------------------------------------------

    #[test]
    fn initialize_without_imu_does_not_require_imu() {
        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };

        let mut controller = SpiderController::new(config, servo_bus, MockCamera::default(), None);
        controller
            .initialize()
            .expect("initialize without IMU should succeed");

        // Torque must be enabled even without an IMU.
        assert_eq!(controller.servo_bus.torque_enabled, Some(true));
        // Camera must have been started.
        assert!(controller.camera.started);
        // No IMU should be present.
        assert!(controller.imu.is_none());
    }

    // -----------------------------------------------------------------------
    // poll_snapshot with a live IMU sample propagates it into the snapshot
    // -----------------------------------------------------------------------

    #[test]
    fn poll_snapshot_with_imu_returns_imu_telemetry() {
        use arachno_msg::ImuTelemetry;

        let config = load_test_config();
        let servo_bus = MockServoBus {
            ids: vec![11, 12, 13],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    13,
                    ServoTelemetry {
                        servo_id: 13,
                        present_position_ticks: 2048,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };
        let imu = Some(Box::new(MockImu {
            sample: Some(ImuTelemetry {
                accel_mps2: [1.0, 2.0, 9.8],
                gyro_rad_s: [0.1, 0.2, 0.3],
                ..ImuTelemetry::default()
            }),
            ..MockImu::default()
        }) as Box<dyn ImuSource>);

        let mut controller = SpiderController::new(config, servo_bus, MockCamera::default(), imu);
        let snapshot = controller
            .poll_snapshot("imu_test")
            .expect("poll_snapshot with IMU should succeed");

        let imu_data = snapshot.imu.expect("IMU telemetry must be present");
        assert!((imu_data.accel_mps2[0] - 1.0).abs() < 1e-6);
        assert!((imu_data.accel_mps2[2] - 9.8).abs() < 1e-5);
        assert!((imu_data.gyro_rad_s[1] - 0.2).abs() < 1e-6);
    }
}
