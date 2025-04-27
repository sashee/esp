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
        modem::Modem,
    },
    http::client::{Configuration, EspHttpConnection},
};
use crc16::*;
use embedded_svc::wifi::{ClientConfiguration, Wifi, Configuration as WifiConfiguration};
use heapless::{String as HeaplessString, Vec as HeaplessVec};
use esp_idf_svc::wifi::config::ScanConfig;

use embassy_time::{Duration, Timer, Instant};

/// This configuration is picked up at compile time by `build.rs` from the
/// file `cfg.toml`.
#[derive(Debug)]
#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    thingspeak_write_api_key: &'static str,
    #[default("")]
    known_wifis: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    block_on(async_main());
    panic!("async_main returned that should not happened. Restarting");
}

fn command_bytes(command: String) -> Vec<u8> {
    let a = State::<XMODEM>::calculate(command.as_bytes());
    [command.as_bytes(), &a.to_be_bytes(), b"\x0d"].concat()
}

async fn send_command<T>(uart: &uart::UartDriver<'_>, command: String, parser: &dyn Fn(String) -> Option<T>) -> Option<T> {
    let cmdbytes = command_bytes(command.to_string());
    let mut input_bytes = Vec::new();

    let mut buf: [u8; 8] = [0; 8];

    // drain the uart input buffer
    let mut drained_bytes = 0;
    info!("Draining uart input before sending the command");
    loop {
        let bytes_read = uart.read(&mut buf, 0);
        match bytes_read {
            Ok(x) => {
                if (x == 0) {
                    break;
                }else {
                    drained_bytes += x;
                }
            }
            Err(e) => {
                break;
            }
        }
    }
    info!("Finished draining, {} bytes drained", drained_bytes);
    info!("UART BYTES: {:?}", cmdbytes.as_slice());
    uart.write(cmdbytes.as_slice()).unwrap();
    
    // Blue!
    for x in 0..1000 {
        let bytes_read = uart.read(&mut buf, 0);

        match bytes_read {
            Ok(x) if x > 0 => {
                info!("Bytes read: {:?}", bytes_read);
                info!("got some bytes: {:?}", input_bytes);
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
                            if x.len() > 2 {
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
                            }
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
            _ => {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
    info!("giving up waiting");
    None
}

#[derive(Debug)]
struct QPIGS_response {
    grid_voltage: f64,
    grid_frequency: f64,
    ac_output_voltage: f64,
    ac_output_frequency: f64,
    ac_output_apparent_power: u64,
    ac_output_active_power: u64,
    output_load_percent: u64,
    bus_voltage: u64,
    battery_voltage: f64,
    battery_charging_current: u64,
    battery_capacity: u64,
    inverter_heat_sink_temperature: u64,
    pv_input_current1: f64,
    pv_input_voltage1: f64,
    battery_voltage_from_scc1: f64,
    battery_discharge_current: u64,
    add_sbu_priority_version: bool,
    configuration_status: bool,
    scc_firmware_version: bool,
    load_status: bool,
    battery_voltage_to_steady_while_charging: bool,
    charging_status: bool,
    charging_status_scc_1: bool,
    charging_status_ac: bool,
    battery_voltage_from_fans_on: u64,
    eeprom_version: u64,
    pv_charging_power1: u64,
    flag_for_charging_to_flating_mode: bool,
    switch_on: bool,
    device_status_2_reserved: bool,
}

fn parse_qpigs(message: String) -> Option<QPIGS_response> {
    let re = Regex::new(r"^(?<grid_voltage>\d\d\d\.\d) (?<grid_frequency>\d\d\.\d) (?<ac_output_voltage>\d\d\d\.\d) (?<ac_output_frequency>\d\d\.\d) (?<ac_output_apparent_power>\d\d\d\d) (?<ac_output_active_power>\d\d\d\d) (?<output_load_percent>\d\d\d) (?<bus_voltage>\d\d\d) (?<battery_voltage>\d\d\.\d\d) (?<battery_charging_current>\d\d\d) (?<battery_capacity>\d\d\d) (?<inverter_heat_sink_temperature>\d\d\d\d) (?<pv_input_current1>\d\d\.\d) (?<pv_input_voltage1>\d\d\d\.\d) (?<battery_voltage_from_scc1>\d\d\.\d\d) (?<battery_discharge_current>\d\d\d\d\d) (?<add_sbu_priority_version>[01])(?<configuration_status>[01])(?<scc_firmware_version>[01])(?<load_status>[01])(?<battery_voltage_to_steady_while_charging>[01])(?<charging_status>[01])(?<charging_status_scc_1>[01])(?<charging_status_ac>[01]) (?<battery_voltage_from_fans_on>\d\d) (?<eeprom_version>\d\d) (?<pv_charging_power1>\d\d\d\d\d) (?<flag_for_charging_to_flating_mode>[01])(?<switch_on>[01])(?<device_status_2_reserved>[01])$").unwrap();
    let caps = re.captures(&message);
    
    match caps {
        Some(x) => {
            Some(QPIGS_response {
                grid_voltage: x["grid_voltage"].parse::<f64>().unwrap(),
                grid_frequency: x["grid_frequency"].parse::<f64>().unwrap(),
                ac_output_voltage: x["ac_output_voltage"].parse::<f64>().unwrap(),
                ac_output_frequency: x["ac_output_frequency"].parse::<f64>().unwrap(),
                ac_output_apparent_power: x["ac_output_apparent_power"].parse::<u64>().unwrap(),
                ac_output_active_power: x["ac_output_active_power"].parse::<u64>().unwrap(),
                output_load_percent: x["output_load_percent"].parse::<u64>().unwrap(),
                bus_voltage: x["bus_voltage"].parse::<u64>().unwrap(),
                battery_voltage: x["battery_voltage"].parse::<f64>().unwrap(),
                battery_charging_current: x["battery_charging_current"].parse::<u64>().unwrap(),
                battery_capacity: x["battery_capacity"].parse::<u64>().unwrap(),
                inverter_heat_sink_temperature: x["inverter_heat_sink_temperature"].parse::<u64>().unwrap(),
                pv_input_current1: x["pv_input_current1"].parse::<f64>().unwrap(),
                pv_input_voltage1: x["pv_input_voltage1"].parse::<f64>().unwrap(),
                battery_voltage_from_scc1: x["battery_voltage_from_scc1"].parse::<f64>().unwrap(),
                battery_discharge_current: x["battery_discharge_current"].parse::<u64>().unwrap(),
                add_sbu_priority_version: x["add_sbu_priority_version"].parse::<u8>().unwrap() == 1,
                configuration_status: x["configuration_status"].parse::<u8>().unwrap() == 1,
                scc_firmware_version: x["scc_firmware_version"].parse::<u8>().unwrap() == 1,
                load_status: x["load_status"].parse::<u8>().unwrap() == 1,
                battery_voltage_to_steady_while_charging: x["battery_voltage_to_steady_while_charging"].parse::<u8>().unwrap() == 1,
                charging_status: x["charging_status"].parse::<u8>().unwrap() == 1,
                charging_status_scc_1: x["charging_status_scc_1"].parse::<u8>().unwrap() == 1,
                charging_status_ac: x["charging_status_ac"].parse::<u8>().unwrap() == 1,
                battery_voltage_from_fans_on: x["battery_voltage_from_fans_on"].parse::<u64>().unwrap(),
                eeprom_version: x["eeprom_version"].parse::<u64>().unwrap(),
                pv_charging_power1: x["pv_charging_power1"].parse::<u64>().unwrap(),
                flag_for_charging_to_flating_mode: x["flag_for_charging_to_flating_mode"].parse::<u8>().unwrap() == 1,
                switch_on: x["switch_on"].parse::<u8>().unwrap() == 1,
                device_status_2_reserved: x["device_status_2_reserved"].parse::<u8>().unwrap() == 1,
            })
        }
        None => None
    }
}

#[derive(Debug)]
struct QPIGS2_response {
    pv_input_current2: f64,
    pv_input_voltage2: f64,
    pv_charging_power2: u64,
}

fn parse_qpigs2(message: String) -> Option<QPIGS2_response> {
    let re = Regex::new(r"^(?<pv_input_current2>\d\d\.\d) (?<pv_input_voltage2>\d\d\d\.\d) (?<pv_charging_power2>\d\d\d\d\d) $").unwrap();
    let caps = re.captures(&message);
    
    match caps {
        Some(x) => {
            Some(QPIGS2_response {
                pv_input_current2: x["pv_input_current2"].parse::<f64>().unwrap(),
                pv_input_voltage2: x["pv_input_voltage2"].parse::<f64>().unwrap(),
                pv_charging_power2: x["pv_charging_power2"].parse::<u64>().unwrap(),
            })
        }
        None => None
    }
}

#[derive(Debug)]
struct QPIWS_response {
    reserved1: bool,
    inverter_fault: bool,
    bus_over: bool,
    bus_under: bool,
    bus_soft_fail: bool,
    line_fail: bool,
    opvshort: bool,
    inverter_voltage_too_low: bool,
    inverter_voltage_too_high: bool,
    over_temperature: bool,
    fan_locked: bool,
    battery_voltage_high: bool,
    battery_low_alarm: bool,
    reserved_overcharge: bool,
    battery_under_shutdown: bool,
    reserved_battery_derating: bool,
    over_load: bool,
    eeprom_fault: bool,
    inverter_over_current: bool,
    inverter_soft_fail: bool,
    self_test_fail: bool,
    op_dv_voltage_over: bool,
    bat_open: bool,
    current_sensor_fail: bool,
    battery_short: bool,
    power_limit: bool,
    pv_voltage_high_1: bool,
    mppt_overload_fault_1: bool,
    mppt_overload_warning_1: bool,
    battery_too_low_to_charge_1: bool,
    pv_voltage_high_2: bool,
    mppt_overload_fault_2: bool,
    mppt_overload_warning_2: bool,
    battery_too_low_to_charge_2: bool,
    unknown1: bool,
    unknown2: bool,
}


fn parse_qpiws(message: String) -> Option<QPIWS_response> {
    let re = Regex::new(r"^(?<reserved1>[01])(?<inverter_fault>[01])(?<bus_over>[01])(?<bus_under>[01])(?<bus_soft_fail>[01])(?<line_fail>[01])(?<opvshort>[01])(?<inverter_voltage_too_low>[01])(?<inverter_voltage_too_high>[01])(?<over_temperature>[01])(?<fan_locked>[01])(?<battery_voltage_high>[01])(?<battery_low_alarm>[01])(?<reserved_overcharge>[01])(?<battery_under_shutdown>[01])(?<reserved_battery_derating>[01])(?<over_load>[01])(?<eeprom_fault>[01])(?<inverter_over_current>[01])(?<inverter_soft_fail>[01])(?<self_test_fail>[01])(?<op_dv_voltage_over>[01])(?<bat_open>[01])(?<current_sensor_fail>[01])(?<battery_short>[01])(?<power_limit>[01])(?<pv_voltage_high_1>[01])(?<mppt_overload_fault_1>[01])(?<mppt_overload_warning_1>[01])(?<battery_too_low_to_charge_1>[01])(?<pv_voltage_high_2>[01])(?<mppt_overload_fault_2>[01])(?<mppt_overload_warning_2>[01])(?<battery_too_low_to_charge_2>[01])(?<unknown1>[01])(?<unknown2>[01])$").unwrap();
    let caps = re.captures(&message);
    
    match caps {
        Some(x) => {
            Some(QPIWS_response {
                reserved1: x["reserved1"].parse::<u8>().unwrap() == 1,
                inverter_fault: x["inverter_fault"].parse::<u8>().unwrap() == 1,
                bus_over: x["bus_over"].parse::<u8>().unwrap() == 1,
                bus_under: x["bus_under"].parse::<u8>().unwrap() == 1,
                bus_soft_fail: x["bus_soft_fail"].parse::<u8>().unwrap() == 1,
                line_fail: x["line_fail"].parse::<u8>().unwrap() == 1,
                opvshort: x["opvshort"].parse::<u8>().unwrap() == 1,
                inverter_voltage_too_low: x["inverter_voltage_too_low"].parse::<u8>().unwrap() == 1,
                inverter_voltage_too_high: x["inverter_voltage_too_high"].parse::<u8>().unwrap() == 1,
                over_temperature: x["over_temperature"].parse::<u8>().unwrap() == 1,
                fan_locked: x["fan_locked"].parse::<u8>().unwrap() == 1,
                battery_voltage_high: x["battery_voltage_high"].parse::<u8>().unwrap() == 1,
                battery_low_alarm: x["battery_low_alarm"].parse::<u8>().unwrap() == 1,
                reserved_overcharge: x["reserved_overcharge"].parse::<u8>().unwrap() == 1,
                battery_under_shutdown: x["battery_under_shutdown"].parse::<u8>().unwrap() == 1,
                reserved_battery_derating: x["reserved_battery_derating"].parse::<u8>().unwrap() == 1,
                over_load: x["over_load"].parse::<u8>().unwrap() == 1,
                eeprom_fault: x["eeprom_fault"].parse::<u8>().unwrap() == 1,
                inverter_over_current: x["inverter_over_current"].parse::<u8>().unwrap() == 1,
                inverter_soft_fail: x["inverter_soft_fail"].parse::<u8>().unwrap() == 1,
                self_test_fail: x["self_test_fail"].parse::<u8>().unwrap() == 1,
                op_dv_voltage_over: x["op_dv_voltage_over"].parse::<u8>().unwrap() == 1,
                bat_open: x["bat_open"].parse::<u8>().unwrap() == 1,
                current_sensor_fail: x["current_sensor_fail"].parse::<u8>().unwrap() == 1,
                battery_short: x["battery_short"].parse::<u8>().unwrap() == 1,
                power_limit: x["power_limit"].parse::<u8>().unwrap() == 1,
                pv_voltage_high_1: x["pv_voltage_high_1"].parse::<u8>().unwrap() == 1,
                mppt_overload_fault_1: x["mppt_overload_fault_1"].parse::<u8>().unwrap() == 1,
                mppt_overload_warning_1: x["mppt_overload_warning_1"].parse::<u8>().unwrap() == 1,
                battery_too_low_to_charge_1: x["battery_too_low_to_charge_1"].parse::<u8>().unwrap() == 1,
                pv_voltage_high_2: x["pv_voltage_high_2"].parse::<u8>().unwrap() == 1,
                mppt_overload_fault_2: x["mppt_overload_fault_2"].parse::<u8>().unwrap() == 1,
                mppt_overload_warning_2: x["mppt_overload_warning_2"].parse::<u8>().unwrap() == 1,
                battery_too_low_to_charge_2: x["battery_too_low_to_charge_2"].parse::<u8>().unwrap() == 1,
                unknown1: x["unknown1"].parse::<u8>().unwrap() == 1,
                unknown2: x["unknown2"].parse::<u8>().unwrap() == 1,
            })
        }
        None => None
    }
}

async fn setup_wifi(modem: Modem, sysloop: EspSystemEventLoop, app_config: &Config) -> EspWifi {
    // Connect to the Wi-Fi network
    let mut wifi_driver = EspWifi::new(
        modem,
        sysloop,
        None,
    ).unwrap();

    wifi_driver.set_configuration(
        &WifiConfiguration::Client(ClientConfiguration::default())
    ).unwrap();

    wifi_driver.start().unwrap();

    wifi_driver.start_scan(&ScanConfig::default(), false).unwrap();

    struct WifiCredentials {
        ssid: String,
        pw: String,
    };

    let wifi_credentials : Result<WifiCredentials> = loop {
        let scan_done = wifi_driver.is_scan_done();
        info!("scan_done: {:?}", scan_done);
        if let Ok(done) = scan_done {
            if (done) {
                info!("scan done");
                let res = wifi_driver.get_scan_result().unwrap();
                info!("scan result: {:?}", res);
                let found_credentials = res
                    .iter()
                    .filter_map(|access_point_info| {
                        app_config.known_wifis.split(",")
                            .filter_map(|wifi_config_string| {
                                let ssid = wifi_config_string.split(":").nth(0).unwrap();
                                let pw = wifi_config_string.split(":").nth(1).unwrap();
                                if access_point_info.ssid == ssid {
                                    Some(WifiCredentials{ssid: ssid.to_string(), pw: pw.to_string()})
                                }else {
                                    None
                                }
                            })
                            .nth(0)
                    })
                    .nth(0);

                if let Some(credentials) = found_credentials {
                    break Ok(credentials);
                }
            }
        }

        Timer::after(Duration::from_millis(1000)).await;
    };

    let unwrapped_wifi_credentials = wifi_credentials.unwrap();

    let mut ssid = HeaplessString::new();
    ssid.push_str(&unwrapped_wifi_credentials.ssid).unwrap();
    let mut password = HeaplessString::new();
    password.push_str(&unwrapped_wifi_credentials.pw).unwrap();

    wifi_driver.set_configuration(&WifiConfiguration::Client(ClientConfiguration{
        ssid: ssid,
        password: password,
        ..Default::default()
    })).unwrap();
    wifi_driver.connect().unwrap();
    while !wifi_driver.is_connected().unwrap(){
        let config = wifi_driver.get_configuration().unwrap();
        Timer::after(Duration::from_millis(1000)).await;
    }
    println!("Should be connected now");
    wifi_driver
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>>{
    Timer::after(Duration::from_millis(5000)).await;
    // The constant `CONFIG` is auto-generated by `toml_config`.
    let app_config = CONFIG;
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take().unwrap();

    info!("Hello, world2!");

    // Start the LED off yellow
    let mut led = WS2812RMT::new(peripherals.pins.gpio8, peripherals.rmt.channel0)?;
    led.set_pixel(RGB8::new(5, 5, 0))?;

    let esp_wifi = setup_wifi(peripherals.modem, sysloop, &app_config).await;

    let config = uart::config::Config::default().baudrate(Hertz(2400));
    info!("CONFIG: {:?}", config);
    let mut uart: uart::UartDriver = uart::UartDriver::new(
        peripherals.uart1,
        peripherals.pins.gpio18,
        peripherals.pins.gpio19,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        &config
    ).unwrap();

/*
    let (tx, rx) = UsbSerialJtag::new_async(peripherals.uart1).split();
*/

    // let command = [81, 80, 79, 77, 83, 183, 169, 13];


    loop {
        led.set_pixel(RGB8::new(0, 0, 5))?;
        info!("sending command: QPIGS");
        let qpigs = send_command(&uart, "QPIGS".to_string(), &parse_qpigs).await;
        let qpigs2 = send_command(&uart, "QPIGS2".to_string(), &parse_qpigs2).await;
        let qpiws = send_command(&uart, "QPIWS".to_string(), &parse_qpiws).await;
        info!("qpigs2: {:?}", qpigs2);
        info!("qpiws: {:?}", qpiws);
        match (qpigs, qpigs2, qpiws) {
            (Some(qpigs), Some(qpigs2), Some(qpiws)) => {
                led.set_pixel(RGB8::new(5, 0, 5))?;
                info!("parts: {:?}", qpigs);
                let current_time = Instant::now().as_secs().to_string();
                info!("Current time: {}", current_time);
                get("https://api.thingspeak.com/update?api_key=".to_owned() + app_config.thingspeak_write_api_key + "&field1=" + &current_time +
                "&field2=" + &qpigs.ac_output_active_power.to_string() +
                "&field3=" + &qpigs.battery_voltage.to_string() +
                "&field4=" + &(&qpigs.battery_charging_current - &qpigs.battery_discharge_current).to_string() +
                "&field5=" + &qpigs.battery_capacity.to_string() +
                "&field6=" + &qpigs.inverter_heat_sink_temperature.to_string() +
                "&field7=" + &qpigs.pv_charging_power1.to_string() +
                "&field8=" + &qpigs2.pv_charging_power2.to_string())?;
                led.set_pixel(RGB8::new(1, 1, 1))?;
            }
            _ => {
                led.set_pixel(RGB8::new(0, 5, 0))?;
                info!("Could not get a proper response for the query!");
            }
        }

        //Timer::after(Duration::from_secs(60)).await;
        Timer::after(Duration::from_secs(15)).await;
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

