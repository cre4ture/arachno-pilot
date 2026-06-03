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
