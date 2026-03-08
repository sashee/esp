use anyhow::{anyhow, bail, Result};
use embassy_time::{Duration, Timer};
use embedded_svc::{
    http::{client::Client, Method},
    io::Read,
    wifi::Wifi,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{AnyIOPin, Gpio1, Gpio2, Output, PinDriver},
        modem::Modem,
        peripherals::Peripherals,
        prelude::*,
        spi::{
            config::{Config as SpiConfig, DriverConfig as SpiDriverConfig},
            SpiDeviceDriver, SpiDriver,
        },
        task::block_on,
    },
    http::client::{Configuration as HttpConfiguration, EspHttpConnection},
    wifi::{config::ScanConfig, ClientConfiguration, Configuration as WifiConfiguration, EspWifi},
};
use log::{error, info};
use rgb_led::{RGB8, WS2812RMT};

const TFT_WIDTH: u16 = 128;
const TFT_HEIGHT: u16 = 160;

#[derive(Debug, Clone, Copy)]
struct KnownWifi {
    ssid: &'static str,
    password: &'static str,
}

type SpiDev<'d> = SpiDeviceDriver<'d, SpiDriver<'d>>;
type DcPin<'d> = PinDriver<'d, Gpio2, Output>;
type RstPin<'d> = PinDriver<'d, Gpio1, Output>;

include!(concat!(env!("OUT_DIR"), "/wifi_config.rs"));

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    block_on(async_main())
}

async fn async_main() -> Result<()> {
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let mut pins = peripherals.pins;
    let mut led = WS2812RMT::new(pins.gpio8, peripherals.rmt.channel0)?;

    if KNOWN_WIFIS.is_empty() {
        return reboot_with_error(&mut led, "known_wifis is empty").await;
    }
    if INFO_PANEL_URL.is_empty() {
        return reboot_with_error(&mut led, "info_panel_url is empty").await;
    }

    let mut wifi = match connect_known_wifi(peripherals.modem, sysloop, KNOWN_WIFIS, &mut led).await
    {
        Ok(wifi) => wifi,
        Err(err) => {
            return reboot_with_error(&mut led, &format!("wifi connect failed: {err:#}")).await
        }
    };

    led.set_pixel(RGB8::new(0, 0, 12))?;
    info!("Wi-Fi connected");

    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        pins.gpio4,
        pins.gpio5,
        Option::<AnyIOPin>::None,
        &SpiDriverConfig::new(),
    )?;
    let spi_cfg = SpiConfig::new().baudrate(26.MHz().into());
    let mut spi = SpiDeviceDriver::new(spi_driver, Some(pins.gpio3), &spi_cfg)?;
    let mut dc = PinDriver::output(pins.gpio2)?;
    let mut rst = PinDriver::output(pins.gpio1)?;

    if let Err(err) = init_tft(&mut spi, &mut dc, &mut rst).await {
        return reboot_with_error(&mut led, &format!("tft init failed: {err:#}")).await;
    }
    if let Err(err) = fill_rect(
        &mut spi,
        &mut dc,
        0,
        0,
        TFT_WIDTH,
        TFT_HEIGHT,
        rgb565(0, 0, 0),
    ) {
        return reboot_with_error(&mut led, &format!("tft clear failed: {err:#}")).await;
    }

    Timer::after(Duration::from_millis(500)).await;

    if let Err(err) = fetch_and_draw_rgb565_with_retries(&mut spi, &mut dc, INFO_PANEL_URL).await {
        return reboot_with_error(&mut led, &format!("rgb565 download failed: {err:#}")).await;
    }

    loop {
        Timer::after(Duration::from_secs(30)).await;

        if !wifi.is_connected().unwrap_or(false) {
            return reboot_with_error(&mut led, "wifi disconnected").await;
        }

        if let Err(err) =
            fetch_and_draw_rgb565_with_retries(&mut spi, &mut dc, INFO_PANEL_URL).await
        {
            return reboot_with_error(&mut led, &format!("rgb565 refresh failed: {err:#}")).await;
        }
    }
}

async fn fetch_and_draw_rgb565_with_retries<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    url: &str,
) -> Result<()> {
    let mut last_err: Option<anyhow::Error> = None;
    for _ in 0..3 {
        match download_rgb565(url) {
            Ok(frame_bytes) => {
                info!(
                    "RGB565 frame downloaded successfully: {} bytes",
                    frame_bytes.len()
                );
                draw_rgb565_on_tft(spi, dc, &frame_bytes)?;
                info!("RGB565 frame rendered on TFT");
                return Ok(());
            }
            Err(err) => {
                error!("rgb565 download attempt failed: {err:#}");
                last_err = Some(err);
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    match last_err {
        Some(err) => Err(err),
        None => bail!("rgb565 download failed with unknown error"),
    }
}

async fn init_tft<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    rst: &mut RstPin<'d>,
) -> Result<()> {
    rst.set_high()?;
    Timer::after(Duration::from_millis(20)).await;
    rst.set_low()?;
    Timer::after(Duration::from_millis(20)).await;
    rst.set_high()?;
    Timer::after(Duration::from_millis(150)).await;

    write_cmd(spi, dc, 0x01)?;
    Timer::after(Duration::from_millis(150)).await;

    write_cmd(spi, dc, 0x11)?;
    Timer::after(Duration::from_millis(250)).await;

    write_cmd_data(spi, dc, 0x3A, &[0x05])?;
    write_cmd_data(spi, dc, 0x36, &[0xC8])?;
    write_cmd(spi, dc, 0x20)?;

    write_cmd(spi, dc, 0x13)?;
    Timer::after(Duration::from_millis(10)).await;
    write_cmd(spi, dc, 0x29)?;
    Timer::after(Duration::from_millis(100)).await;

    Ok(())
}

fn set_window<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) -> Result<()> {
    let x1 = x;
    let x2 = x + w - 1;
    let y1 = y;
    let y2 = y + h - 1;

    write_cmd_data(
        spi,
        dc,
        0x2A,
        &[
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
            (x2 >> 8) as u8,
            (x2 & 0xFF) as u8,
        ],
    )?;
    write_cmd_data(
        spi,
        dc,
        0x2B,
        &[
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
            (y2 >> 8) as u8,
            (y2 & 0xFF) as u8,
        ],
    )?;
    write_cmd(spi, dc, 0x2C)?;

    Ok(())
}

fn fill_rect<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    color: u16,
) -> Result<()> {
    set_window(spi, dc, x, y, w, h)?;
    dc.set_high()?;

    let hi = (color >> 8) as u8;
    let lo = (color & 0xFF) as u8;
    let mut line = [0u8; (TFT_WIDTH as usize) * 2];
    for idx in 0..(w as usize) {
        line[idx * 2] = hi;
        line[idx * 2 + 1] = lo;
    }

    for _ in 0..h {
        spi.write(&line[..(w as usize) * 2])?;
    }

    Ok(())
}

fn draw_rgb565_on_tft<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    frame_bytes: &[u8],
) -> Result<()> {
    let expected_len = (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2;
    if frame_bytes.len() != expected_len {
        bail!(
            "unexpected RGB565 payload size {}, expected {}",
            frame_bytes.len(),
            expected_len
        );
    }

    set_window(spi, dc, 0, 0, TFT_WIDTH, TFT_HEIGHT)?;
    dc.set_high()?;
    spi.write(frame_bytes)?;

    Ok(())
}

fn write_cmd<'d>(spi: &mut SpiDev<'d>, dc: &mut DcPin<'d>, cmd: u8) -> Result<()> {
    dc.set_low()?;
    spi.write(&[cmd])?;
    Ok(())
}

fn write_cmd_data<'d>(
    spi: &mut SpiDev<'d>,
    dc: &mut DcPin<'d>,
    cmd: u8,
    data: &[u8],
) -> Result<()> {
    write_cmd(spi, dc, cmd)?;
    dc.set_high()?;
    spi.write(data)?;
    Ok(())
}

const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3)
}

async fn connect_known_wifi(
    modem: Modem,
    sysloop: EspSystemEventLoop,
    known_wifis: &[KnownWifi],
    led: &mut WS2812RMT<'_>,
) -> Result<EspWifi<'static>> {
    led.set_pixel(RGB8::new(12, 10, 0))?;
    info!("Starting Wi-Fi (yellow: scanning/connecting)");

    let mut wifi = EspWifi::new(modem, sysloop, None)?;
    wifi.set_configuration(&WifiConfiguration::Client(ClientConfiguration::default()))?;
    wifi.start()?;

    wifi.start_scan(&ScanConfig::default(), false)?;

    let scan_results = loop {
        if wifi.is_scan_done().unwrap_or(false) {
            break wifi.get_scan_result()?;
        }
        Timer::after(Duration::from_millis(250)).await;
    };

    let selected = known_wifis
        .iter()
        .find(|known| scan_results.iter().any(|ap| ap.ssid.as_str() == known.ssid));

    let selected = selected.ok_or_else(|| anyhow!("none of the configured SSIDs were found"))?;
    info!("Selected configured SSID: {}", selected.ssid);

    let mut client_cfg = ClientConfiguration::default();
    client_cfg.ssid = selected
        .ssid
        .try_into()
        .map_err(|_| anyhow!("SSID is too long"))?;
    client_cfg.password = selected
        .password
        .try_into()
        .map_err(|_| anyhow!("password is too long"))?;

    wifi.set_configuration(&WifiConfiguration::Client(client_cfg))?;
    wifi.connect()?;

    for _ in 0..180 {
        if wifi.is_connected().unwrap_or(false) {
            if let Ok(ip_info) = wifi.sta_netif().get_ip_info() {
                let ip_str = ip_info.ip.to_string();
                if ip_str != "0.0.0.0" {
                    info!(
                        "Wi-Fi netif is up: ip={}, mask={}, dns={:?}",
                        ip_info.ip, ip_info.subnet, ip_info.dns
                    );
                    return Ok(wifi);
                }
            }
        }
        Timer::after(Duration::from_millis(250)).await;
    }

    bail!("timed out waiting for Wi-Fi netif/DHCP")
}

fn download_rgb565(url: &str) -> Result<Vec<u8>> {
    let connection = EspHttpConnection::new(&HttpConfiguration {
        use_global_ca_store: false,
        ..Default::default()
    })?;
    let mut client = Client::wrap(connection);

    let request = client.request(Method::Get, url, &[])?;
    let mut response = request.submit()?;

    let status = response.status();
    if !(200..=299).contains(&status) {
        bail!("HTTP status {} while downloading {}", status, url);
    }

    let mut data = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = Read::read(&mut response, &mut buf)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }

    Ok(data)
}

async fn reboot_with_error(led: &mut WS2812RMT<'_>, message: &str) -> Result<()> {
    error!("{message}");
    let _ = led.set_pixel(RGB8::new(0, 15, 0));
    Timer::after(Duration::from_secs(30)).await;

    unsafe {
        esp_idf_svc::sys::esp_restart();
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
