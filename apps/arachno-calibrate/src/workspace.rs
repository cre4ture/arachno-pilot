//! Standing workspace calibration for `sense-workspace`.
//!
//! Per-leg procedure — called once per leg while the robot is already standing and the
//! other five legs hold stance:
//!
//!   Step 1  Compute movement bounds from range scan data and stand-pose FK:
//!             • tibia_retract  — just inside the tibia's mechanical lift stop (foot folded up)
//!             • femur_lift     — just inside the measured femur lift limit
//!             • femur_ext_safe — femur angle at which the *knee* would reach floor level;
//!                                this is the safe extension limit to avoid knee-floor collision
//!
//!   Step 2  Move to (femur_lift, tibia_retract): foot is high in the air.
//!             This is the only safe position for the mechanical-limit probe because the
//!             foot has clear air beneath it throughout the full tibia extension.
//!
//!   Step 3  Probe tibia downward with low torque (target = 4095).
//!             Because the foot is in the air, the torque limit finds the *mechanical* stop,
//!             not the floor. The range-scan tibia extension data is useless here — it was
//!             measured in the laying pose and stopped at the floor, not the joint stop.
//!
//!   Step 4  Compute the kinematic workspace envelope geometrically (no servo movement).
//!             Sweep femur from femur_ext_safe → femur_lift in ENVELOPE_STEPS increments.
//!             At each femur angle record FK at:
//!               • tibia retracted → upper boundary of the workspace
//!               • tibia at mechanical stop → lower boundary of the workspace
//!
//!   Step 5  Return the bounding box of all FK points as a LegWorkspace.

use std::{collections::BTreeMap, thread, time::Duration};

use anyhow::{Context, anyhow};
use arachno_core::{LegConfig, LegWorkspace};
use arachno_feetech_sts::RealStsBus;
use arachno_hal::{ServoPollParams, wait_for_servos_to_settle};
use tracing::info;

use crate::{
    JointRangeMeasurement, PHASE_SETTLE_SLEEP_MS, SenseParams, TorqueMode, interpolate_ticks,
    move_pose_until_close, set_verified_torque_limit_on_current_position_for_ids, sync_full_pose,
    sync_pose_targets_to_current_for_ids, try_move_pose,
};

/// Ticks backed off from any mechanical limit to avoid commanding into the hard stop.
const LIMIT_MARGIN_TICKS: i32 = 40;

/// Minimum clearance (cm) between the femur knee and the floor when computing the safe
/// extension limit. Prevents the knee from driving into the floor during the sweep.
const KNEE_FLOOR_MARGIN_CM: f32 = 1.5;

/// Number of femur positions sampled when computing the geometric workspace envelope.
const ENVELOPE_STEPS: u32 = 20;

// ─── Public entry point ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkspaceTorqueLimits {
    pub(crate) sense_limit: u16,
    pub(crate) move_limit: u16,
}

impl From<&SenseParams> for WorkspaceTorqueLimits {
    fn from(params: &SenseParams) -> Self {
        Self {
            sense_limit: params.probe_torque_limit,
            move_limit: params.restore_torque_limit,
        }
    }
}

impl WorkspaceTorqueLimits {
    fn limit_for(self, mode: TorqueMode) -> u16 {
        match mode {
            TorqueMode::Sense | TorqueMode::TryMove => self.sense_limit,
            TorqueMode::Move | TorqueMode::Hold => self.move_limit,
        }
    }
}

pub(crate) struct WorkspaceLegMotion<'a> {
    bus: &'a mut RealStsBus,
    pose: &'a mut BTreeMap<u8, u16>,
    leg: &'a LegConfig,
    params: &'a SenseParams,
    torque_limits: WorkspaceTorqueLimits,
}

impl<'a> WorkspaceLegMotion<'a> {
    pub(crate) fn new(
        bus: &'a mut RealStsBus,
        pose: &'a mut BTreeMap<u8, u16>,
        leg: &'a LegConfig,
        params: &'a SenseParams,
    ) -> Self {
        Self {
            bus,
            pose,
            leg,
            params,
            torque_limits: WorkspaceTorqueLimits::from(params),
        }
    }

    pub(crate) fn move_femur_and_tibia_with_torque_mode(
        &mut self,
        torque_mode: TorqueMode,
        restore_mode: TorqueMode,
        raise_label: &str,
        lower_label: &str,
        move_label: &str,
    ) -> anyhow::Result<()> {
        let tracked_servo_ids = self.femur_and_tibia_ids();
        self.move_tracked_pose_with_torque_mode(
            &tracked_servo_ids,
            torque_mode,
            restore_mode,
            raise_label,
            lower_label,
            move_label,
        )
    }

    fn move_to_retracted_lift(&mut self, bounds: &WorkspaceBounds) -> anyhow::Result<()> {
        self.pose
            .insert(self.leg.femur_servo_id, bounds.femur_lift_ticks);
        self.pose
            .insert(self.leg.tibia_servo_id, bounds.tibia_retract_ticks);
        self.move_femur_and_tibia_with_torque_mode(
            TorqueMode::TryMove,
            TorqueMode::Hold,
            "raising torque for lift + retract move",
            "restoring low torque after lift + retract move",
            "moving to lift + retract for tibia mechanical limit probe",
        )?;
        thread::sleep(Duration::from_millis(PHASE_SETTLE_SLEEP_MS));
        Ok(())
    }

    fn probe_tibia_mechanical_limit(&mut self, bounds: &WorkspaceBounds) -> anyhow::Result<u16> {
        self.set_tibia_torque_mode(
            TorqueMode::Sense,
            "lowering tibia torque for mechanical limit probe",
        )?;

        self.pose.insert(self.leg.tibia_servo_id, 4095);
        self.sync_pose()?;
        thread::sleep(Duration::from_millis(self.params.poll_ms));

        let mech_limit_ticks = self.wait_for_tibia_mechanical_limit()?;

        self.pose
            .insert(self.leg.tibia_servo_id, bounds.tibia_retract_ticks);
        self.move_tibia_with_torque_mode(
            TorqueMode::Sense,
            TorqueMode::Hold,
            "raising torque for tibia retract move",
            "restoring low torque after tibia retract move",
            "retracting tibia after mechanical limit probe",
        )?;

        Ok(mech_limit_ticks)
    }

    fn move_tibia_with_torque_mode(
        &mut self,
        torque_mode: TorqueMode,
        restore_mode: TorqueMode,
        raise_label: &str,
        lower_label: &str,
        move_label: &str,
    ) -> anyhow::Result<()> {
        let tracked_servo_ids = self.tibia_ids();
        self.move_tracked_pose_with_torque_mode(
            &tracked_servo_ids,
            torque_mode,
            restore_mode,
            raise_label,
            lower_label,
            move_label,
        )
    }

    fn move_tracked_pose_with_torque_mode(
        &mut self,
        tracked_servo_ids: &[u8],
        torque_mode: TorqueMode,
        restore_mode: TorqueMode,
        raise_label: &str,
        lower_label: &str,
        move_label: &str,
    ) -> anyhow::Result<()> {
        self.run_with_torque_modes(
            tracked_servo_ids,
            torque_mode,
            restore_mode,
            raise_label,
            lower_label,
            |motion| {
                let pose = &mut *motion.pose;
                let poll_ms = motion.params.poll_ms;
                let move_timeout_ms = motion.params.move_timeout_ms;

                match torque_mode {
                    TorqueMode::TryMove => {
                        try_move_pose(
                            motion.bus,
                            pose,
                            tracked_servo_ids,
                            poll_ms,
                            move_timeout_ms,
                            move_label,
                        )?;
                        sync_pose_targets_to_current_for_ids(motion.bus, pose, tracked_servo_ids)
                            .with_context(|| {
                                format!(
                                    "failed to refresh try-move pose targets for {}",
                                    motion.leg.name
                                )
                            })
                    }
                    _ => move_pose_until_close(
                        motion.bus,
                        pose,
                        tracked_servo_ids,
                        poll_ms,
                        move_timeout_ms,
                        move_label,
                    ),
                }
            },
        )
    }

    fn run_with_torque_modes<T, F>(
        &mut self,
        servo_ids: &[u8],
        active_mode: TorqueMode,
        idle_mode: TorqueMode,
        active_label: &str,
        idle_label: &str,
        action: F,
    ) -> anyhow::Result<T>
    where
        F: FnOnce(&mut Self) -> anyhow::Result<T>,
    {
        with_torque_window(
            self,
            servo_ids,
            active_mode,
            idle_mode,
            active_label,
            idle_label,
            |motion, servo_ids, torque_mode, label| {
                motion.set_torque_mode(servo_ids, torque_mode, label)
            },
            action,
        )
    }

    fn set_tibia_torque_mode(
        &mut self,
        torque_mode: TorqueMode,
        label: &str,
    ) -> anyhow::Result<()> {
        let tracked_servo_ids = self.tibia_ids();
        self.set_torque_mode(&tracked_servo_ids, torque_mode, label)
    }

    fn set_torque_mode(
        &mut self,
        servo_ids: &[u8],
        torque_mode: TorqueMode,
        label: &str,
    ) -> anyhow::Result<()> {
        if matches!(torque_mode, TorqueMode::Hold) {
            sync_pose_targets_to_current_for_ids(self.bus, self.pose, servo_ids).with_context(
                || format!("failed to refresh hold pose targets for {}", self.leg.name),
            )?;
        }

        set_verified_torque_limit_on_current_position_for_ids(
            self.bus,
            servo_ids,
            self.torque_limits.limit_for(torque_mode),
            label,
        )
    }

    fn sync_pose(&mut self) -> anyhow::Result<()> {
        sync_full_pose(self.bus, self.pose)
    }

    fn wait_for_tibia_mechanical_limit(&mut self) -> anyhow::Result<u16> {
        let tracked_servo_ids = self.tibia_ids();
        let poll_params = ServoPollParams {
            poll_ms: self.params.poll_ms,
            stop_speed_ticks: self.params.stop_speed_ticks,
            confirm_stopped_samples: self.params.confirm_stopped_samples,
            timeout_ms: self.params.move_timeout_ms,
        };
        let settled = wait_for_servos_to_settle(self.bus, &tracked_servo_ids, poll_params)
            .with_context(|| {
                format!(
                    "timeout waiting for tibia mechanical limit probe on leg {}",
                    self.leg.name
                )
            })?;
        Ok(settled[&self.leg.tibia_servo_id].present_position_ticks)
    }

    fn femur_and_tibia_ids(&self) -> [u8; 2] {
        [self.leg.femur_servo_id, self.leg.tibia_servo_id]
    }

    fn tibia_ids(&self) -> [u8; 1] {
        [self.leg.tibia_servo_id]
    }
}

pub(crate) fn calibrate_leg_workspace(
    bus: &mut RealStsBus,
    stand_pose: &BTreeMap<u8, u16>,
    leg: &LegConfig,
    femur_ranges: &JointRangeMeasurement,
    tibia_ranges: &JointRangeMeasurement,
    params: &SenseParams,
) -> anyhow::Result<LegWorkspace> {
    // Step 1 ─ Compute movement bounds.
    let bounds = compute_bounds(leg, stand_pose, femur_ranges, tibia_ranges)?;
    info!(
        leg = %leg.name,
        floor_height_cm = bounds.floor_height_cm,
        femur_lift = bounds.femur_lift_ticks,
        femur_ext_safe = bounds.femur_ext_ticks,
        tibia_retract = bounds.tibia_retract_ticks,
        "step 1: bounds"
    );

    let mut pose = stand_pose.clone();
    let mut leg_motion = WorkspaceLegMotion::new(bus, &mut pose, leg, params);

    // Step 2 ─ Move to (femur_lift, tibia_retract): foot is high in the air.
    leg_motion.move_to_retracted_lift(&bounds)?;
    info!(leg = %leg.name, "step 2: at lift + retract");

    // Step 3 ─ Probe tibia to mechanical extension limit (foot in air — no floor contact).
    let tibia_mech_limit_ticks = leg_motion.probe_tibia_mechanical_limit(&bounds)?;
    info!(
        leg = %leg.name,
        ticks = tibia_mech_limit_ticks,
        deg = leg.tibia_deg_from_ticks(tibia_mech_limit_ticks),
        "step 3: tibia mechanical limit"
    );

    // Step 4 ─ Compute the kinematic workspace envelope geometrically (no servo movement).
    let fk_points = compute_envelope(leg, &bounds, tibia_mech_limit_ticks);
    info!(leg = %leg.name, points = fk_points.len(), "step 4: envelope computed");

    // Step 5 ─ Derive bounding box from all envelope points.
    let ws = envelope_bounding_box(&fk_points);
    info!(
        leg = %leg.name,
        reach = format!("{:.1}..{:.1} cm", ws.min_reach_cm, ws.max_reach_cm),
        height = format!("{:.1}..{:.1} cm", ws.min_height_cm, ws.max_height_cm),
        "step 5: bounding box"
    );
    Ok(ws)
}

// ─── Per-step helpers ─────────────────────────────────────────────────────────

struct WorkspaceBounds {
    /// Height of the floor below the coxa joint (cm, positive = below), from stand-pose FK.
    floor_height_cm: f32,
    /// Femur target for the lift (foot-up) limit, backed off from the mechanical stop.
    femur_lift_ticks: u16,
    /// Femur target for the safe extension limit — the angle at which the femur knee
    /// would be KNEE_FLOOR_MARGIN_CM above the floor. Beyond this the knee would collide.
    femur_ext_ticks: u16,
    /// Tibia target for the retracted (foot-up) position, backed off from the lift stop.
    tibia_retract_ticks: u16,
}

fn compute_bounds(
    leg: &LegConfig,
    stand_pose: &BTreeMap<u8, u16>,
    femur_ranges: &JointRangeMeasurement,
    tibia_ranges: &JointRangeMeasurement,
) -> anyhow::Result<WorkspaceBounds> {
    let femur_stand_ticks = *stand_pose
        .get(&leg.femur_servo_id)
        .with_context(|| format!("missing stand pose for femur servo {}", leg.femur_servo_id))?;
    let tibia_stand_ticks = *stand_pose
        .get(&leg.tibia_servo_id)
        .with_context(|| format!("missing stand pose for tibia servo {}", leg.tibia_servo_id))?;

    let floor_height_cm = {
        let femur_deg = leg.femur_deg_from_ticks(femur_stand_ticks);
        let tibia_deg = leg.tibia_deg_from_ticks(tibia_stand_ticks);
        let sv = leg.side_view_pose(femur_deg, tibia_deg);
        sv.tibia_end.y - sv.coxa_end.y
    };

    let femur_lift_ticks = back_off_from_limit(
        femur_ranges.logical_positive_end_ticks,
        femur_stand_ticks,
        LIMIT_MARGIN_TICKS,
    );

    let femur_ext_ticks =
        safe_femur_ext_limit(leg, femur_ranges, femur_stand_ticks, floor_height_cm);

    // The tibia's logical_positive_end (foot-up direction) IS the mechanical stop —
    // the range scan couldn't have hit the floor folding upward, so this data is reliable.
    let tibia_retract_ticks = back_off_from_limit(
        tibia_ranges.logical_positive_end_ticks,
        tibia_ranges.midpoint_ticks,
        LIMIT_MARGIN_TICKS,
    );

    Ok(WorkspaceBounds {
        floor_height_cm,
        femur_lift_ticks,
        femur_ext_ticks,
        tibia_retract_ticks,
    })
}

/// Back off from a hard limit by `margin` ticks toward `center`.
fn back_off_from_limit(limit_ticks: u16, center_ticks: u16, margin: i32) -> u16 {
    let dir = (center_ticks as i32 - limit_ticks as i32).signum();
    (limit_ticks as i32 + dir * margin).clamp(0, 4095) as u16
}

/// Safe femur extension limit: the tick value at which the femur knee would be exactly
/// KNEE_FLOOR_MARGIN_CM above the floor. The range-scan mechanical limit is applied as an
/// absolute bound, and whichever is closer to stand position wins (more conservative).
fn safe_femur_ext_limit(
    leg: &LegConfig,
    femur_ranges: &JointRangeMeasurement,
    femur_stand_ticks: u16,
    floor_height_cm: f32,
) -> u16 {
    // FK formula: knee_y = sin((-semantic_deg).to_radians()) * femur_length
    // Solving for semantic_deg when knee_y = floor_height - margin:
    let max_knee_cm = (floor_height_cm - KNEE_FLOOR_MARGIN_CM).clamp(0.0, leg.femur_length_cm());
    let safe_deg = -(max_knee_cm / leg.femur_length_cm()).asin().to_degrees();
    let safe_ticks = (leg.femur_zero_reference_ticks() as f32
        + leg.femur_lift_sign() as f32 * safe_deg * 4096.0 / 360.0)
        .round()
        .clamp(0.0, 4095.0) as u16;

    // Also clamp by the range-scan mechanical limit (with margin).
    let hard_ext_ticks = back_off_from_limit(
        femur_ranges.logical_negative_end_ticks,
        femur_stand_ticks,
        LIMIT_MARGIN_TICKS,
    );

    // Pick whichever limit is closer to the stand position (more conservative).
    let safe_delta = (safe_ticks as i32 - femur_stand_ticks as i32).abs();
    let hard_delta = (hard_ext_ticks as i32 - femur_stand_ticks as i32).abs();
    let ext_delta = safe_delta.min(hard_delta);
    let dir = (hard_ext_ticks as i32 - femur_stand_ticks as i32).signum();
    (femur_stand_ticks as i32 + dir * ext_delta).clamp(0, 4095) as u16
}

fn compute_envelope(
    leg: &LegConfig,
    bounds: &WorkspaceBounds,
    tibia_mech_limit_ticks: u16,
) -> Vec<(f32, f32)> {
    let tibia_retract_deg = leg.tibia_deg_from_ticks(bounds.tibia_retract_ticks);
    let tibia_mech_deg = leg.tibia_deg_from_ticks(tibia_mech_limit_ticks);

    let mut points = Vec::new();
    for i in 0..=ENVELOPE_STEPS {
        let ratio = i as f32 / ENVELOPE_STEPS as f32;
        let femur_ticks = interpolate_ticks(bounds.femur_ext_ticks, bounds.femur_lift_ticks, ratio);
        let femur_deg = leg.femur_deg_from_ticks(femur_ticks);

        // Upper boundary: tibia retracted — foot as high as possible at this femur angle.
        let sv = leg.side_view_pose(femur_deg, tibia_retract_deg);
        points.push((
            (sv.tibia_end.x - sv.coxa_end.x).abs(),
            sv.tibia_end.y - sv.coxa_end.y,
        ));

        // Lower boundary: tibia at mechanical stop — foot as deep as possible.
        let sv = leg.side_view_pose(femur_deg, tibia_mech_deg);
        points.push((
            (sv.tibia_end.x - sv.coxa_end.x).abs(),
            sv.tibia_end.y - sv.coxa_end.y,
        ));
    }
    points
}

fn envelope_bounding_box(fk_points: &[(f32, f32)]) -> LegWorkspace {
    let min_reach = fk_points.iter().map(|(r, _)| *r).fold(f32::MAX, f32::min);
    let max_reach = fk_points.iter().map(|(r, _)| *r).fold(f32::MIN, f32::max);
    let min_height = fk_points.iter().map(|(_, h)| *h).fold(f32::MAX, f32::min);
    let max_height = fk_points.iter().map(|(_, h)| *h).fold(f32::MIN, f32::max);
    LegWorkspace {
        min_reach_cm: (min_reach * 10.0).round() / 10.0,
        max_reach_cm: (max_reach * 10.0).round() / 10.0,
        min_height_cm: (min_height * 10.0).round() / 10.0,
        max_height_cm: (max_height * 10.0).round() / 10.0,
    }
}

fn with_torque_window<C, TValue, T, FSet, FAction>(
    context: &mut C,
    servo_ids: &[u8],
    active_value: TValue,
    idle_value: TValue,
    active_label: &str,
    idle_label: &str,
    set_limit: FSet,
    action: FAction,
) -> anyhow::Result<T>
where
    TValue: Copy,
    FSet: Fn(&mut C, &[u8], TValue, &str) -> anyhow::Result<()>,
    FAction: FnOnce(&mut C) -> anyhow::Result<T>,
{
    set_limit(context, servo_ids, active_value, active_label)?;
    let action_result = action(context);
    let idle_result = set_limit(context, servo_ids, idle_value, idle_label);

    match (action_result, idle_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(action_err), Ok(())) => Err(action_err),
        (Ok(_), Err(idle_err)) => Err(idle_err),
        (Err(action_err), Err(idle_err)) => Err(anyhow!(
            "{action_err:#}; additionally failed to restore sensing torque: {idle_err:#}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};

    use super::{WorkspaceTorqueLimits, with_torque_window};
    use crate::{SenseParams, TorqueMode};

    #[derive(Debug, Default)]
    struct FakeTorqueContext {
        set_calls: Vec<(Vec<u8>, u16, String)>,
        fail_on_call: Option<usize>,
    }

    fn record_limit(
        context: &mut FakeTorqueContext,
        servo_ids: &[u8],
        torque_limit: u16,
        label: &str,
    ) -> Result<()> {
        let call_index = context.set_calls.len();
        context
            .set_calls
            .push((servo_ids.to_vec(), torque_limit, label.to_owned()));

        if context.fail_on_call == Some(call_index) {
            return Err(anyhow!("failed to set torque limit {torque_limit}"));
        }

        Ok(())
    }

    #[test]
    fn workspace_torque_limits_use_probe_for_sensing_and_restore_for_moves() {
        let params = SenseParams {
            probe_torque_limit: 120,
            restore_torque_limit: 640,
            poll_ms: 40,
            stop_speed_ticks: 2,
            confirm_stopped_samples: 2,
            move_timeout_ms: 1_000,
        };

        let torque_limits = WorkspaceTorqueLimits::from(&params);

        assert_eq!(torque_limits.limit_for(TorqueMode::Sense), 120);
        assert_eq!(torque_limits.limit_for(TorqueMode::Move), 640);
        assert_eq!(torque_limits.limit_for(TorqueMode::Hold), 640);
        assert_eq!(torque_limits.limit_for(TorqueMode::TryMove), 120);
    }

    #[test]
    fn with_torque_window_uses_move_then_sensing_limits_around_success() {
        let mut context = FakeTorqueContext::default();

        let result = with_torque_window(
            &mut context,
            &[12, 13],
            700,
            150,
            "raise torque for move",
            "restore torque for sensing",
            record_limit,
            |_| Ok("done"),
        )
        .expect("torque window should succeed");

        assert_eq!(result, "done");
        assert_eq!(
            context.set_calls,
            vec![
                (vec![12, 13], 700, "raise torque for move".to_owned()),
                (vec![12, 13], 150, "restore torque for sensing".to_owned(),),
            ]
        );
    }

    #[test]
    fn with_torque_window_restores_sensing_limit_after_action_failure() {
        let mut context = FakeTorqueContext::default();

        let err = with_torque_window(
            &mut context,
            &[33],
            600,
            110,
            "raise torque",
            "restore torque",
            record_limit,
            |_| Err::<(), _>(anyhow!("move failed")),
        )
        .expect_err("action should fail");

        assert!(err.to_string().contains("move failed"));
        assert_eq!(
            context.set_calls,
            vec![
                (vec![33], 600, "raise torque".to_owned()),
                (vec![33], 110, "restore torque".to_owned()),
            ]
        );
    }

    #[test]
    fn with_torque_window_returns_restore_error_after_successful_action() {
        let mut context = FakeTorqueContext {
            fail_on_call: Some(1),
            ..FakeTorqueContext::default()
        };

        let err = with_torque_window(
            &mut context,
            &[42],
            900,
            200,
            "raise torque",
            "restore torque",
            record_limit,
            |_| Ok(()),
        )
        .expect_err("restore should fail");

        assert!(err.to_string().contains("failed to set torque limit 200"));
        assert_eq!(
            context.set_calls,
            vec![
                (vec![42], 900, "raise torque".to_owned()),
                (vec![42], 200, "restore torque".to_owned()),
            ]
        );
    }
}
