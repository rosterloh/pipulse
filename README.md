# PiPulse

Rust app for a Raspberry Pi that shows the device's IP address and a live system load average on an [Adafruit miniPiTFT](https://www.adafruit.com/product/4484).

## Features

* Reads IPv4 address from active, non-loopback interfaces
* Reads 1/5/15-minute load averages from `/proc/loadavg`
* Smooth text rendering using `embedded-graphics`
* Updates once per second

## Hardware & wiring (Raspberry Pi ➜ ST7789 SPI)

Assumes SPI0 on `/dev/spidev0.0` and **DC=GPIO25**, **RST=NC**. Chip Select uses CE0.

|          Pi Pin |   Signal    |
| --------------: | ----------- |
| GPIO11 (Pin 23) | SCLK        |
| GPIO11 (Pin 21) | MISO        |
| GPIO10 (Pin 19) | MOSI        |
| GPIO8  (Pin 24) | TFT_CS      |
| GPIO25 (Pin 22) | TFT_DC      |
| GPIO22 (Pin 15) | BACKLIGHT   |
| GPIO23 (Pin 16) | BUTTON A    |
| GPIO24 (Pin 18) | BUTTON B    |
|     GND (Pin 6) | GND         |
|     3V3 (Pin 1) | VCC         |
|     5V0 (Pin 2) | V_BACKLIGHT |

> Enable SPI with `sudo raspi-config` → *Interface Options* → *SPI* → **Enable** or just `sudo raspi-config nonint do_spi 0`

# Useful Links
- [Adafruit Learning Guide](https://learn.adafruit.com/adafruit-mini-pitft-135x240-color-tft-add-on-for-raspberry-pi/pinouts)
- [Embedded Graphics](https://docs.rs/embedded-graphics/latest/embedded_graphics/index.html)