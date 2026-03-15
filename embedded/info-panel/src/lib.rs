#![feature(never_type)]

use anyhow::{anyhow, bail, Result};
use config_portal::{
    enter_config_mode, AccessPointConfig as PortalAccessPointConfig,
    AccessPointEvent as PortalAccessPointEvent, ConfigClock, ConfigHttpBackend, ConfigPlatform,
    ConfigSpec, ConfigState, ConfigStore, ConfigWifi, IpConfig as PortalIpConfig, StoredConfig,
};
use embassy_time::Duration;
use log::{error, info};
use rgb_led::Rgb;
use wifi::{
    AccessPointConfig as WifiAccessPointConfig, AccessPointEvent as WifiAccessPointEvent,
    IpConfig as WifiIpConfig, Wifi, WifiBackend, WifiCredentials, ConnectState,
};

pub use config_portal::{ConfigTiming, FieldSpec};

pub const TFT_WIDTH: u16 = 128;
pub const TFT_HEIGHT: u16 = 160;

pub fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3)
}

pub fn fill_frame_with_color(color: u16) -> Vec<u8> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootReason {
    Software,
    ExternalPin,
    Watchdog,
    Sdio,
    Panic,
    InterruptWatchdog,
    PowerOn,
    Unknown,
    Brownout,
    TaskWatchdog,
    DeepSleep,
    USBPeripheral,
    JTAG,
    EfuseError,
    PowerGlitch,
    CPULockup,
}

#[allow(async_fn_in_trait)]
pub trait HttpClient {
    async fn get(&mut self, url: &str) -> Result<Vec<u8>>;
}

pub trait Platform {
    fn boot_reason(&self) -> BootReason;
    fn mac_address(&self) -> Result<[u8; 6]>;
    fn reboot(&self) -> !;
}

#[allow(async_fn_in_trait)]
pub trait Clock {
    fn now(&self) -> embassy_time::Instant;
    async fn sleep(&self, duration: Duration);
}

pub struct ConfigPlatformAdapter<P> {
    inner: P,
}

impl<P> ConfigPlatformAdapter<P> {
    pub fn new(inner: P) -> Self {
        Self { inner }
    }
}

impl<P: Platform> ConfigPlatform for ConfigPlatformAdapter<P> {
    fn mac_address(&self) -> Result<[u8; 6]> {
        Platform::mac_address(&self.inner)
    }

    fn reboot(&self) -> ! {
        Platform::reboot(&self.inner)
    }
}

pub struct ConfigClockAdapter<C> {
    inner: C,
}

impl<C> ConfigClockAdapter<C> {
    pub fn new(inner: C) -> Self {
        Self { inner }
    }
}

impl<C: Clock> ConfigClock for ConfigClockAdapter<C> {
    fn now(&self) -> embassy_time::Instant {
        Clock::now(&self.inner)
    }

    async fn sleep(&self, duration: Duration) {
        Clock::sleep(&self.inner, duration).await
    }
}

#[derive(Debug, Clone)]
pub struct DeviceConfig {
    ssid: String,
    password: String,
    url: String,
    led_brightness: u8,
}

impl DeviceConfig {
    pub fn from_stored(config: StoredConfig) -> Result<Self> {
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

    pub fn led_brightness(&self) -> f32 {
        self.led_brightness as f32 / 255.0
    }

    pub fn ssid(&self) -> &str {
        &self.ssid
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

pub struct PortalWifi<'a, W> {
    inner: &'a mut Wifi<W>,
}

impl<'a, W> PortalWifi<'a, W> {
    pub fn new(inner: &'a mut Wifi<W>) -> Self {
        Self { inner }
    }
}

impl<W> ConfigWifi for PortalWifi<'_, W>
where
    W: WifiBackend,
{
    async fn start_access_point(
        &mut self,
        config: &PortalAccessPointConfig,
    ) -> Result<PortalIpConfig> {
        let mut wifi_config = WifiAccessPointConfig::new(
            &config.ssid,
            WifiIpConfig::new(
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
                WifiAccessPointEvent::Started { ip_config } => {
                    on_event(PortalAccessPointEvent::Started {
                        ip_config: PortalIpConfig::new(
                            ip_config.ip,
                            ip_config.gateway,
                            ip_config.netmask,
                        ),
                    });
                }
                WifiAccessPointEvent::ClientCountChanged { client_count } => {
                    on_event(PortalAccessPointEvent::ClientCountChanged { client_count });
                }
                WifiAccessPointEvent::Stopped => on_event(PortalAccessPointEvent::Stopped),
            })
            .await
    }
}

static PORTAL_FIELDS: &[FieldSpec] = &[
    FieldSpec::text("ssid", "Wi-Fi SSID"),
    FieldSpec::password("pw", "Wi-Fi password"),
    FieldSpec::text("url", "Info panel URL"),
    FieldSpec::number("led_brightness", "LED brightness", 0, 255),
];

pub static CONFIG_SPEC: ConfigSpec = ConfigSpec {
    namespace: "config",
    ap_prefix: "InfoPanel",
    title: "Info Panel Setup",
    fields: PORTAL_FIELDS,
};

pub const PREBOOT_PORTAL_TIMING: ConfigTiming = ConfigTiming {
    idle_timeout: Duration::from_secs(30),
    connected_timeout: Duration::from_secs(10 * 60),
};

pub const REQUIRED_PORTAL_TIMING: ConfigTiming = ConfigTiming {
    idle_timeout: Duration::from_secs(60),
    connected_timeout: Duration::from_secs(10 * 60),
};

pub const RUNTIME_ERROR_REBOOT_DELAY: Duration = Duration::from_secs(10 * 60);

const ERROR_LED: Rgb = Rgb::new(1.0, 0.0, 0.0);
const PREBOOT_PORTAL_LED: Rgb = Rgb::new(0.0, 0.53, 1.0);
const REQUIRED_PORTAL_LED: Rgb = Rgb::new(0.0, 1.0, 0.0);
const OFF_LED: Rgb = Rgb::new(0.0, 0.0, 0.0);
const CONNECTING_LED: Rgb = Rgb::new(1.0, 0.78, 0.0);
const CONNECTED_LED: Rgb = Rgb::new(0.0, 0.0, 1.0);

const PORTAL_LED_BRIGHTNESS: f32 = 0.06;

pub trait Led {
    fn set_pixel(&mut self, rgb: Rgb, brightness: f32) -> Result<()>;
}

impl<B> Led for rgb_led::RgbLed<B>
where
    B: rgb_led::RgbLedBackend,
{
    fn set_pixel(&mut self, rgb: Rgb, brightness: f32) -> Result<()> {
        rgb_led::RgbLed::set_pixel(self, rgb, brightness).map_err(|_e| anyhow!("LED error"))
    }
}

pub async fn run<W, S, H, P, Ck, L, HC>(
    wifi: &mut Wifi<W>,
    store: S,
    http_backend: H,
    platform: P,
    clock: Ck,
    mut http_client: HC,
    mut display: impl DisplayWrite,
    led: &mut L,
) -> Result<!>
where
    W: WifiBackend,
    S: ConfigStore + Clone + Send + Sync + 'static,
    H: ConfigHttpBackend,
    P: Platform + Clone,
    Ck: Clock + Clone,
    L: Led,
    HC: HttpClient + Send,
{
    display.init().await?;

    let boot_reason = platform.boot_reason();
    let run_preboot_portal = matches!(boot_reason, BootReason::PowerOn);

    let config = match read_config(&CONFIG_SPEC, &store)? {
        ConfigState::Ready(config) => match DeviceConfig::from_stored(config) {
            Ok(config) => config,
            Err(err) => {
                return run_required_config_mode(
                    platform.clone(),
                    wifi,
                    store,
                    http_backend,
                    clock.clone(),
                    led,
                    &format!("stored configuration is invalid: {err:#}"),
                )
                .await;
            }
        },
        ConfigState::Missing => {
            return run_required_config_mode(
                platform.clone(),
                wifi,
                store,
                http_backend,
                clock.clone(),
                led,
                "configuration missing",
            )
            .await;
        }
        ConfigState::SchemaMismatch(_) => {
            return run_required_config_mode(
                platform.clone(),
                wifi,
                store,
                http_backend,
                clock.clone(),
                led,
                "stored configuration schema mismatch",
            )
            .await;
        }
    };

    if run_preboot_portal {
        if let Err(err) = run_preboot_portal_inner(
            wifi,
            store,
            http_backend,
            platform.clone(),
            clock.clone(),
            led,
        )
        .await
        {
            error!("preboot config portal failed: {err:#}");
        }
    }

    connect_device_wifi(wifi, &config, led).await?;

    led.set_pixel(CONNECTED_LED, config.led_brightness())?;
    info!("Wi-Fi connected");

    display.write_frame(&fill_frame_with_color(rgb565(0, 0, 0)))?;

    Clock::sleep(&clock, Duration::from_millis(500)).await;

    fetch_and_draw_rgb565_with_retries(&mut http_client, &mut display, &clock, config.url()).await?;

    loop {
        Clock::sleep(&clock, Duration::from_secs(30)).await;

        if !wifi.is_connected().await? {
            bail!("wifi disconnected");
        }

        fetch_and_draw_rgb565_with_retries(&mut http_client, &mut display, &clock, config.url()).await?;
    }
}

#[allow(async_fn_in_trait)]
pub trait DisplayWrite {
    async fn init(&mut self) -> Result<()>;
    fn write_frame(&mut self, data: &[u8]) -> Result<()>;
}

impl<B, C> DisplayWrite for tft_display::TftDisplay<B, C>
where
    B: tft_display::TftBackend<Error = anyhow::Error>,
    C: tft_display::Clock,
{
    async fn init(&mut self) -> Result<()> {
        tft_display::TftDisplay::init(self).await
    }

    fn write_frame(&mut self, data: &[u8]) -> Result<()> {
        tft_display::TftDisplay::write_frame(self, data)
    }
}

fn read_config<S>(spec: &'static ConfigSpec, store: &S) -> Result<ConfigState>
where
    S: ConfigStore,
{
    config_portal::read_config(spec, store)
}

async fn run_preboot_portal_inner<W, S, H, P, Ck, L>(
    wifi: &mut Wifi<W>,
    store: S,
    http_backend: H,
    platform: P,
    clock: Ck,
    led: &mut L,
) -> Result<()>
where
    W: WifiBackend,
    S: ConfigStore + Clone + Send + Sync + 'static,
    H: ConfigHttpBackend,
    P: Platform,
    Ck: Clock,
    L: Led,
{
    let _ = led.set_pixel(PREBOOT_PORTAL_LED, PORTAL_LED_BRIGHTNESS);
    let mut portal_wifi = PortalWifi::new(wifi);
    let config_platform = ConfigPlatformAdapter::new(platform);
    let config_clock = ConfigClockAdapter::new(clock);
    enter_config_mode(
        &CONFIG_SPEC,
        "preboot configuration window",
        &mut portal_wifi,
        store,
        http_backend,
        config_platform,
        config_clock,
        PREBOOT_PORTAL_TIMING,
    )
    .await?;
    let _ = led.set_pixel(OFF_LED, PORTAL_LED_BRIGHTNESS);
    Ok(())
}

async fn run_required_config_mode<W, S, H, P, Ck, L>(
    platform: P,
    wifi: &mut Wifi<W>,
    store: S,
    http_backend: H,
    clock: Ck,
    led: &mut L,
    message: &str,
) -> Result<!>
where
    W: WifiBackend,
    S: ConfigStore + Clone + Send + Sync + 'static,
    H: ConfigHttpBackend,
    P: Platform + Clone,
    Ck: Clock + Clone,
    L: Led,
{
    error!("{message}");
    let _ = led.set_pixel(REQUIRED_PORTAL_LED, PORTAL_LED_BRIGHTNESS);
    let mut portal_wifi = PortalWifi::new(wifi);
    let config_platform = ConfigPlatformAdapter::new(platform.clone());
    let config_clock = ConfigClockAdapter::new(clock.clone());
    enter_config_mode(
        &CONFIG_SPEC,
        message,
        &mut portal_wifi,
        store,
        http_backend,
        config_platform,
        config_clock,
        REQUIRED_PORTAL_TIMING,
    )
    .await?;
    Platform::reboot(&platform);
}

async fn connect_device_wifi<W, L>(wifi: &mut Wifi<W>, config: &DeviceConfig, led: &mut L) -> Result<()>
where
    W: WifiBackend,
    L: Led,
{
    let brightness = config.led_brightness();
    let connection = wifi
        .connect(
            &WifiCredentials::new(config.ssid(), config.password()),
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

async fn fetch_and_draw_rgb565_with_retries<HC, D, Ck>(
    http_client: &mut HC,
    display: &mut D,
    clock: &Ck,
    url: &str,
) -> Result<()>
where
    HC: HttpClient,
    D: DisplayWrite,
    Ck: Clock,
{
    let mut last_err: Option<anyhow::Error> = None;
    for _ in 0..3 {
        match http_client.get(url).await {
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
                Clock::sleep(clock, Duration::from_secs(1)).await;
            }
        }
    }

    match last_err {
        Some(err) => Err(err),
        None => bail!("rgb565 download failed with unknown error"),
    }
}
