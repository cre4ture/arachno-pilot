use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, SystemTime},
};

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
    #[error("probe timed out after {elapsed_ms}ms; lingering servos: {lingering:?}")]
    ProbeTimeout { elapsed_ms: u64, lingering: Vec<u8> },
}

/// Parameters controlling the poll loop used to detect when servos have stopped moving.
///
/// Used by `wait_for_servos_to_settle` and by `arachno-calibrate`'s range scan.
#[derive(Debug, Clone, Copy)]
pub struct ServoPollParams {
    /// How long to sleep between telemetry reads (ms).
    pub poll_ms: u64,
    /// Speed (ticks/s) at or below which a servo is considered stopped.
    pub stop_speed_ticks: u16,
    /// Number of consecutive polls that must show stopped before confirming.
    pub confirm_stopped_samples: u8,
    /// Maximum time to wait before returning a timeout error (ms).
    pub timeout_ms: u64,
}

/// Returns `true` when a servo is considered stopped based on its telemetry.
pub fn servo_has_stopped(telemetry: &ServoTelemetry, stop_speed_ticks: u16) -> bool {
    let speed_abs = i32::from(telemetry.present_speed_ticks).abs();
    !telemetry.moving || speed_abs <= i32::from(stop_speed_ticks)
}

/// Poll the given servo IDs until all have stopped moving, then return the final telemetry
/// for each servo.
///
/// Unlike the calibrate range-scan loop, this function does **not** require detecting a motion
/// start first — it is intended for cases where motion has already been commanded and is
/// expected to be underway immediately (e.g. stair descent probing).
pub fn wait_for_servos_to_settle<B>(
    bus: &mut B,
    servo_ids: &[u8],
    params: ServoPollParams,
) -> HalResult<BTreeMap<u8, ServoTelemetry>>
where
    B: ServoBus,
{
    let mut confirm_counts: BTreeMap<u8, u8> =
        servo_ids.iter().map(|&id| (id, 0u8)).collect();
    let mut settled: BTreeMap<u8, ServoTelemetry> = BTreeMap::new();
    let started = SystemTime::now();

    loop {
        thread::sleep(Duration::from_millis(params.poll_ms));

        for &servo_id in servo_ids {
            if settled.contains_key(&servo_id) {
                continue;
            }
            let telemetry = bus.read_feedback(servo_id)?;
            if servo_has_stopped(&telemetry, params.stop_speed_ticks) {
                let count = confirm_counts.entry(servo_id).or_default();
                *count = count.saturating_add(1);
                if *count >= params.confirm_stopped_samples {
                    settled.insert(servo_id, telemetry);
                }
            } else {
                *confirm_counts.entry(servo_id).or_default() = 0;
            }
        }

        if settled.len() == servo_ids.len() {
            return Ok(settled);
        }

        let elapsed_ms = started
            .elapsed()
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);

        if elapsed_ms > params.timeout_ms {
            let lingering = servo_ids
                .iter()
                .filter(|id| !settled.contains_key(*id))
                .copied()
                .collect();
            return Err(HalError::ProbeTimeout { elapsed_ms, lingering });
        }
    }
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
    fn servo_has_stopped_when_not_moving_and_slow() {
        let t = ServoTelemetry {
            moving: false,
            present_speed_ticks: 1,
            ..ServoTelemetry::default()
        };
        assert!(servo_has_stopped(&t, 5));
    }

    #[test]
    fn servo_has_not_stopped_when_moving_fast() {
        let t = ServoTelemetry {
            moving: true,
            present_speed_ticks: 100,
            ..ServoTelemetry::default()
        };
        assert!(!servo_has_stopped(&t, 5));
    }

    #[test]
    fn wait_for_servos_to_settle_returns_when_all_settled() {
        // Servo that is already stopped from the first read.
        let mut bus = MockServoBus {
            ids: vec![11],
            feedback: BTreeMap::from([(
                11,
                ServoTelemetry {
                    servo_id: 11,
                    present_position_ticks: 2048,
                    moving: false,
                    present_speed_ticks: 0,
                    ..ServoTelemetry::default()
                },
            )]),
            ..MockServoBus::default()
        };
        let params = ServoPollParams {
            poll_ms: 1,
            stop_speed_ticks: 5,
            confirm_stopped_samples: 2,
            timeout_ms: 500,
        };
        let result = wait_for_servos_to_settle(&mut bus, &[11], params)
            .expect("should settle");
        assert_eq!(result[&11].present_position_ticks, 2048);
    }

    #[test]
    fn wait_for_servos_to_settle_times_out_when_servo_keeps_moving() {
        let mut bus = MockServoBus {
            ids: vec![11],
            feedback: BTreeMap::from([(
                11,
                ServoTelemetry {
                    servo_id: 11,
                    moving: true,
                    present_speed_ticks: 200,
                    ..ServoTelemetry::default()
                },
            )]),
            ..MockServoBus::default()
        };
        let params = ServoPollParams {
            poll_ms: 1,
            stop_speed_ticks: 5,
            confirm_stopped_samples: 2,
            timeout_ms: 10,
        };
        let result = wait_for_servos_to_settle(&mut bus, &[11], params);
        assert!(
            matches!(result, Err(HalError::ProbeTimeout { .. })),
            "expected ProbeTimeout, got {result:?}"
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
