mod display;
mod metrics;
mod ui;

use std::time::Duration;
use mousefood::prelude::*;
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
    let backend = EmbeddedBackend::new(&mut display, EmbeddedBackendConfig::default());
    let mut terminal = Terminal::new(backend).unwrap();

    loop {
        let state = ui::AppState {
            ip: metrics::get_ipv4(),
            load: metrics::get_loadavg(),
        };
        ui::render(&mut terminal, &state);
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(feature = "sim")]
fn run_sim() {
    use embedded_graphics_simulator::SimulatorEvent;

    let mut setup = display::sim::init();

    loop {
        {
            let backend = EmbeddedBackend::new(&mut setup.display, EmbeddedBackendConfig::default());
            let mut terminal = Terminal::new(backend).unwrap();
            let state = ui::AppState {
                ip: metrics::get_ipv4(),
                load: metrics::get_loadavg(),
            };
            ui::render(&mut terminal, &state);
        }

        setup.window.update(&setup.display);

        if setup.window.events().any(|e| e == SimulatorEvent::Quit) {
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
