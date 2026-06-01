# Locomotion Roadmap

The safest next move for this robot is not end-to-end reinforcement learning on hardware. The better path is:

1. Make it stand reliably.
2. Make it walk with a simple hand-built gait.
3. Log everything.
4. Train a policy in simulation.
5. Deploy the learned policy as a bounded residual on top of the stable gait.

That approach fits this platform well because the Feetech STS servos are position-servo oriented and already provide rich feedback. The current repo now covers the first two steps with a cautious `stand` mode and a slow tripod gait in `arachno-brain`.

## Phase 1: Non-Learning Baseline

Goals:

- move into a safe neutral stand pose and hold it
- walk with a very slow tripod gait
- verify joint directions, leg geometry, and safe offsets one leg at a time

Current implementation:

- `arachno-brain --mode stand` holds the measured startup pose so the robot can safely stiffen in place
- `arachno-brain --mode slow-walk` starts from that startup stand pose and runs a small tripod gait around it
- motion is bounded by the configured home ticks, gait offsets, and shared safety limits

## Phase 2: Minimum Sensing For Walking

Priorities:

- use the IMU for roll and pitch awareness first
- use servo feedback for achieved position, speed, load/current, voltage, and temperature
- treat the camera as secondary for early locomotion work

Why:

- the first walking problem is body stability and repeatable foot motion
- terrain-aware vision can come later after flat-ground walking is dependable

## Phase 3: Data Recorder

Log these together:

- commanded mode and gait parameters
- actual servo positions and speeds
- servo load/current, voltage, and temperature
- IMU attitude and angular rates
- any emergency-stop or safety-trip events

This gives us debugging data, imitation data, and calibration targets for simulation.

## Phase 4: Simulation Training

Use Python for training and keep Rust as the deployment runtime.

Recommended shape:

- simulate the robot in Isaac Lab or an equivalent simulator
- train on a workstation instead of the Jetson
- export the learned policy to ONNX
- load the policy from Rust for deployment

## Phase 5: Residual Learning

Do not replace the baseline gait with unconstrained learned joint commands at first.

Use the hand-built gait as the base controller and let the learned policy output bounded corrections for:

- foot lift adjustment
- body stabilization
- stride scaling
- yaw compensation

This is safer on hardware and usually converges faster than raw end-to-end walking from scratch.

## First Learned Task

Start with blind velocity tracking on flat ground.

Inputs:

- commanded `vx`, `vy`, and yaw rate
- IMU orientation and angular rates
- joint positions and velocities
- servo load/current
- previous action

Outputs:

- small residual offsets to gait parameters or joint targets

Rewards:

- track the commanded velocity
- stay upright
- reduce slip
- reduce high load/current
- reduce body oscillation
- avoid saturating joints

Vision-based walking should come after the flat-ground controller is stable.

## Recommended Repo Order

1. Wire the real STS bus into `arachno-brain`.
2. Add `stand` and `slow_walk`.
3. Add synchronized telemetry logging.
4. Expand the IMU abstraction in `arachno-hal`.
5. Scaffold Python simulation and ONNX export tooling.
6. Load bounded learned policies from Rust.

## References

- Isaac Lab overview: <https://isaac-sim.github.io/IsaacLab/v2.2.0/index.html>
- Isaac Lab quickstart: <https://isaac-sim.github.io/IsaacLab/release/3.0.0-beta2/source/setup/quickstart.html>
- Isaac Lab training guide: <https://isaac-sim.github.io/IsaacLab/main/source/overview/reinforcement-learning/training_guide.html>
- Isaac Lab robot setup: <https://isaac-sim.github.io/IsaacLab/release/3.0.0-beta2/source/tutorials/01_assets/add_new_robot.html>
- ONNX Runtime docs: <https://onnxruntime.ai/docs/>
- ONNX Runtime C/C++ API: <https://onnxruntime.ai/docs/api/c/c_cpp_api.html>
