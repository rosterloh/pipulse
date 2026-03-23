mod display;
#[cfg(all(feature = "hw", not(feature = "sim")))]
mod input;
mod metrics;
mod ui;

use std::collections::VecDeque;
use embedded_graphics::{image::Image, pixelcolor::Rgb565, prelude::*};
use embedded_icon::{
    NewIcon,
    mdi::size18px::{ClockOutline, Ethernet, Network, Server, Thermometer, Wifi},
};
use mousefood::prelude::*;
use ratatui::Terminal;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("GPIO error")]
    Gpio,
    #[error("SPI error")]
    Spi,
    #[error("Display error")]
    Display,
}

/// Height of one character row in pixels (matches mousefood's default font).
const CHAR_H: i32 = 10;

/// Number of network samples kept in history for the sparkline graphs.
const HISTORY_LEN: usize = 40;

/// Draw embedded-icon MDI icons at the pixel positions of rows 7-10.
///
/// Must be called *after* `terminal.draw()` has completed and the display
/// borrow has been released, so that the icons overwrite the leading spaces
/// that Ratatui filled with background colour.
///
/// Only call this on the Overview page — the icon positions are baked into
/// that layout.
fn draw_icons<D>(display: &mut D, state: &ui::AppState)
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

fn main() -> Result<(), AppError> {
    #[cfg(all(feature = "hw", not(feature = "sim")))]
    run_hw()?;

    #[cfg(feature = "sim")]
    run_sim();

    Ok(())
}

#[cfg(all(feature = "hw", not(feature = "sim")))]
fn run_hw() -> Result<(), AppError> {
    let mut display = display::hw::init()?;
    let mut cpu = metrics::CpuCollector::new();
    let mut net = metrics::NetSampler::new();
    let mut buttons = input::ButtonReader::new()?;
    let mut page = ui::Page::default();
    let mut rx_history: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);
    let mut tx_history: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);

    loop {
        // Handle button presses (edge-triggered, active-low)
        match buttons.poll() {
            input::ButtonEvent::Next => page = page.next(),
            input::ButtonEvent::Prev => page = page.prev(),
            input::ButtonEvent::None => {}
        }

        // Update network history ring buffers
        let (rx, tx) = net.sample();
        if rx_history.len() >= HISTORY_LEN {
            rx_history.pop_front();
        }
        rx_history.push_back(rx);
        if tx_history.len() >= HISTORY_LEN {
            tx_history.pop_front();
        }
        tx_history.push_back(tx);

        let (ip, iface) = metrics::get_net_info();
        let state = ui::AppState {
            ip,
            iface,
            hostname: metrics::get_hostname(),
            cpu_pct: cpu.sample(),
            mem_pct: metrics::get_memory_percent(),
            disk_pct: metrics::get_disk_percent(),
            temp: metrics::get_cpu_temp(),
            uptime: metrics::get_uptime_str(),
            load: metrics::get_loadavg(),
            page,
            rx_history: rx_history.clone(),
            tx_history: tx_history.clone(),
        };

        {
            // Terminal is scoped so the mutable borrow of `display` is
            // released before we draw icons directly onto it.
            let backend = EmbeddedBackend::new(&mut display, EmbeddedBackendConfig::default());
            let mut terminal = Terminal::new(backend).unwrap();
            ui::render(&mut terminal, &state);
        }

        if state.page == ui::Page::Overview {
            draw_icons(&mut display, &state);
        }

        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(feature = "sim")]
fn run_sim() {
    use embedded_graphics_simulator::SimulatorEvent;

    let mut setup = display::sim::init();
    let mut cpu = metrics::CpuCollector::new();
    let mut net = metrics::NetSampler::new();
    let mut page = ui::Page::default();
    let mut rx_history: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);
    let mut tx_history: VecDeque<u64> = VecDeque::with_capacity(HISTORY_LEN);

    loop {
        // Update network history ring buffers
        let (rx, tx) = net.sample();
        if rx_history.len() >= HISTORY_LEN {
            rx_history.pop_front();
        }
        rx_history.push_back(rx);
        if tx_history.len() >= HISTORY_LEN {
            tx_history.pop_front();
        }
        tx_history.push_back(tx);

        let (ip, iface) = metrics::get_net_info();
        let state = ui::AppState {
            ip,
            iface,
            hostname: metrics::get_hostname(),
            cpu_pct: cpu.sample(),
            mem_pct: metrics::get_memory_percent(),
            disk_pct: metrics::get_disk_percent(),
            temp: metrics::get_cpu_temp(),
            uptime: metrics::get_uptime_str(),
            load: metrics::get_loadavg(),
            page,
            rx_history: rx_history.clone(),
            tx_history: tx_history.clone(),
        };

        {
            let backend =
                EmbeddedBackend::new(&mut setup.display, EmbeddedBackendConfig::default());
            let mut terminal = Terminal::new(backend).unwrap();
            ui::render(&mut terminal, &state);
        }

        if state.page == ui::Page::Overview {
            draw_icons(&mut setup.display, &state);
        }

        setup.window.update(&setup.display);

        #[cfg(feature = "ci")]
        break;

        let mut should_quit = false;
        for event in setup.window.events() {
            match event {
                SimulatorEvent::Quit => should_quit = true,
                SimulatorEvent::KeyDown { keycode, .. } => {
                    use embedded_graphics_simulator::sdl2::keyboard::Keycode;
                    match keycode {
                        Keycode::Right | Keycode::D => page = page.next(),
                        Keycode::Left | Keycode::A => page = page.prev(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        if should_quit {
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
