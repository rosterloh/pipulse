mod display;
#[cfg(all(feature = "hw", not(feature = "sim")))]
mod input;
mod metrics;
mod ui;

use embedded_graphics::{image::Image, pixelcolor::Rgb565, prelude::*};
use embedded_icon::{
    NewIcon,
    mdi::size18px::{ClockOutline, Ethernet, Network, Server, Thermometer, Wifi},
};
use mousefood::{fonts, prelude::*};
use ratatui::Terminal;
use std::collections::VecDeque;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[cfg(feature = "hw")]
    #[error("GPIO error: {0}")]
    Gpio(String),
    #[cfg(feature = "hw")]
    #[error("SPI error: {0}")]
    Spi(String),
    #[error("display error: {0}")]
    Display(String),
}

/// Height of one character row in pixels (matches the 7x13 font set in
/// `new_terminal`).
const CHAR_H: i32 = 13;

/// How often metrics are sampled and the screen redrawn.
const TICK: Duration = Duration::from_secs(1);

/// How often input is polled between ticks, so short button presses aren't
/// missed and page changes show immediately.
const POLL: Duration = Duration::from_millis(20);

/// Number of network samples kept in history for the sparkline graphs.
const HISTORY_LEN: usize = 40;

/// Draw embedded-icon MDI icons beside the Overview info rows
/// (`ui::OVERVIEW_ICON_ROW` onwards).
///
/// Must be called *after* `terminal.draw()` has completed, so that the icons
/// overwrite the leading spaces that Ratatui filled with background colour.
///
/// Only call this on the Overview page — the icon positions are baked into
/// that layout.
fn draw_icons<D>(display: &mut D, state: &ui::AppState<'_>)
where
    D: DrawTarget<Color = Rgb565>,
{
    // Rows above the info rows each occupy CHAR_H pixels; info rows are 2*CHAR_H.
    let base_y = ui::OVERVIEW_ICON_ROW * CHAR_H;
    let row_h = 2 * CHAR_H;
    let icon_h = 18_i32;
    let offset = (row_h - icon_h) / 2; // vertical centering within 2-char row

    let white = Rgb565::WHITE;
    let gray = Rgb565::new(15, 30, 15);

    // Row 7 — network icon varies by interface type
    let y0 = base_y + offset;
    if state.snap.iface.starts_with("wlan") {
        Image::new(&Wifi::new(white), Point::new(0, y0))
            .draw(display)
            .ok();
    } else if state.snap.iface.starts_with("eth") {
        Image::new(&Ethernet::new(white), Point::new(0, y0))
            .draw(display)
            .ok();
    } else {
        Image::new(&Network::new(white), Point::new(0, y0))
            .draw(display)
            .ok();
    }

    // Row 8 — thermometer, colour-coded like the text
    let y1 = base_y + row_h + offset;
    let temp_col = state.snap.temp.map_or(gray, |t| {
        if t >= 70.0 {
            Rgb565::new(26, 22, 10) // ~(210, 90, 80)
        } else if t >= 55.0 {
            Rgb565::new(26, 42, 8) // ~(210, 170, 70)
        } else {
            Rgb565::new(12, 46, 12) // ~(100, 185, 100)
        }
    });
    Image::new(&Thermometer::new(temp_col), Point::new(0, y1))
        .draw(display)
        .ok();

    // Row 9 — clock for uptime
    let y2 = base_y + 2 * row_h + offset;
    Image::new(&ClockOutline::new(gray), Point::new(0, y2))
        .draw(display)
        .ok();

    // Row 10 — server icon for load average
    let y3 = base_y + 3 * row_h + offset;
    Image::new(&Server::new(gray), Point::new(0, y3))
        .draw(display)
        .ok();
}

/// Metrics collection and page state shared by the hw and sim run loops.
struct App {
    metrics: metrics::Metrics,
    page: ui::Page,
    rx_history: VecDeque<u64>,
    tx_history: VecDeque<u64>,
}

impl App {
    fn new() -> Self {
        Self {
            metrics: metrics::Metrics::new(),
            page: ui::Page::default(),
            rx_history: VecDeque::with_capacity(HISTORY_LEN),
            tx_history: VecDeque::with_capacity(HISTORY_LEN),
        }
    }

    /// Samples all metrics and draws the current page, then the icons on top.
    fn tick<D>(
        &mut self,
        terminal: &mut Terminal<EmbeddedBackend<'_, D, Rgb565>>,
    ) -> Result<(), AppError>
    where
        D: DrawTarget<Color = Rgb565> + 'static,
    {
        let m = self.metrics.sample();
        push_bounded(&mut self.rx_history, m.rx_bps);
        push_bounded(&mut self.tx_history, m.tx_bps);

        let state = ui::AppState {
            snap: m,
            page: self.page,
            rx_history: &self.rx_history,
            tx_history: &self.tx_history,
        };

        ui::render(terminal, &state).map_err(|e| AppError::Display(format!("{e:?}")))?;
        if state.page == ui::Page::Overview {
            draw_icons(terminal.backend_mut().display_mut(), &state);
        }
        Ok(())
    }

    /// Switches page. The terminal is cleared so the next draw repaints every
    /// cell: Ratatui only redraws changed cells, so icons drawn directly onto
    /// the display would otherwise linger over the blank cells of other pages.
    fn set_page<D>(
        &mut self,
        page: ui::Page,
        terminal: &mut Terminal<EmbeddedBackend<'_, D, Rgb565>>,
    ) -> Result<(), AppError>
    where
        D: DrawTarget<Color = Rgb565> + 'static,
    {
        if page != self.page {
            self.page = page;
            terminal
                .clear()
                .map_err(|e| AppError::Display(format!("{e:?}")))?;
        }
        Ok(())
    }
}

fn push_bounded(history: &mut VecDeque<u64>, value: u64) {
    if history.len() >= HISTORY_LEN {
        history.pop_front();
    }
    history.push_back(value);
}

fn new_terminal<D>(display: &mut D) -> Result<Terminal<EmbeddedBackend<'_, D, Rgb565>>, AppError>
where
    D: DrawTarget<Color = Rgb565> + 'static,
{
    let config = EmbeddedBackendConfig {
        font_regular: fonts::mono_7x13_atlas(),
        font_bold: Some(fonts::mono_7x13_bold_atlas()),
        ..Default::default()
    };
    Terminal::new(EmbeddedBackend::new(display, config))
        .map_err(|e| AppError::Display(format!("{e:?}")))
}

fn main() -> ExitCode {
    #[cfg(all(feature = "hw", not(feature = "sim")))]
    let result = run_hw();

    #[cfg(feature = "sim")]
    let result = run_sim();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pipulse: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(feature = "hw", not(feature = "sim")))]
fn run_hw() -> Result<(), AppError> {
    let mut hw = display::hw::init()?;
    let mut terminal = new_terminal(&mut hw.display)?;
    let mut buttons = input::ButtonReader::new()?;
    let mut app = App::new();
    let mut next_tick = Instant::now();

    loop {
        // Handle button presses (edge-triggered, active-low); a page change
        // redraws straight away rather than waiting for the next tick.
        let page = match buttons.poll()? {
            input::ButtonEvent::Next => Some(app.page.next()),
            input::ButtonEvent::Prev => Some(app.page.prev()),
            input::ButtonEvent::None => None,
        };
        if let Some(page) = page {
            app.set_page(page, &mut terminal)?;
            next_tick = Instant::now();
        }

        if Instant::now() >= next_tick {
            app.tick(&mut terminal)?;
            next_tick = Instant::now() + TICK;
        }

        std::thread::sleep(POLL);
    }
}

#[cfg(feature = "sim")]
fn run_sim() -> Result<(), AppError> {
    use display::sim::Button;
    use embedded_graphics_simulator::{SimulatorEvent, sdl2::Keycode};

    // Keys mirror the board: A/Up = top button (previous page),
    // B/Down = bottom button (next page). Left/Right also work.
    fn button(keycode: Keycode) -> Option<Button> {
        match keycode {
            Keycode::A | Keycode::Up | Keycode::Left => Some(Button::A),
            Keycode::B | Keycode::Down | Keycode::Right => Some(Button::B),
            _ => None,
        }
    }

    let mut screen = display::sim::new_screen();
    let mut terminal = new_terminal(&mut screen)?;
    let mut device = display::sim::Device::new();
    let mut app = App::new();
    let mut next_tick = Instant::now();
    let mut dirty = false;

    loop {
        // Same cadence as the hardware: sample once per tick, poll input often
        if Instant::now() >= next_tick {
            app.tick(&mut terminal)?;
            next_tick = Instant::now() + TICK;
            dirty = true;
        }
        if dirty {
            device.present(terminal.backend().display());
            dirty = false;
        }

        for event in device.events() {
            match event {
                SimulatorEvent::Quit => return Ok(()),
                SimulatorEvent::KeyDown {
                    keycode,
                    repeat: false,
                    ..
                } => {
                    if let Some(b) = button(keycode) {
                        device.set_pressed(b, true);
                        let page = match b {
                            Button::A => app.page.prev(),
                            Button::B => app.page.next(),
                        };
                        app.set_page(page, &mut terminal)?;
                        next_tick = Instant::now();
                    }
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    if let Some(b) = button(keycode) {
                        device.set_pressed(b, false);
                        dirty = true;
                    }
                }
                _ => {}
            }
        }

        std::thread::sleep(POLL);
    }
}
