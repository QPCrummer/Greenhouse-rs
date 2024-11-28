greenhouse-rs
=============

Rust implementation for a Greenhouse monitor for the _Arduino Uno R3_.

## Features
- Integrated LCD 1602 Module
- Fire suppression (if smoke detector is installed)
- Automatic watering schedule
- Temperature and Humidity thresholds
- Intricate Date and Time system
- Integrated Alarm for emergencies
- Low memory footprint

## Build Instructions
1. Install prerequisites as described in the [`avr-hal` README] (`avr-gcc`, `avr-libc`, `avrdude`, [`ravedude`]).

2. Run `cargo build` to build the firmware.

3. Run `cargo run` to flash the firmware to a connected board.  If `ravedude`
   fails to detect your board, check its documentation at
   <https://crates.io/crates/ravedude>.

4. `ravedude` will open a console session after flashing where you can interact
   with the UART console of your board.

[`avr-hal` README]: https://github.com/Rahix/avr-hal#readme
[`ravedude`]: https://crates.io/crates/ravedude

## License
Licensed under the [MIT license](LICENSE-MIT)
