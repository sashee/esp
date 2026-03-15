use anyhow::{anyhow, bail, Result};
use config_portal::esp_idf::{EspClock, EspHttpBackend, EspPlatform, NvsConfigStore};
use config_portal::{
    enter_config_mode, read_config, AccessPointConfig as PortalAccessPointConfig,
    AccessPointEvent as PortalAccessPointEvent, ConfigSpec, ConfigState, ConfigStore, ConfigTiming,
    ConfigWifi, FieldSpec, IpConfig as PortalIpConfig, StoredConfig,
};
use embassy_time::{Duration, Timer};
use embedded_svc::{
    http::{client::Client, Method},
    io::Read,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{AnyIOPin, PinDriver},
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
};
use log::{error, info};
use rgb_led::esp_idf::Ws2812RmtBackend;
use rgb_led::{ColorOrder, Rgb, RgbLed};
use std::string::String;
use tft_display::esp_idf::{SpiTftBackend, EspDelay};
use tft_display::{TftBackend, TftDisplay};
use wifi::{esp_idf::EspWifiBackend, ConnectState, Wifi as WifiController, WifiCredentials};

static PORTAL_FIELDS: &[FieldSpec] = &[
    FieldSpec::text("ssid", "Wi-Fi SSID"),
    FieldSpec::password("pw", "Wi-Fi password"),
    FieldSpec::text("url", "Info panel URL"),
    FieldSpec::number("led_brightness", "LED brightness", 0, 255),
];

static CONFIG_SPEC: ConfigSpec = ConfigSpec {
    namespace: "config",
    ap_prefix: "InfoPanel",
    title: "Info Panel Setup",
    fields: PORTAL_FIELDS,
};

const PREBOOT_PORTAL_TIMING: ConfigTiming = ConfigTiming {
    idle_timeout: Duration::from_secs(30),
    connected_timeout: Duration::from_secs(10 * 60),
};

const REQUIRED_PORTAL_TIMING: ConfigTiming = ConfigTiming {
    idle_timeout: Duration::from_secs(60),
    connected_timeout: Duration::from_secs(10 * 60),
};

const RUNTIME_ERROR_REBOOT_DELAY: Duration = Duration::from_secs(10 * 60);
const PORTAL_LED_BRIGHTNESS: f32 = 0.06;
const CONNECTING_LED: Rgb = Rgb::new(1.0, 0.78, 0.0);
const CONNECTED_LED: Rgb = Rgb::new(0.0, 0.0, 1.0);

const TFT_WIDTH: u16 = 128;
const TFT_HEIGHT: u16 = 160;

fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3)
}

fn fill_frame_with_color(color: u16) -> Vec<u8> {
    let pixel_count = (TFT_WIDTH as usize) * (TFT_HEIGHT as usize);
    let mut frame = Vec::with_capacity(pixel_count * 2);
    let hi = (color >> 8) as u8;
    let lo = (color & 0xFF) as u8;
    for _ in 0..pixel_count {
        frame.push(hi);
        frame.push(lo);
    }
    frame
}
const ERROR_LED: Rgb = Rgb::new(1.0, 0.0, 0.0);
const PREBOOT_PORTAL_LED: Rgb = Rgb::new(0.0, 0.53, 1.0);
const REQUIRED_PORTAL_LED: Rgb = Rgb::new(0.0, 1.0, 0.0);
const OFF_LED: Rgb = Rgb::new(0.0, 0.0, 0.0);

type Led<'d> = RgbLed<Ws2812RmtBackend<'d>>;
type DeviceWifi<'d> = WifiController<EspWifiBackend<'d>>;

struct PortalWifi<'a, 'd> {
    inner: &'a mut DeviceWifi<'d>,
}

impl<'a, 'd> PortalWifi<'a, 'd> {
    fn new(inner: &'a mut DeviceWifi<'d>) -> Self {
        Self { inner }
    }
}

impl ConfigWifi for PortalWifi<'_, 'static> {
    async fn start_access_point(
        &mut self,
        config: &PortalAccessPointConfig,
    ) -> Result<PortalIpConfig> {
        let mut wifi_config = wifi::AccessPointConfig::new(
            &config.ssid,
            wifi::IpConfig::new(
                &config.ip_config.ip,
                &config.ip_config.gateway,
                &config.ip_config.netmask,
            ),
        );
        wifi_config.channel = config.channel;
        wifi_config.max_connections = config.max_connections;

        let ip_config = self.inner.start_access_point(&wifi_config).await?;
        Ok(PortalIpConfig::new(
            ip_config.ip,
            ip_config.gateway,
            ip_config.netmask,
        ))
    }

    async fn stop_access_point(&mut self) -> Result<()> {
        self.inner.stop_access_point().await
    }

    async fn is_access_point_started(&mut self) -> Result<bool> {
        self.inner.is_started().await
    }

    async fn poll_access_point_events<F>(&mut self, mut on_event: F) -> Result<()>
    where
        F: FnMut(PortalAccessPointEvent),
    {
        self.inner
            .poll_access_point_events(|event| match event {
                wifi::AccessPointEvent::Started { ip_config } => {
                    on_event(PortalAccessPointEvent::Started {
                        ip_config: PortalIpConfig::new(
                            ip_config.ip,
                            ip_config.gateway,
                            ip_config.netmask,
                        ),
                    });
                }
                wifi::AccessPointEvent::ClientCountChanged { client_count } => {
                    on_event(PortalAccessPointEvent::ClientCountChanged { client_count });
                }
                wifi::AccessPointEvent::Stopped => on_event(PortalAccessPointEvent::Stopped),
            })
            .await
    }
}

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

    fn led_brightness(&self) -> f32 {
        self.led_brightness as f32 / 255.0
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
    let store = NvsConfigStore::new(nvs.clone(), CONFIG_SPEC.namespace);
    let reset_reason = ResetReason::get();
    info!("boot reset reason: {:?}", reset_reason);

    let modem = peripherals.modem;
    let spi2 = peripherals.spi2;
    let pins = peripherals.pins;
    let mut led = RgbLed::new(Ws2812RmtBackend::new(pins.gpio8)?, ColorOrder::RGB);
    let mut wifi = WifiController::new(EspWifiBackend::new_with_default_nvs(
        modem,
        sysloop,
        Some(nvs.clone()),
    )?);

    let config = match read_config(&CONFIG_SPEC, &store)? {
        ConfigState::Ready(config) => match DeviceConfig::from_stored(config) {
            Ok(config) => config,
            Err(err) => {
                return enter_required_config_mode(
                    &mut led,
                    &mut wifi,
                    store,
                    &format!("stored configuration is invalid: {err:#}"),
                )
                .await;
            }
        },
        ConfigState::Missing => {
            return enter_required_config_mode(&mut led, &mut wifi, store, "configuration missing")
                .await;
        }
        ConfigState::SchemaMismatch(_) => {
            return enter_required_config_mode(
                &mut led,
                &mut wifi,
                store,
                "stored configuration schema mismatch",
            )
            .await;
        }
    };

    if should_offer_preboot_config(reset_reason) {
        if let Err(err) = maybe_run_preboot_config_portal(&mut led, &mut wifi, store.clone()).await
        {
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

        led.set_pixel(CONNECTED_LED, config.led_brightness())?;
        info!("Wi-Fi connected");

        let spi_driver = SpiDriver::new(
            spi2,
            pins.gpio4,
            pins.gpio3,
            Option::<AnyIOPin>::None,
            &SpiDriverConfig::new(),
        )?;

        let spi_cfg = SpiConfig::new().baudrate(26.MHz().into());
        let spi = SpiDeviceDriver::new(spi_driver, Some(pins.gpio5), &spi_cfg)?;

        let dc = PinDriver::output(pins.gpio2)?;
        let rst = PinDriver::output(pins.gpio1)?;

        let mut display = TftDisplay::new(SpiTftBackend::new(spi, dc, rst), EspDelay, 128, 160);
        display.init().await?;

        display.write_frame(&fill_frame_with_color(rgb565(0, 0, 0)))?;

        Timer::after(Duration::from_millis(500)).await;

        fetch_and_draw_rgb565_with_retries(&mut display, &config.url).await?;

        loop {
            Timer::after(Duration::from_secs(30)).await;

            if !wifi.is_connected().await? {
                bail!("wifi disconnected");
            }

            fetch_and_draw_rgb565_with_retries(&mut display, &config.url).await?;
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

async fn maybe_run_preboot_config_portal<S>(
    led: &mut Led<'_>,
    wifi: &mut DeviceWifi<'static>,
    store: S,
) -> Result<()>
where
    S: ConfigStore + Clone + Send + Sync + 'static,
{
    let _ = led.set_pixel(PREBOOT_PORTAL_LED, PORTAL_LED_BRIGHTNESS);
    let mut portal_wifi = PortalWifi::new(wifi);
    enter_config_mode(
        &CONFIG_SPEC,
        "preboot configuration window",
        &mut portal_wifi,
        store,
        EspHttpBackend::new(),
        EspPlatform::new(),
        EspClock::new(),
        PREBOOT_PORTAL_TIMING,
    )
    .await?;
    let _ = led.set_pixel(OFF_LED, PORTAL_LED_BRIGHTNESS);
    Ok(())
}

async fn enter_required_config_mode<S>(
    led: &mut Led<'_>,
    wifi: &mut DeviceWifi<'static>,
    store: S,
    message: &str,
) -> Result<()>
where
    S: ConfigStore + Clone + Send + Sync + 'static,
{
    error!("{message}");
    let _ = led.set_pixel(REQUIRED_PORTAL_LED, PORTAL_LED_BRIGHTNESS);
    let mut portal_wifi = PortalWifi::new(wifi);
    enter_config_mode(
        &CONFIG_SPEC,
        message,
        &mut portal_wifi,
        store,
        EspHttpBackend::new(),
        EspPlatform::new(),
        EspClock::new(),
        REQUIRED_PORTAL_TIMING,
    )
    .await?;
    reboot();
}

async fn handle_runtime_error(
    led: &mut Led<'_>,
    wifi: &mut DeviceWifi<'static>,
    config: &DeviceConfig,
    message: &str,
) -> Result<()> {
    error!("runtime error: {message}");
    let _ = led.set_pixel(ERROR_LED, config.led_brightness());
    let _ = wifi.reset().await;
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
    wifi: &mut DeviceWifi<'static>,
    config: &DeviceConfig,
    led: &mut Led<'_>,
) -> Result<()> {
    let brightness = config.led_brightness();
    let connection = wifi
        .connect(
            &WifiCredentials::new(&config.ssid, &config.password),
            |state| match state {
                ConnectState::Starting => {
                    let _ = led.set_pixel(CONNECTING_LED, brightness);
                    info!("Starting Wi-Fi (yellow: connecting)");
                }
                ConnectState::Scanning => info!("Scanning for Wi-Fi networks"),
                ConnectState::ScanComplete { networks_found } => {
                    info!("Wi-Fi scan complete: {networks_found} networks found");
                }
                ConnectState::Configuring { ssid, channel, .. } => {
                    info!(
                        "Configuring Wi-Fi client for SSID {ssid} on channel {:?}",
                        channel
                    );
                }
                ConnectState::Connecting => info!("Connecting to Wi-Fi access point"),
                ConnectState::WaitingForIp => info!("Waiting for Wi-Fi DHCP lease"),
                ConnectState::Connected { ip } => {
                    info!("Wi-Fi netif is up: ip={ip}");
                }
            },
        )
        .await?;

    info!("Wi-Fi connected with IP {}", connection.ip);
    Ok(())
}

async fn fetch_and_draw_rgb565_with_retries<B, C>(display: &mut TftDisplay<B, C>, url: &str) -> Result<()>
where
    B: TftBackend<Error = anyhow::Error>,
    C: tft_display::Clock,
{
    let mut last_err: Option<anyhow::Error> = None;
    for _ in 0..3 {
        match download_rgb565(url) {
            Ok(frame_bytes) => {
                info!(
                    "RGB565 frame downloaded successfully: {} bytes",
                    frame_bytes.len()
                );
                display.write_frame(&frame_bytes)?;
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
