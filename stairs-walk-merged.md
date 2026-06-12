# Stair Descent — Merged Implementation Plan

## Principles

- Build a dedicated `BrainMode::DescendStairs` controller, not an extension of the tripod gait
- Probe each leg by setting a torque limit and commanding to max probe depth; the servo stops itself on contact; compute drop from the settled position via FK
- Descend front → middle → rear, one tripod at a time (3 legs always on solid ground)
- v1 trigger is manual (dashboard button); automatic edge detection is v2
- v1 is a slow sensing-and-placement controller — do not optimize for speed until hardware validation is complete

## Key existing building blocks

| Asset | Location |
|-------|----------|
| Torque-limited stop-on-resistance probe | `apps/arachno-calibrate/src/main.rs:950` |
| Forward kinematics (side view) | `crates/arachno-core/src/lib.rs:653` |
| Semantic angle → ticks conversion | `apps/arachno-brain/src/main.rs:2737` |
| Servo load / current / moving telemetry | `crates/arachno-msg/src/lib.rs:12` |
| Servo measured range envelopes (coxa, femur) | `config/robot/servo-ranges.toml` |
| IMU roll/pitch estimation | `apps/arachno-brain/src/main.rs:3842` |
| Tripod grouping A: FL MR RL / B: FR ML RR | `crates/arachno-core/src/lib.rs:685` |

**Gaps**: no IK solver, no per-leg Cartesian foot workspace definition, no per-leg position targeting, safety does not yet use load/current for control decisions (`apps/arachno-brain/src/main.rs:905`).

**Note on tibia limits**: `servo-ranges.toml` stores independent per-joint limits. Coxa and femur limits are usable as-is. The tibia's safe range is not independent — it depends on the femur's current angle. A tibia angle valid at femur=45° may cause collision or overextension at femur=70°. The solution is to not use per-joint tibia limits at all; instead define the reachable workspace in Cartesian foot space (see Step 2).

---

## Implementation steps (ordered by dependency)

### - [x] Step 1 — Extract probing helper into shared crate

Extract the torque-limited stop-on-resistance logic from `apps/arachno-calibrate/src/main.rs:950` into `crates/arachno-core` so it can be used at runtime without duplication. Add unit tests for contact-detection traces alongside the extraction.

### - [x] Step 2 — Define and calibrate per-leg Cartesian foot workspace

Rather than using per-joint tibia limits (which are coupled to the femur position), represent each leg's safe operating envelope as a **2D foot workspace** in the side-view plane: `(reach_cm, height_cm)` relative to the coxa joint.

Add a new config section (e.g. `config/robot/leg-workspace.toml`):

```toml
[front_left]
min_reach_cm = 8.0    # minimum horizontal distance from coxa
max_reach_cm = 22.0   # maximum horizontal distance
min_height_cm = -18.0 # lowest the foot can go (negative = below coxa plane)
max_height_cm = 2.0   # highest the foot can go
```

Extend `arachno-calibrate` to derive these bounds empirically by sweeping the (femur, tibia) joint space and recording the Cartesian envelope. This captures real mechanical limits rather than relying solely on ideal link lengths.

**Calibration pose constraint**: the current calibration runs in the laying pose. In that pose the body rests on the ground, so the femur can only move upward — the downward half of the femur range and most of the tibia extension range are blocked by the floor. For stair descent the downward range is exactly what matters.

The workspace sweep must therefore be done in a **standing calibration mode**:

1. Robot stands on all six legs (normal stand-reference pose)
2. Sweep one leg at a time while the other five hold stance
3. For each leg: sweep femur across its full upward range from `servo-ranges.toml`; at each femur step, sweep tibia across the range that doesn't require the foot to push through the floor (detect ground contact via torque limit, the same mechanism used during stair probing)
4. Record the Cartesian envelope of all (reach, height) pairs reached before contact or joint limit
5. Restore leg to stand-reference before moving to the next leg

This is a new calibration mode in `arachno-calibrate` — distinct from the existing range scan. It reuses the torque-limited probe helper from Step 1.

Load the resulting `leg-workspace.toml` into `arachno-core` at runtime. For the probe specifically: given the current foot x-position (reach), `min_height_cm` at that reach directly gives the maximum safe probe depth — no tibia joint range query needed.

### - [x] Step 3 — IK solver and reachability API (`crates/arachno-core/src/lib.rs`)

Add to `LegConfig`:
- 3-DOF IK: coxa heading from `atan2(y, x)`, then 2-link planar IK (law of cosines) for femur + tibia using `femur_length_cm` / `tibia_length_cm`
- Reachability pre-check: before running the solver, project the target `(x, y, z)` to `(reach, height)` in the side-view plane and verify it falls within the leg's Cartesian workspace from Step 2 — if not, return `Err(ReachabilityViolation)` immediately without solving
- Post-solve check: verify the resulting coxa and femur angles are within their independent measured limits from `servo-ranges.toml` (tibia limits are implicitly satisfied by the workspace pre-check)

### - [x] Step 4 — `arachno-calibrate`: `sense-workspace` subcommand

Standing per-leg femur+tibia sweep. Sweeps the femur toward its lift limit in N steps; at each step lifts tibia to safe-up position, then probes tibia downward with torque limit to detect floor contact. Records FK (reach, height) for: floor contact, tibia at max range-scan extension, and tibia fully retracted. Computes bounding-box `LegWorkspace` per leg and writes `config/robot/leg-workspace.toml`. New CLI flags: `--workspace-output` and `--workspace-femur-steps`.

Added to `LegConfig` in `arachno-core`: `femur_deg_from_ticks` / `tibia_deg_from_ticks` (inverse of `pose_ticks_from_angles`).

### - [ ] Step 5 — Contact detection (`apps/arachno-brain/src/main.rs`)

The servo's own firmware handles the stop: set a torque limit on the tibia servo before probing, then command the target position at max probe depth. When the foot contacts the surface the servo stops itself — in its own control loop, far faster than the host's 20 Hz. No high-rate polling race is needed.

The host only needs to confirm the servo has settled:

- Poll `moving` at the normal strided rate; probe is complete when `moving` clears
- Read `present_position_ticks` once to compute contact height via FK
- Secondary sanity check: `present_position_ticks` significantly short of commanded target confirms contact rather than range limit

Tuning challenge: the torque limit must be low enough to stop cleanly on contact, but high enough that the leg doesn't stall mid-air under its own weight. This is a calibration task (hardware validation stage 1).

Per-leg result type: `ProbeOutcome { contact: bool, settled_ticks: u16 }`.

Extend safety in `apps/arachno-brain/src/main.rs:905` to use load/current as a control signal, not only as a shutdown trigger.

### - [ ] Step 5 — Per-leg position targeting

Add a `LegTargetPose` mode: each leg gets an explicit foot `(x, y, z)` target resolved via IK to servo ticks. Non-descending legs hold their current position. Decouples individual leg control from the shared phase clock.

### - [ ] Step 6 — Step probing behavior

Using the helper from Step 1 and the torque-limit approach from Step 4:

1. Set torque limit on the tibia servo (configurable `probe_torque_limit_pct`)
2. Compute max probe depth: `min(max_step_height_cm, current_height - workspace.min_height_cm_at(current_reach))` — the workspace from Step 2 gives the hard floor, `max_step_height_cm` is the policy limit above it; command the tibia to the shallower of the two
3. Wait for `moving` to clear (polled at normal strided rate — no special windowing needed)
4. If `settled_ticks` ≈ commanded target → no contact within range → step too large, abort
5. If `settled_ticks` significantly short of target → contact; compute step drop = reference_z − FK(settled_ticks)
6. Restore normal torque limit

Add `StepEstimate { drop_cm: f32, settled_ticks: u16 }` to carry the result through the state machine.

### - [ ] Step 7 — Center of mass shift

Before lowering a front tripod: shift all stance-leg targets rearward by `com_shift_cm` (body moves forward over support polygon). After weight transfer to lower step: shift forward to normalize. Guard with IMU pitch feedback — abort if pitch exceeds `max_body_pitch_deg` during transfer.

### - [ ] Step 8 — Stair descent state machine (`BrainMode::DescendStairs`)

```
Idle
  └─ dashboard trigger → ProbePhase

ProbePhase
  ├─ CoM shift rearward
  ├─ Probe front-left + front-right simultaneously
  ├─ disagreement > tolerance → Abort (uneven terrain)
  ├─ step_drop > max_step_height_cm → Abort (too large)
  └─ contact OK on both → LowerFrontLegs

LowerFrontLegs
  ├─ Commit front legs to contact position
  ├─ CoM shift forward (weight transfer)
  ├─ load/current spike without stable support → Abort
  └─ → LowerMiddleLegs

LowerMiddleLegs
  ├─ Probe + lower middle legs
  └─ → LowerRearLegs

LowerRearLegs
  ├─ Probe + lower rear legs
  └─ → NormalizeStance

NormalizeStance
  └─ Return to stand-reference pose on new level → Idle

Abort (any phase)
  └─ Hold current position, report reason, wait for manual recovery
```

Abort conditions (any phase): no-contact timeout, position error during probe exceeds deadband, load/current spike without stable support, step drop above `max_step_height_cm`, pitch/roll outside safe margins.

### - [ ] Step 9 — Config additions (`config/robot/servo-config.toml`)

```toml
[locomotion.stair_descent]
max_step_height_cm = 8.0
probe_torque_limit_pct = 20.0   # Low enough to stop on contact, high enough to not stall under leg weight
max_front_leg_disagreement_cm = 1.5
com_shift_cm = 3.0
```

### - [ ] Step 10 — Dashboard telemetry

Expose stair state in the dashboard: current phase, active probing leg, measured drop per leg, contact confidence, abort reason. This is essential for tuning contact thresholds during hardware validation.

### - [ ] Step 11 — Hardware validation (staged)

0. Calibration — run the new standing workspace sweep in `arachno-calibrate` to populate `leg-workspace.toml`; the existing laying-pose range scan is insufficient as it cannot reach the downward femur/tibia range needed for stair probing
1. Flat ground — tune `probe_torque_limit_pct`: high enough the leg doesn't stall mid-air, low enough it stops cleanly on contact; confirm no false positives during normal stance
2. Single known step height — verify drop measurement and state machine transitions
3. Several known step heights — verify `max_step_height_cm` abort triggers correctly
4. Unknown stairs — full system test

Only after stage 4 should speed or gait smoothness be optimized.

### - [ ] Step 12 — Automatic edge detection (v2)

During normal walk: if front legs find no ground at expected height on the stance phase, trigger `DescendStairs` automatically. Implement only after hardware validation is complete.

---

## Dependency table

| # | Task | Blocks |
|---|------|--------|
| 1 | Extract probe helper | 6 |
| 2 | Load servo-ranges.toml | 3, 6 |
| 3 | IK solver + reachability API | 5, 6 |
| 4 | Contact detection | 6 |
| 5 | Per-leg position targeting | 6, 7 |
| 6 | Step probing behavior | 8 |
| 7 | CoM shift | 8 |
| 8 | State machine + config | 10, 11 |
| 9 | Config additions | 8 |
| 10 | Dashboard telemetry | 11 |
| 11 | Hardware validation | 12 |
| 12 | Auto edge detection | — |

**Start with Steps 1 and 2 in parallel — both are self-contained and unblock everything else.**
