#![no_std]
#![no_main]
use core::time::Duration;
use core::fmt::Write;
use arduino_hal::{pins, Delay, I2c, Peripherals, Pins};
use arduino_hal::hal::port::Dynamic;
use arduino_hal::pac::TWI;
use arduino_hal::port::mode::{Input, OpenDrain, Output, PullUp};
use arduino_hal::port::Pin;
use bme680::{Bme680, FieldData, I2CAddress, IIRFilterSize, OversamplingSetting, PowerMode, SettingsBuilder};
use heapless::String;
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
    let (mut delayer, mut bme, mut lcd, up_button,
        down_button, select_button, mut buzzer, smoke_detector,
        mut sprinklers, mut roof_vent) = setup(pins, twi);
    let current_screen_index = 0;
    let wait_time: u16 = 0;
    let mut data: FieldData = FieldData::default(); // TODO Make sure this is set to a valid value before using it
    let mut preferences: Preferences = Preferences::default();

    loop {
        arduino_hal::delay_ms(10);

        let (update_needed, action) = should_update(&up_button, &down_button, &select_button, wait_time, &mut preferences);

        if update_needed {
            match action {
                RefreshAction::UP => {
                    next_screen(current_screen_index, true);
                }
                RefreshAction::DOWN => {
                    next_screen(current_screen_index, false);
                }
                RefreshAction::SELECT => {
                    // Handle SELECT action
                }
                _ => {
                    if fire_present(&smoke_detector) {
                        // Panic!!!
                        let roof_open = &roof_vent.is_set_high();
                        render_screen(Some(String::try_from("Warning!").unwrap()), Some(String::try_from("Fire Present!").unwrap()), &mut lcd);
                        while fire_present(&smoke_detector) {
                            // Enable sprinklers
                            &sprinklers.set_high();
                            // Ensure windows are closed
                            &roof_vent.set_low();
                            // Sound alarm
                            &buzzer.set_high();
                            arduino_hal::delay_ms(100);
                        }
                        // Safe; Disable sprinklers and open vent if it was open before
                        &buzzer.set_low();
                        &sprinklers.set_low();
                        if *roof_open {
                            &roof_vent.set_high();
                        }
                    }

                    data = get_bme_data(&mut bme, &mut delayer);
                }
            }
        } else {
            continue;
        }


        let current_screen = get_screen(current_screen_index).unwrap();
        match current_screen {
            Screen::Loading => {

            }
            Screen::Temp => {
                let mut upper_string: String<16> = Default::default();
                write!(&mut upper_string, "Temp: {}F", get_temperature(&data)).unwrap();
                let mut lower_string: String<16> = Default::default();
                write!(&mut lower_string, "Safe ({}, {})", preferences.temperature.0, preferences.temperature.1).unwrap();
                render_screen(Some(upper_string), Some(lower_string), &mut lcd);
            }
            Screen::Humidity => {
                let mut upper_string: String<16> = Default::default();
                write!(&mut upper_string, "Humidity: {}%", get_humidity(&data)).unwrap();
                let mut lower_string: String<16> = Default::default();
                write!(&mut lower_string, "Safe ({}, {})", preferences.humidity.0, preferences.humidity.1).unwrap();
                render_screen(Some(upper_string), Some(lower_string), &mut lcd);
            }
            Screen::Pressure => {
                let mut upper_string: String<16> = Default::default();
                write!(&mut upper_string, "PRS: {:.3} atm", get_pressure(&data)).unwrap();
                render_screen(Some(upper_string), None, &mut lcd);
            }
            Screen::Date => {
                let (time, date) = preferences.get_date_formatted();
                render_screen(Some(time.parse().unwrap()), Some(date.parse().unwrap()), &mut lcd);
            }
            _ => {
            }
        }
    }
}

/// Gets data from the BME sensor
/// param bme: BME sensor instance
/// param delayer: BME sensor delay
fn get_bme_data(bme: &mut Bme680<I2c, Delay>, delayer: &mut Delay) -> FieldData {
    prep_bme(bme, delayer);
    let (data, _state) = bme.get_sensor_data(delayer).map_err(|e| {
        // TODO Handle error
    }).unwrap();
    data
}

/// Gets temperature in Fahrenheit
/// param data: FieldData from get_bme_data()
fn get_temperature(data: &FieldData) -> f32 {
    data.temperature_celsius() * (9./5.) + 32.
}

/// Gets percent humidity
/// param data: FieldData from get_bme_data()
fn get_humidity(data: &FieldData) -> f32 {
    data.humidity_percent()
}

/// Gets atmospheric pressure in atmospheres
/// param data: FieldData from get_bme_data()
fn get_pressure(data: &FieldData) -> f32 {
    data.pressure_hpa() * 0.000987
}

/// Sets the sensor's mode to Forced
/// This should be called before getting data
/// param bme: BME sensor reference
/// param delayer: BME delay
fn prep_bme(bme: &mut Bme680<I2c, Delay>, delayer: &mut Delay) {
    bme.set_sensor_mode(delayer, PowerMode::ForcedMode)
        .map_err(|e| {
            // TODO Handle error
        }).unwrap();
}

/// Detects if a fire is present
/// param smoke_detector: Smoke Detector input pin
/// returns: if a fire is present
fn fire_present(smoke_detector: &Pin<Input<PullUp>>) -> bool {
    smoke_detector.is_high()
}

/// Basic function for rendering text onto the LCD
/// param line_one: optionally render text on the first line
/// param line_two: optionally render text on the second line
/// param lcd: LCD instance
fn render_screen(line_one: Option<String<16>>, line_two: Option<String<16>>, lcd: &mut Lcd<'static, 'static, ParallelSender<Pin<Output>, Pin<OpenDrain>, Pin<Output>, 4>, Delay<>>) {
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

enum RefreshAction {
    UP,
    DOWN,
    SELECT,
    SENSOR,
}

/// Whether to update the LCD
/// param up: Up Button
/// param down: Down Button
/// param select: Selection Button
/// param wait_time: The amount of time between sensor polling
/// param preferences: Client Preferences
/// returns: if the LCD needs an update
fn should_update(up: &Pin<Input<PullUp>, Dynamic>, down: &Pin<Input<PullUp>, Dynamic>, select: &Pin<Input<PullUp>, Dynamic>, mut wait_time: u16, preferences: &mut Preferences) -> (bool, RefreshAction) {
    wait_time += 1;
    // Make sure time is kept track of
    if wait_time % 100 == 0 {
        preferences.tick_time();
    }

    // Prioritize button pressing
    if up.is_high() {
        return (true, RefreshAction::UP);
    } else if down.is_high() {
        return (true, RefreshAction::DOWN);
    } else if select.is_high() {
        return (true, RefreshAction::SELECT);
    }
    // Check if sensors need updated
    if wait_time >= 100 || wait_time < 0 { // It could be negative due to rollover
        wait_time = 0; // TODO See if this actually works
        return (true, RefreshAction::SENSOR);
    }
    (false, RefreshAction::SENSOR) // It's ok to return SENSOR since it gets ignored
}

/// Creates all sensors and LCD
/// param pins: instance of all Pins
/// param twi: instance of TWI
/// returns: Delay, BME680, LCD, Up Button, Down Button, Selection Button, Buzzer, Smoke Detector, Sprinklers, Roof Vent
fn setup(pins: Pins, twi: TWI) -> (Delay, Bme680<I2c, Delay>, Lcd<'static, 'static, ParallelSender<Pin<Output>, Pin<OpenDrain>, Pin<Output>, 4>, Delay>, Pin<Input<PullUp>, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Output, Dynamic>, Pin<Input<PullUp>, Dynamic>, Pin<Output, Dynamic>, Pin<Output, Dynamic>) {
    let mut delayer = Delay::new();
    let i2c = I2c::new(
        twi,
        pins.a4.into_pull_up_input(),
        pins.a5.into_pull_up_input(),
        50000,
    );

    // Set up BME680
    let mut bme = Bme680::init(i2c, &mut delayer, I2CAddress::Primary)
        .map_err(|e| {
        // TODO Handle error
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
            // TODO Handle error
        }).unwrap();

    bme.set_sensor_mode(&mut delayer, PowerMode::ForcedMode)
        .map_err(|e| {
            // TODO Handle error
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

    (delayer, bme, lcd, up_button, down_button, select_button, buzzer, smoke_detector, sprinklers, roof_vent)
}

enum Screen {
    Loading,
    Warning,
    Temp,
    Humidity,
    Pressure,
    Date,
}

/// Gets the Screen from an index
/// param index: index to search for
/// returns: Optional Screen
fn get_screen(index: i8) -> Option<Screen> {
    match index {
        0 => Some(Screen::Temp),
        1 => Some(Screen::Humidity),
        2 => Some(Screen::Pressure),
        3 => Some(Screen::Date),
        _ => None,
    }
}

/// Iterates forwards or backwards through Screens
/// param current_screen: The current screen being displayed
/// param next: Whether to iterate forward; If false, iterate backwards
/// returns: The next Screen
fn next_screen(mut current_screen_index: i8, next: bool) -> Screen {
    let offset:i8 = if next { 1 } else { -1 };
    current_screen_index += offset;
    if current_screen_index < 0 {
        current_screen_index = 3;
    } else if current_screen_index > 3 {
        current_screen_index = 0;
    }
    get_screen(current_screen_index).unwrap()
}

pub struct Preferences {
    pub temperature: (f32, f32),
    pub humidity: (f32, f32),
    pub date: (u8, u8, u8, u8, u8, u16), // Sec, Min, Hour, Day, Month, Year
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            temperature: (60.0, 80.0), // Ideal range is 60F - 80F
            humidity: (0.6, 0.7), // Ideal range is 60% - 70%
            date: (0, 0, 0, 1, 1, 2000), // Date: 00:00:00 Jan 1 2000
        }
    }
}

impl Preferences {
    /// Increments by 1 second
    fn tick_time(&mut self) {
        let (mut sec, min, hour, mut day, mut month, mut year) = self.date;
        sec += 1;

        // Check for rollovers
        if sec >= 60 {
            self.date.1 += sec / 60;
            self.date.0 = sec % 60;
        } else {
            return;
        }

        if min >= 60 {
            self.date.2 += min / 60;
            self.date.1 = min % 60;
        } else {
            return;
        }

        if hour >= 24 {
            day += hour / 24;
            self.date.2 = hour % 24;
        } else {
            return;
        }

        // Handle month and day rollovers
        loop {
            let days_in_month = match month {
                2 => if Self::is_leap_year(year) { 29 } else { 28 },
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };

            if day > days_in_month {
                day -= days_in_month;
                month += 1;
            } else {
                break;
            }

            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        // Update the date tuple
        self.date = (self.date.0, self.date.1, self.date.2, day, month, year);
    }

    /// Gets the date in the HH:MM:SS DD/MM/YYYY format
    /// returns: (HH:MM:SS, DD/MM/YYYY)
    fn get_date_formatted(&mut self) -> (String<8>, String<10>) {
        let (sec, min, hour, day, month, year) = self.date;

        // Format the date as a string
        let mut val1: String<8> = Default::default();
        let mut val2: String<10> = Default::default();
        write!(&mut val1, "{:02}:{:02}:{:02}", hour, min, sec).unwrap();
        write!(&mut val2, "{:02}/{:02}/{:04}", day, month, year).unwrap();
        (val1, val2)
    }

    /// Calculates if it is leap year
    fn is_leap_year(year: u16) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }
}

