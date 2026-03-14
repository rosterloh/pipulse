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

# Lint
cargo clippy
cargo clippy --no-default-features --features sim
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

The codebase is split into four concerns, each in its own file:

- **`src/metrics.rs`** — reads system data: `get_ipv4()` (filters non-virtual interfaces) and `get_loadavg()` (reads `/proc/loadavg`). No hardware dependency.

- **`src/ui.rs`** — builds the Ratatui layout and renders it. `render<B: Backend>(terminal, state)` is fully generic over the backend, so it works identically for both hw and sim. To add a new metric: add a field to `AppState`, add a `Constraint::Length(3)` row to the `Layout`, and render a new `Paragraph`. No display code needed.

- **`src/display/`** — conditionally compiled display backends:
  - `hw.rs` (feature `hw`): opens `/dev/gpiochip0` (GPIO via `linux-embedded-hal::CdevPin`) and `/dev/spidev0.0` (SPI via `linux-embedded-hal::SpidevDevice`), then initialises the ST7789 via `mipidsi::Builder`. Returns a concrete `mipidsi::Display` type.
  - `sim.rs` (feature `sim`): wraps `embedded_graphics_simulator::SimulatorDisplay<Rgb565>` + `Window` into a `SimSetup` struct.

- **`src/main.rs`** — wires everything together. `run_hw()` creates a long-lived `Terminal` (preserving Ratatui's frame-diff state). `run_sim()` uses a block scope each iteration to release the mutable borrow on `SimSetup.display` before calling `window.update(&display)`.

### Key design decisions

- **`mousefood`** (`EmbeddedBackend`) is the bridge between Ratatui and `embedded-graphics`. It takes `&mut D` where `D: DrawTarget<Color = Rgb565>`, so both the real display and `SimulatorDisplay` satisfy it.
- **No `ExclusiveDevice`/`NoCs` needed**: `SpidevDevice` already implements `embedded_hal::spi::SpiDevice` directly; CE0 chip-select is managed by the kernel.
- **`Box::leak`** is used to give the 512-byte SPI buffer a `'static` lifetime, which `mipidsi::SpiInterface<'static, ...>` requires (so the display type is `'static`, satisfying `EmbeddedBackend`'s `D: 'static` bound).
- The systemd unit (`scripts/pipulse.service`) runs without `CAP_SYS_RAWIO`; instead the service user needs membership in the `spi` and `gpio` groups.
