use embedded_hal::digital::InputPin;
use linux_embedded_hal::{
    CdevPin, CdevPinError,
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
        let mut chip = Chip::new("/dev/gpiochip0")
            .map_err(|e| AppError::Gpio(format!("/dev/gpiochip0: {e}")))?;
        let mut request = |line: u32, label: &str| {
            let handle = chip
                .get_line(line)
                .and_then(|l| l.request(LineRequestFlags::INPUT, 0, label))
                .map_err(|e| AppError::Gpio(format!("button line {line}: {e}")))?;
            CdevPin::new(handle).map_err(|e| AppError::Gpio(format!("button line {line}: {e}")))
        };
        let mut btn_a = request(BTN_A, "pipulse-btn-a")?;
        let mut btn_b = request(BTN_B, "pipulse-btn-b")?;
        // Buttons are active-low; initialize previous state from actual pin levels
        // to avoid a spurious press event if a button is held at startup.
        let prev_a = btn_a.is_high().map_err(read_err(BTN_A))?;
        let prev_b = btn_b.is_high().map_err(read_err(BTN_B))?;
        Ok(Self {
            btn_a,
            btn_b,
            prev_a,
            prev_b,
        })
    }

    /// Returns an edge-triggered event on the falling edge (button pressed).
    pub fn poll(&mut self) -> Result<ButtonEvent, AppError> {
        let a = self.btn_a.is_high().map_err(read_err(BTN_A))?;
        let b = self.btn_b.is_high().map_err(read_err(BTN_B))?;
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

fn read_err(line: u32) -> impl FnOnce(CdevPinError) -> AppError {
    move |e| AppError::Gpio(format!("reading button line {line}: {e}"))
}
