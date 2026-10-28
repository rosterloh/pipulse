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
use mousefood::prelude::*;
use ratatui::Terminal;
use std::collections::VecDeque;
use std::process::ExitCode;
#[cfg(all(feature = "hw", not(feature = "sim")))]
use std::time::Duration;
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

/// Height of one character row in pixels (matches mousefood's default font).
const CHAR_H: i32 = 10;

/// Number of network samples kept in history for the sparkline graphs.
const HISTORY_LEN: usize = 40;

/// Draw embedded-icon MDI icons at the pixel positions of rows 7-10.
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
    // Rows 0-6 each occupy CHAR_H pixels. Rows 7-10 are 2*CHAR_H each.
    let base_y = 7 * CHAR_H;
    let row_h = 2 * CHAR_H;
    let icon_h = 18_i32;
    let offset = (row_h - icon_h) / 2; // vertical centering within 2-char row

    let white = Rgb565::WHITE;
    let gray = Rgb565::new(15, 30, 15);

    // Row 7 — network icon varies by interface type
    let y0 = base_y + offset;
    if state.iface.starts_with("wlan") {
        Image::new(&Wifi::new(white), Point::new(0, y0))
            .draw(display)
            .ok();
    } else if state.iface.starts_with("eth") {
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
    let temp_col = state.temp.map_or(gray, |t| {
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
            ip: m.ip,
            iface: m.iface,
            hostname: m.hostname,
            cpu_pct: m.cpu_pct,
            mem_pct: m.mem_pct,
            disk_pct: m.disk_pct,
            temp: m.temp,
            uptime: m.uptime,
            load: m.load,
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
    Terminal::new(EmbeddedBackend::new(
        display,
        EmbeddedBackendConfig::default(),
    ))
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

    loop {
        // Handle button presses (edge-triggered, active-low)
        match buttons.poll()? {
            input::ButtonEvent::Next => app.set_page(app.page.next(), &mut terminal)?,
            input::ButtonEvent::Prev => app.set_page(app.page.prev(), &mut terminal)?,
            input::ButtonEvent::None => {}
        }

        app.tick(&mut terminal)?;

        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(feature = "sim")]
fn run_sim() -> Result<(), AppError> {
    use embedded_graphics_simulator::{SimulatorEvent, sdl2::Keycode};

    let mut setup = display::sim::init();
    let mut terminal = new_terminal(&mut setup.display)?;
    let mut app = App::new();

    loop {
        app.tick(&mut terminal)?;
        setup.window.update(terminal.backend().display());

        if cfg!(feature = "ci") {
            return Ok(());
        }

        for event in setup.window.events() {
            match event {
                SimulatorEvent::Quit => return Ok(()),
                SimulatorEvent::KeyDown { keycode, .. } => match keycode {
                    Keycode::Right | Keycode::D => app.set_page(app.page.next(), &mut terminal)?,
                    Keycode::Left | Keycode::A => app.set_page(app.page.prev(), &mut terminal)?,
                    _ => {}
                },
                _ => {}
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
