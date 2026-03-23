use embedded_hal::digital::InputPin;
use linux_embedded_hal::{
    CdevPin,
    gpio_cdev::{Chip, LineRequestFlags},
};

use crate::AppError;

const BTN_A: u32 = 23; // top button    → previous page (active-low)
const BTN_B: u32 = 24; // bottom button → next page (active-low)

pub enum ButtonEvent {
    None,
    Prev,
    Next,
}

pub struct ButtonReader {
    btn_a: CdevPin,
    btn_b: CdevPin,
    prev_a: bool,
    prev_b: bool,
}

impl ButtonReader {
    pub fn new() -> Result<Self, AppError> {
        let mut chip = Chip::new("/dev/gpiochip0").map_err(|_| AppError::Gpio)?;
        let btn_a = CdevPin::new(
            chip.get_line(BTN_A)
                .map_err(|_| AppError::Gpio)?
                .request(LineRequestFlags::INPUT, 0, "pipulse-btn-a")
                .map_err(|_| AppError::Gpio)?,
        )
        .map_err(|_| AppError::Gpio)?;
        let btn_b = CdevPin::new(
            chip.get_line(BTN_B)
                .map_err(|_| AppError::Gpio)?
                .request(LineRequestFlags::INPUT, 0, "pipulse-btn-b")
                .map_err(|_| AppError::Gpio)?,
        )
        .map_err(|_| AppError::Gpio)?;
        // Buttons are active-low; initialize previous state from actual pin levels
        // to avoid a spurious press event if a button is held at startup.
        let prev_a = btn_a.is_high().map_err(|_| AppError::Gpio)?;
        let prev_b = btn_b.is_high().map_err(|_| AppError::Gpio)?;
        Ok(Self {
            btn_a,
            btn_b,
            prev_a,
            prev_b,
        })
    }

    /// Returns an edge-triggered event on the falling edge (button pressed).
    pub fn poll(&mut self) -> Result<ButtonEvent, AppError> {
        let a = self.btn_a.is_high().map_err(|_| AppError::Gpio)?;
        let b = self.btn_b.is_high().map_err(|_| AppError::Gpio)?;
        let event = if !a && self.prev_a {
            ButtonEvent::Prev
        } else if !b && self.prev_b {
            ButtonEvent::Next
        } else {
            ButtonEvent::None
        };
        self.prev_a = a;
        self.prev_b = b;
        Ok(event)
    }
}
