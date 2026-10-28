# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Check Commands

```bash
# Type-check (no binary produced) — fast feedback during development
cargo check                                  # hw (default)
cargo check --no-default-features --features sim  # simulator

# Build release binary for the Pi
cargo build --release                        # hw (default)

# Run the simulator locally (requires SDL2: sudo apt install libsdl2-dev)
cargo run --no-default-features --features sim

# Headless PNG snapshot — single frame then exits (used in CI)
EG_SIMULATOR_DUMP=/tmp/pipulse.png cargo run --no-default-features --features "sim,ci"

# Lint (CI runs these with -D warnings)
cargo fmt --check
cargo clippy
cargo clippy --no-default-features --features sim
cargo clippy --no-default-features --features sim,ci
```

There are no automated tests. `cargo check` + `cargo clippy` are the verification loop.

## Building a .deb Package

Packaging uses [`cargo-deb`](https://crates.io/crates/cargo-deb). Install it once with `cargo install cargo-deb`.

```bash
# Build and package in one step (native, e.g. on the Pi itself)
cargo deb

# Build the binary separately first, then package without rebuilding
cargo build --release
cargo deb --no-build

# Cross-compile for 64-bit Pi from a dev machine
# Requires: rustup target add aarch64-unknown-linux-gnu
#           apt install crossbuild-essential-arm64
#           .cargo/config.toml linker entry for aarch64
cargo deb --target aarch64-unknown-linux-gnu

# Install on the target machine
sudo dpkg -i target/debian/pipulse_*.deb
```

The `.deb` installs:
- `/usr/local/bin/pipulse` — the binary
- `/usr/lib/systemd/system/pipulse.service` — the unit file

`postinst` automatically enables and starts the service via `deb-systemd-helper`. `prerm`/`postrm` stop and clean up on removal. The `debian/` directory is intentionally empty — `cargo-deb` generates the maintainer scripts from its built-in systemd templates.

## Feature Flags

| Flag | Default | Effect |
|---|---|---|
| `hw` | yes | Enables `src/display/hw.rs` — real SPI/GPIO hardware path |
| `sim` | no | Enables `src/display/sim.rs` — SDL2 simulator window |
| `ci` | no | When combined with `sim`, exits after the first frame (used for headless PNG snapshots in CI) |

`hw` and `sim` are mutually exclusive at runtime (main.rs dispatches via `#[cfg]`). Always pass `--no-default-features` when enabling `sim`. Combine `ci` with `sim` for CI screenshot dumps: `--no-default-features --features "sim,ci"`.

## Architecture

The codebase is split into five files:

- **`src/metrics.rs`** — reads system data. `Metrics` holds long-lived `sysinfo` handles (`System`, `Networks`, `Disks`, `Components`) that are refreshed in place each tick; `Metrics::sample()` returns a `Snapshot` (IP/interface, hostname, CPU/mem/disk %, temperature, uptime, load average and network rx/tx rates). The primary interface is the first non-virtual one with an IPv4 address, in name order. No hardware dependency.

- **`src/ui.rs`** — builds the Ratatui layout and renders it. `render<B: Backend>(terminal, state)` is fully generic over the backend, so it works identically for both hw and sim. `AppState` borrows the network history rather than cloning it. To add a new metric: add a field to `Snapshot` and `AppState`, add a row to the page's `Layout`, and render a new `Paragraph`. No display code needed.

- **`src/input.rs`** (feature `hw`) — `ButtonReader` polls the two miniPiTFT buttons (GPIO 23/24, active-low) and reports edge-triggered `Prev`/`Next` events.

- **`src/display/`** — conditionally compiled display backends:
  - `hw.rs` (feature `hw`): opens `/dev/gpiochip0` (GPIO via `linux-embedded-hal::CdevPin`) and `/dev/spidev0.0` (SPI via `linux-embedded-hal::SpidevDevice`), then initialises the ST7789 via `mipidsi::Builder`. Returns `HwDisplay`, which owns the `mipidsi::Display` and the backlight `LineHandle` (the line is released if the handle is dropped).
  - `sim.rs` (feature `sim`): wraps `embedded_graphics_simulator::SimulatorDisplay<Rgb565>` + `Window` into a `SimSetup` struct.

- **`src/main.rs`** — wires everything together. `App` owns the metrics collector, current page and network history; `App::tick()` samples, renders and draws icons, and is shared by `run_hw()` and `run_sim()`. Both create one long-lived `Terminal`, so Ratatui only redraws changed cells. Icons are drawn straight onto the display via `terminal.backend_mut().display_mut()` after each draw; `App::set_page()` clears the terminal on a page change so stale icons don't survive on the new page. Errors are returned as `AppError` (with the underlying cause) and printed by `main()`.

### Key design decisions

- **`mousefood`** (`EmbeddedBackend`) is the bridge between Ratatui and `embedded-graphics`. It takes `&mut D` where `D: DrawTarget<Color = Rgb565>`, so both the real display and `SimulatorDisplay` satisfy it.
- **No `ExclusiveDevice`/`NoCs` needed**: `SpidevDevice` already implements `embedded_hal::spi::SpiDevice` directly; CE0 chip-select is managed by the kernel.
- **`Box::leak`** is used to give the 512-byte SPI buffer a `'static` lifetime, which `mipidsi::SpiInterface<'static, ...>` requires (so the display type is `'static`, satisfying the `D: 'static` bound on mousefood's `Backend` impl).
- **Hardware crates are optional**: `embedded-hal`, `linux-embedded-hal` and `mipidsi` are only compiled with the `hw` feature, and `linux-embedded-hal`/`sysinfo` have their unused default features disabled.
- The systemd unit (`scripts/pipulse.service`) runs as a `DynamicUser` with `SupplementaryGroups=spi gpio` and standard sandboxing; no root or `CAP_SYS_RAWIO` is needed.

## CI

- **CI** (`ci.yml`): `cargo fmt --check` and `cargo clippy` with `-D warnings` for `hw`, `sim` and `sim,ci`. Run the same locally before pushing.
- **Screenshots** (`screenshots.yml`): on PRs, renders before/after PNGs and comments links to them; on pushes to `main`, builds the sim to warm the cargo cache for future PRs.
