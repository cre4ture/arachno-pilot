# Architecture

## Preferred split

Treat deployment mode as a runtime choice, not as a fork in the codebase. The same Rust core should support both a tethered host computer and an onboard Jetson.

- Rust process `arachno-brain`
  - Owns the servo loop, safety policy, robot state, gait generation, telemetry, and command arbitration.
  - Talks to Feetech STS servos over a dedicated bus adapter.
  - Pulls camera frames from a configurable camera backend.
  - Consumes model outputs from either embedded Rust inference or a sidecar process.
- Python sidecar `python/arachno_ml`
  - Handles dataset work, policy training, experiment notebooks, quick behavior iteration, and offline evaluation.
  - Can run on the Jetson for lightweight experiments, but should not own the hard realtime-ish actuator loop.
- C++ bridge `native/`
  - Only for libraries whose best interface is still C++ on Jetson, such as TensorRT wrappers or lower-level camera integration.
  - Expose a minimal API to Rust through `cxx`, not a large object graph.

## Deployment profiles

### Host USB

Use this mode when the robot is controlled by a regular Linux PC over cables:

- servo bridge reaches the robot over USB
- camera is a USB UVC device
- optional IMU reaches the host through a USB CDC bridge on the RP2040
- good for bring-up, debugging, calibration, and early walking experiments

### Jetson onboard

Use this mode when the Jetson is mounted on the robot:

- servo bridge is still reachable through the same Rust bus abstraction
- camera backend uses CSI via `nvarguscamerasrc`
- best fit for untethered perception and onboard inference

## Runtime loops

Treat the robot as three loops with different rates:

- Servo/safety loop: 100-200 Hz
  - Reads position/load/current-style feedback from STS servos.
  - Applies limit checks, stall detection, recovery logic, and command shaping.
- Perception loop: 15-30 Hz
  - Captures and preprocesses camera frames.
  - Runs detection, terrain, or policy perception models.
- Learning/logging loop: 1-10 Hz or asynchronous
  - Records trajectories and telemetry.
  - Updates non-safety-critical policy state.
  - Never bypasses the safety gate.

## Recommended crate roles

### `crates/arachno-core`

Pure robot logic:

- robot configuration
- leg naming and joint mapping
- stand-reference pose and gait primitives
- kinematics later

### `crates/arachno-hal`

Stable hardware-facing traits:

- servo bus
- camera source
- IMU source
- future range finder, foot contact, battery monitor

The rest of the code should depend on these traits, not on concrete devices.

### `crates/arachno-feetech-sts`

Servo implementation details:

- packet encoding/decoding
- sync write and batched read helpers
- servo discovery
- configuration registers
- telemetry normalization

### `crates/arachno-camera`

Camera entrypoint:

- build the `nvarguscamerasrc` or `v4l2src` pipeline string
- later own the `gstreamer-rs` `appsink` consumer for both modes
- lens profile and calibration metadata

### `crates/arachno-imu-proto`

Shared bridge framing:

- no-std packet encoder/decoder
- resynchronizing frame parser
- shared sample format for RP2040 firmware and Linux host

### `crates/arachno-imu-host`

USB CDC bridge adapter:

- opens the RP2040 bridge as a serial device on Linux
- parses `arachno-imu-proto` frames
- converts raw sensor units into robot-facing telemetry

### `crates/arachno-control`

Policy boundary and safety envelope:

- choose what command source wins
- limit velocity, acceleration, and body pose
- reject dangerous learning outputs
- degrade gracefully when camera or model stalls

## Why not pure Python

Python is excellent for learning and tooling, but not the best place to put the servo control core for an expensive hexapod:

- Linux scheduling jitter matters more when 18 bus servos are moving at once.
- Safety code benefits from Rust’s stronger typing and easier fault containment.
- You will likely want more devices later, which makes a trait-driven Rust core age better.

## Why not pure C++

You absolutely can build this in C++, but a Rust-first setup buys you:

- safer concurrency around shared robot state
- cleaner boundaries between hardware, control, and learning
- an easier path to mix Python and native code without letting the whole system become glue code

## FFI rules

Keep language boundaries narrow:

- Rust <-> Python: use `PyO3` only for small control surfaces or data exchange helpers.
- Rust <-> C++: use `cxx` for thin bridges around TensorRT or special Jetson APIs.
- Avoid calling Python from the fast servo loop.
- Avoid exposing raw vendor SDK types across the whole Rust codebase.

## Deployment notes

- Keep Jetson OS and host-PC robot app setup separate from training experiments.
- Use `systemd` for the robot runtime.
- Put model artifacts and calibration files under `config/` or a dedicated `artifacts/` directory.
- Keep power for Jetson and servo rail separate, with common ground and a hard e-stop path.

## Future hardware extensions

Add new hardware by implementing `arachno-hal` traits in new crates:

- `crates/arachno-imu-bno085`
- `crates/arachno-lidar-ld06`
- `crates/arachno-power-monitor`
- `crates/arachno-foot-contact`

That keeps extensions additive instead of forcing rewrites across the whole project.

## RP2040 bridge firmware

Keep microcontroller firmware in the separate `firmware/` workspace:

- host-side `cargo check --workspace` stays focused on Linux binaries
- RP2040 firmware can use its own target, linker, and flash workflow
- shared protocol crates still live in `crates/` so the firmware and host stay aligned
