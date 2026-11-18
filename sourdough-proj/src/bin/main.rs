#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{DriveMode, Flex, Level, Output, OutputConfig, Pull},
    timer::timg::TimerGroup,
};

use dht_sensor::dht11;

use esp_println::println;
esp_bootloader_esp_idf::esp_app_desc!();

/// Baseline time between feeds at a "normal" room temperature (~20-24C).
const BASE_FEED_INTERVAL: Duration = Duration::from_secs(60 * 30);

/// Latest DHT11 reading (temperature C, humidity %), shared between the
/// sensor task and the feeder task so feeding can react to fermentation
/// conditions instead of running on a fixed clock.
static LATEST_READING: Mutex<CriticalSectionRawMutex, Option<(i8, u8)>> = Mutex::new(None);

/// Holds a 50Hz servo PWM pulse (`pulse_us` high out of a 20ms period) for `cycles` repetitions.
fn hold_pulse(servo: &mut Output, delay: &mut Delay, pulse_us: u32, cycles: u32) {
    for _ in 0..cycles {
        servo.set_high();
        delay.delay_micros(pulse_us);
        servo.set_low();
        delay.delay_micros(20_000 - pulse_us);
    }
}

/// Scales the feed interval against dough temperature: warmer dough
/// ferments faster and needs feeding sooner, colder dough can wait longer.
/// Bands are a simplified stand-in for the ~Q10 doubling rule of thumb for
/// fermentation rate, done in integer math to avoid pulling in libm.
fn feed_interval_for_temp(temp_c: i8) -> Duration {
    if temp_c >= 28 {
        BASE_FEED_INTERVAL / 2
    } else if temp_c >= 24 {
        (BASE_FEED_INTERVAL * 3) / 4
    } else if temp_c >= 20 {
        BASE_FEED_INTERVAL
    } else {
        BASE_FEED_INTERVAL * 2
    }
}

#[embassy_executor::task]
async fn sensor_task(mut dht_pin: Flex<'static>) {
    let mut delay = Delay::new();

    // Sensor needs time to stabilize after power-on before its first read.
    Timer::after(Duration::from_millis(1000)).await;

    loop {
        match dht11::blocking::read(&mut delay, &mut dht_pin) {
            Ok(reading) => {
                println!(
                    "sensor -> temperature: {} C, humidity: {} %",
                    reading.temperature, reading.relative_humidity
                );
                *LATEST_READING.lock().await = Some((reading.temperature, reading.relative_humidity));
            }
            Err(e) => println!("DHT11 read failed: {:?}", e),
        }
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
async fn feeder_task(mut servo: Output<'static>) {
    let mut delay = Delay::new();

    loop {
        let temp = LATEST_READING.lock().await.map(|(temp_c, _)| temp_c);
        let wait = temp.map_or(BASE_FEED_INTERVAL, feed_interval_for_temp);
        println!(
            "feeder -> next feed in {}s (last temp sample: {:?} C)",
            wait.as_secs(),
            temp
        );
        Timer::after(wait).await;

        println!("feeder -> running scrapper sweep");
        hold_pulse(&mut servo, &mut delay, 1000, 50); // servo -> min (1.0ms)
        hold_pulse(&mut servo, &mut delay, 2000, 50); // servo -> max (2.0ms)
        hold_pulse(&mut servo, &mut delay, 1500, 50); // servo -> center (1.5ms)
    }
}

#[esp_hal::main]
async fn main(spawner: Spawner) {
    // generator version: 1.4.0
    // generator parameters: -o esp32 -o unstable-hal -o embassy

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let servo = Output::new(peripherals.GPIO13, Level::Low, OutputConfig::default());

    // DHT11 needs a single open-drain, pulled-up pin: driven low to request a
    // reading, released (set high / let the pull-up take over) to listen for
    // the sensor's response on the same wire.
    let mut dht_pin = Flex::new(peripherals.GPIO23);
    dht_pin.apply_output_config(&OutputConfig::default().with_drive_mode(DriveMode::OpenDrain));
    dht_pin.apply_input_config(&esp_hal::gpio::InputConfig::default().with_pull(Pull::Up));
    dht_pin.set_output_enable(true);
    dht_pin.set_input_enable(true);
    dht_pin.set_high();

    spawner.spawn(sensor_task(dht_pin).unwrap());
    spawner.spawn(feeder_task(servo).unwrap());
}
