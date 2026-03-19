use anyhow::Result;
use config_portal::esp_idf::NvsConfigStore;
use embassy_time::{Duration, Instant, Timer};
use embedded_svc::{
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
use rgb_led::esp_idf::Ws2812RmtBackend;
use rgb_led::{ColorOrder, RgbLed};
use tft_display::esp_idf::{SpiTftBackend, EspDelay};
use tft_display::TftDisplay;
use wifi::esp_idf::EspWifiBackend;
use wifi::Wifi as WifiController;

use info_panel_lib::{
    BootReason, Clock, HttpClient, Platform,
};

#[derive(Clone)]
struct EspPlatform;

impl Platform for EspPlatform {
    fn boot_reason(&self) -> BootReason {
        match ResetReason::get() {
            ResetReason::Software => BootReason::Software,
            ResetReason::ExternalPin => BootReason::ExternalPin,
            ResetReason::Watchdog => BootReason::Watchdog,
            ResetReason::Sdio => BootReason::Sdio,
            ResetReason::Panic => BootReason::Panic,
            ResetReason::InterruptWatchdog => BootReason::InterruptWatchdog,
            ResetReason::PowerOn => BootReason::PowerOn,
            ResetReason::Unknown => BootReason::Unknown,
            ResetReason::Brownout => BootReason::Brownout,
            ResetReason::TaskWatchdog => BootReason::TaskWatchdog,
            ResetReason::DeepSleep => BootReason::DeepSleep,
            ResetReason::USBPeripheral => BootReason::USBPeripheral,
            ResetReason::JTAG => BootReason::JTAG,
            ResetReason::EfuseError => BootReason::EfuseError,
            ResetReason::PowerGlitch => BootReason::PowerGlitch,
            ResetReason::CPULockup => BootReason::CPULockup,
        }
    }

    fn mac_address(&self) -> Result<[u8; 6]> {
        let mut mac = [0u8; 6];
        let ret = unsafe {
            esp_idf_svc::sys::esp_wifi_get_mac(
                esp_idf_svc::sys::wifi_interface_t_WIFI_IF_STA,
                &mut mac as *mut u8,
            )
        };
        if ret != 0 {
            anyhow::bail!("failed to get MAC address: {}", ret);
        }
        Ok(mac)
    }

    fn reboot(&self) -> ! {
        unsafe {
            esp_idf_svc::sys::esp_restart();
        }
    }
}

#[derive(Clone)]
struct EspClock;

impl Clock for EspClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    async fn sleep(&self, duration: Duration) {
        Timer::after(duration).await;
    }
}

struct EspHttpClient;

struct EspHttpFrameSource {
    connection: EspHttpConnection,
}

impl EspHttpFrameSource {
    fn new(connection: EspHttpConnection) -> Self {
        Self { connection }
    }
}

impl tft_display::FrameSource for EspHttpFrameSource {
    type Error = anyhow::Error;

    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, Self::Error> {
        Ok(Read::read(&mut self.connection, buf)?)
    }
}

impl HttpClient for EspHttpClient {
    async fn get(
        &mut self,
        url: &str,
    ) -> Result<Box<dyn tft_display::FrameSource<Error = anyhow::Error>>> {
        let mut connection = EspHttpConnection::new(&HttpConfiguration {
            timeout: Some(core::time::Duration::from_secs(30)),
            use_global_ca_store: false,
            ..Default::default()
        })?;

        connection.initiate_request(embedded_svc::http::Method::Get, url, &[])?;
        connection.initiate_response()?;

        let status = connection.status();
        if !(200..=299).contains(&status) {
            anyhow::bail!("HTTP status {} while downloading {}", status, url);
        }

        Ok(Box::new(EspHttpFrameSource::new(connection)))
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
    let store = NvsConfigStore::new(nvs.clone(), info_panel_lib::CONFIG_NAMESPACE);

    let platform = EspPlatform;

    let modem = peripherals.modem;
    let spi2 = peripherals.spi2;
    let pins = peripherals.pins;

    let mut led = RgbLed::new(Ws2812RmtBackend::new(pins.gpio8)?, ColorOrder::RGB);

    let mut wifi = WifiController::new(EspWifiBackend::new_with_default_nvs(
        modem,
        sysloop,
        Some(nvs),
    )?);

    let http_backend = config_portal::esp_idf::EspHttpBackend::new();
    let clock = EspClock;
    let http_client = EspHttpClient;

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

    let display = TftDisplay::new(SpiTftBackend::new(spi, dc, rst), EspDelay, info_panel_lib::TFT_WIDTH, info_panel_lib::TFT_HEIGHT);

    // run() never returns
    info_panel_lib::run(
        &mut wifi,
        store,
        http_backend,
        platform,
        clock,
        http_client,
        display,
        &mut led,
    )
    .await;
}
