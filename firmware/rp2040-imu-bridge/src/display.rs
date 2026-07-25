use embassy_rp::Peri;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{PIN_0, PIN_1, PIN_22, PIN_26, PIN_27, PIN_28, SPI1};
use embassy_rp::spi::{self, Async as SpiAsync, Spi};
use embassy_rp::{dma, interrupt};
use embassy_time::{Duration, Timer};
use rp2040_imu_bridge::{
    DISPLAY_STATUS_COLUMNS, DISPLAY_STATUS_LINE_COUNT, DisplayStatus, WAVESHARE_1IN83_REV2_INIT,
};

const DISPLAY_SPI_HZ: u32 = 24_000_000;
const DISPLAY_WIDTH: usize = 240;
const DISPLAY_HEIGHT: usize = 284;
const DISPLAY_Y_OFFSET: u16 = 0;

const FONT_WIDTH: usize = 5;
const FONT_HEIGHT: usize = 7;
const FONT_SCALE: usize = 2;
const GLYPH_WIDTH: usize = (FONT_WIDTH + 1) * FONT_SCALE;
const GLYPH_HEIGHT: usize = FONT_HEIGHT * FONT_SCALE;
const LINE_HEIGHT: usize = GLYPH_HEIGHT + 4;
const GLYPH_BYTES: usize = GLYPH_WIDTH * GLYPH_HEIGHT * 2;

const CYAN: u16 = 0x07FF;
const GREEN: u16 = 0x07E0;
const YELLOW: u16 = 0xFFE0;

pub struct St7789Display<'d> {
    spi: Spi<'d, SPI1, SpiAsync>,
    cs: Output<'d>,
    dc: Output<'d>,
    reset: Output<'d>,
    _backlight: Output<'d>,
}

impl<'d> St7789Display<'d> {
    pub async fn new<TxDma>(
        spi1: Peri<'d, SPI1>,
        sck: Peri<'d, PIN_26>,
        mosi: Peri<'d, PIN_27>,
        cs: Peri<'d, PIN_0>,
        dc: Peri<'d, PIN_1>,
        reset: Peri<'d, PIN_22>,
        backlight: Peri<'d, PIN_28>,
        tx_dma: Peri<'d, TxDma>,
        irq: impl interrupt::typelevel::Binding<TxDma::Interrupt, dma::InterruptHandler<TxDma>> + 'd,
    ) -> Result<Self, DisplayError>
    where
        TxDma: dma::ChannelInstance,
    {
        let mut config = spi::Config::default();
        config.frequency = DISPLAY_SPI_HZ;

        let mut display = Self {
            spi: Spi::new_txonly(spi1, sck, mosi, tx_dma, irq, config),
            cs: Output::new(cs, Level::High),
            dc: Output::new(dc, Level::Low),
            reset: Output::new(reset, Level::High),
            _backlight: Output::new(backlight, Level::High),
        };
        display.hardware_reset().await;
        display.initialize().await?;
        display.clear().await?;
        Ok(display)
    }

    pub async fn draw_status(&mut self, status: &DisplayStatus) -> Result<(), DisplayError> {
        for (line_index, line) in status.lines.iter().enumerate() {
            debug_assert!(line_index < DISPLAY_STATUS_LINE_COUNT);
            self.draw_line(line_index, line.as_str(), status_line_color(line_index))
                .await?;
        }
        Ok(())
    }

    async fn hardware_reset(&mut self) {
        self.reset.set_high();
        Timer::after(Duration::from_millis(10)).await;
        self.reset.set_low();
        Timer::after(Duration::from_millis(10)).await;
        self.reset.set_high();
        Timer::after(Duration::from_millis(120)).await;
    }

    async fn initialize(&mut self) -> Result<(), DisplayError> {
        for init in WAVESHARE_1IN83_REV2_INIT {
            self.command(init.command, init.data)?;
        }
        self.command(0x11, &[])?; // Sleep out
        Timer::after(Duration::from_millis(200)).await;
        self.command(0x29, &[])?; // Display on
        Timer::after(Duration::from_millis(10)).await;
        Ok(())
    }

    async fn clear(&mut self) -> Result<(), DisplayError> {
        self.set_window(0, 0, DISPLAY_WIDTH, DISPLAY_HEIGHT)?;
        let black_pixels = [0u8; DISPLAY_WIDTH * 2];
        self.cs.set_low();
        self.dc.set_high();
        let mut result = Ok(());
        for _ in 0..DISPLAY_HEIGHT {
            if result.is_ok() {
                result = self.spi.write(&black_pixels).await;
            }
        }
        self.cs.set_high();
        result.map_err(DisplayError::Spi)
    }

    async fn draw_line(
        &mut self,
        line_index: usize,
        text: &str,
        color: u16,
    ) -> Result<(), DisplayError> {
        debug_assert!(text.len() <= DISPLAY_STATUS_COLUMNS);
        let text_bytes = text.as_bytes();
        let y = line_index * LINE_HEIGHT;

        for column in 0..DISPLAY_STATUS_COLUMNS {
            let character = text_bytes.get(column).copied().unwrap_or(b' ');
            self.draw_glyph(column * GLYPH_WIDTH, y, character, color)
                .await?;
        }
        Ok(())
    }

    async fn draw_glyph(
        &mut self,
        x: usize,
        y: usize,
        character: u8,
        color: u16,
    ) -> Result<(), DisplayError> {
        let glyph = glyph(character);
        let mut pixels = [0u8; GLYPH_BYTES];
        let [foreground_high, foreground_low] = color.to_be_bytes();

        for pixel_y in 0..GLYPH_HEIGHT {
            for pixel_x in 0..GLYPH_WIDTH {
                let glyph_x = pixel_x / FONT_SCALE;
                let glyph_y = pixel_y / FONT_SCALE;
                let enabled = glyph_x < FONT_WIDTH
                    && glyph_y < FONT_HEIGHT
                    && glyph[glyph_y] & (1 << (FONT_WIDTH - 1 - glyph_x)) != 0;
                let offset = (pixel_y * GLYPH_WIDTH + pixel_x) * 2;
                if enabled {
                    pixels[offset] = foreground_high;
                    pixels[offset + 1] = foreground_low;
                }
            }
        }

        self.set_window(x, y, GLYPH_WIDTH, GLYPH_HEIGHT)?;
        self.write_pixels(&pixels).await
    }

    fn set_window(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Result<(), DisplayError> {
        debug_assert!(x + width <= DISPLAY_WIDTH);
        debug_assert!(y + height <= DISPLAY_HEIGHT);
        let x_start = x as u16;
        let x_end = (x + width - 1) as u16;
        let y_start = y as u16 + DISPLAY_Y_OFFSET;
        let y_end = (y + height - 1) as u16 + DISPLAY_Y_OFFSET;

        self.command(
            0x2A,
            &[
                x_start.to_be_bytes()[0],
                x_start.to_be_bytes()[1],
                x_end.to_be_bytes()[0],
                x_end.to_be_bytes()[1],
            ],
        )?;
        self.command(
            0x2B,
            &[
                y_start.to_be_bytes()[0],
                y_start.to_be_bytes()[1],
                y_end.to_be_bytes()[0],
                y_end.to_be_bytes()[1],
            ],
        )?;
        self.command(0x2C, &[])
    }

    fn command(&mut self, command: u8, data: &[u8]) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low();
        let mut result = self.spi.blocking_write(&[command]);
        if result.is_ok() && !data.is_empty() {
            self.dc.set_high();
            result = self.spi.blocking_write(data);
        }
        self.cs.set_high();
        result.map_err(DisplayError::Spi)
    }

    async fn write_pixels(&mut self, pixels: &[u8]) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_high();
        let result = self.spi.write(pixels).await;
        self.cs.set_high();
        result.map_err(DisplayError::Spi)
    }
}

fn status_line_color(line_index: usize) -> u16 {
    match line_index {
        0 => CYAN,
        6.. => YELLOW,
        _ => GREEN,
    }
}

fn glyph(character: u8) -> [u8; FONT_HEIGHT] {
    match character {
        b'0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        b'1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        b'3' => [0x1E, 0x01, 0x02, 0x06, 0x01, 0x11, 0x0E],
        b'4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        b'5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        b'6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        b'7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        b'9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        b'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        b'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        b'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0E],
        b'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        b'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        b'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'm' => [0x00, 0x00, 0x1A, 0x15, 0x15, 0x15, 0x15],
        b'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        b'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        b'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        b'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        b'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        b'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        b'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        b'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        b'+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        b'-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        b'/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        _ => [0; FONT_HEIGHT],
    }
}

#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum DisplayError {
    Spi(spi::Error),
}
