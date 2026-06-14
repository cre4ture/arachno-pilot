set shell := ["bash", "-cu"]

default:
    @just --list

check:
    cargo check

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

brain:
    cargo run -p arachno-brain -- --config config/robot/default.toml --listen 127.0.0.1:4000

manual:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode manual --dashboard

lay-down:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode lay-down --dashboard

stand-up:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand-up --dashboard

stand-up-high:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand-up-high --dashboard

stand:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand --dashboard

stand-high:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode stand-high --dashboard

slow-walk:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode slow-walk --walk-seconds 8 --dashboard

backward-walk:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode backward-walk --walk-seconds 8 --dashboard

rotate-left:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode rotate-left --walk-seconds 8 --dashboard

rotate-right:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --mode rotate-right --walk-seconds 8 --dashboard

calibrate:
    cargo run -p arachno-calibrate -- --config config/robot/default.toml

apply-eeprom:
    cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode apply-eeprom

verify-eeprom:
    cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode verify-eeprom

sense-ranges:
    cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode sense-ranges --output config/robot/servo-ranges.toml

check-poses:
    cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode check-poses --ranges config/robot/servo-ranges.toml

suggest-poses:
    cargo run -p arachno-calibrate -- --config config/robot/host-usb.toml --mode suggest-poses --ranges config/robot/servo-ranges.toml --suggestions-output /tmp/servo-pose-suggestions.toml

probe:
    cargo run -p arachno-probe -- --config config/robot/default.toml

dashboard:
    cargo run -p arachno-brain -- --config config/robot/host-usb.toml --listen 127.0.0.1:4000 --dashboard

codex-quota:
    cd python && PYTHONPATH=src python3 -m arachno_ml.codex_quota

claude-quota:
    cd python && PYTHONPATH=src python3 -m arachno_ml.claude_quota

fw-version:
    cargo run -p arachno-fw-info -- --config config/robot/default.toml

firmware-check:
    cargo check --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi

firmware-build:
    cargo build --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi

firmware-build-release:
    cargo build --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --release --target thumbv6m-none-eabi

firmware-uf2:
    cargo build --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --release --target thumbv6m-none-eabi
    elf2uf2-rs firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge.uf2
