use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use arachno_core::{LegConfig, RobotConfig, SemanticPoseKind};
use arachno_hal::{HalError, HalResult, ImuSource, ServoBus};
use arachno_msg::{ImuTelemetry, JointCommand, ServoTelemetry};

// ── Public body state ─────────────────────────────────────────────────────────

/// Body state shared between [`SimServoBus`] and [`SimImu`].
///
/// Updated every `sync_write_positions` call via a critically-damped second-order
/// body model driven by FK-computed equilibrium targets.
#[derive(Debug, Clone)]
pub struct SimBodyState {
    /// Estimated height of the body frame origin above the ground plane (cm).
    pub body_height_cm: f32,
    /// Body pitch: positive = nose up (radians).
    pub pitch_rad: f32,
    /// Body roll: positive = right side down (radians).
    pub roll_rad: f32,
    /// Vertical body acceleration (m/s²). Non-zero during settling or pose changes.
    pub vertical_accel_m_s2: f32,
    /// Body pitch rate (rad/s). Non-zero during transitions.
    pub pitch_rate_rad_s: f32,
    /// Body roll rate (rad/s). Non-zero during transitions.
    pub roll_rate_rad_s: f32,
}

impl Default for SimBodyState {
    fn default() -> Self {
        Self {
            body_height_cm: 6.5,
            pitch_rad: 0.0,
            roll_rad: 0.0,
            vertical_accel_m_s2: 0.0,
            pitch_rate_rad_s: 0.0,
            roll_rate_rad_s: 0.0,
        }
    }
}

// ── Simulated IMU ─────────────────────────────────────────────────────────────

/// IMU that synthesizes readings from a shared [`SimBodyState`].
///
/// Construct via [`SimServoBus::build_pair`] so the bus and IMU share the same
/// body-state arc and stay in sync after every command step.
pub struct SimImu {
    body_state: Arc<Mutex<SimBodyState>>,
    sample_hz: u16,
    sample_count: u64,
}

impl SimImu {
    pub fn new(body_state: Arc<Mutex<SimBodyState>>, sample_hz: u16) -> Self {
        Self {
            body_state,
            sample_hz,
            sample_count: 0,
        }
    }
}

impl ImuSource for SimImu {
    fn start(&mut self) -> HalResult<()> {
        Ok(())
    }

    fn next_sample(&mut self) -> HalResult<Option<ImuTelemetry>> {
        self.sample_count += 1;
        let state = self.body_state.lock().unwrap();

        let (sp, cp) = (state.pitch_rad.sin(), state.pitch_rad.cos());
        let (sr, cr) = (state.roll_rad.sin(), state.roll_rad.cos());

        // Accelerometer specific force = gravity rotated into body frame
        // plus the body's vertical acceleration (non-zero during settling).
        //   ax =  G·sin(pitch)
        //   ay = −G·sin(roll)·cos(pitch)
        //   az =  G·cos(pitch)·cos(roll) + vertical_accel
        const G: f32 = 9.806_65;
        let az = G * cp * cr + state.vertical_accel_m_s2;

        // Gyroscope: angular velocity of body (body-frame, small-angle approximation).
        // Positive pitch rate (nose up) → +gyro_y. Positive roll rate (right down) → +gyro_x.
        Ok(Some(ImuTelemetry {
            timestamp_ms: self.sample_count * 1000 / u64::from(self.sample_hz.max(1)),
            accel_mps2: [G * sp, -G * sr * cp, az],
            gyro_rad_s: [
                state.roll_rate_rad_s,
                state.pitch_rate_rad_s,
                0.0,
            ],
            temperature_c: Some(35.0),
            status_bits: Some(0x0001),
            faults: Vec::new(),
        }))
    }

    fn description(&self) -> &str {
        "sim-imu"
    }
}

// ── Physics parameters (pre-computed from RobotConfig) ────────────────────────

#[derive(Debug, Clone)]
struct BodyPhysics {
    dt_s: f32,
    /// Second-order natural frequency for the body dynamics filter (rad/s).
    /// Chosen so that ω_n · dt ≪ √2 to keep explicit Euler stable.
    omega_n: f32,
}

impl BodyPhysics {
    fn from_config(config: &RobotConfig) -> Self {
        let dt_s = 1.0 / config.locomotion.command_hz.max(1) as f32;
        // Natural frequency: 5 rad/s (~0.8 Hz) gives realistic settling in ~0.5 s.
        // Stability check: ω_n · dt = 5 * 0.05 = 0.25 ≪ √2. ✓
        Self {
            dt_s,
            omega_n: 5.0,
        }
    }
}

// ── Private body dynamics integrator ─────────────────────────────────────────

/// Critically-damped second-order body integrator (internal to SimServoBus).
#[derive(Debug, Clone)]
struct BodyDynamics {
    height_m: f32,
    velocity_m_s: f32,
    pitch_rad: f32,
    dpitch_rad_s: f32,
    roll_rad: f32,
    droll_rad_s: f32,
}

impl BodyDynamics {
    fn from_initial_height_m(height_m: f32) -> Self {
        Self {
            height_m,
            velocity_m_s: 0.0,
            pitch_rad: 0.0,
            dpitch_rad_s: 0.0,
            roll_rad: 0.0,
            droll_rad_s: 0.0,
        }
    }

    /// Step the critically-damped 2nd-order filter toward `target`.
    /// Returns the instantaneous acceleration (for IMU synthesis).
    fn step_vertical(&mut self, target_m: f32, p: &BodyPhysics) -> f32 {
        let wn = p.omega_n;
        let kd = 2.0 * wn; // critical damping
        let accel = wn * wn * (target_m - self.height_m) - kd * self.velocity_m_s;
        self.velocity_m_s += accel * p.dt_s;
        self.height_m += self.velocity_m_s * p.dt_s;
        self.height_m = self.height_m.max(0.005); // cannot go underground
        accel
    }

    fn step_pitch(&mut self, target_rad: f32, p: &BodyPhysics) {
        let wn = p.omega_n;
        let kd = 2.0 * wn;
        let alpha = wn * wn * (target_rad - self.pitch_rad) - kd * self.dpitch_rad_s;
        self.dpitch_rad_s += alpha * p.dt_s;
        self.pitch_rad += self.dpitch_rad_s * p.dt_s;
        self.pitch_rad = self.pitch_rad.clamp(-0.52, 0.52); // ±30°
    }

    fn step_roll(&mut self, target_rad: f32, p: &BodyPhysics) {
        let wn = p.omega_n;
        let kd = 2.0 * wn;
        let alpha = wn * wn * (target_rad - self.roll_rad) - kd * self.droll_rad_s;
        self.droll_rad_s += alpha * p.dt_s;
        self.roll_rad += self.droll_rad_s * p.dt_s;
        self.roll_rad = self.roll_rad.clamp(-0.52, 0.52);
    }
}

// ── Per-servo state ───────────────────────────────────────────────────────────

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

// ── SimServoBus ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SimServoBus {
    ids: Vec<u8>,
    servos: BTreeMap<u8, SimServoState>,
    torque_enabled: bool,
    last_commands: Vec<JointCommand>,
    max_step_ticks: u16,
    present_voltage_v: f32,
    present_temperature_c: u8,
    legs: Vec<LegConfig>,
    physics: BodyPhysics,
    dynamics: BodyDynamics,
    body_state: Arc<Mutex<SimBodyState>>,
}

impl SimServoBus {
    /// Create a bus seeded from `seed_pose` with an internal body-state arc.
    /// Use this when you don't need IMU integration.
    pub fn from_robot_config(config: &RobotConfig, seed_pose: SemanticPoseKind) -> Self {
        Self::new_with_body_state(config, seed_pose, Arc::new(Mutex::new(SimBodyState::default())))
    }

    /// Create a bus and a paired [`SimImu`] that share the same body-state arc.
    ///
    /// After every [`ServoBus::sync_write_positions`] the body state is updated
    /// via a critically-damped second-order model. The IMU will synthesize
    /// gravity, dynamic acceleration, and angular rate readings from that state.
    pub fn build_pair(config: &RobotConfig, seed_pose: SemanticPoseKind) -> (Self, SimImu) {
        let body_state = Arc::new(Mutex::new(SimBodyState::default()));
        let bus = Self::new_with_body_state(config, seed_pose, Arc::clone(&body_state));
        let sample_hz = config.imu.as_ref().map(|i| i.sample_hz).unwrap_or(200);
        let imu = SimImu::new(body_state, sample_hz);
        (bus, imu)
    }

    fn new_with_body_state(
        config: &RobotConfig,
        seed_pose: SemanticPoseKind,
        body_state: Arc<Mutex<SimBodyState>>,
    ) -> Self {
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

        let physics = BodyPhysics::from_config(config);
        let initial_height_m = fk_equilibrium_height_m(&servos, &config.legs);
        let dynamics = BodyDynamics::from_initial_height_m(initial_height_m);

        Self {
            ids,
            servos,
            torque_enabled: false,
            last_commands: Vec::new(),
            max_step_ticks: max_step_ticks.max(1),
            present_voltage_v: config.safety.min_bus_voltage_v.max(6.4),
            present_temperature_c: 32,
            legs: config.legs.clone(),
            physics,
            dynamics,
            body_state,
        }
    }

    pub fn last_commands(&self) -> &[JointCommand] {
        &self.last_commands
    }

    pub fn torque_enabled(&self) -> bool {
        self.torque_enabled
    }

    /// Snapshot of the most recently computed body state.
    pub fn body_state(&self) -> SimBodyState {
        self.body_state.lock().unwrap().clone()
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

    /// Step the body dynamics toward the FK-derived equilibrium and publish
    /// the result into the shared [`SimBodyState`].
    ///
    /// Uses a critically-damped second-order filter so pose transitions produce
    /// realistic settling transients in body height, tilt, and the synthesized
    /// IMU readings.
    fn update_body_state(&mut self) {
        // ── FK: classify feet by group ────────────────────────────────────
        let mut all_z_m: Vec<f32> = Vec::with_capacity(self.legs.len());
        let mut front_z_m: Vec<f32> = Vec::new();
        let mut rear_z_m: Vec<f32> = Vec::new();
        let mut left_z_m: Vec<f32> = Vec::new();
        let mut right_z_m: Vec<f32> = Vec::new();

        for leg in &self.legs {
            let Some(c) = self.servos.get(&leg.coxa_servo_id) else { continue };
            let Some(f) = self.servos.get(&leg.femur_servo_id) else { continue };
            let Some(t) = self.servos.get(&leg.tibia_servo_id) else { continue };

            let coxa_deg = leg.coxa_deg_from_ticks(c.present_position_ticks);
            let femur_deg = leg.femur_deg_from_ticks(f.present_position_ticks);
            let tibia_deg = leg.tibia_deg_from_ticks(t.present_position_ticks);

            let pose = leg.body_frame_pose(coxa_deg, femur_deg, tibia_deg);
            let z_m = pose.tibia_end.z / 100.0; // cm → m
            all_z_m.push(z_m);

            if leg.name.starts_with("front_") {
                front_z_m.push(z_m);
            }
            if leg.name.starts_with("rear_") {
                rear_z_m.push(z_m);
            }
            if leg.name.contains("_left") {
                left_z_m.push(z_m);
            }
            if leg.name.contains("_right") {
                right_z_m.push(z_m);
            }
        }

        // ── FK equilibrium targets ─────────────────────────────────────────
        let below: Vec<f32> = all_z_m.iter().filter(|&&z| z < 0.0).map(|&z| -z).collect();
        if below.is_empty() {
            return; // no supporting feet — skip integration
        }
        let target_h = below.iter().sum::<f32>() / below.len() as f32;

        fn avg_support(zs: &[f32]) -> Option<f32> {
            if zs.is_empty() {
                return None;
            }
            Some(-zs.iter().sum::<f32>() / zs.len() as f32)
        }

        // Pitch target: nose up when front feet reach further down.
        // Longitudinal body span ≈ 15 cm.
        let target_pitch = match (avg_support(&front_z_m), avg_support(&rear_z_m)) {
            (Some(fs), Some(rs)) => (fs - rs) / 0.15,
            _ => 0.0,
        };

        // Roll target: right side down when right feet reach further down.
        // Lateral body span ≈ 10 cm.
        let target_roll = match (avg_support(&right_z_m), avg_support(&left_z_m)) {
            (Some(rs), Some(ls)) => (rs - ls) / 0.10,
            _ => 0.0,
        };

        // ── Dynamics integration ───────────────────────────────────────────
        let p = self.physics.clone();
        let vert_accel = self.dynamics.step_vertical(target_h, &p);
        self.dynamics.step_pitch(target_pitch, &p);
        self.dynamics.step_roll(target_roll, &p);

        // ── Publish to shared state ────────────────────────────────────────
        let mut state = self.body_state.lock().unwrap();
        state.body_height_cm = self.dynamics.height_m * 100.0;
        state.pitch_rad = self.dynamics.pitch_rad;
        state.roll_rad = self.dynamics.roll_rad;
        state.vertical_accel_m_s2 = vert_accel;
        state.pitch_rate_rad_s = self.dynamics.dpitch_rad_s;
        state.roll_rate_rad_s = self.dynamics.droll_rad_s;
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

        self.update_body_state();
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

// ── FK helpers ────────────────────────────────────────────────────────────────

/// Compute the steady-state body height in metres from the current servo positions.
fn fk_equilibrium_height_m(
    servos: &BTreeMap<u8, SimServoState>,
    legs: &[LegConfig],
) -> f32 {
    let below: Vec<f32> = legs
        .iter()
        .filter_map(|leg| {
            let c = servos.get(&leg.coxa_servo_id)?;
            let f = servos.get(&leg.femur_servo_id)?;
            let t = servos.get(&leg.tibia_servo_id)?;
            let coxa_deg = leg.coxa_deg_from_ticks(c.present_position_ticks);
            let femur_deg = leg.femur_deg_from_ticks(f.present_position_ticks);
            let tibia_deg = leg.tibia_deg_from_ticks(t.present_position_ticks);
            let z_m = leg.body_frame_pose(coxa_deg, femur_deg, tibia_deg).tibia_end.z / 100.0;
            (z_m < 0.0).then_some(-z_m)
        })
        .collect();

    if below.is_empty() {
        return 0.065; // 6.5 cm fallback
    }
    below.iter().sum::<f32>() / below.len() as f32
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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

    #[test]
    fn sim_servo_bus_updates_body_height_after_stand_reference_commands() {
        use arachno_core::TripodGait;

        let config = load_test_config();
        let mut bus = SimServoBus::from_robot_config(&config, SemanticPoseKind::LayDown);

        let gait = TripodGait;
        let commands = gait.stand_reference_commands(&config);
        bus.sync_write_positions(&commands)
            .expect("stand commands should apply");

        let state = bus.body_state();
        assert!(
            state.body_height_cm > 2.0 && state.body_height_cm < 20.0,
            "body_height_cm={} out of expected range",
            state.body_height_cm
        );
    }

    #[test]
    fn sim_servo_bus_seeded_at_stand_reference_has_small_dynamics_transient() {
        use arachno_core::TripodGait;

        let config = load_test_config();
        // Seed at stand pose so dynamics start near equilibrium.
        let mut bus =
            SimServoBus::from_robot_config(&config, SemanticPoseKind::StandReference);
        let commands = TripodGait.stand_reference_commands(&config);

        // Run a few steps so the dynamics settle.
        for _ in 0..10 {
            bus.sync_write_positions(&commands).unwrap();
        }

        let state = bus.body_state();
        // Accelerations and rates should be small after settling.
        assert!(
            state.vertical_accel_m_s2.abs() < 2.0,
            "accel={} should be small at equilibrium",
            state.vertical_accel_m_s2
        );
        assert!(
            state.pitch_rate_rad_s.abs() < 0.2,
            "pitch_rate={} should be small at equilibrium",
            state.pitch_rate_rad_s
        );
    }

    #[test]
    fn sim_imu_synthesizes_gravity_when_level() {
        use arachno_core::TripodGait;

        let config = load_test_config();
        let (mut bus, mut imu) =
            SimServoBus::build_pair(&config, SemanticPoseKind::StandReference);

        let commands = TripodGait.stand_reference_commands(&config);
        for _ in 0..20 {
            bus.sync_write_positions(&commands).unwrap();
        }

        imu.start().unwrap();
        let sample = imu.next_sample().unwrap().expect("sample should be available");

        let az = sample.accel_mps2[2];
        assert!(
            az > 9.0 && az <= 9.82,
            "az={az} should be close to 9.81 for settled level stance"
        );
        let ax = sample.accel_mps2[0].abs();
        let ay = sample.accel_mps2[1].abs();
        assert!(ax < 1.0, "ax={ax} too large for level stance");
        assert!(ay < 1.0, "ay={ay} too large for level stance");
    }

    #[test]
    fn sim_imu_and_bus_share_body_state() {
        let config = load_test_config();
        let (bus, _imu) = SimServoBus::build_pair(&config, SemanticPoseKind::StandReference);
        let state = bus.body_state();
        assert!(state.body_height_cm > 0.0);
    }

    #[test]
    fn sim_imu_gyro_is_near_zero_when_settled() {
        use arachno_core::TripodGait;

        let config = load_test_config();
        let (mut bus, mut imu) =
            SimServoBus::build_pair(&config, SemanticPoseKind::StandReference);
        let commands = TripodGait.stand_reference_commands(&config);

        for _ in 0..30 {
            bus.sync_write_positions(&commands).unwrap();
        }

        imu.start().unwrap();
        let sample = imu.next_sample().unwrap().unwrap();
        let [gx, gy, gz] = sample.gyro_rad_s;
        assert!(
            gx.abs() < 0.05 && gy.abs() < 0.05 && gz.abs() < 0.001,
            "gyro {gx:.3} {gy:.3} {gz:.3} should be near zero when settled"
        );
    }
}
