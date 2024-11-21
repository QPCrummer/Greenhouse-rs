#![no_std]
#![no_main]

use core::ptr::null;
use core::time::Duration;
use arduino_hal::{DefaultClock, Delay, I2c, Peripherals, Pins};
use arduino_hal::adc::channel::Gnd;
use arduino_hal::hal::port::{PB3, PB4, PD2, PD3, PD4, PD5};
use arduino_hal::pac::TWI;
use arduino_hal::port::mode::{Output};
use arduino_hal::port::Pin;
use arduino_hal::prelude::_embedded_hal_timer_CountDown;
use bme680::{Bme680, I2CAddress, IIRFilterSize, OversamplingSetting, PowerMode, SettingsBuilder};
use lcd1602_driver::command::DataWidth;
use lcd1602_driver::lcd;
use lcd1602_driver::lcd::Lcd;
use lcd1602_driver::sender::ParallelSender;
use panic_halt as _;

// How to flash arduino: https://github.com/creativcoder/rust-arduino-blink
/// Pin out for our project
///
/// LCD 1602: # Character Display
///     VSS: GND
///     VDD: 5V
///     V0: 3.3V (Contrast)
///     RS: P2
///     RW: GND
///     E: P3
///     A: 5V
///     K: GND
///     D4: P4
///     D5: P5
///     D6: P6
///     D7: P7
///
/// BME680: # Temperature, Humidity, and Air Pressure Sensor
///     Vin: 5V
///     GND: GND
///     SCK: A5
///     SDI: A4
///
/// Buzzer: # Audial alert
///     +: P9 (PWM)
///     -: GND
///
/// Smoke Detector: # Enables Sprinklers upon Smoke Detection
///     +: P8
///
/// Button Up: # Goes Up Screen/Element
///     +: 3.3V
///     -: A0
///
/// Button Down: # Goes Down Screen/Element
///     +: 3.3V
///     -: A1
///
/// Button Select: # Selects Element
///     +: 3.3V
///     -: A2
///
/// Sprinklers:
///     +: P1
///     -: GND

#[arduino_hal::entry]
fn main() -> ! {
    let dp = Peripherals::take().unwrap();
    let twi = dp.TWI;
    let pins = arduino_hal::pins!(dp);
    let mut components = setup(pins, twi);


    loop {
        arduino_hal::delay_ms(1000);
    }
}

fn setup(
    pins: Pins,
    twi: TWI,
) -> (Bme680<I2c, Delay>, Lcd<'static, 'static, ParallelSender<Pin<Output, PD2>, Pin<Output, PD4>, Pin<Output, PD5>, 4>, Delay>) {
    let i2c = I2c::new(
        twi,
        pins.a4.into_pull_up_input(),
        pins.a5.into_pull_up_input(),
        50000,
    );

    let mut delayer = Delay::new();

    // Set up BME680
    let mut dev = Bme680::init(i2c, &mut delayer, I2CAddress::Primary).map_err(|e| {
        log::error!("Error at bme680 init {e:?}");
    }).unwrap();

    let settings = SettingsBuilder::new()
        .with_humidity_oversampling(OversamplingSetting::OS2x)
        .with_pressure_oversampling(OversamplingSetting::OS4x)
        .with_temperature_oversampling(OversamplingSetting::OS8x)
        .with_temperature_filter(IIRFilterSize::Size3)
        .with_gas_measurement(Duration::from_millis(1500), 320, 25)
        .with_run_gas(true)
        .build();

    dev.set_sensor_settings(&mut delayer, settings)
        .map_err(|e| {
            log::error!("Unable to apply sensor settings {e:?}");
        }).unwrap();

    dev.set_sensor_mode(&mut delayer, PowerMode::ForcedMode)
        .map_err(|e| {
            log::error!("Unable to set sensor mode {e:?}");
        }).unwrap();

    // Set up LCD1602
    let mut sender = ParallelSender::new_4pin(
        pins.d2.into_output(),
        pins.d0.into_output(),
        pins.d3.into_output(),
        pins.d4.into_output(),
        pins.d5.into_output(),
        pins.d6.into_output(),
        pins.d7.into_output(),
        None,
    );

    let mut delayer1 = Delay::new();
    let lcd_config = lcd::Config::default().set_data_width(DataWidth::Bit4);

    let lcd = Lcd::new(
        &mut sender,
        &mut delayer1,
        lcd_config,
        10,
    );

    (dev, lcd)
}

fn get_temperature(bme: &Bme680<I2c, Delay>) {

}

fn get_humidity() {

}

enum Screen {
    Temp,
    Humidity,
    Pressure,
    Date,
    Warning,
}
