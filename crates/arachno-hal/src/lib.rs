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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockServoBus {
        ids: Vec<u8>,
        feedback: BTreeMap<u8, ServoTelemetry>,
        torque_enabled: Option<bool>,
        writes: Vec<Vec<JointCommand>>,
    }

    impl ServoBus for MockServoBus {
        fn servo_ids(&self) -> &[u8] {
            &self.ids
        }

        fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
            self.torque_enabled = Some(enabled);
            Ok(())
        }

        fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()> {
            self.writes.push(commands.to_vec());
            Ok(())
        }

        fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
            self.feedback.get(&servo_id).cloned().ok_or_else(|| {
                HalError::Communication(format!("missing mock feedback for servo {servo_id}"))
            })
        }
    }

    #[test]
    fn read_current_pose_collects_feedback_ticks() {
        let mut bus = MockServoBus {
            ids: vec![11, 12],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 1234,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2345,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };

        let pose = read_current_pose(&mut bus, &[11, 12]).expect("pose should read");

        assert_eq!(pose.get(&11), Some(&1234));
        assert_eq!(pose.get(&12), Some(&2345));
    }

    #[test]
    fn sync_current_pose_writes_live_positions_back_as_targets() {
        let mut bus = MockServoBus {
            ids: vec![11, 12],
            feedback: BTreeMap::from([
                (
                    11,
                    ServoTelemetry {
                        servo_id: 11,
                        present_position_ticks: 1500,
                        ..ServoTelemetry::default()
                    },
                ),
                (
                    12,
                    ServoTelemetry {
                        servo_id: 12,
                        present_position_ticks: 2600,
                        ..ServoTelemetry::default()
                    },
                ),
            ]),
            ..MockServoBus::default()
        };

        sync_current_pose(&mut bus, &[11, 12]).expect("sync should succeed");

        assert_eq!(bus.writes.len(), 1);
        assert_eq!(
            bus.writes[0],
            vec![
                JointCommand {
                    servo_id: 11,
                    position_ticks: 1500,
                    speed_ticks: 0,
                    acceleration: 0,
                },
                JointCommand {
                    servo_id: 12,
                    position_ticks: 2600,
                    speed_ticks: 0,
                    acceleration: 0,
                },
            ]
        );
    }

    #[test]
    fn enable_torque_on_current_position_syncs_before_enabling() {
        let mut bus = MockServoBus {
            ids: vec![11],
            feedback: BTreeMap::from([(
                11,
                ServoTelemetry {
                    servo_id: 11,
                    present_position_ticks: 1700,
                    ..ServoTelemetry::default()
                },
            )]),
            ..MockServoBus::default()
        };

        enable_torque_on_current_position(&mut bus).expect("torque enable should succeed");

        assert_eq!(bus.writes.len(), 1);
        assert_eq!(bus.writes[0][0].position_ticks, 1700);
        assert_eq!(bus.torque_enabled, Some(true));
    }
}
