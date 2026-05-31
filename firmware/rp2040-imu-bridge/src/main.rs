#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::mem::MaybeUninit;

use arachno_imu_proto::{
    CAP_ACCEL, CAP_GYRO, CAP_TEMP, DeviceInfo, ImuSample, MAX_FRAME_LEN, SPI_MODE_UNKNOWN,
    SENSOR_FAULT_NONE, SENSOR_FAULT_PROBE_NO_RESPONSE, SENSOR_FAULT_READ,
    SENSOR_FAULT_UNEXPECTED_WHO_AM_I, SensorKind, encode_device_info_frame,
    encode_sample_frame,
};
use defmt::{info, panic, unwrap, warn};
use embassy_executor::Spawner;
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{PIN_2, PIN_3, PIN_4, PIN_5, SPI0, USB};
use embassy_rp::spi::{self, Blocking, Spi};
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use {defmt_rtt as _, panic_probe as _};

const SAMPLE_HZ: u32 = 200;
const SAMPLE_PERIOD_MS: u64 = 1_000 / SAMPLE_HZ as u64;
const DEVICE_INFO_ANNOUNCE_INTERVAL_SAMPLES: u32 = SAMPLE_HZ / 4;
const SENSOR_WARMUP_SAMPLES: u32 = SAMPLE_HZ;
const SENSOR_STATUS_FAULT: u16 = 0x0001;
const SENSOR_STATUS_ACCEL_CLIPPED: u16 = 0x0002;
const SENSOR_STATUS_GYRO_CLIPPED: u16 = 0x0004;
const SENSOR_STATUS_CALIBRATING: u16 = 0x0020;

const MPU9250_SPI_HZ: u32 = 1_000_000;
const MPU9250_SPI_PROBE_HZ: u32 = 125_000;
const MPU_WHO_AM_I_MPU6500: u8 = 0x70;
const MPU_WHO_AM_I_MPU9250: u8 = 0x71;
const MPU9250_REG_SMPLRT_DIV: u8 = 0x19;
const MPU9250_REG_CONFIG: u8 = 0x1A;
const MPU9250_REG_GYRO_CONFIG: u8 = 0x1B;
const MPU9250_REG_ACCEL_CONFIG: u8 = 0x1C;
const MPU9250_REG_ACCEL_CONFIG2: u8 = 0x1D;
const MPU9250_REG_ACCEL_XOUT_H: u8 = 0x3B;
const MPU9250_REG_SIGNAL_PATH_RESET: u8 = 0x68;
const MPU9250_REG_USER_CTRL: u8 = 0x6A;
const MPU9250_REG_PWR_MGMT_1: u8 = 0x6B;
const MPU9250_REG_PWR_MGMT_2: u8 = 0x6C;
const MPU9250_REG_WHO_AM_I: u8 = 0x75;

const MPU9250_USER_CTRL_I2C_IF_DIS: u8 = 1 << 4;
const MPU9250_PWR_MGMT_1_H_RESET: u8 = 1 << 7;
const MPU9250_PWR_MGMT_1_CLKSEL_AUTO: u8 = 0x01;
const MPU9250_SIGNAL_PATH_RESET_ALL: u8 = 0x07;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("starting arachno RP2040 IMU bridge");

    let p = embassy_rp::init(Default::default());
    let driver = Driver::new(p.USB, Irqs);

    let mut config = embassy_usb::Config::new(0xC0DE, 0xCAFE);
    config.manufacturer = Some("arachno-pilot");
    config.product = Some("RP2040 IMU bridge");
    config.serial_number = Some("arachno-imu-bridge-0001");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static mut CONFIG_DESCRIPTOR: [u8; 256] = [0; 256];
    static mut BOS_DESCRIPTOR: [u8; 256] = [0; 256];
    static mut CONTROL_BUF: [u8; 64] = [0; 64];
    static mut CDC_STATE: MaybeUninit<State<'static>> = MaybeUninit::uninit();

    let mut builder = unsafe {
        embassy_usb::Builder::new(
            driver,
            config,
            &mut CONFIG_DESCRIPTOR,
            &mut BOS_DESCRIPTOR,
            &mut [],
            &mut CONTROL_BUF,
        )
    };

    let mut class = unsafe {
        let state = CDC_STATE.write(State::new());
        CdcAcmClass::new(&mut builder, state, 64)
    };
    let usb = builder.build();

    spawner.spawn(unwrap!(usb_task(usb)));

    // Keep the IMU on SPI0 away from the board's CH9120 control pins.
    // Wiring:
    // GP2 -> MPU-9250 SCL/SCLK
    // GP3 -> MPU-9250 SDA/SDI
    // GP4 -> MPU-9250 AD0/SDO
    // GP5 -> MPU-9250 NCS/CS
    let mut sensor = SensorState::new(p.SPI0, p.PIN_2, p.PIN_3, p.PIN_4, p.PIN_5).await;
    let mut sequence = 0u8;
    let mut frame_buf = [0u8; MAX_FRAME_LEN];

    loop {
        class.wait_connection().await;
        info!("USB host connected");

        let _ = stream_samples(&mut class, &mut sensor, &mut sequence, &mut frame_buf).await;
        info!("USB host disconnected");
    }
}

type MyUsbDriver = Driver<'static, USB>;
type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

#[embassy_executor::task]
async fn usb_task(mut usb: MyUsbDevice) -> ! {
    usb.run().await
}

async fn stream_samples<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    sensor: &mut SensorState<'d>,
    sequence: &mut u8,
    frame_buf: &mut [u8; MAX_FRAME_LEN],
) -> Result<(), Disconnected> {
    let mut ticker = Ticker::every(Duration::from_millis(SAMPLE_PERIOD_MS));
    let mut samples_until_info = 0u32;

    send_device_info(class, sensor.device_info(), sequence, frame_buf).await?;

    loop {
        if samples_until_info == 0 {
            send_device_info(class, sensor.device_info(), sequence, frame_buf).await?;
            samples_until_info = DEVICE_INFO_ANNOUNCE_INTERVAL_SAMPLES;
        }

        let sample = sensor.next_sample();
        let frame_len = encode_sample_frame(*sequence, &sample, frame_buf)
            .expect("IMU frame buffer is statically sized for the protocol");

        class.write_packet(&frame_buf[..frame_len]).await?;
        *sequence = sequence.wrapping_add(1);
        samples_until_info = samples_until_info.saturating_sub(1);
        ticker.next().await;
    }
}

async fn send_device_info<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    device_info: DeviceInfo,
    sequence: &mut u8,
    frame_buf: &mut [u8; MAX_FRAME_LEN],
) -> Result<(), Disconnected> {
    let info_len = encode_device_info_frame(*sequence, &device_info, frame_buf)
        .expect("device info frame fits in the shared protocol buffer");
    class.write_packet(&frame_buf[..info_len]).await?;
    *sequence = sequence.wrapping_add(1);
    Ok(())
}

enum SensorState<'d> {
    Real(Mpu9250Sensor<'d>),
    Faulted(FaultedSensor),
}

impl<'d> SensorState<'d> {
    async fn new(
        spi0: Peri<'d, SPI0>,
        sck: Peri<'d, PIN_2>,
        mosi: Peri<'d, PIN_3>,
        miso: Peri<'d, PIN_4>,
        cs: Peri<'d, PIN_5>,
    ) -> Self {
        let mut config = spi::Config::default();
        config.frequency = MPU9250_SPI_HZ;

        let spi = Spi::new_blocking(spi0, sck, mosi, miso, config);
        let cs = Output::new(cs, Level::High);

        match Mpu9250Sensor::new(spi, cs).await {
            Ok(sensor) => Self::Real(sensor),
            Err(err) => {
                warn!("mpu9250 init failed: {:?}", err);
                Self::Faulted(FaultedSensor::new(err.fault_info()))
            }
        }
    }

    fn next_sample(&mut self) -> ImuSample {
        match self {
            Self::Real(sensor) => match sensor.next_sample() {
                Ok(sample) => sample,
                Err(err) => {
                    warn!("mpu9250 read failed: {:?}", err);
                    fault_sample(SENSOR_STATUS_FAULT)
                }
            },
            Self::Faulted(sensor) => sensor.next_sample(),
        }
    }

    fn device_info(&self) -> DeviceInfo {
        DeviceInfo {
            firmware_version: firmware_version(),
            sensor_kind: match self {
                Self::Real(sensor) => sensor.sensor_kind(),
                Self::Faulted(_) => SensorKind::Faulted,
            },
            sample_hz: SAMPLE_HZ as u16,
            capabilities: CAP_ACCEL | CAP_GYRO | CAP_TEMP,
            fault_code: match self {
                Self::Real(_) => SENSOR_FAULT_NONE,
                Self::Faulted(sensor) => sensor.fault_info.code,
            },
            observed_who_am_i: match self {
                Self::Real(sensor) => sensor.observed_who_am_i(),
                Self::Faulted(sensor) => sensor.fault_info.observed_who_am_i,
            },
            spi_mode: match self {
                Self::Real(sensor) => sensor.spi_mode(),
                Self::Faulted(sensor) => sensor.fault_info.spi_mode,
            },
            reserved: 0,
        }
    }
}

struct Mpu9250Sensor<'d> {
    spi: Spi<'d, SPI0, Blocking>,
    cs: Output<'d>,
    warmup_remaining: u32,
    sensor_kind: SensorKind,
    spi_mode: u8,
}

impl<'d> Mpu9250Sensor<'d> {
    async fn new(spi: Spi<'d, SPI0, Blocking>, cs: Output<'d>) -> Result<Self, Mpu9250Error> {
        let mut sensor = Self {
            spi,
            cs,
            warmup_remaining: SENSOR_WARMUP_SAMPLES,
            sensor_kind: SensorKind::Unknown,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        // Give the sensor time to finish power-up before probing SPI modes and chip IDs.
        Timer::after(Duration::from_millis(100)).await;
        let probe = sensor.probe_transport().await?;
        sensor.sensor_kind = probe.sensor_kind;
        sensor.spi_mode = probe.spi_mode;
        sensor.apply_spi_mode(probe.spi_mode, MPU9250_SPI_HZ);

        sensor.write_register(MPU9250_REG_USER_CTRL, MPU9250_USER_CTRL_I2C_IF_DIS)?;
        sensor.write_register(MPU9250_REG_PWR_MGMT_1, MPU9250_PWR_MGMT_1_H_RESET)?;
        Timer::after(Duration::from_millis(100)).await;

        sensor.write_register(MPU9250_REG_USER_CTRL, MPU9250_USER_CTRL_I2C_IF_DIS)?;
        sensor.write_register(MPU9250_REG_SIGNAL_PATH_RESET, MPU9250_SIGNAL_PATH_RESET_ALL)?;
        Timer::after(Duration::from_millis(10)).await;

        sensor.write_register(MPU9250_REG_PWR_MGMT_1, MPU9250_PWR_MGMT_1_CLKSEL_AUTO)?;
        sensor.write_register(MPU9250_REG_PWR_MGMT_2, 0x00)?;
        sensor.write_register(MPU9250_REG_CONFIG, 0x03)?;
        sensor.write_register(MPU9250_REG_SMPLRT_DIV, 0x04)?;
        sensor.write_register(MPU9250_REG_GYRO_CONFIG, 0x00)?;
        sensor.write_register(MPU9250_REG_ACCEL_CONFIG, 0x00)?;
        sensor.write_register(MPU9250_REG_ACCEL_CONFIG2, 0x03)?;
        Timer::after(Duration::from_millis(20)).await;

        let who_am_i = sensor.read_u8(MPU9250_REG_WHO_AM_I)?;
        if who_am_i != probe.who_am_i {
            return Err(Mpu9250Error::Probe(FaultInfo {
                code: if who_am_i == 0x00 || who_am_i == 0xFF {
                    SENSOR_FAULT_PROBE_NO_RESPONSE
                } else {
                    SENSOR_FAULT_UNEXPECTED_WHO_AM_I
                },
                observed_who_am_i: who_am_i,
                spi_mode: probe.spi_mode,
            }));
        }

        info!(
            "imu online over SPI mode {=u8}, who_am_i={=u8}, kind={=u8}",
            probe.spi_mode,
            who_am_i,
            probe.sensor_kind as u8
        );
        Ok(sensor)
    }

    async fn probe_transport(&mut self) -> Result<ProbeResult, Mpu9250Error> {
        let mut best_fault = FaultInfo {
            code: SENSOR_FAULT_PROBE_NO_RESPONSE,
            observed_who_am_i: 0,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        for spi_mode in 0..=3u8 {
            self.apply_spi_mode(spi_mode, MPU9250_SPI_PROBE_HZ);
            Timer::after(Duration::from_millis(2)).await;

            let who_am_i = self
                .read_u8(MPU9250_REG_WHO_AM_I)
                .map_err(|_| Mpu9250Error::Probe(FaultInfo {
                    code: SENSOR_FAULT_READ,
                    observed_who_am_i: 0,
                    spi_mode,
                }))?;

            if let Some(sensor_kind) = sensor_kind_from_who_am_i(who_am_i) {
                return Ok(ProbeResult {
                    sensor_kind,
                    who_am_i,
                    spi_mode,
                });
            }

            if who_am_i != 0x00 && who_am_i != 0xFF {
                best_fault = FaultInfo {
                    code: SENSOR_FAULT_UNEXPECTED_WHO_AM_I,
                    observed_who_am_i: who_am_i,
                    spi_mode,
                };
            } else if best_fault.spi_mode == SPI_MODE_UNKNOWN {
                best_fault = FaultInfo {
                    code: SENSOR_FAULT_PROBE_NO_RESPONSE,
                    observed_who_am_i: who_am_i,
                    spi_mode,
                };
            }
        }

        Err(Mpu9250Error::Probe(best_fault))
    }

    fn apply_spi_mode(&mut self, spi_mode: u8, frequency: u32) {
        let mut config = spi::Config::default();
        config.frequency = frequency;
        match spi_mode {
            0 => {}
            1 => config.phase = spi::Phase::CaptureOnSecondTransition,
            2 => config.polarity = spi::Polarity::IdleHigh,
            3 => {
                config.phase = spi::Phase::CaptureOnSecondTransition;
                config.polarity = spi::Polarity::IdleHigh;
            }
            _ => return,
        }
        self.spi.set_config(&config);
    }

    fn sensor_kind(&self) -> SensorKind {
        self.sensor_kind
    }

    fn observed_who_am_i(&self) -> u8 {
        match self.sensor_kind {
            SensorKind::Mpu9250 => MPU_WHO_AM_I_MPU9250,
            SensorKind::Mpu6500 => MPU_WHO_AM_I_MPU6500,
            _ => 0,
        }
    }

    fn spi_mode(&self) -> u8 {
        self.spi_mode
    }

    fn next_sample(&mut self) -> Result<ImuSample, Mpu9250Error> {
        let payload = self.read_measurement_block()?;
        let accel_raw = [
            be_i16(payload[0], payload[1]),
            be_i16(payload[2], payload[3]),
            be_i16(payload[4], payload[5]),
        ];
        let temp_raw = be_i16(payload[6], payload[7]);
        let gyro_raw = [
            be_i16(payload[8], payload[9]),
            be_i16(payload[10], payload[11]),
            be_i16(payload[12], payload[13]),
        ];

        let mut status = 0u16;
        if self.warmup_remaining > 0 {
            status |= SENSOR_STATUS_CALIBRATING;
            self.warmup_remaining -= 1;
        }
        if near_limit(&accel_raw) {
            status |= SENSOR_STATUS_ACCEL_CLIPPED;
        }
        if near_limit(&gyro_raw) {
            status |= SENSOR_STATUS_GYRO_CLIPPED;
        }

        Ok(ImuSample {
            timestamp_us: Instant::now().as_micros() as u32,
            accel_mg: accel_raw.map(raw_accel_to_mg),
            gyro_mdps: gyro_raw.map(raw_gyro_to_mdps),
            temperature_centi_c: raw_temp_to_centi_c(temp_raw),
            status,
        })
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), Mpu9250Error> {
        let frame = [register & 0x7F, value];
        self.cs.set_low();
        let result = self.spi.blocking_write(&frame).map_err(Mpu9250Error::Spi);
        self.cs.set_high();
        result
    }

    fn read_u8(&mut self, register: u8) -> Result<u8, Mpu9250Error> {
        let mut frame = [register | 0x80, 0];
        self.cs.set_low();
        let result = self
            .spi
            .blocking_transfer_in_place(&mut frame)
            .map_err(Mpu9250Error::Spi);
        self.cs.set_high();
        result?;
        Ok(frame[1])
    }

    fn read_measurement_block(&mut self) -> Result<[u8; 14], Mpu9250Error> {
        let mut frame = [0u8; 15];
        frame[0] = MPU9250_REG_ACCEL_XOUT_H | 0x80;

        self.cs.set_low();
        let result = self
            .spi
            .blocking_transfer_in_place(&mut frame)
            .map_err(Mpu9250Error::Spi);
        self.cs.set_high();
        result?;

        let mut payload = [0u8; 14];
        payload.copy_from_slice(&frame[1..]);
        Ok(payload)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
struct FaultInfo {
    code: u8,
    observed_who_am_i: u8,
    spi_mode: u8,
}

struct ProbeResult {
    sensor_kind: SensorKind,
    who_am_i: u8,
    spi_mode: u8,
}

struct FaultedSensor {
    fault_info: FaultInfo,
}

impl FaultedSensor {
    const fn new(fault_info: FaultInfo) -> Self {
        Self { fault_info }
    }

    fn next_sample(&mut self) -> ImuSample {
        fault_sample(SENSOR_STATUS_FAULT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
enum Mpu9250Error {
    Probe(FaultInfo),
    Spi(spi::Error),
}

impl Mpu9250Error {
    fn fault_info(self) -> FaultInfo {
        match self {
            Self::Probe(info) => info,
            Self::Spi(_) => FaultInfo {
                code: SENSOR_FAULT_READ,
                observed_who_am_i: 0,
                spi_mode: SPI_MODE_UNKNOWN,
            },
        }
    }
}

fn fault_sample(status: u16) -> ImuSample {
    ImuSample {
        timestamp_us: Instant::now().as_micros() as u32,
        accel_mg: [0; 3],
        gyro_mdps: [0; 3],
        temperature_centi_c: 0,
        status,
    }
}

fn be_i16(high: u8, low: u8) -> i16 {
    i16::from_be_bytes([high, low])
}

fn raw_accel_to_mg(raw: i16) -> i16 {
    ((raw as i32 * 1000) / 16_384) as i16
}

fn raw_gyro_to_mdps(raw: i16) -> i32 {
    (raw as i32 * 1000) / 131
}

fn raw_temp_to_centi_c(raw: i16) -> i16 {
    (((raw as i32) * 10_000) / 33_387 + 2_100) as i16
}

fn near_limit(values: &[i16; 3]) -> bool {
    values
        .iter()
        .copied()
        .any(|value| value >= 32_000 || value <= -32_000)
}

fn firmware_version() -> [u8; 3] {
    [
        parse_version_component(env!("CARGO_PKG_VERSION_MAJOR")),
        parse_version_component(env!("CARGO_PKG_VERSION_MINOR")),
        parse_version_component(env!("CARGO_PKG_VERSION_PATCH")),
    ]
}

fn sensor_kind_from_who_am_i(who_am_i: u8) -> Option<SensorKind> {
    match who_am_i {
        MPU_WHO_AM_I_MPU9250 => Some(SensorKind::Mpu9250),
        MPU_WHO_AM_I_MPU6500 => Some(SensorKind::Mpu6500),
        _ => None,
    }
}

fn parse_version_component(value: &str) -> u8 {
    let mut parsed = 0u16;

    for byte in value.as_bytes() {
        let digit = byte.wrapping_sub(b'0') as u16;
        parsed = parsed.saturating_mul(10).saturating_add(digit);
    }

    parsed as u8
}

struct Disconnected;

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("USB endpoint buffer overflow"),
            EndpointError::Disabled => Disconnected,
        }
    }
}
