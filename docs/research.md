# PiPulse — Deep Research Document

## Overview

PiPulse is a minimal Rust application that runs on a Raspberry Pi and drives an **Adafruit miniPiTFT** (ST7789-based, 240×240 SPI TFT display). It renders two live system metrics every second: the device's IPv4 address and the kernel load average (1/5/15 minute). It is designed to be deployed as a `systemd` service so it starts on boot and restarts on failure.

---

## Hardware

### Display

| Attribute | Value |
|---|---|
| Controller | ST7789 |
| Panel size | 240 × 240 pixels |
| Color depth | RGB565 (16-bit) |
| Interface | SPI (SPI0, CE0) |
| Product | Adafruit miniPiTFT 1.3" (#4484) |

### GPIO Pin Assignments

| Constant | GPIO BCM | Board Pin | Function |
|---|---|---|---|
| `SPI_DC` | 25 | Pin 22 | Data/Command select |
| `BACKLIGHT` | 22 | Pin 15 | Backlight PWM control |
| `BUTTON_A` | 23 | Pin 16 | Push button A (pull-up) |
| `BUTTON_B` | 24 | Pin 18 | Push button B (pull-up) |
| — | 8 (CE0) | Pin 24 | Chip Select (hardware-managed) |
| — | 11 (SCLK) | Pin 23 | SPI clock |
| — | 10 (MOSI) | Pin 19 | SPI data out |

SPI bus speed is set to **60 MHz**.

The **Reset pin is left unconnected** (NC). Software reset is handled by the `mipidsi` driver during `Builder::init()`.

---

## Software Architecture

### Single-file design

The entire application lives in `src/main.rs` (~180 lines). There are no modules, no sub-crates, no async runtime. The logic is intentionally flat:

1. GPIO and SPI setup
2. Display initialisation
3. Draw static header once
4. Enter an infinite loop: read metrics → draw metrics → sleep 1 s

### Crate Edition

`Cargo.toml` declares `edition = "2024"` (Rust 2024 edition).

---

## Dependencies

| Crate | Version | Role |
|---|---|---|
| `rppal` | 0.22 (with `hal` feature) | Raspberry Pi GPIO, SPI, PWM via `/dev/mem` & `/dev/spidev` |
| `mipidsi` | 0.9.0 | High-level ST7789 display driver (init, clear, pixel write) |
| `embedded-graphics` | 0.8 | 2D graphics primitives and text rendering |
| `embedded-hal` | 1.0.0 | Hardware abstraction layer traits |
| `embedded-hal-bus` | 0.3.0 | SPI bus sharing utilities (`ExclusiveDevice`) |
| `display-interface-spi` | 0.5 | Low-level SPI display interface trait (declared but unused at runtime — see notes) |
| `linux-embedded-hal` | 0.4 | Linux implementations of `embedded-hal` traits (provides `Delay`) |
| `get_if_addrs` | 0.5 | Lists network interfaces and their IP addresses |
| `thiserror` | 2.0 | Derive macro for ergonomic error types |

### Noteworthy dependency detail: `display-interface-spi`

This crate is listed in `Cargo.toml` and its import is visible as a commented-out `use` at the top of `main.rs`. The current code uses `mipidsi::interface::SpiInterface` directly, making `display-interface-spi` effectively a dead dependency from a prior iteration. It is still compiled in but none of its symbols are used at runtime.

---

## SPI Interface Construction

The chain of types used to wire SPI → display is:

```
rppal::spi::Spi   (raw Linux SPI)
  └─ ExclusiveDevice<Spi, NoCs>   (wraps Spi + a no-op CS pin)
       └─ SpiInterface<ExclusiveDevice<Spi, NoCs>, OutputPin, &mut [u8; 512]>
            └─ mipidsi::Builder<ST7789, SpiInterface<...>>
                 └─ Display<ST7789, SpiInterface<...>>
```

### The `NoCs` struct

Because the Adafruit miniPiTFT ties CS to the hardware CE0 line (managed automatically by the SPI peripheral), no software CS pin is needed. The code defines a local `NoCs` unit struct that implements `embedded_hal::digital::OutputPin` as a no-op. Both `set_low()` and `set_high()` return `Ok(())` immediately. `ErrorType` is `core::convert::Infallible`.

### SPI buffer

A stack-allocated `[u8; 512]` buffer is passed to `SpiInterface`. This is the write buffer used by `mipidsi` to batch pixel data before it is sent over SPI.

---

## Display Initialisation Sequence

```rust
Builder::new(ST7789, di)
    .display_size(240, 240)
    .invert_colors(ColorInversion::Inverted)
    .init(&mut delay)
```

`ColorInversion::Inverted` is required because the ST7789 on the Adafruit board ships with colour inversion on by default; without this flag all colours appear inverted.

After init, `display.clear(Rgb565::BLACK)` fills the screen once. This avoids garbage pixels from uninitialised GRAM.

---

## Backlight Control

The backlight is controlled via **hardware PWM** on GPIO 22:

```rust
backlight.set_pwm_frequency(50., 0.5)
```

- Frequency: **50 Hz**
- Duty cycle: **50%** (half brightness)

There is a commented-out alternative `backlight.set_high()` for full brightness with no PWM. The current choice of 50% is presumably a power/heat trade-off for continuous operation.

`CAP_SYS_RAWIO` in the systemd unit is required for `rppal` to access `/dev/mem` to configure the PWM hardware registers.

---

## Text Rendering

Two font sizes from `embedded-graphics`'s built-in ASCII bitmap fonts are used:

| Style variable | Font constant | Glyph size | Usage |
|---|---|---|---|
| `big` | `FONT_10X20` | 10 × 20 px | "PiPulse" header |
| `small` | `FONT_8X13` | 8 × 13 px | IP and load lines |

Font colours are `Rgb565::WHITE` on a `Rgb565::BLACK` background.

### Layout geometry

All layout is computed from the character dimensions in pixels:

```
Y positions (top of character cell, increasing downward):
  Header "PiPulse":   y = big_char_h           = 20
  IP line:            y = big_char_h + small_char_h     = 20 + 13 = 33
  Load line:          y = big_char_h + small_char_h * 2 = 20 + 26 = 46
```

The header is horizontally centred:
```
header_x = (240 / 2) - (len("PiPulse") * 10 / 2) = 120 - 35 = 85
```
Then drawn with `Alignment::Center` (redundant given the manual calculation, but harmless).

The IP and load lines are left-aligned at `x = 0`.

**The header is drawn only once, before the loop.** Metric lines are redrawn every iteration directly over the previous frame. Because the text is always drawn at fixed positions with a fixed-width font and the background is never explicitly cleared between frames, this works as long as the text content never gets shorter than the previous frame (otherwise old characters would ghost). In practice:
- The IP string is stable (same length after the initial connection).
- The load average has a fixed format (`"? ? ?"` or three floats).

---

## Main Loop

```
loop {
    get IPv4 address
    get load average
    draw IP text
    draw load text
    delay 1000 ms
}
```

Update rate: **1 Hz** (1-second blocking sleep via `delay.delay_ms(1000u32)`).

### Commented-out rate-limiting code

There is commented-out code that would gate the loop body to **8 Hz** (every 125 ms):

```rust
// if last.elapsed().as_secs_f64() < 0.125 { continue; }
```

This suggests the developer experimented with faster refresh before settling on the 1 s sleep.

### Commented-out button handling

Button A and B pins are configured as inputs with pull-ups but their values are never read. Commented-out stubs show the intended use:

```rust
// if button_a.is_low() { /* Do something */ }
// if button_b.is_low() { break; }
```

Button B was intended to break out of the loop (exit the program). Neither button does anything currently.

---

## System Metric Collection

### IPv4 address (`get_ipv4`)

Uses `get_if_addrs::get_if_addrs()` to enumerate all network interfaces. The selection logic:

1. Exclude loopback interfaces (`is_loopback()`).
2. Keep only IPv4 addresses.
3. Exclude interfaces whose names start with `docker`, `veth`, or `lo` (container/virtual interfaces).
4. Return the IP of the first remaining interface as a string.
5. Falls back to `"no ip"` if none found or on error.

This is a `find`, not a `fold`, so it returns the **first** matching interface in enumeration order (which is typically the primary physical or Wi-Fi interface on a standard Pi setup).

### Load average (`get_loadavg`)

Reads `/proc/loadavg` directly via `fs::read_to_string`. The file format is:
```
<1min> <5min> <15min> <running/total> <last-pid>
```

Only the first three whitespace-delimited tokens (1, 5, 15 minute averages) are extracted and joined with spaces. Falls back to `"? ? ?"` on read error.

---

## Error Handling

```rust
enum AppError {
    Spi(rppal::spi::Error),  // maps from rppal SPI errors
    Gpio,                    // used for all GPIO failures
    Display,                 // used for all display failures
}
```

`main()` returns `Result<(), AppError>`. Fatal errors during setup (GPIO init, SPI open, display init) propagate up and terminate the process. Inside the loop, display draw errors use `map_err(|_| AppError::Display)?` — any draw error will also terminate the loop and exit the process, which the systemd `Restart=on-failure` policy will then restart.

---

## Systemd Service

File: `scripts/pipulse.service`

| Field | Value | Notes |
|---|---|---|
| `After` / `Wants` | `network-online.target` | Ensures network is up before starting (so the IP address is readable) |
| `Type` | `simple` | Process is considered started as soon as exec'd |
| `ExecStart` | `/usr/local/bin/pipulse` | Binary must be installed here manually |
| `Restart` | `on-failure` | Auto-restart if the process exits non-zero |
| `AmbientCapabilities` | `CAP_SYS_RAWIO` | Required by `rppal` for direct hardware register access (PWM, SPI) |
| `WantedBy` | `multi-user.target` | Starts in normal multi-user boot |

There is no `User=` directive, so the service runs as **root** by default.

---

## Known Limitations / Open TODOs (from the code)

1. **Ghost pixels**: Metrics are drawn over old text without clearing the region first. If a value ever shrinks in string length, stale characters will remain on screen.
2. **Buttons not implemented**: A and B buttons are wired but do nothing.
3. **No display flush**: `display.flush()` is commented out. For the ST7789 with `mipidsi`, each `draw()` call writes directly to the hardware so this is fine, but the flush line suggests uncertainty about the driver model.
4. **Dead dependency**: `display-interface-spi` is listed in `Cargo.toml` and its import is commented out in source. It adds compile weight with no runtime benefit.
5. **Single interface assumption**: `get_ipv4` returns only the first non-virtual IPv4. On a Pi with both Ethernet and Wi-Fi active, which one appears first depends on OS enumeration order.
6. **No cross-compilation setup**: The project has no `.cargo/config.toml` with a cross-compilation target. Building requires either native compilation on a Pi or a manually configured cross toolchain.

---

## File Structure

```
pipulse/
├── Cargo.toml               # Package manifest (edition 2024)
├── Cargo.lock               # Locked dependency tree
├── README.md                # Hardware wiring table & setup instructions
├── src/
│   └── main.rs              # Entire application (~180 lines)
└── scripts/
    └── pipulse.service      # systemd unit for deployment
```

---

## Build & Deploy

No build script exists in the repo. Implied workflow based on README and service file:

```bash
# On the Pi (native build):
cargo build --release

# Install binary:
sudo cp target/release/pipulse /usr/local/bin/

# Install and enable service:
sudo cp scripts/pipulse.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now pipulse
```

SPI must be enabled first: `sudo raspi-config nonint do_spi 0`
