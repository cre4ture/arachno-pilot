use arachno_msg::{CameraFrameMeta, JointCommand, ServoTelemetry};
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

pub trait CameraSource {
    fn start(&mut self) -> HalResult<()>;
    fn next_frame(&mut self) -> HalResult<Option<CameraFrameMeta>>;
    fn pipeline_description(&self) -> &str;
}

pub trait ExtensionDevice {
    fn name(&self) -> &str;
    fn tick(&mut self) -> HalResult<()>;
}
