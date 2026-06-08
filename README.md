# arachno-pilot

Rust-first starter workspace for a hexapod that can run either on a tethered Linux PC or on an onboard Jetson, with Feetech STS bus servos and interchangeable camera backends.

## Why this layout

- Rust owns the safety-critical control loop, robot state, kinematics, telemetry, and hardware abstraction.
- Python stays available for data collection, training, and fast experiments.
- C/C++ is kept behind thin bridges for Jetson-specific libraries such as TensorRT or lower-level camera APIs when needed.

## Workspace map

- `apps/arachno-brain`: hardware-owning runtime that now serves telemetry, camera, dashboard, manual control, `lay_down`, `stand_up`, `stand`, `slow_walk`, `backward_walk`, `rotate_left`, and `rotate_right` from one process.
- `apps/arachno-calibrate`: servo ID, EEPROM-profile, range-scan, pose-check, and pose-suggestion tooling.
- `apps/arachno-fw-info`: host-side firmware version and capability query for the RP2040 IMU bridge.
- `apps/arachno-probe`: host-device reachability checks for configured camera and servo bridge paths.
- `crates/arachno-core`: robot config, gait primitives, and shared domain logic.
- `crates/arachno-hal`: hardware traits for servo buses, cameras, and future devices.
- `crates/arachno-feetech-sts`: STS/TTL bus implementation area.
- `crates/arachno-imu-proto`: shared no-std IMU packet format for the host and RP2040 bridge.
- `crates/arachno-imu-host`: Linux-side USB CDC reader for the RP2040 IMU bridge.
- `crates/arachno-camera`: camera pipeline builder and camera-facing code for both `argus` and `v4l2`.
- `crates/arachno-control`: loop orchestration and safety boundaries.
- `crates/arachno-msg`: message and telemetry types shared across crates.
- `python/`: training, evaluation, and experiment scripts.
- `native/`: narrow C++ bridge area for TensorRT, Argus, or vendor SDK shims.
- `firmware/`: embedded Rust workspace for microcontroller-side bridge firmware.
- `config/robot`: robot and hardware configuration files.
- `config/robot/servo-config.toml`: single source of truth for Feetech bus settings, expected EEPROM values, safety limits, locomotion tuning, servo IDs, semantic zero-reference ticks, and joint direction signs.
- `config/robot/servo-poses.toml`: named robot poses stored as logical joint angles in degrees.
- `docs/architecture.md`: the recommended runtime and integration model.
- `docs/roadmap.md`: staged locomotion and learning roadmap for the spider.

## Deployment profiles

- `config/robot/host-usb.toml`: regular Linux PC connected to the robot over USB, with a USB camera and Feetech bridge.
- `config/robot/jetson-onboard.toml`: Jetson mounted on the robot, with the CSI camera connected locally.
- `config/robot/default.toml`: current local-development default, aligned with the host USB setup for now.
- `config/robot/servo-config.toml`: shared servo/bus/safety/locomotion map loaded by all deployment profiles.
- `config/robot/servo-poses.toml`: shared semantic pose map loaded by all deployment profiles.
- `config/robot/servo-ranges.toml`: measured free-movement envelopes written by the low-torque self-stop calibration scan.
- `config/robot/servo-semantic-calibration.toml`: dashboard-captured semantic zero-reference corrections for joint-angle display and manual control.

## Locomotion roadmap

The current development plan is documented in [docs/roadmap.md](/home/uli/rust-dev/arachno-pilot/docs/roadmap.md:1).

Implemented now:

- `apply-eeprom`: temporarily clears the servo EEPROM `Lock Mark` (`0x37`), writes the configured persistent profile, verifies every entry by readback, then restores the lock to `1`
- `verify-eeprom`: performs the same EEPROM profile validation without writing any values
- `lay-down`: moves into a known stretched rest pose
- `stand-up`: raises the femurs first, lowers the tibias to replant the feet, then lifts the body with coordinated femur+tibia motion before aligning the coxae
- `stand`: settles into and holds the configured stand-reference pose
- `manual`: captures the current robot pose as a zero-reference and accepts grouped semantic angle commands from the dashboard in `forward/back` and `up/down` space
- `slow-walk`: a cautious tripod gait that now derives semantic swing and lift amplitudes from the calibrated stand pose, leg lengths, and angle-to-tick conversion instead of using tiny fixed tick offsets
- `backward-walk`: the same derived tripod gait profile as `slow-walk`, but with reversed coxa swing for backward motion
- `rotate-left`: the same derived tripod gait profile, but with left/right coxa swing opposed to rotate the body left
- `rotate-right`: the same derived tripod gait profile, but with left/right coxa swing opposed to rotate the body right
- `sense-ranges`: lowers torque limit, drives tibia/femur/coxa toward full-range endpoints, and writes the self-stopped travel envelopes to TOML
  It validates the configured EEPROM profile first and refuses to start the scan if any servo does not match.
  Use `--skip-initial-lay-down` to resume a partially completed scan from the robot's current posture.
  The run also emits a mixed workflow + low-level STS trace log next to the output TOML by default, or to a custom path via `--trace-output`.
- `check-poses`: compares the currently resolved `stand_reference` and `lay-down` poses against measured bounds from `servo-ranges.toml`
- `suggest-poses`: generates candidate `stand_reference` and `lay-down` ticks from the measured ranges for pose tuning
- shared hard safety checks for roll, pitch, bus voltage, and temperature, with servo load still exposed in telemetry
- `arachno-brain` validates the configured EEPROM profile on startup and refuses to start if any servo does not match

Next up:

1. Add synchronized servo + IMU logging.
2. Add IMU-assisted posture stabilization on top of the hand-built gait.
3. Add a Jetson-native live camera backend for the onboard `argus` profile.
4. Keep training and policy tooling in `python/`, then export deployable artifacts back to Rust.

## Debug dashboard

The host USB profile now uses a single hardware-owning process. The browser UI is optional and is served directly by `arachno-brain`:

```bash
just dashboard
```

It currently provides:

- a single hardware owner in `arachno-brain` for the Feetech bridge, IMU bridge, camera route, and optional browser dashboard
- live motion status for `telemetry`, `manual`, `lay_down`, `stand_up`, `stand`, `slow_walk`, `backward_walk`, `rotate_left`, and `rotate_right`
- live servo polling through the real Feetech bus path via the brain API
- live RP2040 IMU bridge state with roll/pitch sanity estimates and raw motion health
- fault-tolerant telemetry cards per configured servo
- a browser camera stream for the USB V4L2 camera path
- grouped manual servo control in angles, with `all legs`, left/right, front/middle/rear pairs, tripod groups, and individual legs available from the dashboard
- manual utility actions to sync the selected group target to the live pose and to apply a verified RAM torque limit to the selected group without fighting the current target position
- a `Copy Current Pose To Clipboard` action that exports the live joint pose as a TOML snippet grouped by leg
- semantic joint calibration capture in the dashboard, with named reference poses per leg/joint that correct the zero tick while keeping the `4096/360` slope fixed

This removes the old serial-port ownership conflict where the brain and dashboard could not run together, because there is now only one process touching hardware. The dashboard is intentionally tolerant of partial hardware bring-up: if only one servo replies or a servo reports fault flags, that state is shown directly instead of being hidden behind a generic failure.

## IMU bridge

The repo now includes a Rust-to-Rust IMU bridge path:

- `firmware/rp2040-imu-bridge`: Embassy-based RP2040 USB CDC firmware
- `crates/arachno-imu-proto`: binary framing shared with the host
- `crates/arachno-imu-host`: Linux reader used by `arachno-brain`

The firmware now probes a real `MPU-9250`-class sensor over `SPI`, supports common `MPU-6500`-compatible IDs during bring-up, and reports the selected `SPI` mode plus observed `WHO_AM_I` value through `fw-version`. The host USB and current default profiles ship with the IMU bridge enabled.

Build helpers:

- `just fw-version`
- `just firmware-check`
- `just firmware-build`
- `just firmware-build-release`
- `just firmware-uf2`

## Quick start

```bash
cargo run -p arachno-brain -- --config config/robot/default.toml --listen 127.0.0.1:4000
cargo run -p arachno-calibrate -- --config config/robot/default.toml
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode apply-eeprom
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode verify-eeprom
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode sense-ranges --output config/robot/servo-ranges.toml
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode sense-ranges --output config/robot/servo-ranges.toml --trace-output /tmp/servo-ranges.trace.log
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode check-poses --ranges config/robot/servo-ranges.toml
cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode suggest-poses --ranges config/robot/servo-ranges.toml --suggestions-output /tmp/servo-pose-suggestions.toml
cargo run -p arachno-probe -- --config config/robot/default.toml
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode manual --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode lay-down --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand-up --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode slow-walk --walk-seconds 8 --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode backward-walk --walk-seconds 8 --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode rotate-left --walk-seconds 8 --dashboard
cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode rotate-right --walk-seconds 8 --dashboard
cargo run -p arachno-brain -- --config config/robot/jetson-onboard.toml --listen 127.0.0.1:4000
cargo check --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi
```

`arachno-brain` now owns the live hardware-facing telemetry API at `/api/state`, the camera route at `/camera.mjpg`, the rich dashboard UI at `/` and `/dashboard` when started with `--dashboard`, the grouped manual-control API at `/api/manual/*`, the dashboard pose-copy utility, and the first hardware motion modes through `--mode telemetry`, `--mode manual`, `--mode lay-down`, `--mode stand-up`, `--mode stand`, `--mode slow-walk`, `--mode backward-walk`, `--mode rotate-left`, and `--mode rotate-right`.

Servo EEPROM policy lives in `config/robot/servo-config.toml` under `[[servo_eeprom.entries]]`. Only `arachno-calibrate --mode apply-eeprom` writes those persistent registers. Normal runtime writes are blocked from EEPROM registers in the STS driver, and `arachno-brain` validates the configured EEPROM values before it starts the control worker.
