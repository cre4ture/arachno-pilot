# arachno-pilot

Rust-first starter workspace for a hexapod that can run either on a tethered Linux PC or on an onboard Jetson, with Feetech STS bus servos and interchangeable camera backends.

## Why this layout

- Rust owns the safety-critical control loop, robot state, kinematics, telemetry, and hardware abstraction.
- Python stays available for data collection, training, and fast experiments.
- C/C++ is kept behind thin bridges for Jetson-specific libraries such as TensorRT or lower-level camera APIs when needed.

## Workspace map

- `apps/arachno-brain`: main runtime entrypoint for the robot.
- `apps/arachno-calibrate`: servo ID, zero-point, and home-pose tooling.
- `apps/arachno-dashboard`: browser-based debug dashboard for live servo telemetry and camera streaming.
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
- `docs/architecture.md`: the recommended runtime and integration model.

## Deployment profiles

- `config/robot/host-usb.toml`: regular Linux PC connected to the robot over USB, with a USB camera and Feetech bridge.
- `config/robot/jetson-onboard.toml`: Jetson mounted on the robot, with the CSI camera connected locally.
- `config/robot/default.toml`: current local-development default, aligned with the host USB setup for now.

## Suggested next steps

1. Wire the real STS bus into `apps/arachno-brain` once you want motion commands to leave the mock path.
2. Replace the RP2040 bridge's mock IMU stream with a real `MPU-9250` or `ICM-42688-P` backend.
3. Fuse the resulting IMU stream into `arachno-control`.
4. Add a Jetson-native live camera backend for the onboard `argus` profile.
5. Keep learning and heavy model tooling in `python/`, then export deployable artifacts back to Rust.

## Debug dashboard

The host USB profile now includes a live dashboard:

```bash
just dashboard
```

It currently provides:

- live servo polling through the real Feetech bus path
- fault-tolerant telemetry cards per configured servo
- a browser camera stream for the USB V4L2 camera path

The dashboard is intentionally tolerant of partial hardware bring-up. If only one servo replies or a servo reports fault flags, that state is shown directly instead of being hidden behind a generic failure.

## IMU bridge

The repo now includes a Rust-to-Rust IMU bridge path:

- `firmware/rp2040-imu-bridge`: Embassy-based RP2040 USB CDC firmware
- `crates/arachno-imu-proto`: binary framing shared with the host
- `crates/arachno-imu-host`: Linux reader that will plug into `arachno-brain`

The firmware currently streams a mock IMU payload so the USB link and framing can be validated before the real sensor backend is attached.

Build helpers:

- `just firmware-check`
- `just firmware-build`
- `just firmware-build-release`
- `just firmware-uf2`

## Quick start

```bash
cargo run -p arachno-brain -- --config config/robot/default.toml
cargo run -p arachno-calibrate -- --config config/robot/default.toml
cargo run -p arachno-probe -- --config config/robot/default.toml
cargo run -p arachno-dashboard -- --config config/robot/host-usb.toml --listen 127.0.0.1:3000

cargo run -p arachno-brain -- --config config/robot/host-usb.toml
cargo run -p arachno-brain -- --config config/robot/jetson-onboard.toml
cargo check --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi
```
