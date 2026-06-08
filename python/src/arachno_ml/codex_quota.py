from __future__ import annotations

import argparse
import json
import selectors
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Any

from arachno_ml.quota_common import RateLimitWindow, render_window


REQUEST_TIMEOUT_SECONDS = 15.0


@dataclass
class CreditsSnapshot:
    has_credits: bool
    unlimited: bool
    balance: str | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "has_credits": self.has_credits,
            "unlimited": self.unlimited,
            "balance": self.balance,
        }


@dataclass
class CodexQuotaSnapshot:
    plan_type: str | None
    limit_id: str | None
    limit_name: str | None
    primary: RateLimitWindow | None
    secondary: RateLimitWindow | None
    credits: CreditsSnapshot | None
    rate_limit_reached_type: str | None
    raw_rate_limits: dict[str, Any]
    raw_account: dict[str, Any]

    def to_dict(self, include_raw: bool = False) -> dict[str, Any]:
        data = {
            "plan_type": self.plan_type,
            "limit_id": self.limit_id,
            "limit_name": self.limit_name,
            "primary": self.primary.to_dict() if self.primary else None,
            "secondary": self.secondary.to_dict() if self.secondary else None,
            "credits": self.credits.to_dict() if self.credits else None,
            "rate_limit_reached_type": self.rate_limit_reached_type,
        }
        if include_raw:
            data["raw_rate_limits"] = self.raw_rate_limits
            data["raw_account"] = self.raw_account
        return data


class CodexQuotaError(RuntimeError):
    pass


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fetch the current Codex subscription rate-limit status."
    )
    parser.add_argument(
        "--codex-bin",
        default=shutil.which("codex"),
        help="Path to the codex binary. Defaults to the first codex in PATH.",
    )
    parser.add_argument(
        "--limit-id",
        default="codex",
        help="Metered limit bucket to read from rateLimitsByLimitId.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit the parsed quota snapshot as JSON.",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="Include raw app-server payloads in human-readable output.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=REQUEST_TIMEOUT_SECONDS,
        help="Timeout in seconds for the app-server exchange.",
    )
    return parser.parse_args(argv)


def _send_request(
    process: subprocess.Popen[str],
    request_id: int,
    method: str,
    params: Any | None = None,
) -> None:
    request: dict[str, Any] = {"id": request_id, "method": method}
    if params is not None:
        request["params"] = params
    assert process.stdin is not None
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()


def _read_response(
    process: subprocess.Popen[str],
    request_id: int,
    timeout_seconds: float,
) -> Any:
    assert process.stdout is not None
    assert process.stderr is not None
    deadline = time.monotonic() + timeout_seconds
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    selector.register(process.stderr, selectors.EVENT_READ)
    stderr_lines: list[str] = []
    try:
        while time.monotonic() < deadline:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break

            events = selector.select(timeout=remaining)
            if not events:
                continue

            for key, _ in events:
                line = key.fileobj.readline()
                if not line:
                    continue
                if key.fileobj is process.stderr:
                    stderr_lines.append(line.rstrip("\n"))
                    continue

                message = json.loads(line)
                if message.get("id") != request_id:
                    continue
                if "error" in message:
                    raise CodexQuotaError(
                        f"codex app-server returned an error for {request_id}: {message['error']}"
                    )
                if "result" not in message:
                    raise CodexQuotaError(
                        "codex app-server returned a malformed response "
                        f"for {request_id}: {message}"
                    )
                return message["result"]
    finally:
        selector.close()

    raise CodexQuotaError(
        f"timed out waiting for codex app-server response to request {request_id}"
        + (f"; stderr: {' | '.join(stderr_lines)}" if stderr_lines else "")
    )


def fetch_quota_snapshot(
    codex_bin: str | None,
    limit_id: str,
    timeout_seconds: float = REQUEST_TIMEOUT_SECONDS,
) -> CodexQuotaSnapshot:
    if not codex_bin:
        raise CodexQuotaError("could not find `codex` in PATH")

    process = subprocess.Popen(
        [codex_bin, "app-server", "--stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )

    try:
        _send_request(
            process,
            1,
            "initialize",
            {
                "clientInfo": {
                    "name": "arachno-codex-quota",
                    "version": "0.1.0",
                },
                "capabilities": None,
            },
        )
        _read_response(process, 1, timeout_seconds)

        _send_request(process, 2, "account/read", {"refreshToken": True})
        account_result = _read_response(process, 2, timeout_seconds)

        _send_request(process, 3, "account/rateLimits/read")
        rate_limits_result = _read_response(process, 3, timeout_seconds)
    finally:
        if process.stdin is not None and not process.stdin.closed:
            process.stdin.close()
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3)

    limit_map = rate_limits_result.get("rateLimitsByLimitId") or {}
    snapshot = limit_map.get(limit_id) or rate_limits_result.get("rateLimits")
    if not snapshot:
        raise CodexQuotaError(
            f"codex did not return a rate-limit snapshot for limit_id={limit_id!r}"
        )

    return CodexQuotaSnapshot(
        plan_type=(
            account_result.get("account", {}).get("planType")
            or snapshot.get("planType")
        ),
        limit_id=snapshot.get("limitId"),
        limit_name=snapshot.get("limitName"),
        primary=parse_window(snapshot.get("primary")),
        secondary=parse_window(snapshot.get("secondary")),
        credits=parse_credits(snapshot.get("credits")),
        rate_limit_reached_type=snapshot.get("rateLimitReachedType"),
        raw_rate_limits=rate_limits_result,
        raw_account=account_result,
    )


def parse_window(raw_window: dict[str, Any] | None) -> RateLimitWindow | None:
    if raw_window is None:
        return None
    return RateLimitWindow(
        used_percent=float(raw_window["usedPercent"]),
        window_duration_mins=raw_window.get("windowDurationMins"),
        resets_at=raw_window.get("resetsAt"),
    )


def parse_credits(raw_credits: dict[str, Any] | None) -> CreditsSnapshot | None:
    if raw_credits is None:
        return None
    return CreditsSnapshot(
        has_credits=bool(raw_credits["hasCredits"]),
        unlimited=bool(raw_credits["unlimited"]),
        balance=raw_credits.get("balance"),
    )


def render_human(snapshot: CodexQuotaSnapshot, include_raw: bool = False) -> str:
    lines = [
        f"plan: {snapshot.plan_type or 'unknown'}",
        f"limit: {snapshot.limit_id or 'unknown'}",
    ]
    if snapshot.limit_name:
        lines.append(f"limit_name: {snapshot.limit_name}")

    if snapshot.primary:
        lines.extend(render_window("primary", snapshot.primary))
    if snapshot.secondary:
        lines.extend(render_window("secondary", snapshot.secondary))

    if snapshot.credits:
        lines.append(
            "credits: "
            f"has_credits={'yes' if snapshot.credits.has_credits else 'no'}, "
            f"unlimited={'yes' if snapshot.credits.unlimited else 'no'}, "
            f"balance={snapshot.credits.balance or 'n/a'}"
        )

    if snapshot.rate_limit_reached_type:
        lines.append(f"rate_limit_reached_type: {snapshot.rate_limit_reached_type}")

    if include_raw:
        lines.append("raw_account:")
        lines.append(json.dumps(snapshot.raw_account, indent=2, sort_keys=True))
        lines.append("raw_rate_limits:")
        lines.append(json.dumps(snapshot.raw_rate_limits, indent=2, sort_keys=True))

    return "\n".join(lines)
def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        snapshot = fetch_quota_snapshot(
            codex_bin=args.codex_bin,
            limit_id=args.limit_id,
            timeout_seconds=args.timeout,
        )
    except (CodexQuotaError, OSError, json.JSONDecodeError) as exc:
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
