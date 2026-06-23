# Python sidecar

Keep Python focused on tasks where it shines:

- dataset tooling
- experiment scripts
- training and evaluation
- model export back into the Rust runtime

Do not put the fast actuator loop here. Let Rust stay in charge of motion authority and safety.

Useful local utility:

- `python3 -m arachno_ml.codex_quota` reads the active Codex subscription rate-limit snapshot through the installed `codex` app-server
- `python3 -m arachno_ml.claude_quota` reads the active Claude Code subscription rate-limit snapshot through the installed `claude` CLI statusline feed

## Training Data Stub

`arachno_ml.train_policy` still returns a stub result, but it now resolves a future robot spec export and trajectory log in a small, forward-compatible way.

Supported inputs:

- `TrainingConfig(dataset_path=...)` may point at a dataset directory or directly at a trajectory log file
- `TrainingConfig(robot_spec_path=...)` may be used to override spec discovery when the spec is not next to the log
- dataset directories are searched for `robot-spec.json` or `robot_spec.json`, plus `trajectory.jsonl`, `trajectory.log`, or `trajectory.json`

Accepted robot spec shape:

- top-level JSON object or `{ "robot_spec": { ... } }`
- reads `robot.name`, top-level `control_hz`, `legs[*]`, top-level `joint_order`, and optional `arm` servo metadata when present
- accepts either per-leg `coxa_servo_id`/`femur_servo_id`/`tibia_servo_id` fields or the exported nested `servo_ids.{coxa,femur,tibia}` shape

Accepted trajectory log shape:

- line-delimited JSON or regular JSON
- accepts the Rust JSONL wrapper shape `{ "record_type": "...", "record": { ... } }`
- direct step records, top-level `steps`/`samples`/`transitions`, or `episodes[*]` containing one of those collections
- step records may expose observations as `observation`, `obs`, `state`, or `snapshot`
- step records may expose actions as `action`, `commands`, `command`, `target`, or `joint_command`
