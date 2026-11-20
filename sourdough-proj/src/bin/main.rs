//! dht11 -> 23
//! servo -> 13
//! hc-sr04 trigger -> 14
//! hc-sr04 echo -> 27
//! led -> 25
//! buzzer -> 26
//! button -> 32

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{DriveMode, Flex, Input, InputConfig, Level, Output, OutputConfig, Pull},
    timer::timg::TimerGroup,
};

use dht_sensor::dht11;

use esp_println::println;
esp_bootloader_esp_idf::esp_app_desc!();

/// Baseline time between feeds at a "normal" room temperature (~20-24C).
const BASE_FEED_INTERVAL: Duration = Duration::from_secs(60 * 30);

/// HC-SR04 echo above this distance means the sensor is seeing the bottom of
/// the container rather than the dough/flour surface, i.e. it's run low.
const CONTAINER_EMPTY_CM: u32 = 15;

/// How long to wait for an echo before treating the HC-SR04 reading as failed.
const ECHO_TIMEOUT: Duration = Duration::from_millis(30);

/// Latest DHT11 reading (temperature C, humidity %), shared between the
/// sensor task and the feeder task so feeding can react to fermentation
/// conditions instead of running on a fixed clock.
static LATEST_READING: Mutex<CriticalSectionRawMutex, Option<(i8, u8)>> = Mutex::new(None);

/// Signaled by `button_task` to wake the feeder immediately instead of
/// waiting out the rest of its current interval.
static MANUAL_FEED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Debounce window: ignore further presses for this long after one is seen.
const BUTTON_DEBOUNCE: Duration = Duration::from_millis(300);

/// Holds a 50Hz servo PWM pulse (`pulse_us` high out of a 20ms period) for `cycles` repetitions.
fn hold_pulse(servo: &mut Output, delay: &mut Delay, pulse_us: u32, cycles: u32) {
    for _ in 0..cycles {
        servo.set_high();
        delay.delay_micros(pulse_us);
        servo.set_low();
        delay.delay_micros(20_000 - pulse_us);
    }
}

/// Pulses the HC-SR04 trigger and times the echo to get a distance in cm.
/// Returns `None` if no echo arrives within `ECHO_TIMEOUT` (e.g. out of range).
fn measure_distance_cm(trigger: &mut Output, echo: &mut Input, delay: &mut Delay) -> Option<u32> {
    trigger.set_low();
    delay.delay_micros(2);
    trigger.set_high();
    delay.delay_micros(10);
    trigger.set_low();

    let deadline = Instant::now() + ECHO_TIMEOUT;
    while echo.is_low() {
        if Instant::now() > deadline {
            return None;
        }
    }
    let start = Instant::now();
    while echo.is_high() {
        if Instant::now() > deadline {
            return None;
        }
    }
    Some((Instant::now() - start).as_micros() as u32 / 58)
}

/// Short confirmation beep after a completed feed.
fn beep_confirm(buzzer: &mut Output, delay: &mut Delay) {
    buzzer.set_high();
    delay.delay_millis(80);
    buzzer.set_low();
}

/// Blinks the LED and buzzer together to flag a skipped feed (empty container
/// or a failed HC-SR04 reading).
fn alarm(led: &mut Output, buzzer: &mut Output, delay: &mut Delay) {
    for _ in 0..3 {
        led.set_high();
        buzzer.set_high();
        delay.delay_millis(150);
        led.set_low();
        buzzer.set_low();
        delay.delay_millis(150);
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

/// Watches the manual feed button (active-low, pulled up) and signals
/// `MANUAL_FEED` on each press, debounced so a single press can't re-trigger.
#[embassy_executor::task]
async fn button_task(mut button: Input<'static>) {
    loop {
        button.wait_for_falling_edge().await;
        println!("button -> manual feed requested");
        MANUAL_FEED.signal(());
        Timer::after(BUTTON_DEBOUNCE).await;
    }
}

#[embassy_executor::task]
async fn feeder_task(
    mut servo: Output<'static>,
    mut trigger: Output<'static>,
    mut echo: Input<'static>,
    mut led: Output<'static>,
    mut buzzer: Output<'static>,
) {
    let mut delay = Delay::new();

    loop {
        let temp = LATEST_READING.lock().await.map(|(temp_c, _)| temp_c);
        let wait = temp.map_or(BASE_FEED_INTERVAL, feed_interval_for_temp);
        println!(
            "feeder -> next feed in {}s (last temp sample: {:?} C)",
            wait.as_secs(),
            temp
        );
        match select(Timer::after(wait), MANUAL_FEED.wait()).await {
            Either::First(()) => {}
            Either::Second(()) => println!("feeder -> manual feed triggered"),
        }

        match measure_distance_cm(&mut trigger, &mut echo, &mut delay) {
            Some(distance_cm) if distance_cm < CONTAINER_EMPTY_CM => {
                println!("feeder -> container level ok ({distance_cm} cm), running scrapper sweep");
                led.set_high();
                hold_pulse(&mut servo, &mut delay, 1000, 50); // servo -> min (1.0ms)
                hold_pulse(&mut servo, &mut delay, 2000, 50); // servo -> max (2.0ms)
                hold_pulse(&mut servo, &mut delay, 1500, 50); // servo -> center (1.5ms)
                led.set_low();
                beep_confirm(&mut buzzer, &mut delay);
            }
            Some(distance_cm) => {
                println!("feeder -> container running low ({distance_cm} cm), skipping feed");
                alarm(&mut led, &mut buzzer, &mut delay);
            }
            None => {
                println!("feeder -> hc-sr04 read timed out, skipping feed");
                alarm(&mut led, &mut buzzer, &mut delay);
            }
        }
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
    let trigger = Output::new(peripherals.GPIO14, Level::Low, OutputConfig::default());
    let echo = Input::new(peripherals.GPIO27, InputConfig::default());
    let led = Output::new(peripherals.GPIO25, Level::Low, OutputConfig::default());
    let buzzer = Output::new(peripherals.GPIO26, Level::Low, OutputConfig::default());
    let button = Input::new(peripherals.GPIO32, InputConfig::default().with_pull(Pull::Up));

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
    spawner.spawn(feeder_task(servo, trigger, echo, led, buzzer).unwrap());
    spawner.spawn(button_task(button).unwrap());
}

