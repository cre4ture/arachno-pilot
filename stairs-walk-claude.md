# Stair Descent Strategy (adaptive, unknown step height)

## Key constraints from codebase

- Servo feedback: `present_position_ticks`, `present_load_pct`, `present_current_ma` (optional) — in `ServoTelemetry` (`crates/arachno-msg/src/lib.rs`)
- FK exists in `crates/arachno-core/src/lib.rs:629-682` (`top_view_pose`, `side_view_pose`)
- **No IK solver exists** — must be added first
- Gait is fixed-amplitude phase-based tripod; no per-leg position targeting yet
- IMU roll/pitch available (`apps/arachno-brain/src/main.rs:3842`)
- Tripod A: FL, MR, RL / Tripod B: FR, ML, RR — always lower one tripod at a time for stability

The core sensing principle: when a foot contacts a surface, the tibia servo stalls — `present_position_ticks` stops advancing while a downward command is active, and `present_load_pct` spikes. No additional hardware needed.

---

## Implementation plan (ordered by dependency)

### Step 1 — IK solver (`crates/arachno-core/src/lib.rs`)

3-DOF IK on `LegConfig`: coxa heading from `atan2(y, x)` (top-down projection), then reduce to 2D plane (reach = `sqrt(x²+y²) - coxa_length`, height = z), then standard 2-link planar IK via law of cosines using `femur_length_cm` and `tibia_length_cm`.

**Prerequisite for all steps below.**

### Step 2 — Contact detection (`apps/arachno-brain/src/main.rs`)

Per-leg `LegContactState { contacted: bool, contact_position_ticks: u16 }`:

- **Primary signal**: tibia `present_load_pct` exceeds threshold (~35%) while a downward command is active
- **Secondary signal**: commanded ticks vs. actual `present_position_ticks` diverge beyond a deadband
- **Optional upgrade**: use `present_current_ma` if reported — more responsive than load percentage
- During probing: disable telemetry windowing (`telemetry_stride`) on the active probe legs (full-rate polling)

### Step 3 — Per-leg position targeting (`apps/arachno-brain/src/main.rs`)

Add a `LegTargetPose` mode alongside the phase-gait: each leg gets an explicit foot position `(x, y, z)` target that IK resolves to servo ticks. Non-descending legs hold their current position (static stance). This decouples individual leg control from the shared phase clock.

### Step 4 — Step probing behavior

Incremental downward extension: lower z by ~0.5 cm per control tick (≈ 1–2 cm/s at 20 Hz). On contact:

1. Record actual servo positions
2. Compute foot z via FK from actual positions
3. Step depth = reference_z − contact_z

If probe reaches `max_step_height_cm` without contact → step is too large, abort.

### Step 5 — Center of mass shift

Before lowering a front tripod, shift all stance-leg targets rearward by `com_shift_cm` (body moves forward over support polygon). Use IMU pitch feedback to confirm the shift stays within `max_body_pitch_deg`. Shift forward again after weight is transferred to the lower step.

### Step 6 — Stair descent state machine (`BrainMode::StairDescend`)

```
Idle
  └─ triggered → ProbePhase

ProbePhase
  ├─ CoM shift rearward
  ├─ Probe front-left + front-right down simultaneously
  ├─ step_depth > max_step_height_cm → TooLarge (abort, stay put)
  └─ contact OK → LowerFrontLegs

LowerFrontLegs
  ├─ Command front legs to contact position (commit)
  ├─ CoM shift forward (transfer weight)
  └─ → LowerMiddleLegs

LowerMiddleLegs
  ├─ Probe + lower middle legs
  └─ → LowerRearLegs

LowerRearLegs
  ├─ Probe + lower rear legs
  └─ → NormalizeStance

NormalizeStance
  └─ Return to stand-reference pose on new level → Idle
```

### Step 7 — Config additions (`config/robot/servo-config.toml`)

```toml
[locomotion.stair_descent]
probe_speed_cm_per_s = 1.5
max_step_height_cm = 8.0
contact_load_threshold_pct = 35.0
com_shift_cm = 3.0
```

### Step 8 — Edge/stair detection (v2)

**Automatic**: during normal walk, if front legs find no ground at expected height → trigger `StairDescend`.
**For v1**: use a manual dashboard button to engage `StairDescend` mode.

---

## Dependency table

| # | Task | Blocks |
|---|------|--------|
| 1 | IK solver | 3, 4, 5, 6 |
| 2 | Contact detection | 4, 6 |
| 3 | Per-leg position targeting | 4, 5, 6 |
| 4 | Step probing | 6 |
| 5 | CoM shift | 6 |
| 6 | State machine + config | — |
| 7 | Auto edge detection | v2 |

**Start with Step 1 (IK solver).**
