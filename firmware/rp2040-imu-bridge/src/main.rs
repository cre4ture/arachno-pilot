#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::mem::MaybeUninit;

use arachno_imu_proto::{ImuSample, MAX_FRAME_LEN, encode_sample_frame};
use defmt::{info, panic, unwrap};
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_time::{Duration, Timer};
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use {defmt_rtt as _, panic_probe as _};

const SAMPLE_HZ: u32 = 200;
const SAMPLE_PERIOD_MS: u64 = 1_000 / SAMPLE_HZ as u64;

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

    let mut sensor = MockImuSensor::new();
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
    sensor: &mut MockImuSensor,
    sequence: &mut u8,
    frame_buf: &mut [u8; MAX_FRAME_LEN],
) -> Result<(), Disconnected> {
    loop {
        let sample = sensor.next_sample();
        let frame_len = encode_sample_frame(*sequence, &sample, frame_buf)
            .expect("IMU frame buffer is statically sized for the protocol");

        class.write_packet(&frame_buf[..frame_len]).await?;
        *sequence = sequence.wrapping_add(1);

        Timer::after(Duration::from_millis(SAMPLE_PERIOD_MS)).await;
    }
}

#[derive(Debug, Clone, Copy)]
struct MockImuSensor {
    tick: u32,
}

impl MockImuSensor {
    const fn new() -> Self {
        Self { tick: 0 }
    }

    fn next_sample(&mut self) -> ImuSample {
        self.tick = self.tick.wrapping_add(1);
        let phase = (self.tick % 200) as i16 - 100;
        let status = if self.tick < SAMPLE_HZ {
            0x0020
        } else {
            0x0000
        };

        ImuSample {
            timestamp_us: self.tick.saturating_mul(1_000_000 / SAMPLE_HZ),
            accel_mg: [phase / 4, -phase / 5, 1_000],
            gyro_mdps: [phase as i32 * 500, 0, -phase as i32 * 250],
            temperature_centi_c: 2_600,
            status,
        }
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
