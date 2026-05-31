from dataclasses import dataclass


@dataclass
class TrainingConfig:
    dataset_path: str
    output_path: str
    epochs: int = 10


def train_policy(config: TrainingConfig) -> str:
    """Placeholder for policy training.

    The intended flow is:
    1. train or fine-tune in Python
    2. export an artifact such as ONNX
    3. deploy that artifact back into the Rust runtime
    """

    return (
        f"stub training finished after {config.epochs} epochs; "
        f"export artifact to {config.output_path}"
    )
