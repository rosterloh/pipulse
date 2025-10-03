use std::{fs, time::Instant};

// use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::{
    mono_font::{ascii::{FONT_8X13, FONT_10X20}, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::{Alignment, Text}
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use get_if_addrs::get_if_addrs;
use linux_embedded_hal::{
    // spidev::{SpiModeFlags, SpidevOptions},
    Delay,
    // Spidev,
    // sysfs_gpio::{Direction, Pin}
};
use mipidsi::interface::SpiInterface;
use mipidsi::{models::ST7789, options::ColorInversion, Builder};
use rppal::{gpio::Gpio, spi::{Bus, Mode, SlaveSelect, Spi}};
use thiserror::Error;

const SPI_DC: u8 = 25;
const BACKLIGHT: u8 = 22;
const BUTTON_A: u8 = 23;
const BUTTON_B: u8 = 24;
const W: i32 = 240;
const H: i32 = 240;

#[derive(Debug, Error)]
enum AppError {
    #[error("SPI error: {0}")]
    Spi(#[from] rppal::spi::Error), // std::io::Error
    #[error("GPIO error")]
    Gpio,
    #[error("Display error")]
    Display,
}

 
fn get_ipv4() -> String {
    match get_if_addrs() {
        Ok(addrs) => {
            let ip_opt = addrs.into_iter()
                .filter(|ifa| !ifa.is_loopback())
                .filter_map(|ifa| match ifa.ip() {
                    std::net::IpAddr::V4(v4) => Some((ifa.name, v4)),
                    _ => None,
                })
                .find(|(name, _)| !name.starts_with("docker") && !name.starts_with("veth") && !name.starts_with("lo"));

                if let Some((_, ip)) = ip_opt { ip.to_string() } else { "no ip".into() }
        },
        Err(_) => "no ip".into(),
    }
}


fn get_loadavg() -> String {
    if let Ok(s) = fs::read_to_string("/proc/loadavg") {
        let mut parts = s.split_whitespace();
        let one = parts.next().unwrap_or("?");
        let five = parts.next().unwrap_or("?");
        let fifteen = parts.next().unwrap_or("?");
        format!("{one} {five} {fifteen}")
    } else {
        "? ? ?".into()
    }
}


fn main() -> Result<(), AppError> {
    // GPIO
    let gpio = Gpio::new().map_err(|_| AppError::Gpio)?;
    let dc = gpio.get(SPI_DC).map_err(|_| AppError::Gpio)?.into_output();
    let mut backlight = gpio.get(BACKLIGHT).map_err(|_| AppError::Gpio)?.into_output();
    // Buttons
    let _button_a = gpio.get(BUTTON_A).map_err(|_| AppError::Gpio)?.into_input_pullup();
    let _button_b = gpio.get(BUTTON_B).map_err(|_| AppError::Gpio)?.into_input_pullup();

    // SPI Display
    let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 60_000_000_u32, Mode::Mode0).map_err(AppError::Spi)?;
    let spi_device = ExclusiveDevice::new_no_delay(spi, NoCs).unwrap();
    let mut buffer = [0_u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut buffer);
    let mut delay = Delay;
    let mut display = Builder::new(ST7789, di)
        .display_size(W as u16, H as u16)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .map_err(|_| AppError::Display)?;

    // with linux-embedded-hal
    // let mut spi = linux_embedded_hal::Spidev::open("/dev/spidev0.0")?;
    // let options = linux_embedded_hal::spidev::SpidevOptions::new()
    //     .bits_per_word(8)
    //     .max_speed_hz(8_000_000)
    //     .mode(linux_embedded_hal::spidev::SpiModeFlags::SPI_MODE_0)
    //     .build();
    // spi.configure(&options)?;
    // let dc = linux_embedded_hal::sysfs_gpio::Pin::new(SPI_DC);
    // dc.export().map_err(|_| AppError::Gpio)?;
    // dc.set_direction(linux_embedded_hal::sysfs_gpio::Direction::Out).map_err(|_| AppError::Gpio)?;

    let big = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let big_char_w = 10;
    let big_char_h = 20;
    let small = MonoTextStyle::new(&FONT_8X13, Rgb565::WHITE);
    let _small_char_w = 8;
    let small_char_h = 13;

    // Clear the display initially
    display.clear(Rgb565::BLACK).map_err(|_| AppError::Display)?;

    // Title
    let header_text = "PiPulse";
    let header_x = (W / 2) - ((header_text.len() * big_char_w as usize) as i32 / 2);
    Text::with_alignment(
        header_text,
        Point::new(header_x, big_char_h),
        big,
        Alignment::Center,
    ).draw(&mut display).map_err(|_| AppError::Display)?;

    // Turn on backlight
    // backlight.set_high();
    backlight.set_pwm_frequency(50., 0.5).map_err(|_| AppError::Gpio)?;

    let _start = Instant::now();
    // let mut last = Instant::now();
    // let mut counter = 0;
    loop {
        // if last.elapsed().as_secs_f64() < 0.125 {
        //     continue;
        // }
        // last = Instant::now();
        // Build lines
        // if button_a.is_low() {
        //     // Do something
        // }
        // if button_b.is_low() {
        //     break;
        // }

        let ip = get_ipv4();
        let load = get_loadavg();

        // IP line
        Text::new(&format!("IP : {}", ip), Point::new(0, big_char_h + small_char_h), small)
            .draw(&mut display).map_err(|_| AppError::Display)?;

        // Load line
        Text::new(&format!("Load: {}", load), Point::new(0, big_char_h + small_char_h * 2), small)
            .draw(&mut display).map_err(|_| AppError::Display)?;

        // display.flush().map_err(|_| AppError::Display)?;

        delay.delay_ms(1000u32);
    }
}

/// Noop `OutputPin` implementation.
///
/// This is passed to `ExclusiveDevice`, because the CS pin is handle in
/// hardware.
struct NoCs;

impl embedded_hal::digital::OutputPin for NoCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_hal::digital::ErrorType for NoCs {
    type Error = core::convert::Infallible;
}