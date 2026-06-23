from __future__ import annotations

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any

ROBOT_SPEC_CANDIDATES = (
    "robot-spec.json",
    "robot_spec.json",
    "robot-spec-export.json",
    "robot_spec_export.json",
)

TRAJECTORY_LOG_CANDIDATES = (
    "trajectory.jsonl",
    "trajectory-log.jsonl",
    "trajectory.ndjson",
    "trajectory.log",
    "trajectory.trace.log",
    "trajectory.json",
    "trajectory-log.json",
)

STEP_KEYS = (
    "observation",
    "obs",
    "state",
    "snapshot",
    "action",
    "commands",
    "command",
    "target",
    "joint_command",
    "telemetry",
    "imu",
    "body_mode",
    "reward",
    "done",
    "terminal",
    "timestamp_ms",
)

OBSERVATION_KEYS = ("observation", "obs", "state", "snapshot")
ACTION_KEYS = ("action", "commands", "command", "target", "joint_command")
EPISODE_KEYS = ("episode_id", "episode_index", "episode", "rollout_id")
STEP_COLLECTION_KEYS = ("steps", "transitions", "samples", "records")
TRAJECTORY_WRAPPER_KEYS = ("trajectory", "trajectory_log", "log")


@dataclass(frozen=True)
class RobotSpec:
    source_path: Path
    name: str
    control_hz: int | None
    joint_names: tuple[str, ...]
    servo_ids: tuple[int, ...]


@dataclass(frozen=True)
class TrajectoryLog:
    source_path: Path
    episode_count: int
    step_count: int
    observation_fields: tuple[str, ...]
    action_fields: tuple[str, ...]


@dataclass(frozen=True)
class TrainingDataset:
    robot_spec: RobotSpec | None
    trajectory_log: TrajectoryLog


def load_training_dataset(
    dataset_path: str | Path,
    *,
    robot_spec_path: str | Path | None = None,
) -> TrainingDataset:
    spec_path, trajectory_path = resolve_training_paths(
        dataset_path, robot_spec_path=robot_spec_path
    )
    robot_spec = load_robot_spec(spec_path) if spec_path is not None else None
    trajectory_log = load_trajectory_log(trajectory_path)
    return TrainingDataset(robot_spec=robot_spec, trajectory_log=trajectory_log)


def resolve_training_paths(
    dataset_path: str | Path,
    *,
    robot_spec_path: str | Path | None = None,
) -> tuple[Path | None, Path]:
    dataset = Path(dataset_path).expanduser()
    if dataset.is_dir():
        trajectory_path = _find_first_existing(dataset, TRAJECTORY_LOG_CANDIDATES)
        if trajectory_path is None:
            raise FileNotFoundError(
                f"could not find a trajectory log under {dataset}; "
                f"looked for {', '.join(TRAJECTORY_LOG_CANDIDATES)}"
            )
        spec_path = (
            _resolve_input_path(robot_spec_path, dataset)
            if robot_spec_path is not None
            else _find_first_existing(dataset, ROBOT_SPEC_CANDIDATES)
        )
        return spec_path, trajectory_path

    trajectory_path = dataset
    if not trajectory_path.exists():
        raise FileNotFoundError(f"trajectory log not found: {trajectory_path}")
    spec_path = (
        _resolve_input_path(robot_spec_path, trajectory_path.parent)
        if robot_spec_path is not None
        else _find_first_existing(trajectory_path.parent, ROBOT_SPEC_CANDIDATES)
    )
    return spec_path, trajectory_path


def load_robot_spec(path: str | Path) -> RobotSpec:
    source_path = Path(path).expanduser()
    payload = _unwrap_mapping(_load_json_payload(source_path))
    robot_meta = _mapping(payload.get("robot"))

    name = (
        _first_string(payload, "name", "robot_name")
        or _first_string(robot_meta, "name")
        or source_path.stem
    )
    control_hz = _first_int(robot_meta, "control_hz")
    if control_hz is None:
        control_hz = _first_int(payload, "control_hz")

    joint_names = tuple(_dedupe(_extract_joint_names(payload)))
    servo_ids = tuple(_dedupe(_extract_servo_ids(payload)))
    return RobotSpec(
        source_path=source_path,
        name=name,
        control_hz=control_hz,
        joint_names=joint_names,
        servo_ids=servo_ids,
    )


def load_trajectory_log(path: str | Path) -> TrajectoryLog:
    source_path = Path(path).expanduser()
    payload = _load_json_or_jsonl_payload(source_path)
    summary = _summarize_trajectory_payload(payload)
    if summary.step_count == 0:
        raise ValueError(f"trajectory log did not contain any recognized steps: {source_path}")
    return TrajectoryLog(
        source_path=source_path,
        episode_count=summary.episode_count,
        step_count=summary.step_count,
        observation_fields=summary.observation_fields,
        action_fields=summary.action_fields,
    )


@dataclass
class _TrajectorySummary:
    episode_count: int
    step_count: int
    observation_fields: tuple[str, ...]
    action_fields: tuple[str, ...]


def _summarize_trajectory_payload(payload: Any) -> _TrajectorySummary:
    episode_ids: list[str] = []
    observation_fields: list[str] = []
    action_fields: list[str] = []
    step_count = 0

    def visit(node: Any, episode_hint: str | None = None) -> None:
        nonlocal step_count

        if isinstance(node, list):
            for item in node:
                visit(item, episode_hint)
            return

        if not isinstance(node, Mapping):
            return

        record_wrapper = node.get("record")
        if "record_type" in node and isinstance(record_wrapper, (Mapping, list)):
            visit(record_wrapper, episode_hint)
            return

        for key in TRAJECTORY_WRAPPER_KEYS:
            nested = node.get(key)
            if isinstance(nested, (Mapping, list)):
                visit(nested, episode_hint)
                return

        episodes = node.get("episodes")
        if isinstance(episodes, list):
            for index, episode in enumerate(episodes):
                nested_hint = _episode_hint(episode)
                if nested_hint is None:
                    nested_hint = f"episode-{index}"
                visit(episode, nested_hint)
            return

        for key in STEP_COLLECTION_KEYS:
            steps = node.get(key)
            if isinstance(steps, list):
                nested_hint = episode_hint or _episode_hint(node)
                if nested_hint is not None and steps:
                    episode_ids.append(nested_hint)
                for step in steps:
                    visit(step, nested_hint)
                return

        if not _looks_like_step(node):
            return

        step_count += 1
        nested_hint = episode_hint or _episode_hint(node)
        if nested_hint is not None:
            episode_ids.append(nested_hint)
        _extend_field_names(observation_fields, _first_mapping(node, OBSERVATION_KEYS))
        _extend_field_names(action_fields, _first_mapping(node, ACTION_KEYS))

    visit(payload)

    episode_count = len(_dedupe(episode_ids))
    if episode_count == 0 and step_count > 0:
        episode_count = 1

    return _TrajectorySummary(
        episode_count=episode_count,
        step_count=step_count,
        observation_fields=tuple(_dedupe(observation_fields)),
        action_fields=tuple(_dedupe(action_fields)),
    )


def _load_json_payload(path: Path) -> Mapping[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    return _unwrap_mapping(payload)


def _load_json_or_jsonl_payload(path: Path) -> Any:
    raw_text = path.read_text(encoding="utf-8")
    stripped = raw_text.strip()
    if not stripped:
        raise ValueError(f"input file is empty: {path}")

    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        records = []
        for line_number, line in enumerate(raw_text.splitlines(), start=1):
            item = line.strip()
            if not item:
                continue
            try:
                records.append(json.loads(item))
            except json.JSONDecodeError as exc:
                raise ValueError(
                    f"could not parse line {line_number} in trajectory log {path}: {exc}"
                ) from exc
        return records


def _unwrap_mapping(payload: Any) -> Mapping[str, Any]:
    if not isinstance(payload, Mapping):
        raise ValueError("expected a JSON object at the top level")
    for key in ("robot_spec", "spec"):
        nested = payload.get(key)
        if isinstance(nested, Mapping):
            return nested
    return payload


def _extract_joint_names(payload: Mapping[str, Any]) -> list[str]:
    names: list[str] = []
    names.extend(_string_list(payload.get("joint_order")))

    joints = payload.get("joints")
    if isinstance(joints, list):
        for joint in joints:
            if not isinstance(joint, Mapping):
                continue
            joint_name = _first_string(joint, "name", "joint_key", "key")
            if joint_name is not None:
                names.append(joint_name)

    legs = payload.get("legs")
    if isinstance(legs, list):
        for index, leg in enumerate(legs, start=1):
            if not isinstance(leg, Mapping):
                continue
            leg_name = _first_string(leg, "name", "leg_key", "key") or f"leg_{index}"
            joint_order = _string_list(leg.get("joint_order"))
            if joint_order:
                names.extend(f"{leg_name}.{joint_name}" for joint_name in joint_order)
                continue
            nested_servo_ids = _mapping(leg.get("servo_ids"))
            if nested_servo_ids:
                for joint_name in ("coxa", "femur", "tibia"):
                    if _first_int(nested_servo_ids, joint_name) is not None:
                        names.append(f"{leg_name}.{joint_name}")
                continue
            for joint_name in ("coxa", "femur", "tibia"):
                if f"{joint_name}_servo_id" in leg:
                    names.append(f"{leg_name}.{joint_name}")

    arm = _mapping(payload.get("arm"))
    if arm:
        joint_order = _string_list(arm.get("joint_order"))
        if joint_order:
            names.extend(f"arm.{joint_name}" for joint_name in joint_order)
        else:
            servos = arm.get("servos")
            if isinstance(servos, list):
                for servo in servos:
                    if not isinstance(servo, Mapping):
                        continue
                    joint_name = _first_string(servo, "joint_key", "name", "key")
                    if joint_name is not None:
                        names.append(f"arm.{joint_name}")

    return names


def _extract_servo_ids(payload: Mapping[str, Any]) -> list[int]:
    servo_ids: list[int] = []

    servos = payload.get("servos")
    if isinstance(servos, list):
        for servo in servos:
            if not isinstance(servo, Mapping):
                continue
            servo_id = _first_int(servo, "servo_id", "id")
            if servo_id is not None:
                servo_ids.append(servo_id)

    legs = payload.get("legs")
    if isinstance(legs, list):
        for leg in legs:
            if not isinstance(leg, Mapping):
                continue
            nested_servo_ids = _mapping(leg.get("servo_ids"))
            if nested_servo_ids:
                for key in ("coxa", "femur", "tibia"):
                    servo_id = _first_int(nested_servo_ids, key)
                    if servo_id is not None:
                        servo_ids.append(servo_id)
            for key in ("coxa_servo_id", "femur_servo_id", "tibia_servo_id"):
                servo_id = _first_int(leg, key)
                if servo_id is not None:
                    servo_ids.append(servo_id)

    arm = _mapping(payload.get("arm"))
    servos = arm.get("servos")
    if isinstance(servos, list):
        for servo in servos:
            if not isinstance(servo, Mapping):
                continue
            servo_id = _first_int(servo, "servo_id", "id")
            if servo_id is not None:
                servo_ids.append(servo_id)

    return servo_ids


def _episode_hint(payload: Any) -> str | None:
    if not isinstance(payload, Mapping):
        return None
    for key in EPISODE_KEYS:
        value = payload.get(key)
        if value is not None:
            return str(value)
    return None


def _looks_like_step(payload: Mapping[str, Any]) -> bool:
    return any(key in payload for key in STEP_KEYS)


def _extend_field_names(field_names: list[str], payload: Mapping[str, Any] | None) -> None:
    if payload is None:
        return
    field_names.extend(str(key) for key in payload.keys())


def _first_mapping(
    payload: Mapping[str, Any], keys: Iterable[str]
) -> Mapping[str, Any] | None:
    for key in keys:
        value = payload.get(key)
        if isinstance(value, Mapping):
            return value
    return None


def _first_string(payload: Mapping[str, Any], *keys: str) -> str | None:
    for key in keys:
        value = payload.get(key)
        if isinstance(value, str) and value:
            return value
    return None


def _first_int(payload: Mapping[str, Any], *keys: str) -> int | None:
    for key in keys:
        value = payload.get(key)
        if isinstance(value, bool):
            continue
        if isinstance(value, int):
            return value
    return None


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, str) and item]


def _mapping(value: Any) -> Mapping[str, Any]:
    if isinstance(value, Mapping):
        return value
    return {}


def _resolve_input_path(path: str | Path, relative_to: Path) -> Path:
    candidate = Path(path).expanduser()
    if candidate.is_absolute():
        return candidate
    relative_candidate = relative_to / candidate
    if relative_candidate.exists() or not candidate.exists():
        return relative_candidate
    return candidate


def _find_first_existing(root: Path, names: Iterable[str]) -> Path | None:
    for name in names:
        candidate = root / name
        if candidate.exists():
            return candidate
    return None


def _dedupe(values: Iterable[Any]) -> list[Any]:
    seen: set[Any] = set()
    result: list[Any] = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        result.append(value)
    return result
