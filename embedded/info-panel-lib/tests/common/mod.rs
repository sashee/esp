#![allow(dead_code, unused_imports)]

use embassy_time::Duration;
use info_panel_lib::{BootReason, Clock, DisplayWrite, HttpClient, Led, Platform};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub fn block_on<F: Future>(future: F) -> F::Output {
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

// ---- MockStore ----

pub fn valid_config_store() -> MockStore {
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    MockStore { values }
}

pub fn empty_config_store() -> MockStore {
    MockStore {
        values: BTreeMap::new(),
    }
}

pub fn config_store_with_values(values: BTreeMap<String, String>) -> MockStore {
    MockStore { values }
}

#[derive(Clone)]
pub struct MockStore {
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

// ---- MockHttpBackend ----

#[derive(Clone, Default)]
pub struct MockHttpBackend;

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

pub struct MockServer;

// ---- MockLed ----

pub struct MockLed {
    calls: Vec<LedCall>,
    return_error: bool,
}

#[derive(Debug, Clone)]
pub struct LedCall {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub brightness: f32,
}

impl MockLed {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            return_error: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            calls: Vec::new(),
            return_error: true,
        }
    }

    pub fn calls(&self) -> &[LedCall] {
        &self.calls
    }

    pub fn last_call(&self) -> Option<&LedCall> {
        self.calls.last()
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
        if self.return_error {
            Err(anyhow::anyhow!("mock LED error"))
        } else {
            Ok(())
        }
    }
}

// ---- MockDisplay ----

pub struct MockDisplay {
    pub init_called: Arc<Mutex<bool>>,
    pub init_order: Arc<Mutex<Option<u32>>>,
    pub write_frame_calls: Arc<Mutex<usize>>,
    pub write_frame_fail_nth: Arc<Mutex<Option<usize>>>,
    global_counter: Arc<AtomicU32>,
}

impl MockDisplay {
    pub fn new(global_counter: Arc<AtomicU32>) -> Self {
        Self {
            init_called: Arc::new(Mutex::new(false)),
            init_order: Arc::new(Mutex::new(None)),
            write_frame_calls: Arc::new(Mutex::new(0)),
            write_frame_fail_nth: Arc::new(Mutex::new(None)),
            global_counter,
        }
    }

    pub fn with_write_frame_fail_nth(
        global_counter: Arc<AtomicU32>,
        fail_nth: usize,
    ) -> Self {
        Self {
            init_called: Arc::new(Mutex::new(false)),
            init_order: Arc::new(Mutex::new(None)),
            write_frame_calls: Arc::new(Mutex::new(0)),
            write_frame_fail_nth: Arc::new(Mutex::new(Some(fail_nth))),
            global_counter,
        }
    }
}

impl DisplayWrite for MockDisplay {
    async fn init(&mut self) -> anyhow::Result<()> {
        *self.init_called.lock().unwrap() = true;
        *self.init_order.lock().unwrap() =
            Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(())
    }
    fn write_frame(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        let mut count = self.write_frame_calls.lock().unwrap();
        *count += 1;
        if let Some(fail_nth) = *self.write_frame_fail_nth.lock().unwrap() {
            if *count == fail_nth {
                return Err(anyhow::anyhow!("mock write_frame error"));
            }
        }
        Ok(())
    }
}

// ---- MockHttpClient ----

pub struct MockHttpClient {
    pub get_calls: Arc<Mutex<usize>>,
    pub get_urls: Arc<Mutex<Vec<String>>>,
    pub get_fail_nth: Arc<Mutex<Option<usize>>>,
    pub get_always_fail: bool,
    pub get_panic_on_nth: Arc<Mutex<Option<usize>>>,
    pub get_custom_response: Arc<Mutex<Option<Vec<u8>>>>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            get_calls: Arc::new(Mutex::new(0)),
            get_urls: Arc::new(Mutex::new(Vec::new())),
            get_fail_nth: Arc::new(Mutex::new(None)),
            get_always_fail: false,
            get_panic_on_nth: Arc::new(Mutex::new(None)),
            get_custom_response: Arc::new(Mutex::new(None)),
        }
    }

    pub fn always_failing() -> Self {
        Self {
            get_calls: Arc::new(Mutex::new(0)),
            get_urls: Arc::new(Mutex::new(Vec::new())),
            get_fail_nth: Arc::new(Mutex::new(None)),
            get_always_fail: true,
            get_panic_on_nth: Arc::new(Mutex::new(None)),
            get_custom_response: Arc::new(Mutex::new(None)),
        }
    }

    pub fn fail_up_to(n: usize) -> Self {
        Self {
            get_calls: Arc::new(Mutex::new(0)),
            get_urls: Arc::new(Mutex::new(Vec::new())),
            get_fail_nth: Arc::new(Mutex::new(Some(n))),
            get_always_fail: false,
            get_panic_on_nth: Arc::new(Mutex::new(None)),
            get_custom_response: Arc::new(Mutex::new(None)),
        }
    }

    pub fn panic_on_nth(n: usize) -> Self {
        Self {
            get_calls: Arc::new(Mutex::new(0)),
            get_urls: Arc::new(Mutex::new(Vec::new())),
            get_fail_nth: Arc::new(Mutex::new(None)),
            get_always_fail: false,
            get_panic_on_nth: Arc::new(Mutex::new(Some(n))),
            get_custom_response: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_custom_response(data: Vec<u8>) -> Self {
        Self {
            get_calls: Arc::new(Mutex::new(0)),
            get_urls: Arc::new(Mutex::new(Vec::new())),
            get_fail_nth: Arc::new(Mutex::new(None)),
            get_always_fail: false,
            get_panic_on_nth: Arc::new(Mutex::new(None)),
            get_custom_response: Arc::new(Mutex::new(Some(data))),
        }
    }
}

impl HttpClient for MockHttpClient {
    async fn get(&mut self, url: &str) -> anyhow::Result<Vec<u8>> {
        let current;
        let should_panic;
        {
            let mut count = self.get_calls.lock().unwrap();
            *count += 1;
            current = *count;
            self.get_urls.lock().unwrap().push(url.to_string());
            should_panic = self
                .get_panic_on_nth
                .lock()
                .unwrap()
                .map_or(false, |n| current == n);
        }
        if should_panic {
            panic!("mock: http_client.get() call #{} reached", current);
        }
        if self.get_always_fail {
            return Err(anyhow::anyhow!("mock HTTP error"));
        }
        if let Some(fail_up_to) = *self.get_fail_nth.lock().unwrap() {
            if current <= fail_up_to {
                return Err(anyhow::anyhow!("mock HTTP error"));
            }
        }
        if let Some(ref data) = *self.get_custom_response.lock().unwrap() {
            return Ok(data.clone());
        }
        Ok(vec![0u8; 128 * 160 * 2])
    }
}

// ---- MockPlatform ----

#[derive(Clone)]
pub struct MockPlatform {
    pub state: Arc<Mutex<MockPlatformState>>,
    pub reboot_called: Arc<Mutex<bool>>,
}

pub struct MockPlatformState {
    pub mac: [u8; 6],
    pub boot_reason: BootReason,
}

impl MockPlatform {
    pub fn new(mac: [u8; 6], boot_reason: BootReason) -> Self {
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

// ---- MockClock ----

#[derive(Clone)]
pub struct MockClock {
    state: Arc<Mutex<MockClockState>>,
    pub sleep_durations: Arc<Mutex<Vec<Duration>>>,
}

pub struct MockClockState {
    ticks: VecDeque<embassy_time::Instant>,
}

impl MockClock {
    pub fn from_ticks(ticks: &[u64]) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockClockState {
                ticks: ticks
                    .iter()
                    .copied()
                    .map(embassy_time::Instant::from_ticks)
                    .collect(),
            })),
            sleep_durations: Arc::new(Mutex::new(Vec::new())),
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
    async fn sleep(&self, duration: Duration) {
        self.sleep_durations.lock().unwrap().push(duration);
    }
}

// ---- MockWifiBackend ----

#[derive(Clone)]
pub struct MockWifiBackend {
    pub connect_order: Arc<Mutex<Option<u32>>>,
    pub scan_order: Arc<Mutex<Option<u32>>>,
    pub start_ap_order: Arc<Mutex<Option<u32>>>,
    pub global_counter: Arc<AtomicU32>,
    pub state: Arc<Mutex<MockWifiBackendState>>,
}

pub struct MockWifiBackendState {
    pub started: bool,
    pub is_connected: bool,
    pub client_count: usize,
    pub scan_networks_result: Vec<wifi::FoundNetwork>,
    pub configured_ssid: Option<String>,
    pub configured_password: Option<String>,
    pub start_access_point_ssid: Option<String>,
    pub fail_configure_client: bool,
    pub fail_connect: bool,
}

impl Default for MockWifiBackendState {
    fn default() -> Self {
        Self {
            started: false,
            is_connected: true,
            client_count: 0,
            scan_networks_result: Vec::new(),
            configured_ssid: None,
            configured_password: None,
            start_access_point_ssid: None,
            fail_configure_client: false,
            fail_connect: false,
        }
    }
}

impl Default for MockWifiBackend {
    fn default() -> Self {
        Self {
            connect_order: Arc::new(Mutex::new(None)),
            scan_order: Arc::new(Mutex::new(None)),
            start_ap_order: Arc::new(Mutex::new(None)),
            global_counter: Arc::new(AtomicU32::new(1)),
            state: Arc::new(Mutex::new(MockWifiBackendState::default())),
        }
    }
}

impl MockWifiBackend {
    pub fn with_counter(counter: Arc<AtomicU32>) -> Self {
        Self {
            connect_order: Arc::new(Mutex::new(None)),
            scan_order: Arc::new(Mutex::new(None)),
            start_ap_order: Arc::new(Mutex::new(None)),
            global_counter: counter,
            state: Arc::new(Mutex::new(MockWifiBackendState::default())),
        }
    }

    pub fn with_client_count(client_count: usize) -> Self {
        Self {
            connect_order: Arc::new(Mutex::new(None)),
            scan_order: Arc::new(Mutex::new(None)),
            start_ap_order: Arc::new(Mutex::new(None)),
            global_counter: Arc::new(AtomicU32::new(1)),
            state: Arc::new(Mutex::new(MockWifiBackendState {
                client_count,
                ..Default::default()
            })),
        }
    }

    pub fn set_is_connected(&mut self, connected: bool) {
        self.state.lock().unwrap().is_connected = connected;
    }

    pub fn set_fail_configure_client(&mut self, fail: bool) {
        self.state.lock().unwrap().fail_configure_client = fail;
    }

    pub fn set_fail_connect(&mut self, fail: bool) {
        self.state.lock().unwrap().fail_connect = fail;
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
        *self.scan_order.lock().unwrap() =
            Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(self.state.lock().unwrap().scan_networks_result.clone())
    }
    async fn configure_client(
        &mut self,
        credentials: &wifi::WifiCredentials,
        _channel: Option<u8>,
        _auth: wifi::ClientAuth,
    ) -> anyhow::Result<()> {
        if self.state.lock().unwrap().fail_configure_client {
            return Err(anyhow::anyhow!("mock configure_client error"));
        }
        let mut state = self.state.lock().unwrap();
        state.configured_ssid = Some(credentials.ssid.clone());
        state.configured_password = Some(credentials.password.clone());
        Ok(())
    }
    async fn connect(
        &mut self,
        _timeout: std::time::Duration,
    ) -> anyhow::Result<wifi::ConnectionInfo> {
        if self.state.lock().unwrap().fail_connect {
            return Err(anyhow::anyhow!("mock connect error"));
        }
        *self.connect_order.lock().unwrap() =
            Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(wifi::ConnectionInfo::new("0.0.0.0"))
    }
    async fn is_connected(&mut self) -> anyhow::Result<bool> {
        Ok(self.state.lock().unwrap().is_connected)
    }
    async fn connection_info(&mut self) -> anyhow::Result<Option<wifi::ConnectionInfo>> {
        Ok(Some(wifi::ConnectionInfo::new("0.0.0.0")))
    }
    async fn start_access_point(
        &mut self,
        config: &wifi::AccessPointConfig,
    ) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.started = true;
        state.start_access_point_ssid = Some(config.ssid.clone());
        *self.start_ap_order.lock().unwrap() =
            Some(self.global_counter.fetch_add(1, Ordering::SeqCst));
        Ok(())
    }
    async fn stop_access_point(&mut self) -> anyhow::Result<()> {
        self.state.lock().unwrap().started = false;
        Ok(())
    }
    async fn access_point_status(&mut self) -> anyhow::Result<wifi::AccessPointStatus> {
        let state = self.state.lock().unwrap();
        Ok(wifi::AccessPointStatus {
            is_started: state.started,
            client_count: state.client_count,
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
