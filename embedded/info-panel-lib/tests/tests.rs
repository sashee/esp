use info_panel_lib::{
    rgb565, BootReason, Clock, DeviceConfig, Hal, HttpClient, Platform,
    TFT_HEIGHT, TFT_WIDTH,
};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

fn block_on<F: Future>(future: F) -> F::Output {
    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        RawWaker::new(
            core::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(&waker);

    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

struct MockLed {
    colors: Vec<(f32, f32, f32)>,
}

impl MockLed {
    fn new() -> Self {
        Self { colors: Vec::new() }
    }
}

impl rgb_led::RgbLedBackend for MockLed {
    type Error = anyhow::Error;

    fn color_order(&self) -> rgb_led::ColorOrder {
        rgb_led::ColorOrder::RGB
    }

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> anyhow::Result<()> {
        let max = bytes.into_iter().max().unwrap_or(0) as f32;
        let scale = if max == 0.0 { 0.0 } else { 1.0 / max };
        self.colors.push((bytes[0] as f32 * scale, bytes[1] as f32 * scale, bytes[2] as f32 * scale));
        Ok(())
    }
}

impl rgb_led::RgbLedBackend for &mut MockLed {
    type Error = anyhow::Error;

    fn color_order(&self) -> rgb_led::ColorOrder {
        (**self).color_order()
    }

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> anyhow::Result<()> {
        (**self).set_pixel_bytes(bytes)
    }
}

struct MockDisplay;

impl tft_display::TftBackend for MockDisplay {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_dc_high(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_rst_low(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_rst_high(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn write(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

struct MockHttpClient;

impl HttpClient for MockHttpClient {
    async fn get(
        &mut self,
        _url: &str,
    ) -> anyhow::Result<Box<dyn tft_display::FrameSource<Error = anyhow::Error>>> {
        Err(anyhow::anyhow!("should not be called"))
    }
}

#[derive(Clone)]
struct MockPlatform {
    state: Arc<Mutex<MockPlatformState>>,
}

struct MockPlatformState {
    mac: [u8; 6],
    boot_reason: BootReason,
}

impl MockPlatform {
    fn new(mac: [u8; 6], boot_reason: BootReason) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockPlatformState { mac, boot_reason })),
        }
    }
}

impl Platform for MockPlatform {
    fn boot_reason(&self) -> BootReason {
        self.state.lock().unwrap().boot_reason
    }
    fn mac_address(&self) -> anyhow::Result<[u8; 6]> {
        Ok(self.state.lock().unwrap().mac)
    }
    fn reboot(&self) -> ! {
        panic!("mock reboot")
    }
}

#[derive(Clone)]
struct MockClock {
    state: Arc<Mutex<MockClockState>>,
}

struct MockClockState {
    ticks: VecDeque<embassy_time::Instant>,
}

impl MockClock {
    fn from_ticks(ticks: &[u64]) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockClockState {
                ticks: ticks
                    .iter()
                    .copied()
                    .map(embassy_time::Instant::from_ticks)
                    .collect(),
            })),
        }
    }
}

impl Clock for MockClock {
    fn now(&self) -> embassy_time::Instant {
        let mut state = self.state.lock().unwrap();
        if state.ticks.len() > 1 {
            state.ticks.pop_front().unwrap()
        } else {
            *state.ticks.front().unwrap_or(&embassy_time::Instant::from_ticks(0))
        }
    }
    async fn sleep(&self, _duration: embassy_time::Duration) {}
}

#[derive(Clone, Default)]
struct MockWifiBackend {
    state: Arc<Mutex<MockWifiBackendState>>,
}

struct MockWifiBackendState {
    start_configs: Vec<wifi::AccessPointConfig>,
    started: bool,
}

impl Default for MockWifiBackendState {
    fn default() -> Self {
        Self {
            start_configs: Vec::new(),
            started: false,
        }
    }
}

impl wifi::WifiBackend for MockWifiBackend {
    async fn start(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn is_started(&mut self) -> anyhow::Result<bool> {
        Ok(self.state.lock().unwrap().started)
    }
    async fn scan_networks(&mut self) -> anyhow::Result<Vec<wifi::FoundNetwork>> {
        Ok(Vec::new())
    }
    async fn configure_client(
        &mut self,
        _credentials: &wifi::WifiCredentials,
        _channel: Option<u8>,
        _auth: wifi::ClientAuth,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn connect(
        &mut self,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<wifi::ConnectionInfo> {
        Ok(wifi::ConnectionInfo::new("0.0.0.0"))
    }
    async fn is_connected(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn connection_info(&mut self) -> anyhow::Result<Option<wifi::ConnectionInfo>> {
        Ok(None)
    }
    async fn start_access_point(
        &mut self,
        config: &wifi::AccessPointConfig,
    ) -> anyhow::Result<()> {
        self.state.lock().unwrap().start_configs.push(config.clone());
        self.state.lock().unwrap().started = true;
        Ok(())
    }
    async fn stop_access_point(&mut self) -> anyhow::Result<()> {
        self.state.lock().unwrap().started = false;
        Ok(())
    }
    async fn access_point_status(&mut self) -> anyhow::Result<wifi::AccessPointStatus> {
        Ok(wifi::AccessPointStatus {
            is_started: self.state.lock().unwrap().started,
            client_count: 0,
        })
    }
    async fn access_point_ip_config(&mut self) -> anyhow::Result<wifi::IpConfig> {
        Ok(wifi::IpConfig::new(
            "192.168.4.1",
            "192.168.4.1",
            "255.255.255.0",
        ))
    }
}

#[derive(Clone, Default)]
struct MockHttpBackend;

impl config_portal::ConfigHttpBackend for MockHttpBackend {
    type Server = MockServer;

    fn start<H, Fut>(
        self,
        _endpoints: &'static [config_portal::HttpEndpoint],
        _handler: H,
    ) -> anyhow::Result<Self::Server>
    where
        H: Fn(config_portal::HttpRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<config_portal::HttpResponse>> + Send,
    {
        Ok(MockServer)
    }
}

struct MockServer;

#[derive(Clone, Default)]
struct MockStore;

impl config_portal::ConfigStore for MockStore {
    fn read(
        &self,
        _namespace: &str,
        _keys: &[&str],
    ) -> anyhow::Result<BTreeMap<String, String>> {
        Ok(BTreeMap::new())
    }
    fn write(&self, _namespace: &str, _values: &BTreeMap<String, String>) -> anyhow::Result<()> {
        Ok(())
    }
    fn remove(&self, _namespace: &str, _keys: &[&str]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn test_rgb565_red() {
    assert_eq!(rgb565(255, 0, 0), 0xF800);
}

#[test]
fn test_rgb565_green() {
    assert_eq!(rgb565(0, 255, 0), 0x07E0);
}

#[test]
fn test_rgb565_blue() {
    assert_eq!(rgb565(0, 0, 255), 0x001F);
}

#[test]
fn test_rgb565_black() {
    assert_eq!(rgb565(0, 0, 0), 0x0000);
}

#[test]
fn test_rgb565_white() {
    assert_eq!(rgb565(255, 255, 255), 0xFFFF);
}

#[test]
fn test_fill_frame_size() {
    let frame = vec![0u8; (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2];
    assert_eq!(
        frame.len(),
        (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2
    );
}

#[test]
fn test_device_config_from_stored() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());

    let stored = StoredConfig::new(values);
    let config = DeviceConfig::from_stored(stored).unwrap();

    assert_eq!(config.ssid(), "test_ssid");
    assert_eq!(config.password(), "test_password");
    assert_eq!(config.url(), "http://example.com");
    assert!((config.led_brightness() - 0.502).abs() < 0.001); // 128/255 ≈ 0.502
}

#[test]
fn test_device_config_missing_led_brightness() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());

    let stored = StoredConfig::new(values);
    let result = DeviceConfig::from_stored(stored);

    assert!(result.is_err());
}

#[test]
fn test_device_config_invalid_led_brightness() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "not_a_number".to_string());

    let stored = StoredConfig::new(values);
    let result = DeviceConfig::from_stored(stored);

    assert!(result.is_err());
}

#[test]
fn run_enters_ap_mode_when_nvs_empty() {
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let wifi_state = wifi_backend.state.clone();
    let store = MockStore;
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
    let http_client = MockHttpClient;
    let display = MockDisplay;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(Hal {
            wifi_backend,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            tft_backend: display,
            led_backend: &mut led,
        }));
    }));
    let _ = result;

    // LED should have been set to green (REQUIRED_PORTAL_LED = 0.0, 1.0, 0.0)
    assert_eq!(led.colors.last(), Some(&(0.0, 1.0, 0.0)));

    // Wifi AP should have been started with expected SSID
    let state = wifi_state.lock().unwrap();
    assert_eq!(state.start_configs.len(), 1);
    assert_eq!(state.start_configs[0].ssid, "InfoPanel-AABB");
}
