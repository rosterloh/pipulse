use embedded_graphics::{geometry::Size, pixelcolor::Rgb565};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

pub const W: u32 = 240;
pub const H: u32 = 240;

pub struct SimSetup {
    pub display: SimulatorDisplay<Rgb565>,
    pub window: Window,
}

pub fn init() -> SimSetup {
    let display = SimulatorDisplay::<Rgb565>::new(Size::new(W, H));

    let output_settings = OutputSettingsBuilder::new().build();

    let window = Window::new("PiPulse - Simulator", &output_settings);

    SimSetup { display, window }
}
