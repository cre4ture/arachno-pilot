use arachno_core::{RobotConfig, TripodGait};
use arachno_hal::{CameraSource, HalResult, ServoBus};
use arachno_msg::RobotSnapshot;

pub struct SpiderController<B, C> {
    config: RobotConfig,
    gait: TripodGait,
    servo_bus: B,
    camera: C,
}

impl<B, C> SpiderController<B, C>
where
    B: ServoBus,
    C: CameraSource,
{
    pub fn new(config: RobotConfig, servo_bus: B, camera: C) -> Self {
        Self {
            config,
            gait: TripodGait,
            servo_bus,
            camera,
        }
    }

    pub fn initialize(&mut self) -> HalResult<()> {
        self.servo_bus.enable_torque(true)?;
        self.camera.start()?;
        Ok(())
    }

    pub fn step_home_pose(&mut self) -> HalResult<RobotSnapshot> {
        let commands = self.gait.home_commands(&self.config);
        self.servo_bus.sync_write_positions(&commands)?;

        let servo_ids = self.servo_bus.servo_ids().to_vec();
        let telemetry = servo_ids
            .into_iter()
            .map(|servo_id| self.servo_bus.read_feedback(servo_id))
            .collect::<HalResult<Vec<_>>>()?;

        let camera = self.camera.next_frame()?;

        Ok(RobotSnapshot {
            timestamp_ms: 0,
            body_mode: "home".to_owned(),
            telemetry,
            camera,
            imu: None,
        })
    }

    pub fn camera_pipeline(&self) -> &str {
        self.camera.pipeline_description()
    }
}
