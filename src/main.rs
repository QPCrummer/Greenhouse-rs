#![no_std]
#![no_main]
extern crate alloc;

use alloc::string::String;
use core::time::Duration;
use arduino_hal::{pins, Delay, I2c, Peripherals, Pins};
use arduino_hal::hal::port::Dynamic;
use arduino_hal::pac::TWI;
use arduino_hal::port::mode::{Input, OpenDrain, Output, PullUp};
use arduino_hal::port::Pin;
use bme680::{Bme680, FieldData, I2CAddress, IIRFilterSize, OversamplingSetting, PowerMode, SettingsBuilder};
use lcd1602_driver::command::DataWidth;
use lcd1602_driver::lcd;
use lcd1602_driver::lcd::{Basic, Ext, Lcd};
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
///
/// Roof Vent:
///     +: A3
///     -: GND

#[arduino_hal::entry]
fn main() -> ! {
    let dp = Peripherals::take().unwrap();
    let twi = dp.TWI;
    let pins = pins!(dp);
    let devices = setup(pins, twi);
    let current_screen = Screen::Loading;

    loop {
        arduino_hal::delay_ms(1000);

        match current_screen {
            Screen::Loading => {

            }
            Screen::Temp => {

            }
            Screen::Humidity => {}
            Screen::Pressure => {}
            Screen::Date => {}
            Screen::Warning => {}
        }
    }
}

/// Gets data from the BME sensor
/// param bme: BME sensor instance
/// param delayer: BME sensor delay
fn get_bme_data(bme: &mut Bme680<I2c, Delay>, delayer: &mut Delay) -> FieldData {
    prep_bme(bme, delayer);
    let (data, _state) = bme.get_sensor_data(delayer).map_err(|e| {
        log::error!("Unable to get sensor data {e:?}");
    }).unwrap();
    data
}

/// Gets temperature in Fahrenheit
/// param data: FieldData from get_bme_data()
fn get_temperature(data: FieldData) -> f32 {
    data.temperature_celsius() * (9./5.) + 32.
}

/// Gets percent humidity
/// param data: FieldData from get_bme_data()
fn get_humidity(data: FieldData) -> f32 {
    data.humidity_percent()
}

/// Gets atmospheric pressure in atmospheres
/// param data: FieldData from get_bme_data()
fn get_pressure(data: FieldData) -> f32 {
    data.pressure_hpa() * 0.000987
}

/// Sets the sensor's mode to Forced
/// This should be called before getting data
/// param bme: BME sensor reference
/// param delayer: BME delay
fn prep_bme(bme: &mut Bme680<I2c, Delay>, delayer: &mut Delay) {
    bme.set_sensor_mode(delayer, PowerMode::ForcedMode)
        .map_err(|e| {
            log::error!("Unable to set sensor mode {e:?}");
        }).unwrap();
}

/// Basic function for rendering text onto the LCD
/// param line_one: optionally render text on the first line
/// param line_two: optionally render text on the second line
/// param lcd: LCD instance
fn render_screen(line_one: Option<String>, line_two: Option<String>, mut lcd: Lcd<'static, 'static, ParallelSender<Pin<Output>, Pin<OpenDrain>, Pin<Output>, 4>, Delay<>>) {
    // Set cursor to first line if needed
    if let Some(line_one) = line_one {
        lcd.set_cursor_pos((0, 0));
        lcd.write_str_to_cur(&*line_one);
    }

    // Set cursor to second line if needed
    if let Some(line_two) = line_two {
        lcd.set_cursor_pos((0, 1));
        lcd.write_str_to_cur(&*line_two);
    }
}

/// Creates all sensors and LCD
/// param pins: instance of all Pins
/// param twi: instance of TWI
fn setup(pins: Pins, twi: TWI) -> (Bme680<I2c, Delay>, Lcd<'static, 'static, ParallelSender<Pin<Output>, Pin<OpenDrain>, Pin<Output>, 4>, Delay>, Pin<Input<PullUp>, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Output, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Output, Dynamic>, Pin<Output, Dynamic>) {
    let mut delayer = Delay::new();
    let i2c = I2c::new(
        twi,
        pins.a4.into_pull_up_input(),
        pins.a5.into_pull_up_input(),
        50000,
    );

    // Set up BME680
    let mut bme = Bme680::init(i2c, &mut delayer, I2CAddress::Primary).map_err(|e| {
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

    bme.set_sensor_settings(&mut delayer, settings)
        .map_err(|e| {
            log::error!("Unable to apply sensor settings {e:?}");
        }).unwrap();

    bme.set_sensor_mode(&mut delayer, PowerMode::ForcedMode)
        .map_err(|e| {
            log::error!("Unable to set sensor mode {e:?}");
        }).unwrap();

    // Set up LCD1602
    let mut sender = ParallelSender::<Pin<Output, Dynamic>,Pin<OpenDrain, Dynamic>,Pin<Output, Dynamic>, 4>::new_4pin(
        pins.d2.into_output().downgrade(),
        pins.d0.into_output().downgrade(),
        pins.d3.into_output().downgrade(),
        pins.d4.into_opendrain().downgrade(),
        pins.d5.into_opendrain().downgrade(),
        pins.d6.into_opendrain().downgrade(),
        pins.d7.into_opendrain().downgrade(),
        None,
    );

    let lcd_config = lcd::Config::default().set_data_width(DataWidth::Bit4);

    let lcd = Lcd::new(
        &mut sender,
        &mut delayer,
        lcd_config,
        10,
    );

    // Set up button up
    let up_button = pins.a0.into_pull_up_input().downgrade();

    // Set up button down
    let down_button = pins.a1.into_pull_up_input().downgrade();

    // Set up button select
    let select_button = pins.a2.into_pull_up_input().downgrade();

    // Set up buzzer
    let buzzer = pins.d9.into_output().downgrade();

    // Set up smoke detector
    let smoke_detector = pins.d8.into_pull_up_input().downgrade();

    // Set up sprinklers
    let sprinklers = pins.d1.into_output().downgrade();

    // Set up roof vent
    let roof_vent = pins.a3.into_output().downgrade();

    (bme, lcd, up_button, down_button, select_button, buzzer, smoke_detector, sprinklers, roof_vent)
}

enum Screen {
    Loading,
    Temp,
    Humidity,
    Pressure,
    Date,
    Warning,
}
