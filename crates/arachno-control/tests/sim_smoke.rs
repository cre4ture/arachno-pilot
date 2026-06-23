use std::path::PathBuf;

use arachno_camera::RobotCamera;
use arachno_control::SpiderController;
use arachno_core::{RobotConfig, SemanticPoseKind};
use arachno_sim_hal::SimServoBus;

#[test]
fn spider_controller_runs_against_simulated_servo_bus() {
    let config_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/robot/default.toml");
    let config = RobotConfig::load_from_path(config_path).expect("default config should load");
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
