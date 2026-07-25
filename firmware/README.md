# Firmware

This workspace contains embedded Rust firmware that complements the Linux-side `arachno-pilot` workspace without being part of the host `cargo check --workspace` flow.

## Current firmware target

- `rp2040-imu-bridge`: USB CDC IMU bridge for the Waveshare `RP2040-ETH` board

The firmware now auto-probes either:

- an `MPU-6050`-class IMU over `I2C`
- an `MPU-9250`-class IMU over `SPI`

and streams:

- accelerometer
- gyroscope
- temperature

The magnetometer is intentionally not used yet.

During bring-up, the current firmware prefers the `SPI` sensor as the primary stream when both buses are populated. If no supported `SPI` device responds, it probes `I2C` addresses `0x68` and `0x69` and accepts:

- `0x68` -> `MPU-6050`
- `0x70` -> `MPU-6500 compatible`
- `0x71` -> `MPU-9250`

If neither `SPI` nor `I2C` is available immediately, it still probes all four `SPI` modes at a conservative clock rate and accepts:

- `0x71` -> `MPU-9250`
- `0x70` -> `MPU-6500 compatible`

That makes the bridge tolerant of both `GY-521 / MPU-6050`-style boards and clone or relabeled `GY-9250` breakouts.

On each USB connection, the firmware now emits a compact device-info frame before live IMU samples and repeats it periodically so the host can verify:

- firmware semantic version
- sensor backend kind
- sample rate
- reported capabilities
- selected SPI mode
- observed `WHO_AM_I`
- backend fault reason if bring-up fails

## Wiring

Recommended parallel-bus wiring for the Waveshare `RP2040-ETH`:

- keep the existing `SPI0` IMU on `GPIO2-5`
- use `GPIO6/7` as an independent primary `I2C1` IMU bus
- use `GPIO8/9` as the independent `I2C0` bus for the INA228 power monitor
- use `GPIO26/27` as `SPI1` for the status LCD

Recommended `SPI` wiring for a `GY-9250 / MPU-9250` breakout that exposes `SPI` pins:

| RP2040-ETH | MPU-9250 breakout | Notes |
| --- | --- | --- |
| `3V3` | `VCC` | Power the module at `3.3 V` |
| `GND` | `GND` | Common ground |
| `GPIO2` | `SCL` / `SCLK` | `SPI0 SCK` |
| `GPIO3` | `SDA` / `SDI` | `SPI0 MOSI` |
| `GPIO4` | `AD0` / `SDO` | `SPI0 MISO` in SPI mode |
| `GPIO5` | `NCS` / `CS` | Chip select, active low |

Recommended `I2C` wiring for an `MPU-6050 / GY-521` style breakout:

| RP2040-ETH | MPU-6050 breakout | Notes |
| --- | --- | --- |
| `3V3` | `VCC` | Power the module at `3.3 V` |
| `GND` | `GND` | Common ground |
| `GPIO6` | `SDA` | `I2C1 SDA` |
| `GPIO7` | `SCL` | `I2C1 SCL` |
| `AD0` | `GND` or `3V3` | Selects `I2C` address `0x68` or `0x69` |

### SPI1 status LCD and INA228 power monitor

The firmware updates a Waveshare **1.83-inch LCD Module Rev2** (240×284, ST7789P) at roughly 4 Hz with the live IMU and INA228
measurements. LCD pixel writes use SPI1 DMA so that display refreshes yield to the 200 Hz IMU
sampling task. The IMU and display continue to run even while no USB host is connected.

| RP2040-ETH | LCD / INA228 pin | Notes |
| --- | --- | --- |
| `3V3` | LCD `VCC`, INA228 `VCC` | Use 3.3 V logic and power |
| `GND` | LCD `GND`, INA228 `GND` | Common ground |
| `GPIO26` | LCD `SCL` / `CLK` | `SPI1 SCK` |
| `GPIO27` | LCD `SDA` / `DIN` | `SPI1 MOSI`; the LCD has no MISO connection |
| `GPIO0` | LCD `CS` | Active low |
| `GPIO1` | LCD `DC` | Data/command selection |
| `GPIO22` | LCD `RST` | Active low |
| `GPIO28` | LCD `BL` / `BLK` | Backlight output, held on by the firmware |
| `GPIO8` | INA228 `SDA` | `I2C0 SDA` |
| `GPIO9` | INA228 `SCL` | `I2C0 SCL` |

The INA228 driver probes all standard addresses from `0x40` through `0x4F`, verifies the TI
manufacturer and INA228 device IDs, then reads bus voltage, shunt voltage, die temperature, and
derives current and power. The default setting matches an **R002 / 2 mΩ** shunt; change
[`INA228_SHUNT_MICRO_OHMS`](rp2040-imu-bridge/src/ina228.rs) if the resistor fitted to your board
is different. The device must have I2C pull-ups to 3.3 V, either on its breakout or added
externally.

The display driver targets the ST7789B 240×280/284 panel family. If text is vertically displaced
on the particular panel revision, adjust `DISPLAY_Y_OFFSET` in
[`display.rs`](rp2040-imu-bridge/src/display.rs).

Reserved `I2C0` bus allocation:

| RP2040-ETH | Use | Notes |
| --- | --- | --- |
| `GPIO8` | `I2C0 SDA` | INA228 power monitor |
| `GPIO9` | `I2C0 SCL` | INA228 power monitor |

Leave these unconnected for the current firmware:

- `EDA`
- `ECL`
- `FSYNC`

Important notes:

- `SPI` breakouts still need `NCS` or `CS` exposed; modules that only break out `I2C` lines will use the `I2C` backend instead.
- The non-overlapping pin assignment means an `SPI` IMU and an `I2C` IMU can now stay wired at the same time without bus conflicts.
- The current USB bridge protocol still exposes one primary IMU stream. INA228 data is shown on
  the local status LCD only for now; it is not part of the USB IMU packet format.
- Even if the board is marketed as `3-5 V compatible`, the safe target for the RP2040 side is still `3.3 V`.
- `GPIO17` to `GPIO21` are already tied into the onboard `CH9120` Ethernet side functions on the RP2040-ETH, so the firmware avoids them.
- In `SPI` mode, `AD0` is used as `SDO/MISO`, not just as an `I2C` address strap.

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

## Verify

After reconnecting the board over USB, check the firmware and bring-up result from the repo root:

```bash
just fw-version
```

Typical healthy output looks like:

```text
device: /dev/serial/by-id/...
firmware_version: 0.1.1
sensor_kind: mpu9250
sample_hz: 200
capabilities: accel,gyro,temp
spi_mode: 3
observed_who_am_i: 0x71
```

For an `MPU-6050`, a healthy output looks like:

```text
device: /dev/serial/by-id/...
firmware_version: 0.1.1
sensor_kind: mpu6050
sample_hz: 200
capabilities: accel,gyro,temp
observed_who_am_i: 0x68
```

If the board is alive but the IMU still is not, `fw-version` now prints the fault detail too, for example:

```text
sensor_kind: faulted
backend_fault: unexpected_who_am_i
spi_mode: 1
observed_who_am_i: 0x73
```

## Next firmware step

After basic bring-up with the `MPU-9250`, the best follow-up is:

- add startup bias calibration for gyro zero-rate offset
- optionally move to data-ready interrupt handling on `INT`
- optionally swap in an `ICM-42688-P` later while keeping the same USB packet format
