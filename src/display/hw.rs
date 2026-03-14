use linux_embedded_hal::{
    CdevPin, Delay, SpidevDevice,
    gpio_cdev::{Chip, LineRequestFlags},
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

type HwDisplay = mipidsi::Display<SpiInterface<'static, SpidevDevice, CdevPin>, ST7789, NoResetPin>;

pub fn init() -> Result<HwDisplay, AppError> {
    let mut chip = Chip::new("/dev/gpiochip0").map_err(|_| AppError::Gpio)?;

    let dc = CdevPin::new(
        chip.get_line(SPI_DC)
            .map_err(|_| AppError::Gpio)?
            .request(LineRequestFlags::OUTPUT, 0, "pipulse-dc")
            .map_err(|_| AppError::Gpio)?,
    )
    .map_err(|_| AppError::Gpio)?;

    chip.get_line(BACKLIGHT)
        .map_err(|_| AppError::Gpio)?
        .request(LineRequestFlags::OUTPUT, 1, "pipulse-bl")
        .map_err(|_| AppError::Gpio)?;

    let mut spi = SpidevDevice::open("/dev/spidev0.0").map_err(|_| AppError::Spi)?;
    spi.configure(
        &SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(60_000_000)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build(),
    )
    .map_err(|_| AppError::Spi)?;

    let buffer: &'static mut [u8; 512] = Box::leak(Box::new([0u8; 512]));
    let di = SpiInterface::new(spi, dc, buffer);
    let mut delay = Delay;

    Builder::new(ST7789, di)
        .display_size(W, H)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .map_err(|_| AppError::Display)
}
