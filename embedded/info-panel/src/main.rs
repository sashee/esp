use anyhow::{anyhow, bail, Result};
use config_portal::{
    enter_config_mode, read_nvs, FieldSpec, NvsConfigState, PortalSpec, PortalTiming, StoredConfig,
};
use embassy_time::{Duration, Timer};
use embedded_svc::{
    http::{client::Client, Method},
    io::Read,
    wifi::Wifi,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{AnyIOPin, Output, PinDriver},
        peripherals::Peripherals,
        reset::ResetReason,
        spi::{
            config::{Config as SpiConfig, DriverConfig as SpiDriverConfig},
            SpiDeviceDriver, SpiDriver,
        },
        task::block_on,
        units::FromValueType,
    },
    http::client::{Configuration as HttpConfiguration, EspHttpConnection},
    nvs::EspDefaultNvsPartition,
    wifi::{AuthMethod, ClientConfiguration, Configuration as WifiConfiguration, EspWifi},
};
use log::{error, info};
use rgb_led::{RGB8, WS2812RMT};
use std::string::String;

const TFT_WIDTH: u16 = 128;
const TFT_HEIGHT: u16 = 160;

static PORTAL_FIELDS: &[FieldSpec] = &[
    FieldSpec::text("ssid", "Wi-Fi SSID"),
    FieldSpec::password("pw", "Wi-Fi password"),
    FieldSpec::text("url", "Info panel URL"),
    FieldSpec::number("led_brightness", "LED brightness", 0, 255),
];

static PORTAL_SPEC: PortalSpec = PortalSpec {
    namespace: "config",
    ap_prefix: "InfoPanel",
    title: "Info Panel Setup",
    fields: PORTAL_FIELDS,
};

const PREBOOT_PORTAL_TIMING: PortalTiming = PortalTiming {
    idle_timeout: Duration::from_secs(30),
    connected_timeout: Duration::from_secs(10 * 60),
};

const REQUIRED_PORTAL_TIMING: PortalTiming = PortalTiming {
    idle_timeout: Duration::from_secs(60),
    connected_timeout: Duration::from_secs(10 * 60),
};

const RUNTIME_ERROR_REBOOT_DELAY: Duration = Duration::from_secs(10 * 60);

type SpiDev<'d> = SpiDeviceDriver<'d, SpiDriver<'d>>;
type DcPin<'d> = PinDriver<'d, Output>;
type RstPin<'d> = PinDriver<'d, Output>;

#[derive(Debug, Clone)]
struct DeviceConfig {
    ssid: String,
    password: String,
    url: String,
    led_brightness: u8,
}

impl DeviceConfig {
    fn from_stored(config: StoredConfig) -> Result<Self> {
        Ok(Self {
            ssid: config.get("ssid").unwrap_or("").to_string(),
            password: config.get("pw").unwrap_or("").to_string(),
            url: config.get("url").unwrap_or("").to_string(),
            led_brightness: config
                .get("led_brightness")
                .ok_or_else(|| anyhow!("LED brightness missing from stored config"))?
                .parse()
                .map_err(|_| anyhow!("LED brightness is not a valid u8"))?,
        })
    }
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    block_on(async_main())
}

async fn async_main() -> Result<()> {
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let reset_reason = ResetReason::get();
    info!("boot reset reason: {:?}", reset_reason);

    let modem = peripherals.modem;
    let spi2 = peripherals.spi2;
    let pins = peripherals.pins;
    let mut led = WS2812RMT::new(pins.gpio8)?;
    let mut wifi = EspWifi::new(modem, sysloop, Some(nvs.clone()))?;

    let config = match read_nvs(&PORTAL_SPEC, nvs.clone())? {
        NvsConfigState::Ready(config) => match DeviceConfig::from_stored(config) {
            Ok(config) => config,
            Err(err) => {
                return enter_required_config_mode(
                    &mut led,
                    &mut wifi,
                    nvs,
                    &format!("stored configuration is invalid: {err:#}"),
                )
                .await;
            }
        },
        NvsConfigState::Missing => {
            return enter_required_config_mode(&mut led, &mut wifi, nvs, "configuration missing")
                .await;
        }
        NvsConfigState::SchemaMismatch(_) => {
            return enter_required_config_mode(
                &mut led,
                &mut wifi,
                nvs,
                "stored configuration schema mismatch",
            )
            .await;
        }
    };

    if should_offer_preboot_config(reset_reason) {
        if let Err(err) = maybe_run_preboot_config_portal(&mut led, &mut wifi, nvs.clone()).await {
            error!("preboot config portal failed: {err:#}");
        }
    } else {
        info!(
            "skipping preboot config portal for reset reason: {:?}",
            reset_reason
        );
    }

    let managed_run = async {
        connect_device_wifi(&mut wifi, &config, &mut led).await?;

        led.set_pixel(brightness_color(config.led_brightness, 0, 0, 255))?;
        info!("Wi-Fi connected");

        let spi_driver = SpiDriver::new(
            spi2,
            pins.gpio4,
            pins.gpio3,
            Option::<AnyIOPin>::None,
            &SpiDriverConfig::new(),
        )?;

        let spi_cfg = SpiConfig::new().baudrate(26.MHz().into());
        let mut spi = SpiDeviceDriver::new(spi_driver, Some(pins.gpio5), &spi_cfg)?;

        let mut dc = PinDriver::output(pins.gpio2)?;

        let mut rst = PinDriver::output(pins.gpio1)?;

        init_tft(&mut spi, &mut dc, &mut rst).await?;

        fill_rect(
            &mut spi,
            &mut dc,
            0,
            0,
            TFT_WIDTH,
            TFT_HEIGHT,
            rgb565(0, 0, 0),
        )?;

        Timer::after(Duration::from_millis(500)).await;

        fetch_and_draw_rgb565_with_retries(&mut spi, &mut dc, &config.url).await?;

        loop {
            Timer::after(Duration::from_secs(30)).await;

            if !wifi.is_connected().unwrap_or(false) {
                bail!("wifi disconnected");
            }

            fetch_and_draw_rgb565_with_retries(&mut spi, &mut dc, &config.url).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(err) = managed_run {
        return handle_runtime_error(&mut led, &mut wifi, &config, &format!("{err:#}")).await;
    }

    Ok(())
}

async fn maybe_run_preboot_config_portal(
    led: &mut WS2812RMT<'_>,
    wifi: &mut EspWifi<'static>,
    nvs: EspDefaultNvsPartition,
) -> Result<()> {
    let _ = led.set_pixel(RGB8::new(0, 8, 15));
    enter_config_mode(
        &PORTAL_SPEC,
        "preboot configuration window",
        wifi,
        nvs,
        PREBOOT_PORTAL_TIMING,
    )
    .await?;
    let _ = led.set_pixel(RGB8::new(0, 0, 0));
    Ok(())
}

async fn enter_required_config_mode(
    led: &mut WS2812RMT<'_>,
    wifi: &mut EspWifi<'static>,
    nvs: EspDefaultNvsPartition,
    message: &str,
) -> Result<()> {
    error!("{message}");
    let _ = led.set_pixel(RGB8::new(0, 15, 0));
    enter_config_mode(&PORTAL_SPEC, message, wifi, nvs, REQUIRED_PORTAL_TIMING).await?;
    reboot();
}

async fn handle_runtime_error(
    led: &mut WS2812RMT<'_>,
    wifi: &mut EspWifi<'static>,
    config: &DeviceConfig,
    message: &str,
) -> Result<()> {
    error!("runtime error: {message}");
    let _ = led.set_pixel(brightness_color(config.led_brightness, 0, 255, 0));
    let _ = wifi.disconnect();
    let _ = wifi.stop();
    info!(
        "waiting {:?} before restart after runtime error",
        RUNTIME_ERROR_REBOOT_DELAY
    );
    Timer::after(RUNTIME_ERROR_REBOOT_DELAY).await;
    reboot();
}

fn should_offer_preboot_config(reset_reason: ResetReason) -> bool {
    matches!(reset_reason, ResetReason::PowerOn)
}

async fn connect_device_wifi(
    wifi: &mut EspWifi<'static>,
    config: &DeviceConfig,
    led: &mut WS2812RMT<'_>,
) -> Result<()> {
    led.set_pixel(brightness_color(config.led_brightness, 255, 200, 0))?;
    info!("Starting Wi-Fi (yellow: connecting)");

    let _ = wifi.disconnect();
    let _ = wifi.stop();

    let mut client_cfg = ClientConfiguration::default();
    client_cfg.ssid = config
        .ssid
        .as_str()
        .try_into()
        .map_err(|_| anyhow!("SSID is too long"))?;
    client_cfg.password = config
        .password
        .as_str()
        .try_into()
        .map_err(|_| anyhow!("password is too long"))?;

    if config.password.is_empty() {
        client_cfg.auth_method = AuthMethod::None;
    }

    wifi.set_configuration(&WifiConfiguration::Client(client_cfg))?;
    wifi.start()?;
    wifi.connect()?;

    for _ in 0..180 {
        if wifi.is_connected().unwrap_or(false) {
            if let Ok(ip_info) = wifi.sta_netif().get_ip_info() {
                if ip_info.ip.to_string() != "0.0.0.0" {
                    info!(
                        "Wi-Fi netif is up: ip={}, mask={}, dns={:?}",
                        ip_info.ip, ip_info.subnet, ip_info.dns
                    );
                    return Ok(());
                }
            }
        }

        Timer::after(Duration::from_millis(250)).await;
    }

    bail!("timed out waiting for Wi-Fi netif/DHCP")
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

fn download_rgb565(url: &str) -> Result<Vec<u8>> {
    let connection = EspHttpConnection::new(&HttpConfiguration {
        timeout: Some(core::time::Duration::from_secs(30)),
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
    let mut buf = [0u8; 256];
    loop {
        let n = Read::read(&mut response, &mut buf)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }

    Ok(data)
}

fn reboot() -> ! {
    unsafe {
        esp_idf_svc::sys::esp_restart();
    }
}

fn brightness_color(brightness: u8, r: u8, g: u8, b: u8) -> RGB8 {
    fn scale(brightness: u8, channel: u8) -> u8 {
        ((brightness as u16 * channel as u16) / 255) as u8
    }

    RGB8::new(
        scale(brightness, r),
        scale(brightness, g),
        scale(brightness, b),
    )
}
