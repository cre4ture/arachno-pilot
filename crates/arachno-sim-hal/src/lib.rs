use std::collections::BTreeMap;

use arachno_core::{RobotConfig, SemanticPoseKind};
use arachno_hal::{HalError, HalResult, ServoBus};
use arachno_msg::{JointCommand, ServoTelemetry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SimServoState {
    present_position_ticks: u16,
    target_position_ticks: u16,
    commanded_speed_ticks: u16,
    moving: bool,
}

impl SimServoState {
    fn new(position_ticks: u16) -> Self {
        Self {
            present_position_ticks: position_ticks,
            target_position_ticks: position_ticks,
            commanded_speed_ticks: 0,
            moving: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimServoBus {
    ids: Vec<u8>,
    servos: BTreeMap<u8, SimServoState>,
    torque_enabled: bool,
    last_commands: Vec<JointCommand>,
    max_step_ticks: u16,
    present_voltage_v: f32,
    present_temperature_c: u8,
}

impl SimServoBus {
    pub fn from_robot_config(config: &RobotConfig, seed_pose: SemanticPoseKind) -> Self {
        let mut ids = Vec::with_capacity(config.legs.len() * 3);
        let mut servos = BTreeMap::new();

        for leg in &config.legs {
            let pose = config
                .pose_for_leg(seed_pose, &leg.name)
                .unwrap_or_default();
            let (coxa, femur, tibia) = leg.pose_ticks_from_angles(pose);

            for (servo_id, position_ticks) in [
                (leg.coxa_servo_id, coxa),
                (leg.femur_servo_id, femur),
                (leg.tibia_servo_id, tibia),
            ] {
                ids.push(servo_id);
                servos.insert(servo_id, SimServoState::new(position_ticks));
            }
        }

        let ticks_per_degree = 4096.0 / 360.0;
        let max_step_ticks =
            (config.simulation.max_servo_speed_deg_s * ticks_per_degree / 20.0).round() as u16;

        Self {
            ids,
            servos,
            torque_enabled: false,
            last_commands: Vec::new(),
            max_step_ticks: max_step_ticks.max(1),
            present_voltage_v: config.safety.min_bus_voltage_v.max(6.4),
            present_temperature_c: 32,
        }
    }

    pub fn last_commands(&self) -> &[JointCommand] {
        &self.last_commands
    }

    pub fn torque_enabled(&self) -> bool {
        self.torque_enabled
    }

    fn advance_servo(state: &mut SimServoState, max_step_ticks: u16) -> i16 {
        let delta =
            i32::from(state.target_position_ticks) - i32::from(state.present_position_ticks);
        if delta == 0 {
            state.moving = false;
            return 0;
        }

        let configured_step = state.commanded_speed_ticks.max(1);
        let step_limit = configured_step.min(max_step_ticks);
        let step = delta.clamp(-i32::from(step_limit), i32::from(step_limit)) as i16;
        state.present_position_ticks =
            (i32::from(state.present_position_ticks) + i32::from(step)).clamp(0, 4095) as u16;
        state.moving = state.present_position_ticks != state.target_position_ticks;
        step
    }
}

impl ServoBus for SimServoBus {
    fn servo_ids(&self) -> &[u8] {
        &self.ids
    }

    fn enable_torque(&mut self, enabled: bool) -> HalResult<()> {
        self.torque_enabled = enabled;
        Ok(())
    }

    fn sync_write_positions(&mut self, commands: &[JointCommand]) -> HalResult<()> {
        self.last_commands = commands.to_vec();

        for command in commands {
            let state = self.servos.get_mut(&command.servo_id).ok_or_else(|| {
                HalError::Communication(format!(
                    "simulated servo {} is not configured",
                    command.servo_id
                ))
            })?;
            state.target_position_ticks = command.position_ticks;
            state.commanded_speed_ticks = command.speed_ticks;
            state.moving = state.present_position_ticks != state.target_position_ticks;
            Self::advance_servo(state, self.max_step_ticks);
        }

        Ok(())
    }

    fn read_feedback(&mut self, servo_id: u8) -> HalResult<ServoTelemetry> {
        let state = self.servos.get_mut(&servo_id).ok_or_else(|| {
            HalError::Communication(format!("simulated servo {servo_id} is not configured"))
        })?;
        let step = Self::advance_servo(state, self.max_step_ticks);
        let load_scale = if self.torque_enabled { 18.0 } else { 0.0 };
        let current_scale = if self.torque_enabled { 90 } else { 0 };

        Ok(ServoTelemetry {
            servo_id,
            present_position_ticks: state.present_position_ticks,
            present_speed_ticks: step,
            present_load_pct: if self.torque_enabled {
                (step.abs() as f32 / self.max_step_ticks as f32 * load_scale).min(100.0)
            } else {
                0.0
            },
            present_voltage_v: self.present_voltage_v,
            present_current_ma: Some(
                120 + (u16::try_from(step.abs()).unwrap_or(u16::MAX) * current_scale),
            ),
            present_temperature_c: Some(self.present_temperature_c),
            status_bits: Some(if self.torque_enabled { 0b01 } else { 0 }),
            faults: Vec::new(),
            moving: state.moving,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_config() -> RobotConfig {
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/robot/default.toml");
        RobotConfig::load_from_path(config_path).expect("default config should load")
    }

    #[test]
    fn sim_servo_bus_seeds_from_named_pose() {
        let config = load_test_config();
        let mut bus = SimServoBus::from_robot_config(&config, SemanticPoseKind::LayDown);

        let first_leg = &config.legs[0];
        let lay_down_pose = config
            .pose_for_leg(SemanticPoseKind::LayDown, &first_leg.name)
            .expect("lay down pose should exist");
        let (coxa, _femur, _tibia) = first_leg.pose_ticks_from_angles(lay_down_pose);

        let feedback = bus
            .read_feedback(first_leg.coxa_servo_id)
            .expect("feedback should be available");

        assert_eq!(feedback.present_position_ticks, coxa);
        assert!(!feedback.moving);
    }

    #[test]
    fn sim_servo_bus_tracks_commands_and_enables_torque() {
        let config = load_test_config();
        let first_leg = &config.legs[0];
        let mut bus = SimServoBus::from_robot_config(&config, SemanticPoseKind::LayDown);

        bus.enable_torque(true).expect("torque should enable");
        bus.sync_write_positions(&[JointCommand {
            servo_id: first_leg.coxa_servo_id,
            position_ticks: 2400,
            speed_ticks: 120,
            acceleration: 10,
        }])
        .expect("command should apply");

        assert!(bus.torque_enabled());
        assert_eq!(bus.last_commands().len(), 1);

        let feedback = bus
            .read_feedback(first_leg.coxa_servo_id)
            .expect("feedback should be available");
        assert!(feedback.present_position_ticks > 0);
        assert_eq!(feedback.status_bits, Some(0b01));
    }
}
