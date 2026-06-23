from dataclasses import dataclass

from arachno_ml.training_data import load_training_dataset


@dataclass
class TrainingConfig:
    dataset_path: str
    output_path: str
    epochs: int = 10
    robot_spec_path: str | None = None


def train_policy(config: TrainingConfig) -> str:
    """Placeholder for policy training.

    The intended flow is:
    1. train or fine-tune in Python
    2. export an artifact such as ONNX
    3. deploy that artifact back into the Rust runtime
    """

    dataset = load_training_dataset(
        config.dataset_path,
        robot_spec_path=config.robot_spec_path,
    )
    robot_summary = "without a robot spec export"
    if dataset.robot_spec is not None:
        robot = dataset.robot_spec
        control_hz = f", control_hz={robot.control_hz}" if robot.control_hz is not None else ""
        robot_summary = (
            f"for robot={robot.name}, joints={len(robot.joint_names)}, "
            f"servos={len(robot.servo_ids)}{control_hz}"
        )

    trajectory = dataset.trajectory_log
    observation_summary = (
        f", observation_fields={len(trajectory.observation_fields)}"
        if trajectory.observation_fields
        else ""
    )
    action_summary = (
        f", action_fields={len(trajectory.action_fields)}"
        if trajectory.action_fields
        else ""
    )
    return (
        f"stub training consumed {trajectory.step_count} steps across "
        f"{trajectory.episode_count} episodes {robot_summary}"
        f"{observation_summary}{action_summary}; "
        f"finished after {config.epochs} epochs; "
        f"export artifact to {config.output_path}"
    )
