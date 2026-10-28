use linux_embedded_hal::{
    CdevPin, Delay, SpidevDevice,
    gpio_cdev::{Chip, LineHandle, LineRequestFlags},
    spidev::{SpiModeFlags, SpidevOptions},
};
use mipidsi::{
    Builder, NoResetPin, interface::SpiInterface, models::ST7789, options::ColorInversion,
};

use crate::AppError;

const SPI_DC: u32 = 25;
const BACKLIGHT: u32 = 22;
pub const W: u16 = 240;
pub const H: u16 = 240;

type Display = mipidsi::Display<SpiInterface<'static, SpidevDevice, CdevPin>, ST7789, NoResetPin>;

pub struct HwDisplay {
    pub display: Display,
    /// Held for the lifetime of the display: dropping the handle releases the
    /// line, after which the kernel no longer guarantees it stays driven high.
    _backlight: LineHandle,
}

pub fn init() -> Result<HwDisplay, AppError> {
    let mut chip =
        Chip::new("/dev/gpiochip0").map_err(|e| AppError::Gpio(format!("/dev/gpiochip0: {e}")))?;

    let dc = CdevPin::new(
        chip.get_line(SPI_DC)
            .and_then(|l| l.request(LineRequestFlags::OUTPUT, 0, "pipulse-dc"))
            .map_err(|e| AppError::Gpio(format!("DC line {SPI_DC}: {e}")))?,
    )
    .map_err(|e| AppError::Gpio(format!("DC line {SPI_DC}: {e}")))?;

    let backlight = chip
        .get_line(BACKLIGHT)
        .and_then(|l| l.request(LineRequestFlags::OUTPUT, 1, "pipulse-bl"))
        .map_err(|e| AppError::Gpio(format!("backlight line {BACKLIGHT}: {e}")))?;

    let mut spi = SpidevDevice::open("/dev/spidev0.0")
        .map_err(|e| AppError::Spi(format!("/dev/spidev0.0: {e}")))?;
    spi.configure(
        &SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(60_000_000)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build(),
    )
    .map_err(|e| AppError::Spi(format!("configure: {e}")))?;

    let buffer: &'static mut [u8; 512] = Box::leak(Box::new([0u8; 512]));
    let di = SpiInterface::new(spi, dc, buffer);
    let mut delay = Delay;

    let display = Builder::new(ST7789, di)
        .display_size(W, H)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .map_err(|e| AppError::Display(format!("ST7789 init: {e:?}")))?;

    Ok(HwDisplay {
        display,
        _backlight: backlight,
    })
}
