use anyhow::{bail, Result};
use log::info;
use rgb_led::{RGB8, WS2812RMT};
use std::num::NonZeroU32;
use std::fmt::Write;
use std::sync::atomic::{self, AtomicI32, Ordering};
use std::sync::Arc;
use std::vec::Vec;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use regex_lite::Regex;

use core::str;
use embedded_svc::{
    http::{client::Client, Method},
    io::Read,
};
use esp_idf_svc::{
    wifi::EspWifi,
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    hal::{
        gpio::{InterruptType, PinDriver, Pull, AnyIOPin, Level},
        peripherals::Peripherals,
        task::notification::Notification,
        task::queue::Queue,
        task::{yield_now, block_on},
        uart,
        prelude::{Hertz},
        interrupt::{active, free},
    },
    http::client::{Configuration, EspHttpConnection},
};
use crc16::*;
use embedded_svc::wifi::{ClientConfiguration, Wifi, Configuration as WifiConfiguration};
use heapless::{String as HeaplessString, Vec as HeaplessVec};

use embassy_time::{Duration, Timer, Instant};

/// This configuration is picked up at compile time by `build.rs` from the
/// file `cfg.toml`.
#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

/// Entry point to our application.
///
/// It sets up a Wi-Fi connection to the Access Point given in the
/// configuration, then blinks the RGB LED green/blue.
///
/// If the LED goes solid red, then it was unable to connect to your Wi-Fi
/// network.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    block_on(async_main())
}

fn command_bytes(command: String) -> Vec<u8> {
    let a = State::<XMODEM>::calculate(command.as_bytes());
    [command.as_bytes(), &a.to_be_bytes(), b"\x0d"].concat()
}

async fn send_command<T>(uart: &uart::UartDriver<'_>, command: String, parser: &dyn Fn(String) -> Option<T>) -> Option<T> {
    let cmdbytes = command_bytes(command.to_string());
    uart.write(cmdbytes.as_slice()).unwrap();
    let mut input_bytes = Vec::new();

    let mut buf: [u8; 8] = [0; 8];
    // Blue!
    for x in 0..1000 {
        Timer::after(Duration::from_millis(10)).await;
        let bytes_read = uart.read(&mut buf, 0);

        if let Ok(x) = bytes_read {
            if x > 0 {
                let mut v = buf.to_vec();
                v.truncate(x);
                input_bytes.append(&mut v);
                //info!("got some bytes {}, {:?}, orig: {:?}", x, buf, input_bytes);
                #[derive(Debug)]
                struct ResBlock {
                    bytes: Vec<u8>,
                    crc_success: bool,
                    crc: Vec<u8>,
                    calculated_crc: Vec<u8>,
                };

                #[derive(Debug)]
                struct Res {
                    min_potential_start: usize,
                    curr_bytes: Option<Vec<u8>>,
                    results: Vec<ResBlock>,
                };

                let extracted_messages = input_bytes.iter().enumerate().fold(Res {min_potential_start: 0, curr_bytes: None, results: Vec::new()}, |mut acc, (index, e)| {
                    if *e == u8::from(40) {
                        acc.min_potential_start = index;
                        let mut new_current_bytes = Vec::new();
                        new_current_bytes.push(*e);
                        acc.curr_bytes = Some(new_current_bytes);
                    } else if *e == u8::from(13) {
                        if let Some(ref mut x) = acc.curr_bytes {
                            let block_bytes = x.to_vec();
                            let crc = &block_bytes[x.len() - 2 ..];

                            let calculated_crc = State::<XMODEM>::calculate(&block_bytes[.. x.len() - 2]);
                            let crc_success = &calculated_crc.to_be_bytes().iter().zip(crc.iter()).all(|(a, b)| a == b);

                            let bytes = &block_bytes[1 .. x.len() - 2];

                            acc.results.push(ResBlock {
                                bytes: bytes.to_vec(),
                                crc: crc.to_vec(),
                                calculated_crc: calculated_crc.to_be_bytes().to_vec(),
                                crc_success: *crc_success,
                            });
                            acc.curr_bytes = None;
                        }
                    } else {
                        if let Some(ref mut x) = acc.curr_bytes {
                            x.push(*e);
                        }
                    }
                    acc
                });
                input_bytes.drain(..extracted_messages.min_potential_start);
                info!("{:?}", extracted_messages);
                match extracted_messages.results.iter()
                    .filter(|result| result.crc_success)
                    .filter_map(|result| {
                        let as_string = String::from_utf8(extracted_messages.results[0].bytes.clone());
                        match as_string {
                            Ok(x) => {
                                Some(x)
                            }
                            Err(e) => {
                                info!("Could not parse bytes: {:?}", e);
                                None
                            }
                        }
                    })
                    .filter_map(|message| {
                        parser(message)
                    })
                    .nth(0) {
                        Some(x) => {
                            return Some(x);
                        }
                        None => {}
                    }
            }
        }
    }
    None
}

#[derive(Debug)]
struct QPIGS_response {
    ac_output_active_power: String,
    battery_voltage: String,
    battery_charging_current: String,
    battery_capacity: String,
    inverter_heat_sink_temperature: String,
    battery_discharge_current: String,
}

fn parse_qpigs(message: String) -> Option<QPIGS_response> {
    // TODO: safe structure
    /*
    let re = Regex::new(r"^(?<grid_voltage>\d\d\d\.\d) (?<grid_frequency>\d\d\.\d) (\d\d\d\.\d) (\d\d\.\d) (\d\d\d\d) (\d\d\d\d) .*").unwrap();
    let caps = re.captures(&message);
    
    match caps {
        Some(x) => {
            Some(QPIGS_response {
                a:2,
            })
        }
        None => None
    }
    */
    let parts: Vec<&str> = message.split(" ").collect();
    match parts.len() {
        21 => {
            Some(QPIGS_response {
                ac_output_active_power: parts[5].to_string(),
                battery_voltage: parts[8].to_string(),
                battery_charging_current: parts[9].to_string(),
                battery_capacity: parts[10].to_string(),
                inverter_heat_sink_temperature: parts[11].to_string(),
                battery_discharge_current: parts[15].to_string(),
            })
        }
        _ => None
    }
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>>{
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take().unwrap();

    info!("Hello, world2!");

    // Start the LED off yellow
    let mut led = WS2812RMT::new(peripherals.pins.gpio8, peripherals.rmt.channel0)?;
    led.set_pixel(RGB8::new(5, 5, 0))?;

    // The constant `CONFIG` is auto-generated by `toml_config`.
    let app_config = CONFIG;

    // Connect to the Wi-Fi network
    let mut wifi_driver = EspWifi::new(
        peripherals.modem,
        sysloop,
        None,
    ).unwrap();

    let mut ssid = HeaplessString::new();
    ssid.push_str(app_config.wifi_ssid).unwrap();
    let mut password = HeaplessString::new();
    password.push_str(app_config.wifi_psk).unwrap();

    wifi_driver.set_configuration(&WifiConfiguration::Client(ClientConfiguration{
        ssid: ssid,
        password: password,
        ..Default::default()
    })).unwrap();
    wifi_driver.start().unwrap();
    wifi_driver.connect().unwrap();
    while !wifi_driver.is_connected().unwrap(){
        let config = wifi_driver.get_configuration().unwrap();
        Timer::after(Duration::from_millis(1000)).await;
    }
    println!("Should be connected now");

    //let config = uart::config::Config::default().baudrate(Hertz(115_200));
    let config = uart::config::Config::default().baudrate(Hertz(2400)).rx_fifo_size(1000).tx_fifo_size(1000);
    let mut uart: uart::UartDriver = uart::UartDriver::new(
        peripherals.uart1,
        peripherals.pins.gpio14,
        peripherals.pins.gpio15,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        &config
    ).unwrap();
/*
    let (tx, rx) = UsbSerialJtag::new_async(peripherals.uart1).split();
*/

    //let command = [81, 80, 79, 77, 83, 183, 169, 13];
    //uart.write(&command).unwrap();


    //let (tx, rx) = uart.split();

    loop {
        led.set_pixel(RGB8::new(0, 0, 5))?;
        info!("sending command: QPIGS");
        let response = send_command(&uart, "QPIGS".to_string(), &parse_qpigs).await;
        info!("response: {:?}", response);
        match response {
            Some(qpigs) => {
                led.set_pixel(RGB8::new(5, 0, 5))?;
                info!("parts: {:?}", qpigs);
                let current_time = Instant::now().as_secs().to_string();
                info!("Current time: {}", current_time);
                get("https://api.thingspeak.com/update?api_key=1WCXSBQKGAFJU6MJ&field1=".to_owned() + &current_time +
                "&field2=" + &qpigs.ac_output_active_power +
                "&field3=" + &qpigs.battery_voltage +
                "&field4=" + &qpigs.battery_charging_current +
                "&field5=" + &qpigs.battery_capacity +
                "&field6=" + &qpigs.inverter_heat_sink_temperature +
                "&field7=" + &qpigs.battery_discharge_current)?;
            }
            None => {
                led.set_pixel(RGB8::new(0, 5, 0))?;
                info!("Could not get a proper response for the query!");
            }
        }

        Timer::after(Duration::from_secs(10)).await;
    }
}

fn get(url: impl AsRef<str>) -> Result<()> {
    // 1. Create a new EspHttpClient. (Check documentation)
    // ANCHOR: connection
    let connection = EspHttpConnection::new(&Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    })?;
    // ANCHOR_END: connection
    let mut client = Client::wrap(connection);

    // 2. Open a GET request to `url`
    let headers = [("accept", "text/plain")];
    let request = client.request(Method::Get, url.as_ref(), &headers)?;

    // 3. Submit write request and check the status code of the response.
    // Successful http status codes are in the 200..=299 range.
    let response = request.submit()?;
    let status = response.status();

    println!("Response code: {}\n", status);

    match status {
        200..=299 => {
            // 4. if the status is OK, read response data chunk by chunk into a buffer and print it until done
            //
            // NB. see http_client.rs for an explanation of the offset mechanism for handling chunks that are
            // split in the middle of valid UTF-8 sequences. This case is encountered a lot with the given
            // example URL.
            let mut buf = [0_u8; 256];
            let mut offset = 0;
            let mut total = 0;
            let mut reader = response;
            loop {
                if let Ok(size) = Read::read(&mut reader, &mut buf[offset..]) {
                    if size == 0 {
                        break;
                    }
                    total += size;
                    // 5. try converting the bytes into a Rust (UTF-8) string and print it
                    let size_plus_offset = size + offset;
                    match str::from_utf8(&buf[..size_plus_offset]) {
                        Ok(text) => {
                            print!("{}", text);
                            offset = 0;
                        }
                        Err(error) => {
                            let valid_up_to = error.valid_up_to();
                            unsafe {
                                print!("{}", str::from_utf8_unchecked(&buf[..valid_up_to]));
                            }
                            buf.copy_within(valid_up_to.., 0);
                            offset = size_plus_offset - valid_up_to;
                        }
                    }
                }
            }
            println!("Total: {} bytes", total);
        }
        _ => bail!("Unexpected response code: {}", status),
    }

    Ok(())
}

