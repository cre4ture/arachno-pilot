from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any


@dataclass
class RateLimitWindow:
    used_percent: float
    window_duration_mins: int | None
    resets_at: int | None

    @property
    def remaining_percent(self) -> float:
        return max(0.0, 100.0 - self.used_percent)

    def to_dict(self) -> dict[str, Any]:
        return {
            "used_percent": self.used_percent,
            "remaining_percent": self.remaining_percent,
            "window_duration_mins": self.window_duration_mins,
            "resets_at": self.resets_at,
            "resets_at_iso": format_timestamp(self.resets_at),
        }


def format_timestamp(timestamp: int | None) -> str | None:
    if timestamp is None:
        return None
    return datetime.fromtimestamp(timestamp).astimezone().isoformat(timespec="seconds")


def format_window_duration(window_duration_mins: int | None) -> str:
    if window_duration_mins is None:
        return "unknown"
    if window_duration_mins % (60 * 24 * 7) == 0:
        weeks = window_duration_mins // (60 * 24 * 7)
        return f"{weeks}w"
    if window_duration_mins % (60 * 24) == 0:
        days = window_duration_mins // (60 * 24)
        return f"{days}d"
    if window_duration_mins % 60 == 0:
        hours = window_duration_mins // 60
        return f"{hours}h"
    return f"{window_duration_mins}m"


def render_window(label: str, window: RateLimitWindow) -> list[str]:
    return [
        (
            f"{label}: "
            f"used={window.used_percent:.1f}%, "
            f"remaining={window.remaining_percent:.1f}%, "
            f"window={format_window_duration(window.window_duration_mins)}, "
            f"resets_at={format_timestamp(window.resets_at) or 'unknown'}"
        )
    ]
