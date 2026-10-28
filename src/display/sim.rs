//! SDL2 simulator that mimics the Adafruit Mini PiTFT 1.3": the 240×240 screen
//! is drawn inside a mock PCB with the two buttons beside it, scaled up with a
//! visible pixel grid on the LCD only.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, CornerRadii, PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Alignment, Text},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};

pub const W: u32 = 240;
pub const H: u32 = 240;

/// Default scale factor; override with `PIPULSE_SIM_SCALE` (1 disables the grid).
const DEFAULT_SCALE: u32 = 3;

// Board layout in display pixels (multiplied by the scale when drawn).
const MARGIN: u32 = 10;
const BUTTON_COL: u32 = 36;
const BOARD_W: u32 = MARGIN + BUTTON_COL + W + MARGIN;
const BOARD_H: u32 = MARGIN + H + MARGIN;
const SCREEN_X: u32 = MARGIN + BUTTON_COL;
const SCREEN_Y: u32 = MARGIN;
const BUTTON_D: u32 = 20;

const PCB: Rgb565 = Rgb565::new(3, 6, 3);
const BEZEL: Rgb565 = Rgb565::new(1, 2, 1);
const BUTTON: Rgb565 = Rgb565::new(10, 20, 10);
const BUTTON_PRESSED: Rgb565 = Rgb565::new(24, 48, 24);
const SILKSCREEN: Rgb565 = Rgb565::new(28, 56, 28);

/// The two buttons on the board, top to bottom.
#[derive(Clone, Copy)]
pub enum Button {
    /// Top button (GPIO 23) — previous page.
    A,
    /// Bottom button (GPIO 24) — next page.
    B,
}

/// The 240×240 screen that Ratatui renders into.
pub fn new_screen() -> SimulatorDisplay<Rgb565> {
    SimulatorDisplay::new(Size::new(W, H))
}

/// The simulator window showing the screen on a mock board.
pub struct Device {
    window: Window,
    frame: SimulatorDisplay<Rgb565>,
    scale: u32,
    pressed: [bool; 2],
}

impl Device {
    pub fn new() -> Self {
        let scale = std::env::var("PIPULSE_SIM_SCALE")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&s| s >= 1)
            .unwrap_or(DEFAULT_SCALE);
        // Scaling is done in `present` so the pixel grid only covers the LCD.
        let window = Window::new("PiPulse - Simulator", &OutputSettingsBuilder::new().build());
        let frame = SimulatorDisplay::new(Size::new(BOARD_W * scale, BOARD_H * scale));
        Self {
            window,
            frame,
            scale,
            pressed: [false; 2],
        }
    }

    pub fn set_pressed(&mut self, button: Button, pressed: bool) {
        self.pressed[button as usize] = pressed;
    }

    /// Composites `screen` onto the board and shows it in the window.
    pub fn present(&mut self, screen: &SimulatorDisplay<Rgb565>) {
        self.draw_board();
        self.draw_screen(screen);
        self.window.update(&self.frame);
    }

    /// Pending window events. Only valid after the first `present`, which
    /// creates the SDL window.
    pub fn events(&self) -> Vec<SimulatorEvent> {
        self.window.events().collect()
    }

    fn draw_board(&mut self) {
        let s = self.scale;
        let frame = &mut self.frame;
        frame.clear(Rgb565::BLACK).ok();
        RoundedRectangle::new(
            Rectangle::new(Point::zero(), Size::new(BOARD_W * s, BOARD_H * s)),
            CornerRadii::new(Size::new_equal(6 * s)),
        )
        .into_styled(PrimitiveStyle::with_fill(PCB))
        .draw(frame)
        .ok();
        // Black glass border around the active area
        let border = 2 * s;
        Rectangle::new(
            Point::new(
                (SCREEN_X * s - border) as i32,
                (SCREEN_Y * s - border) as i32,
            ),
            Size::new(W * s + 2 * border, H * s + 2 * border),
        )
        .into_styled(PrimitiveStyle::with_fill(BEZEL))
        .draw(frame)
        .ok();

        let font = if s >= 2 { &FONT_10X20 } else { &FONT_6X10 };
        let label_style = MonoTextStyle::new(font, SILKSCREEN);
        let cx = ((MARGIN + BUTTON_COL / 2) * s) as i32;
        for (i, (label, cy)) in [("A", H / 4), ("B", H * 3 / 4)].into_iter().enumerate() {
            let cy = ((SCREEN_Y + cy) * s) as i32;
            let fill = if self.pressed[i] {
                BUTTON_PRESSED
            } else {
                BUTTON
            };
            Circle::with_center(Point::new(cx, cy), BUTTON_D * s)
                .into_styled(PrimitiveStyle::with_fill(fill))
                .draw(frame)
                .ok();
            let label_y = cy + ((BUTTON_D / 2 + 4) * s) as i32 + font.character_size.height as i32;
            Text::with_alignment(
                label,
                Point::new(cx, label_y),
                label_style,
                Alignment::Center,
            )
            .draw(frame)
            .ok();
        }
    }

    /// Copies the screen onto the board, scaled. Each pixel is drawn as an
    /// (s-1)×(s-1) block with a dimmer 1 px gap, approximating the visible
    /// pixel grid of the real LCD.
    fn draw_screen(&mut self, screen: &SimulatorDisplay<Rgb565>) {
        let s = self.scale;
        let origin = Point::new((SCREEN_X * s) as i32, (SCREEN_Y * s) as i32);
        for p in screen.bounding_box().points() {
            let color = screen.get_pixel(p);
            let top_left = origin + p * s as i32;
            if s == 1 {
                Pixel(top_left, color).draw(&mut self.frame).ok();
                continue;
            }
            self.frame
                .fill_solid(&Rectangle::new(top_left, Size::new_equal(s)), dim(color))
                .ok();
            self.frame
                .fill_solid(&Rectangle::new(top_left, Size::new_equal(s - 1)), color)
                .ok();
        }
    }
}

/// Darkens a colour to ~40% for the gaps between LCD pixels.
fn dim(c: Rgb565) -> Rgb565 {
    Rgb565::new(c.r() * 2 / 5, c.g() * 2 / 5, c.b() * 2 / 5)
}
