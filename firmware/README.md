# Firmware

This workspace contains embedded Rust firmware that complements the Linux-side `arachno-pilot` workspace without being part of the host `cargo check --workspace` flow.

## Current firmware target

- `rp2040-imu-bridge`: USB CDC IMU bridge for the Waveshare `RP2040-ETH` board

The current scaffold intentionally starts with a `mock IMU stream` so the USB transport, packet format, and host integration can be validated before a concrete `MPU-9250` or `ICM-42688-P` driver is wired in.

## Build

Prerequisites:

- `rustup target add thumbv6m-none-eabi`
- `cargo install --locked elf2uf2-rs`

From the repo root:

```bash
cargo check --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi
cargo build --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --target thumbv6m-none-eabi
cargo build --manifest-path firmware/Cargo.toml -p rp2040-imu-bridge --release --target thumbv6m-none-eabi
```

From inside `firmware/`, the local `.cargo/config.toml` already selects `thumbv6m-none-eabi`, so this also works:

```bash
cargo build -p rp2040-imu-bridge
cargo build -p rp2040-imu-bridge --release
```

The important detail is that the RP2040 linker flags live in the repo-level `.cargo/config.toml`, so builds from the repo root and builds from inside `firmware/` use the same linker setup, including `defmt.x`, `link.x`, and `link-rp.x`.

## UF2 conversion

Convert the release ELF into a BOOTSEL-flashable UF2:

```bash
elf2uf2-rs firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge \
  firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge.uf2
```

Equivalent repo helper:

```bash
just firmware-uf2
```

This produces:

- ELF: `firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge`
- UF2: `firmware/target/thumbv6m-none-eabi/release/rp2040-imu-bridge.uf2`

## Flash

Recommended options:

- `BOOTSEL + UF2`: build, convert to `uf2`, and copy to the RP2040 mass-storage device
- `probe-rs`: use an SWD probe for faster flash/debug cycles

For `BOOTSEL + UF2`, hold `BOOT`, plug in the board, then copy the `.uf2` file onto the mounted `RPI-RP2` drive.

## Next wiring step

Replace `MockImuSensor` in `rp2040-imu-bridge/src/main.rs` with a real sensor backend:

- `MPU-9250` over `SPI` for immediate bring-up with the hardware you already own
- `ICM-42688-P` over `SPI` for the longer-term robot build

Keep the USB packet format stable while swapping the sensor backend underneath it.
