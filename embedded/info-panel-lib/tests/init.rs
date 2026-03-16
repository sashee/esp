use info_panel_lib::{BootReason, Clock, DisplayWrite, HttpClient, Led, Platform};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
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

fn valid_config_store() -> MockStore {
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    MockStore { values }
}

#[derive(Clone)]
struct MockStore {
    values: BTreeMap<String, String>,
}

impl Default for MockStore {
    fn default() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

impl config_portal::ConfigStore for MockStore {
    fn read(&self, _keys: &[&str]) -> anyhow::Result<BTreeMap<String, String>> {
        Ok(self.values.clone())
    }
    fn write(&self, _values: &BTreeMap<String, String>) -> anyhow::Result<()> {
        Ok(())
    }
    fn remove(&self, _keys: &[&str]) -> anyhow::Result<()> {
        Ok(())
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

struct MockLed {
    calls: Vec<LedCall>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LedCall {
    r: f32,
    g: f32,
    b: f32,
    brightness: f32,
}

impl MockLed {
    fn new() -> Self {
        Self { calls: Vec::new() }
    }
}

impl Led for MockLed {
    fn set_pixel(&mut self, rgb: rgb_led::Rgb, brightness: f32) -> anyhow::Result<()> {
        self.calls.push(LedCall {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
            brightness,
        });
        Ok(())
    }
}

struct MockDisplay {
    init_called: Arc<Mutex<bool>>,
    init_order: Arc<Mutex<Option<u32>>>,
    global_counter: Arc<AtomicU32>,
}

impl MockDisplay {
    fn new(global_counter: Arc<AtomicU32>) -> Self {
        Self {
            init_called: Arc::new(Mutex::new(false)),
            init_order: Arc::new(Mutex::new(None)),
            global_counter,
        }
    }
}

impl DisplayWrite for MockDisplay {
    async fn init(&mut self) -> anyhow::Result<()> {
        *self.init_called.lock().unwrap() = true;
        *self.init_order.lock().unwrap() = Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(())
    }
    fn write_frame(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

struct MockHttpClient;

impl MockHttpClient {
    fn new() -> Self {
        Self
    }
}

impl HttpClient for MockHttpClient {
    async fn get(&mut self, _url: &str) -> anyhow::Result<Vec<u8>> {
        Err(anyhow::anyhow!("test: always fail"))
    }
}

#[derive(Clone)]
struct MockPlatform {
    state: Arc<Mutex<MockPlatformState>>,
    reboot_called: Arc<Mutex<bool>>,
}

struct MockPlatformState {
    mac: [u8; 6],
    boot_reason: BootReason,
}

impl MockPlatform {
    fn new(mac: [u8; 6], boot_reason: BootReason) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockPlatformState { mac, boot_reason })),
            reboot_called: Arc::new(Mutex::new(false)),
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
        *self.reboot_called.lock().unwrap() = true;
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
            *state
                .ticks
                .front()
                .unwrap_or(&embassy_time::Instant::from_ticks(0))
        }
    }
    async fn sleep(&self, _duration: embassy_time::Duration) {}
}

#[derive(Clone)]
struct MockWifiBackend {
    connect_order: Arc<Mutex<Option<u32>>>,
    global_counter: Arc<AtomicU32>,
    state: Arc<Mutex<MockWifiBackendState>>,
}

struct MockWifiBackendState {
    started: bool,
}

impl Default for MockWifiBackendState {
    fn default() -> Self {
        Self { started: false }
    }
}

impl Default for MockWifiBackend {
    fn default() -> Self {
        Self {
            connect_order: Arc::new(Mutex::new(None)),
            global_counter: Arc::new(AtomicU32::new(1)),
            state: Arc::new(Mutex::new(MockWifiBackendState { started: false })),
        }
    }
}

impl MockWifiBackend {
    fn with_counter(counter: Arc<AtomicU32>) -> Self {
        Self {
            connect_order: Arc::new(Mutex::new(None)),
            global_counter: counter,
            state: Arc::new(Mutex::new(MockWifiBackendState { started: false })),
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
        *self.connect_order.lock().unwrap() =
            Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(wifi::ConnectionInfo::new("0.0.0.0"))
    }
    async fn is_connected(&mut self) -> anyhow::Result<bool> {
        Ok(true)
    }
    async fn connection_info(&mut self) -> anyhow::Result<Option<wifi::ConnectionInfo>> {
        Ok(Some(wifi::ConnectionInfo::new("0.0.0.0")))
    }
    async fn start_access_point(
        &mut self,
        _config: &wifi::AccessPointConfig,
    ) -> anyhow::Result<()> {
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

#[test]
fn test_init_clears_tft_on_startup() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new(global_counter);

    let init_called = display.init_called.clone();
    let init_order = display.init_order.clone();
    let connect_order = wifi.backend().connect_order.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert!(*init_called.lock().unwrap(), "display.init() must be called");

    let init = init_order.lock().unwrap();
    let connect = connect_order.lock().unwrap();

    assert!(init.is_some(), "display.init() must be called");
    assert!(connect.is_some(), "wifi.connect() must be called");
    assert!(
        init.unwrap() < connect.unwrap(),
        "display.init() (order {:?}) must be called BEFORE wifi.connect() (order {:?})",
        init,
        connect
    );
}

