use std::collections::BTreeMap;

use arachno_msg::{CameraFrameMeta, ImuTelemetry, JointCommand, ServoTelemetry};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("device not available: {0}")]
    DeviceUnavailable(String),
    #[error("communication failure: {0}")]
    Communication(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

pub type HalResult<T> = Result<T, HalError>;

pub trait ServoBus {
    fn servo_ids(&self) -> &[u8];
    fn enable_torque(&mut self, enabled: bool) -> HalResult<()>;
    fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()>;
    fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry>;
}

pub fn read_current_pose<B>(bus: &mut B, servo_ids: &[u8]) -> HalResult<BTreeMap<u8, u16>>
where
    B: ServoBus,
{
    let mut pose = BTreeMap::new();
    for &servo_id in servo_ids {
        let telemetry = bus.read_feedback(servo_id)?;
        pose.insert(servo_id, telemetry.present_position_ticks);
    }
    Ok(pose)
}

pub fn sync_current_pose<B>(bus: &mut B, servo_ids: &[u8]) -> HalResult<()>
where
    B: ServoBus,
{
    let current_pose = read_current_pose(bus, servo_ids)?;
    let commands = current_pose
        .iter()
        .map(|(&servo_id, &position_ticks)| JointCommand {
            servo_id,
            position_ticks,
            speed_ticks: 0,
            acceleration: 0,
        })
        .collect::<Vec<_>>();
    bus.sync_write_positions(&commands)
}

pub fn enable_torque_on_current_position<B>(bus: &mut B) -> HalResult<()>
where
    B: ServoBus,
{
    let servo_ids = bus.servo_ids().to_vec();
    sync_current_pose(bus, &servo_ids)?;
    bus.enable_torque(true)
}

pub trait CameraSource {
    fn start(&mut self) -> HalResult<()>;
    fn next_frame(&mut self) -> HalResult<Option<CameraFrameMeta>>;
    fn pipeline_description(&self) -> &str;
}

pub trait ImuSource {
    fn start(&mut self) -> HalResult<()>;
    fn next_sample(&mut self) -> HalResult<Option<ImuTelemetry>>;
    fn description(&self) -> &str;
}

pub trait ExtensionDevice {
    fn name(&self) -> &str;
    fn tick(&mut self) -> HalResult<()>;
}
