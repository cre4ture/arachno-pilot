use std::path::PathBuf;

use arachno_camera::RobotCamera;
use arachno_control::SpiderController;
use arachno_core::{RobotConfig, SemanticPoseKind};
use arachno_sim_hal::SimServoBus;

fn load_config() -> RobotConfig {
    let config_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/robot/default.toml");
    RobotConfig::load_from_path(config_path).expect("default config should load")
}

#[test]
fn spider_controller_runs_against_simulated_servo_bus() {
    let config = load_config();
    let servo_ids = config.all_servo_ids();

    let servo_bus = SimServoBus::from_robot_config(&config, SemanticPoseKind::LayDown);
    let camera = RobotCamera::new(config.camera.clone());
    let mut controller = SpiderController::new(config, servo_bus, camera, None);

    controller
        .initialize()
        .expect("controller should initialize against simulated hardware");
    let snapshot = controller
        .step_stand_reference_pose()
        .expect("stand reference step should succeed");

    assert_eq!(snapshot.body_mode, "stand_reference");
    assert_eq!(snapshot.telemetry.len(), servo_ids.len());
    assert!(snapshot.camera.is_some());
}

#[test]
fn spider_controller_with_sim_imu_produces_imu_telemetry() {
    let config = load_config();
    let servo_ids = config.all_servo_ids();

    let (servo_bus, sim_imu) = SimServoBus::build_pair(&config, SemanticPoseKind::StandReference);
    let camera = RobotCamera::new(config.camera.clone());
    let mut controller =
        SpiderController::new(config, servo_bus, camera, Some(Box::new(sim_imu)));

    controller
        .initialize()
        .expect("controller should initialize");
    let snapshot = controller
        .step_stand_reference_pose()
        .expect("stand reference step should succeed");

    assert_eq!(snapshot.telemetry.len(), servo_ids.len());
    let imu = snapshot.imu.expect("IMU telemetry should be present when SimImu is wired");

    // Standing level: z-axis should carry most of gravity (~9.81 m/s²).
    let az = imu.accel_mps2[2];
    assert!(az > 9.0 && az <= 9.82, "az={az} expected near 9.81 for level stance");
}
