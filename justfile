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
    cargo run -p arachno-brain -- --config config/robot/default.toml

calibrate:
    cargo run -p arachno-calibrate -- --config config/robot/default.toml

probe:
    cargo run -p arachno-probe -- --config config/robot/default.toml

dashboard:
    cargo run -p arachno-dashboard -- --config config/robot/host-usb.toml --listen 127.0.0.1:3000
