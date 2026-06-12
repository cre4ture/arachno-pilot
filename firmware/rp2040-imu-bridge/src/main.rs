#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::mem::MaybeUninit;

use arachno_imu_proto::{
    CAP_ACCEL, CAP_GYRO, CAP_TEMP, DeviceInfo, ImuSample, MAX_FRAME_LEN, SENSOR_FAULT_NONE,
    SENSOR_FAULT_PROBE_NO_RESPONSE, SENSOR_FAULT_READ, SPI_MODE_UNKNOWN, SensorKind,
    encode_device_info_frame, encode_sample_frame,
};
use defmt::{info, panic, unwrap, warn};
use embassy_executor::Spawner;
use embassy_rp::Peri;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, AbortReason, I2c};
use embassy_rp::peripherals::{I2C1, PIN_2, PIN_3, PIN_4, PIN_5, SPI0, USB};
use embassy_rp::spi::{self, Blocking as SpiBlocking, Spi};
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_time::{Duration, Instant, Ticker, Timer};
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use rp2040_imu_bridge::{
    FaultInfo, MPU_I2C_ADDRESSES, MPU_MEASUREMENT_PAYLOAD_LEN, MPU_REG_ACCEL_XOUT_H,
    MPU_REG_WHO_AM_I, ProbeResult, SENSOR_STATUS_FAULT, init_steps, sample_from_payload,
    sensor_kind_from_who_am_i, validate_who_am_i,
};
use {defmt_rtt as _, panic_probe as _};

const SAMPLE_HZ: u32 = 200;
const SAMPLE_PERIOD_MS: u64 = 1_000 / SAMPLE_HZ as u64;
const DEVICE_INFO_ANNOUNCE_INTERVAL_SAMPLES: u32 = SAMPLE_HZ / 4;
const SENSOR_WARMUP_SAMPLES: u32 = SAMPLE_HZ;

const MPU_I2C_HZ: u32 = 400_000;
const MPU9250_SPI_HZ: u32 = 1_000_000;
const MPU9250_SPI_PROBE_HZ: u32 = 125_000;

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

    // Keep the IMU on low GPIO numbers away from the board's CH9120 control pins.
    // Wiring:
    // SPI backend:
    //   GP2 -> MPU-9250 SCL/SCLK
    //   GP3 -> MPU-9250 SDA/SDI
    //   GP4 -> MPU-9250 AD0/SDO
    //   GP5 -> MPU-9250 NCS/CS
    // I2C backend:
    //   GP2 -> MPU-6050 SDA
    //   GP3 -> MPU-6050 SCL
    let mut sensor = SensorState::new(p.I2C1, p.SPI0, p.PIN_2, p.PIN_3, p.PIN_4, p.PIN_5).await;
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
    Real(ActiveSensor<'d>),
    Faulted(FaultedSensor),
}

impl<'d> SensorState<'d> {
    async fn new(
        i2c1: Peri<'d, I2C1>,
        spi0: Peri<'d, SPI0>,
        gpio2: Peri<'d, PIN_2>,
        gpio3: Peri<'d, PIN_3>,
        miso: Peri<'d, PIN_4>,
        cs: Peri<'d, PIN_5>,
    ) -> Self {
        let i2c_fault = {
            // Safety: the temporary I2C probe either becomes the sole live backend when we
            // return early on success, or it is fully dropped before the SPI backend is built.
            let i2c_result = {
                let _cs_idle = Output::new(unsafe { cs.clone_unchecked() }, Level::High);
                MpuI2cSensor::new(
                    unsafe { i2c1.clone_unchecked() },
                    unsafe { gpio3.clone_unchecked() },
                    unsafe { gpio2.clone_unchecked() },
                )
                .await
            };

            match i2c_result {
                Ok(sensor) => return Self::Real(ActiveSensor::I2c(sensor)),
                Err(err) => {
                    warn!("i2c imu init failed: {:?}", err);
                    err.fault_info()
                }
            }
        };

        let mut config = spi::Config::default();
        config.frequency = MPU9250_SPI_HZ;

        let spi = Spi::new_blocking(spi0, gpio2, gpio3, miso, config);
        let cs = Output::new(cs, Level::High);

        match MpuSpiSensor::new(spi, cs).await {
            Ok(sensor) => Self::Real(ActiveSensor::Spi(sensor)),
            Err(err) => {
                warn!("spi imu init failed: {:?}", err);
                let fault_info = if i2c_fault.code != SENSOR_FAULT_PROBE_NO_RESPONSE {
                    i2c_fault
                } else {
                    err.fault_info()
                };
                Self::Faulted(FaultedSensor::new(fault_info))
            }
        }
    }

    fn next_sample(&mut self) -> ImuSample {
        match self {
            Self::Real(sensor) => match sensor.next_sample() {
                Ok(sample) => sample,
                Err(err) => {
                    warn!("imu read failed: {:?}", err);
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

enum ActiveSensor<'d> {
    I2c(MpuI2cSensor<'d>),
    Spi(MpuSpiSensor<'d>),
}

impl<'d> ActiveSensor<'d> {
    fn next_sample(&mut self) -> Result<ImuSample, MpuSensorError> {
        match self {
            Self::I2c(sensor) => sensor.next_sample(),
            Self::Spi(sensor) => sensor.next_sample(),
        }
    }

    fn sensor_kind(&self) -> SensorKind {
        match self {
            Self::I2c(sensor) => sensor.sensor_kind(),
            Self::Spi(sensor) => sensor.sensor_kind(),
        }
    }

    fn observed_who_am_i(&self) -> u8 {
        match self {
            Self::I2c(sensor) => sensor.observed_who_am_i(),
            Self::Spi(sensor) => sensor.observed_who_am_i(),
        }
    }

    fn spi_mode(&self) -> u8 {
        match self {
            Self::I2c(sensor) => sensor.spi_mode(),
            Self::Spi(sensor) => sensor.spi_mode(),
        }
    }
}

struct MpuI2cSensor<'d> {
    i2c: I2c<'d, I2C1, i2c::Blocking>,
    warmup_remaining: u32,
    sensor_kind: SensorKind,
    observed_who_am_i: u8,
    address: u8,
}

impl<'d> MpuI2cSensor<'d> {
    async fn new(
        i2c1: Peri<'d, I2C1>,
        scl: Peri<'d, PIN_3>,
        sda: Peri<'d, PIN_2>,
    ) -> Result<Self, MpuSensorError> {
        let mut config = i2c::Config::default();
        config.frequency = MPU_I2C_HZ;

        let i2c = I2c::new_blocking(i2c1, scl, sda, config);
        let mut sensor = Self {
            i2c,
            warmup_remaining: SENSOR_WARMUP_SAMPLES,
            sensor_kind: SensorKind::Unknown,
            observed_who_am_i: 0,
            address: MPU_I2C_ADDRESSES[0],
        };

        Timer::after(Duration::from_millis(100)).await;
        let probe = sensor.probe_transport()?;
        sensor.sensor_kind = probe.probe.sensor_kind;
        sensor.observed_who_am_i = probe.probe.who_am_i;
        sensor.address = probe.address;
        sensor.apply_init_sequence(false).await?;

        let who_am_i = sensor.read_u8(MPU_REG_WHO_AM_I)?;
        validate_who_am_i(who_am_i, probe.probe).map_err(MpuSensorError::Probe)?;

        info!(
            "imu online over I2C addr {=u8}, who_am_i={=u8}, kind={=u8}",
            probe.address, who_am_i, probe.probe.sensor_kind as u8
        );
        Ok(sensor)
    }

    fn probe_transport(&mut self) -> Result<I2cProbeResult, MpuSensorError> {
        let mut best_fault = FaultInfo {
            code: SENSOR_FAULT_PROBE_NO_RESPONSE,
            observed_who_am_i: 0,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        for address in MPU_I2C_ADDRESSES {
            match self.read_u8_at(address, MPU_REG_WHO_AM_I) {
                Ok(who_am_i) => {
                    if let Some(sensor_kind) = sensor_kind_from_who_am_i(who_am_i) {
                        return Ok(I2cProbeResult {
                            address,
                            probe: ProbeResult {
                                sensor_kind,
                                who_am_i,
                                spi_mode: SPI_MODE_UNKNOWN,
                            },
                        });
                    }

                    if who_am_i != 0x00 && who_am_i != 0xFF {
                        best_fault = FaultInfo {
                            code: arachno_imu_proto::SENSOR_FAULT_UNEXPECTED_WHO_AM_I,
                            observed_who_am_i: who_am_i,
                            spi_mode: SPI_MODE_UNKNOWN,
                        };
                    }
                }
                Err(err) => {
                    let fault = fault_from_i2c_probe_error(err);
                    if fault.code != SENSOR_FAULT_PROBE_NO_RESPONSE
                        || best_fault.code == SENSOR_FAULT_PROBE_NO_RESPONSE
                    {
                        best_fault = fault;
                    }
                }
            }
        }

        Err(MpuSensorError::Probe(best_fault))
    }

    async fn apply_init_sequence(
        &mut self,
        disable_i2c_interface: bool,
    ) -> Result<(), MpuSensorError> {
        for step in init_steps(disable_i2c_interface) {
            self.write_register(step.register, step.value)?;
            if step.delay_after_ms > 0 {
                Timer::after(Duration::from_millis(step.delay_after_ms)).await;
            }
        }
        Ok(())
    }

    fn sensor_kind(&self) -> SensorKind {
        self.sensor_kind
    }

    fn observed_who_am_i(&self) -> u8 {
        self.observed_who_am_i
    }

    fn spi_mode(&self) -> u8 {
        SPI_MODE_UNKNOWN
    }

    fn next_sample(&mut self) -> Result<ImuSample, MpuSensorError> {
        let payload = self.read_measurement_block()?;
        Ok(sample_from_payload(
            payload,
            &mut self.warmup_remaining,
            Instant::now().as_micros() as u32,
        ))
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), MpuSensorError> {
        self.i2c
            .blocking_write(self.address, &[register, value])
            .map_err(MpuSensorError::I2c)
    }

    fn read_u8(&mut self, register: u8) -> Result<u8, MpuSensorError> {
        self.read_u8_at(self.address, register)
            .map_err(MpuSensorError::I2c)
    }

    fn read_u8_at(&mut self, address: u8, register: u8) -> Result<u8, i2c::Error> {
        let mut value = [0u8; 1];
        self.i2c
            .blocking_write_read(address, &[register], &mut value)?;
        Ok(value[0])
    }

    fn read_measurement_block(
        &mut self,
    ) -> Result<[u8; MPU_MEASUREMENT_PAYLOAD_LEN], MpuSensorError> {
        let mut payload = [0u8; MPU_MEASUREMENT_PAYLOAD_LEN];
        self.i2c
            .blocking_write_read(self.address, &[MPU_REG_ACCEL_XOUT_H], &mut payload)
            .map_err(MpuSensorError::I2c)?;
        Ok(payload)
    }
}

struct MpuSpiSensor<'d> {
    spi: Spi<'d, SPI0, SpiBlocking>,
    cs: Output<'d>,
    warmup_remaining: u32,
    sensor_kind: SensorKind,
    observed_who_am_i: u8,
    spi_mode: u8,
}

impl<'d> MpuSpiSensor<'d> {
    async fn new(spi: Spi<'d, SPI0, SpiBlocking>, cs: Output<'d>) -> Result<Self, MpuSensorError> {
        let mut sensor = Self {
            spi,
            cs,
            warmup_remaining: SENSOR_WARMUP_SAMPLES,
            sensor_kind: SensorKind::Unknown,
            observed_who_am_i: 0,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        Timer::after(Duration::from_millis(100)).await;
        let probe = sensor.probe_transport().await?;
        sensor.sensor_kind = probe.sensor_kind;
        sensor.observed_who_am_i = probe.who_am_i;
        sensor.spi_mode = probe.spi_mode;
        sensor.apply_spi_mode(probe.spi_mode, MPU9250_SPI_HZ);
        sensor.apply_init_sequence(true).await?;

        let who_am_i = sensor.read_u8(MPU_REG_WHO_AM_I)?;
        validate_who_am_i(who_am_i, probe).map_err(MpuSensorError::Probe)?;

        info!(
            "imu online over SPI mode {=u8}, who_am_i={=u8}, kind={=u8}",
            probe.spi_mode, who_am_i, probe.sensor_kind as u8
        );
        Ok(sensor)
    }

    async fn probe_transport(&mut self) -> Result<ProbeResult, MpuSensorError> {
        let mut best_fault = FaultInfo {
            code: SENSOR_FAULT_PROBE_NO_RESPONSE,
            observed_who_am_i: 0,
            spi_mode: SPI_MODE_UNKNOWN,
        };

        for spi_mode in 0..=3u8 {
            self.apply_spi_mode(spi_mode, MPU9250_SPI_PROBE_HZ);
            Timer::after(Duration::from_millis(2)).await;

            let who_am_i = self.read_u8(MPU_REG_WHO_AM_I).map_err(|_| {
                MpuSensorError::Probe(FaultInfo {
                    code: SENSOR_FAULT_READ,
                    observed_who_am_i: 0,
                    spi_mode,
                })
            })?;

            if let Some(sensor_kind) = sensor_kind_from_who_am_i(who_am_i) {
                return Ok(ProbeResult {
                    sensor_kind,
                    who_am_i,
                    spi_mode,
                });
            }

            if who_am_i != 0x00 && who_am_i != 0xFF {
                best_fault = FaultInfo {
                    code: arachno_imu_proto::SENSOR_FAULT_UNEXPECTED_WHO_AM_I,
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

        Err(MpuSensorError::Probe(best_fault))
    }

    async fn apply_init_sequence(
        &mut self,
        disable_i2c_interface: bool,
    ) -> Result<(), MpuSensorError> {
        for step in init_steps(disable_i2c_interface) {
            self.write_register(step.register, step.value)?;
            if step.delay_after_ms > 0 {
                Timer::after(Duration::from_millis(step.delay_after_ms)).await;
            }
        }
        Ok(())
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
        self.observed_who_am_i
    }

    fn spi_mode(&self) -> u8 {
        self.spi_mode
    }

    fn next_sample(&mut self) -> Result<ImuSample, MpuSensorError> {
        let payload = self.read_measurement_block()?;
        Ok(sample_from_payload(
            payload,
            &mut self.warmup_remaining,
            Instant::now().as_micros() as u32,
        ))
    }

    fn write_register(&mut self, register: u8, value: u8) -> Result<(), MpuSensorError> {
        let frame = [register & 0x7F, value];
        self.cs.set_low();
        let result = self.spi.blocking_write(&frame).map_err(MpuSensorError::Spi);
        self.cs.set_high();
        result
    }

    fn read_u8(&mut self, register: u8) -> Result<u8, MpuSensorError> {
        let mut frame = [register | 0x80, 0];
        self.cs.set_low();
        let result = self
            .spi
            .blocking_transfer_in_place(&mut frame)
            .map_err(MpuSensorError::Spi);
        self.cs.set_high();
        result?;
        Ok(frame[1])
    }

    fn read_measurement_block(
        &mut self,
    ) -> Result<[u8; MPU_MEASUREMENT_PAYLOAD_LEN], MpuSensorError> {
        let mut frame = [0u8; MPU_MEASUREMENT_PAYLOAD_LEN + 1];
        frame[0] = MPU_REG_ACCEL_XOUT_H | 0x80;

        self.cs.set_low();
        let result = self
            .spi
            .blocking_transfer_in_place(&mut frame)
            .map_err(MpuSensorError::Spi);
        self.cs.set_high();
        result?;

        let mut payload = [0u8; MPU_MEASUREMENT_PAYLOAD_LEN];
        payload.copy_from_slice(&frame[1..]);
        Ok(payload)
    }
}

struct I2cProbeResult {
    address: u8,
    probe: ProbeResult,
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
enum MpuSensorError {
    Probe(FaultInfo),
    Spi(spi::Error),
    I2c(i2c::Error),
}

impl MpuSensorError {
    fn fault_info(self) -> FaultInfo {
        match self {
            Self::Probe(info) => info,
            Self::Spi(_) | Self::I2c(_) => FaultInfo {
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

fn firmware_version() -> [u8; 3] {
    [
        parse_version_component(env!("CARGO_PKG_VERSION_MAJOR")),
        parse_version_component(env!("CARGO_PKG_VERSION_MINOR")),
        parse_version_component(env!("CARGO_PKG_VERSION_PATCH")),
    ]
}

fn parse_version_component(value: &str) -> u8 {
    let mut parsed = 0u16;

    for byte in value.as_bytes() {
        let digit = byte.wrapping_sub(b'0') as u16;
        parsed = parsed.saturating_mul(10).saturating_add(digit);
    }

    parsed as u8
}

fn fault_from_i2c_probe_error(err: i2c::Error) -> FaultInfo {
    let code = match err {
        i2c::Error::Abort(AbortReason::NoAcknowledge) => SENSOR_FAULT_PROBE_NO_RESPONSE,
        _ => SENSOR_FAULT_READ,
    };

    FaultInfo {
        code,
        observed_who_am_i: 0,
        spi_mode: SPI_MODE_UNKNOWN,
    }
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
