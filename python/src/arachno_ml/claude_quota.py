from __future__ import annotations

import argparse
import fcntl
import json
import os
import pty
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from arachno_ml.quota_common import RateLimitWindow, render_window


REQUEST_TIMEOUT_SECONDS = 15.0
STATUSLINE_REFRESH_INTERVAL_SECONDS = 1
WINDOW_DURATION_MINS_BY_KEY = {
    "five_hour": 5 * 60,
    "seven_day": 7 * 24 * 60,
    "seven_day_opus": 7 * 24 * 60,
    "seven_day_sonnet": 7 * 24 * 60,
}


@dataclass
class ClaudeQuotaSnapshot:
    subscription_type: str | None
    email: str | None
    org_name: str | None
    auth_method: str | None
    rate_limits: dict[str, RateLimitWindow]
    raw_auth_status: dict[str, Any]
    raw_statusline: dict[str, Any]

    def to_dict(self, include_raw: bool = False) -> dict[str, Any]:
        data = {
            "subscription_type": self.subscription_type,
            "email": self.email,
            "org_name": self.org_name,
            "auth_method": self.auth_method,
            "rate_limits": {
                label: window.to_dict()
                for label, window in ordered_rate_limits(self.rate_limits)
            },
        }
        if include_raw:
            data["raw_auth_status"] = self.raw_auth_status
            data["raw_statusline"] = self.raw_statusline
        return data


class ClaudeQuotaError(RuntimeError):
    pass


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fetch the current Claude Code subscription rate-limit status."
    )
    parser.add_argument(
        "--claude-bin",
        default=shutil.which("claude"),
        help="Path to the claude binary. Defaults to the first claude in PATH.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit the parsed quota snapshot as JSON.",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="Include raw CLI payloads in human-readable output.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=REQUEST_TIMEOUT_SECONDS,
        help="Timeout in seconds for the Claude CLI probe.",
    )
    return parser.parse_args(argv)


def fetch_quota_snapshot(
    claude_bin: str | None,
    timeout_seconds: float = REQUEST_TIMEOUT_SECONDS,
) -> ClaudeQuotaSnapshot:
    if not claude_bin:
        raise ClaudeQuotaError("could not find `claude` in PATH")

    auth_status = fetch_auth_status(claude_bin, timeout_seconds)
    statusline_snapshot = fetch_statusline_snapshot(claude_bin, timeout_seconds)
    raw_rate_limits = statusline_snapshot.get("rate_limits")
    if not isinstance(raw_rate_limits, dict) or not raw_rate_limits:
        raise ClaudeQuotaError("Claude did not provide a statusline rate_limits snapshot")

    rate_limits = {
        label: parse_window(label, raw_window)
        for label, raw_window in raw_rate_limits.items()
        if isinstance(raw_window, dict)
    }
    if not rate_limits:
        raise ClaudeQuotaError("Claude returned an empty rate_limits snapshot")

    return ClaudeQuotaSnapshot(
        subscription_type=auth_status.get("subscriptionType"),
        email=auth_status.get("email"),
        org_name=auth_status.get("orgName"),
        auth_method=auth_status.get("authMethod"),
        rate_limits=rate_limits,
        raw_auth_status=auth_status,
        raw_statusline=statusline_snapshot,
    )


def fetch_auth_status(claude_bin: str, timeout_seconds: float) -> dict[str, Any]:
    try:
        result = subprocess.run(
            [claude_bin, "auth", "status"],
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as exc:
        raise ClaudeQuotaError("timed out waiting for `claude auth status`") from exc

    if result.returncode != 0:
        stderr = result.stderr.strip()
        raise ClaudeQuotaError(
            "`claude auth status` failed"
            + (f": {stderr}" if stderr else f" with exit code {result.returncode}")
        )

    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise ClaudeQuotaError(
            "`claude auth status` did not return valid JSON"
        ) from exc

    if not isinstance(payload, dict):
        raise ClaudeQuotaError("`claude auth status` returned an unexpected payload")
    return payload


def fetch_statusline_snapshot(claude_bin: str, timeout_seconds: float) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="arachno-claude-quota-") as temp_dir:
        snapshot_log = Path(temp_dir) / "statusline.jsonl"
        settings_json = json.dumps(
            {
                "statusLine": {
                    "type": "command",
                    "command": build_statusline_command(snapshot_log),
                    "refreshInterval": STATUSLINE_REFRESH_INTERVAL_SECONDS,
                }
            }
        )

        master_fd, slave_fd = pty.openpty()
        process: subprocess.Popen[bytes] | None = None
        output_chunks: list[bytes] = []
        try:
            flags = fcntl.fcntl(master_fd, fcntl.F_GETFL)
            fcntl.fcntl(master_fd, fcntl.F_SETFL, flags | os.O_NONBLOCK)
            process = subprocess.Popen(
                [claude_bin, "--settings", settings_json],
                stdin=slave_fd,
                stdout=slave_fd,
                stderr=slave_fd,
                close_fds=True,
                start_new_session=True,
                env={**os.environ, "TERM": os.environ.get("TERM", "xterm-256color")},
            )
        finally:
            os.close(slave_fd)

        deadline = time.monotonic() + timeout_seconds
        try:
            while time.monotonic() < deadline:
                output_chunks.extend(drain_pty(master_fd))
                snapshot = read_latest_statusline_snapshot(snapshot_log)
                if snapshot and isinstance(snapshot.get("rate_limits"), dict):
                    return snapshot

                if process.poll() is not None:
                    break

                time.sleep(0.2)
        finally:
            if process is not None:
                terminate_process_group(process)
            output_chunks.extend(drain_pty(master_fd))
            os.close(master_fd)

    output_excerpt = b"".join(output_chunks).decode("utf-8", "ignore")
    if process is not None and process.returncode not in (None, 0, -signal.SIGTERM):
        raise ClaudeQuotaError(
            "Claude exited before exposing statusline rate limits"
            + (f": {output_excerpt[-500:]}" if output_excerpt else "")
        )
    raise ClaudeQuotaError(
        "timed out waiting for Claude statusline rate limits"
        + (f": {output_excerpt[-500:]}" if output_excerpt else "")
    )


def build_statusline_command(snapshot_log: Path) -> str:
    python_code = (
        "import pathlib, sys; "
        f"path = pathlib.Path({str(snapshot_log)!r}); "
        "payload = sys.stdin.read().strip(); "
        "path.parent.mkdir(parents=True, exist_ok=True); "
        "path.open('a', encoding='utf-8').write(payload + '\\n' if payload else '')"
    )
    return f"python3 -c {shlex.quote(python_code)}"


def drain_pty(master_fd: int) -> list[bytes]:
    chunks: list[bytes] = []
    while True:
        try:
            chunk = os.read(master_fd, 65536)
        except BlockingIOError:
            return chunks
        except OSError:
            return chunks

        if not chunk:
            return chunks
        chunks.append(chunk)


def read_latest_statusline_snapshot(snapshot_log: Path) -> dict[str, Any] | None:
    if not snapshot_log.exists():
        return None

    latest_snapshot: dict[str, Any] | None = None
    for line in snapshot_log.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            latest_snapshot = payload

    return latest_snapshot


def terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return

    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=3)


def parse_window(label: str, raw_window: dict[str, Any]) -> RateLimitWindow:
    return RateLimitWindow(
        used_percent=float(raw_window["used_percentage"]),
        window_duration_mins=WINDOW_DURATION_MINS_BY_KEY.get(label),
        resets_at=raw_window.get("resets_at"),
    )


def render_human(snapshot: ClaudeQuotaSnapshot, include_raw: bool = False) -> str:
    lines = [
        f"plan: {snapshot.subscription_type or 'unknown'}",
        f"organization: {snapshot.org_name or 'unknown'}",
    ]
    if snapshot.email:
        lines.append(f"email: {snapshot.email}")
    if snapshot.auth_method:
        lines.append(f"auth_method: {snapshot.auth_method}")

    for label, window in ordered_rate_limits(snapshot.rate_limits):
        lines.extend(render_window(label, window))

    if include_raw:
        lines.append("raw_auth_status:")
        lines.append(json.dumps(snapshot.raw_auth_status, indent=2, sort_keys=True))
        lines.append("raw_statusline:")
        lines.append(json.dumps(snapshot.raw_statusline, indent=2, sort_keys=True))

    return "\n".join(lines)


def ordered_rate_limits(
    rate_limits: dict[str, RateLimitWindow],
) -> list[tuple[str, RateLimitWindow]]:
    preferred_order = [
        "five_hour",
        "seven_day",
        "seven_day_opus",
        "seven_day_sonnet",
    ]
    known_labels = [label for label in preferred_order if label in rate_limits]
    extra_labels = sorted(label for label in rate_limits if label not in preferred_order)
    ordered_labels = known_labels + extra_labels
    return [(label, rate_limits[label]) for label in ordered_labels]


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        snapshot = fetch_quota_snapshot(
            claude_bin=args.claude_bin,
            timeout_seconds=args.timeout,
        )
    except (ClaudeQuotaError, OSError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.json:
        print(
            json.dumps(
                snapshot.to_dict(include_raw=args.raw),
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(render_human(snapshot, include_raw=args.raw))

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
