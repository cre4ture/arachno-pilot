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
