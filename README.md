# PiPulse

Rust system-status display for the Raspberry Pi. Renders live metrics on an [Adafruit miniPiTFT](https://www.adafruit.com/product/4484) (ST7789, 240×240) using [Ratatui](https://ratatui.rs/) via the [mousefood](https://crates.io/crates/mousefood) embedded-graphics backend.

## Display

```
─ hostname ─────────────────────
CPU  42% ████████░░░░░░░░░░░░░░
Mem  61% ████████████░░░░░░░░░░
Dsk  28% █████░░░░░░░░░░░░░░░░░
────────────────────────────────
192.168.1.42
47.3°C
Up  2d 4h 12m
Ld  0.45 0.38 0.31
```

Gauge bars turn yellow at warn thresholds and red at critical (CPU: 50/80%, Mem: 60/85%, Disk: 70/90%). Temperature colours: green <55°C, yellow <70°C, red ≥70°C.

## Metrics

| Metric | Source |
|---|---|
| Hostname | `sysinfo` |
| IPv4 address | `sysinfo` networks (first non-virtual interface by name; skips loopback, docker, veth) |
| CPU usage % | `sysinfo`, delta between samples |
| Memory usage % | `sysinfo` used / total |
| Disk usage % | `sysinfo`, `/` mount |
| CPU temperature | `sysinfo` components, falling back to `/sys/class/thermal/thermal_zone0/temp` |
| Uptime | `sysinfo` |
| Load average (1/5/15 min) | `sysinfo` |
| Network rx/tx rate | `sysinfo` byte counters for the primary interface |

Updates once per second. The buttons switch between the Overview and Network pages.

## Build & run

```bash
# Type-check (fast feedback)
cargo check

# Run simulator locally (requires SDL2)
sudo apt install libsdl2-dev
cargo run --no-default-features --features sim

# Headless PNG snapshot (CI-friendly): the simulator saves the first frame and exits
EG_SIMULATOR_DUMP=/tmp/pipulse.png cargo run --no-default-features --features sim

# Build release binary
cargo build --release
```

### Simulator

The simulator shows the 240×240 screen on a mock Mini PiTFT board, scaled 3× with a visible pixel grid (`PIPULSE_SIM_SCALE=1`–`4` to change; `1` disables the grid). It samples once per second like the device.

| Key | Board button |
|---|---|
| `A` / `↑` / `←` | Top button (A): previous page |
| `B` / `↓` / `→` | Bottom button (B): next page |
| `Esc` | Quit |

## Install on the Pi (.deb)

```bash
# Install cargo-deb once
cargo install cargo-deb

# Build and package
cargo deb

# Or cross-compile from a dev machine (64-bit Pi)
rustup target add aarch64-unknown-linux-gnu
# apt install crossbuild-essential-arm64
cargo deb --target aarch64-unknown-linux-gnu

# Install on the Pi
sudo dpkg -i target/debian/pipulse_*.deb
```

The `.deb` installs `/usr/local/bin/pipulse` and a systemd unit that starts automatically. The service runs as an unprivileged dynamic user in the `spi` and `gpio` groups (no root or `CAP_SYS_RAWIO` required). If it fails to start, `journalctl -u pipulse` shows the underlying GPIO/SPI error.

## Feature flags

| Flag | Default | Effect |
|---|---|---|
| `hw` | yes | Real SPI/GPIO hardware path |
| `sim` | no | SDL2 simulator window |

`hw` and `sim` are mutually exclusive. Always pass `--no-default-features` when enabling `sim`.

## Hardware & wiring (Raspberry Pi → ST7789 SPI)

SPI0 on `/dev/spidev0.0`, DC=GPIO25, RST=NC, CE0 chip-select managed by kernel.

| Pi Pin | Signal |
| ---: | --- |
| GPIO11 (Pin 23) | SCLK |
| GPIO9  (Pin 21) | MISO |
| GPIO10 (Pin 19) | MOSI |
| GPIO8  (Pin 24) | TFT_CS |
| GPIO25 (Pin 22) | TFT_DC |
| GPIO22 (Pin 15) | BACKLIGHT |
| GPIO23 (Pin 16) | BUTTON A |
| GPIO24 (Pin 18) | BUTTON B |
| GND    (Pin 6)  | GND |
| 3V3    (Pin 1)  | VCC |
| 5V0    (Pin 2)  | V_BACKLIGHT |

> Enable SPI: `sudo raspi-config nonint do_spi 0`

## Links

- [Adafruit miniPiTFT pinouts](https://learn.adafruit.com/adafruit-mini-pitft-135x240-color-tft-add-on-for-raspberry-pi/pinouts)
- [Ratatui](https://ratatui.rs/)
- [mousefood — Ratatui embedded-graphics backend](https://crates.io/crates/mousefood)
- [embedded-graphics](https://docs.rs/embedded-graphics)
- [mipidsi — ST7789 driver](https://crates.io/crates/mipidsi)
