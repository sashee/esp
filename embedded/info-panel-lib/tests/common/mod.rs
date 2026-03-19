#![allow(dead_code, unused_imports)]

use anyhow::Result;
use embassy_time::Duration;
use info_panel_lib::{BootReason, Clock, Hal, HttpClient, Platform, TFT_HEIGHT, TFT_WIDTH};
use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

type HookResult<T> = Option<anyhow::Result<T>>;

type StoreReadHook = Arc<Mutex<Box<dyn FnMut(&[&str]) -> HookResult<BTreeMap<String, String>> + Send>>>;
type StoreWriteHook = Arc<Mutex<Box<dyn FnMut(&BTreeMap<String, String>) -> HookResult<()> + Send>>>;
type StoreRemoveHook = Arc<Mutex<Box<dyn FnMut(&[&str]) -> HookResult<()> + Send>>>;
type LedHook = Arc<Mutex<Box<dyn FnMut(LedCall) -> HookResult<()> + Send>>>;
type DisplayInitHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<()> + Send>>>;
type DisplayWriteHook = Arc<Mutex<Box<dyn FnMut(&[u8]) -> HookResult<()> + Send>>>;
type DisplayFillHook = Arc<Mutex<Box<dyn FnMut(u16) -> HookResult<()> + Send>>>;
type HttpGetHook = Arc<Mutex<Box<dyn FnMut(&str) -> HookResult<Vec<u8>> + Send>>>;
type PlatformBootHook = Arc<Mutex<Box<dyn FnMut() -> Option<BootReason> + Send>>>;
type PlatformMacHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<[u8; 6]> + Send>>>;
type PlatformRebootHook = Arc<Mutex<Box<dyn FnMut() + Send>>>;
type ClockNowHook = Arc<Mutex<Box<dyn FnMut() -> Option<embassy_time::Instant> + Send>>>;
type ClockSleepHook = Arc<Mutex<Box<dyn FnMut(Duration) -> HookResult<()> + Send>>>;
type WifiStartHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<()> + Send>>>;
type WifiStopHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<()> + Send>>>;
type WifiDisconnectHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<()> + Send>>>;
type WifiIsStartedHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<bool> + Send>>>;
type WifiScanHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<Vec<wifi::FoundNetwork>> + Send>>>;
type WifiConfigureHook = Arc<Mutex<Box<dyn FnMut(&wifi::WifiCredentials, Option<u8>, wifi::ClientAuth) -> HookResult<()> + Send>>>;
type WifiConnectHook = Arc<Mutex<Box<dyn FnMut(std::time::Duration) -> HookResult<wifi::ConnectionInfo> + Send>>>;
type WifiIsConnectedHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<bool> + Send>>>;
type WifiConnectionInfoHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<Option<wifi::ConnectionInfo>> + Send>>>;
type WifiStartApHook = Arc<Mutex<Box<dyn FnMut(&wifi::AccessPointConfig) -> HookResult<()> + Send>>>;
type WifiStopApHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<()> + Send>>>;
type WifiApStatusHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<wifi::AccessPointStatus> + Send>>>;
type WifiApIpHook = Arc<Mutex<Box<dyn FnMut() -> HookResult<wifi::IpConfig> + Send>>>;

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

pub fn ok(msg: &'static str) -> ! {
    panic!("OK: {msg}")
}

pub fn nok(msg: &'static str) -> ! {
    panic!("NOK: {msg}")
}

pub fn panic_message(err: Box<dyn Any + Send>) -> String {
    match err.downcast::<String>() {
        Ok(msg) => *msg,
        Err(err) => match err.downcast::<&'static str>() {
            Ok(msg) => (*msg).to_string(),
            Err(_) => "<non-string panic>".to_string(),
        },
    }
}

pub fn assert_ok_signal<T>(result: std::thread::Result<T>, expected: &str) {
    match result {
        Ok(_) => panic!("expected OK panic containing `{expected}`, but run returned normally"),
        Err(err) => {
            let msg = panic_message(err);
            assert!(msg.starts_with("OK: "), "expected OK panic, got `{msg}`");
            assert!(
                msg.contains(expected),
                "expected OK panic containing `{expected}`, got `{msg}`"
            );
        }
    }
}

pub fn valid_frame_bytes() -> Vec<u8> {
    vec![0u8; 128 * 160 * 2]
}

fn read_source_to_vec(
    source: &mut dyn tft_display::FrameSource<Error = anyhow::Error>,
) -> anyhow::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut buf = [0u8; 257];

    loop {
        let read = source.read(&mut buf)?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buf[..read]);
    }

    Ok(data)
}

pub fn valid_config_store() -> MockStore {
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    MockStore::new(values)
}

pub fn empty_config_store() -> MockStore {
    MockStore::new(BTreeMap::new())
}

pub fn config_store_with_values(values: BTreeMap<String, String>) -> MockStore {
    MockStore::new(values)
}

#[derive(Clone)]
pub struct MockStore {
    values: BTreeMap<String, String>,
    read_hook: Option<StoreReadHook>,
    write_hook: Option<StoreWriteHook>,
    remove_hook: Option<StoreRemoveHook>,
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new(BTreeMap::new())
    }
}

impl MockStore {
    pub fn new(values: BTreeMap<String, String>) -> Self {
        Self {
            values,
            read_hook: None,
            write_hook: None,
            remove_hook: None,
        }
    }

    pub fn on_read(
        mut self,
        hook: impl FnMut(&[&str]) -> HookResult<BTreeMap<String, String>> + Send + 'static,
    ) -> Self {
        self.read_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_write(
        mut self,
        hook: impl FnMut(&BTreeMap<String, String>) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.write_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_remove(
        mut self,
        hook: impl FnMut(&[&str]) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.remove_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl config_portal::ConfigStore for MockStore {
    fn read(&self, keys: &[&str]) -> anyhow::Result<BTreeMap<String, String>> {
        if let Some(hook) = &self.read_hook {
            if let Some(result) = (hook.lock().unwrap())(keys) {
                return result;
            }
        }
        Ok(self.values.clone())
    }

    fn write(&self, values: &BTreeMap<String, String>) -> anyhow::Result<()> {
        if let Some(hook) = &self.write_hook {
            if let Some(result) = (hook.lock().unwrap())(values) {
                return result;
            }
        }
        Ok(())
    }

    fn remove(&self, keys: &[&str]) -> anyhow::Result<()> {
        if let Some(hook) = &self.remove_hook {
            if let Some(result) = (hook.lock().unwrap())(keys) {
                return result;
            }
        }
        Ok(())
    }
}

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

#[derive(Debug, Clone)]
pub struct LedCall {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub brightness: f32,
}

pub struct MockLed {
    set_pixel_hook: Option<LedHook>,
}

impl MockLed {
    pub fn new() -> Self {
        Self { set_pixel_hook: None }
    }

    pub fn on_set_pixel(
        mut self,
        hook: impl FnMut(LedCall) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.set_pixel_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl Default for MockLed {
    fn default() -> Self {
        Self::new()
    }
}

impl rgb_led::RgbLedBackend for MockLed {
    type Error = anyhow::Error;

    fn color_order(&self) -> rgb_led::ColorOrder {
        rgb_led::ColorOrder::RGB
    }

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> anyhow::Result<()> {
        let max = bytes.into_iter().max().unwrap_or(0) as f32;
        let brightness = ((max / 255.0) * 100.0).round() / 100.0;
        let scale = if max == 0.0 { 0.0 } else { 1.0 / max };
        let call = LedCall {
            r: bytes[0] as f32 * scale,
            g: bytes[1] as f32 * scale,
            b: bytes[2] as f32 * scale,
            brightness,
        };
        if let Some(hook) = &self.set_pixel_hook {
            if let Some(result) = (hook.lock().unwrap())(call) {
                return result;
            }
        }
        Ok(())
    }
}

pub fn tracked_led() -> (MockLed, Arc<Mutex<Vec<LedCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let tracked = calls.clone();
    (
        MockLed::new().on_set_pixel(move |call| {
            tracked.lock().unwrap().push(call);
            None
        }),
        calls,
    )
}

pub fn failing_led() -> (MockLed, Arc<Mutex<Vec<LedCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let tracked = calls.clone();
    (
        MockLed::new().on_set_pixel(move |call| {
            tracked.lock().unwrap().push(call);
            Some(Err(anyhow::anyhow!("mock LED error")))
        }),
        calls,
    )
}

pub fn led_bytes(rgb: rgb_led::Rgb, brightness: f32) -> [u8; 3] {
    rgb_led::pixel_bytes(rgb_led::ColorOrder::RGB, rgb, brightness)
}

pub struct MockDisplay {
    init_hook: Option<DisplayInitHook>,
    write_frame_hook: Option<DisplayWriteHook>,
    fill_solid_hook: Option<DisplayFillHook>,
    dc_high: bool,
    current_transfer: Option<Vec<u8>>,
    saw_fill_solid: bool,
}

impl MockDisplay {
    pub fn new() -> Self {
        Self {
            init_hook: None,
            write_frame_hook: None,
            fill_solid_hook: None,
            dc_high: false,
            current_transfer: None,
            saw_fill_solid: false,
        }
    }

    pub fn on_init(
        mut self,
        hook: impl FnMut() -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.init_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_write_frame(
        mut self,
        hook: impl FnMut(&[u8]) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.write_frame_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_fill_solid(
        mut self,
        hook: impl FnMut(u16) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.fill_solid_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl Default for MockDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDisplay {
    fn finish_transfer(&mut self) -> anyhow::Result<()> {
        let Some(data) = self.current_transfer.take() else {
            return Ok(());
        };

        let full_frame_bytes = (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2;
        let is_uniform_solid = data.len() == full_frame_bytes
            && data.chunks_exact(2).all(|chunk| chunk == &data[..2]);

        if is_uniform_solid && !self.saw_fill_solid {
            self.saw_fill_solid = true;
            if let Some(hook) = &self.fill_solid_hook {
                let color = u16::from_be_bytes([data[0], data[1]]);
                if let Some(result) = (hook.lock().unwrap())(color) {
                    return result;
                }
            }
        } else if let Some(hook) = &self.write_frame_hook {
            if let Some(result) = (hook.lock().unwrap())(&data) {
                return result;
            }
        }

        Ok(())
    }
}

impl tft_display::TftBackend for MockDisplay {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> anyhow::Result<()> {
        self.finish_transfer()?;
        self.dc_high = false;
        Ok(())
    }

    fn set_dc_high(&mut self) -> anyhow::Result<()> {
        self.dc_high = true;
        Ok(())
    }

    fn set_rst_low(&mut self) -> anyhow::Result<()> {
        if let Some(hook) = &self.init_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(())
    }

    fn set_rst_high(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if self.dc_high {
            if let Some(transfer) = &mut self.current_transfer {
                transfer.extend_from_slice(data);
                let full_frame_bytes = (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2;
                if transfer.len() >= full_frame_bytes {
                    self.finish_transfer()?;
                }
            }
            return Ok(());
        }

        self.finish_transfer()?;

        if data == [0x2C] {
            self.current_transfer = Some(Vec::new());
        }

        Ok(())
    }
}

impl Drop for MockDisplay {
    fn drop(&mut self) {
        let _ = self.finish_transfer();
    }
}

pub struct TrackedDisplayState {
    pub init_called: Arc<Mutex<bool>>,
    pub init_order: Arc<Mutex<Option<u32>>>,
    pub write_frame_calls: Arc<Mutex<usize>>,
    pub fill_solid_calls: Arc<Mutex<usize>>,
}

pub fn tracked_display(global_counter: Arc<AtomicU32>) -> (MockDisplay, TrackedDisplayState) {
    let init_called = Arc::new(Mutex::new(false));
    let init_order = Arc::new(Mutex::new(None));
    let write_frame_calls = Arc::new(Mutex::new(0));
    let fill_solid_calls = Arc::new(Mutex::new(0));

    let init_called_hook = init_called.clone();
    let init_order_hook = init_order.clone();
    let counter_for_init = global_counter.clone();
    let write_calls_hook = write_frame_calls.clone();
    let fill_calls_hook = fill_solid_calls.clone();

    (
        MockDisplay::new()
            .on_init(move || {
                *init_called_hook.lock().unwrap() = true;
                *init_order_hook.lock().unwrap() =
                    Some(counter_for_init.fetch_add(1, Ordering::SeqCst));
                None
            })
            .on_write_frame(move |_data| {
                *write_calls_hook.lock().unwrap() += 1;
                None
            })
            .on_fill_solid(move |_color| {
                *fill_calls_hook.lock().unwrap() += 1;
                None
            }),
        TrackedDisplayState {
            init_called,
            init_order,
            write_frame_calls,
            fill_solid_calls,
        },
    )
}

pub fn display_with_write_frame_fail_nth(
    global_counter: Arc<AtomicU32>,
    fail_nth: usize,
) -> (MockDisplay, TrackedDisplayState) {
    let (display, state) = tracked_display(global_counter);
    let write_calls = state.write_frame_calls.clone();
    (
        display.on_write_frame(move |_data| {
            let current = *write_calls.lock().unwrap() + 1;
            if current == fail_nth {
                Some(Err(anyhow::anyhow!("mock write_frame error")))
            } else {
                *write_calls.lock().unwrap() = current;
                Some(Ok(()))
            }
        }),
        state,
    )
}

pub fn display_with_fill_solid_fail_nth(
    global_counter: Arc<AtomicU32>,
    fail_nth: usize,
) -> (MockDisplay, TrackedDisplayState) {
    let (display, state) = tracked_display(global_counter);
    let fill_calls = state.fill_solid_calls.clone();
    (
        display.on_fill_solid(move |_color| {
            let current = *fill_calls.lock().unwrap() + 1;
            if current == fail_nth {
                Some(Err(anyhow::anyhow!("mock fill_solid error")))
            } else {
                *fill_calls.lock().unwrap() = current;
                Some(Ok(()))
            }
        }),
        state,
    )
}

pub fn hal<W, S, H, P, Ck, HC, TB, LB>(
    wifi_backend: W,
    store: S,
    http_backend: H,
    platform: P,
    clock: Ck,
    http_client: HC,
    tft_backend: TB,
    led_backend: LB,
) -> Hal<W, S, H, P, Ck, HC, TB, LB> {
    Hal {
        wifi_backend,
        store,
        http_backend,
        platform,
        clock,
        http_client,
        tft_backend,
        led_backend,
    }
}

pub struct FailingDisplay {
    init_failed: bool,
}

impl FailingDisplay {
    pub fn new() -> Self {
        Self { init_failed: false }
    }
}

impl Default for FailingDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl tft_display::TftBackend for FailingDisplay {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_dc_high(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_rst_low(&mut self) -> anyhow::Result<()> {
        if !self.init_failed {
            self.init_failed = true;
            return Err(anyhow::anyhow!("display init failed"));
        }
        Ok(())
    }

    fn set_rst_high(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn write(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct MockHttpClient {
    response: Vec<u8>,
    get_hook: Option<HttpGetHook>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self::with_response(valid_frame_bytes())
    }

    pub fn with_response(response: Vec<u8>) -> Self {
        Self {
            response,
            get_hook: None,
        }
    }

    pub fn with_valid_frame() -> Self {
        Self::new()
    }

    pub fn on_get(
        mut self,
        hook: impl FnMut(&str) -> HookResult<Vec<u8>> + Send + 'static,
    ) -> Self {
        self.get_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl Default for MockHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for MockHttpClient {
    async fn get(
        &mut self,
        url: &str,
    ) -> anyhow::Result<Box<dyn tft_display::FrameSource<Error = anyhow::Error>>> {
        if let Some(hook) = &self.get_hook {
            if let Some(result) = (hook.lock().unwrap())(url) {
                return result.map(|data| Box::new(info_panel_lib::MemoryFrameSource::new(data)) as _);
            }
        }
        Ok(Box::new(info_panel_lib::MemoryFrameSource::new(
            self.response.clone(),
        )))
    }
}

pub struct TrackedHttpClientState {
    pub get_calls: Arc<Mutex<usize>>,
    pub get_urls: Arc<Mutex<Vec<String>>>,
}

pub fn tracked_http_client() -> (MockHttpClient, TrackedHttpClientState) {
    tracked_http_client_with_response(valid_frame_bytes())
}

pub fn tracked_http_client_with_response(data: Vec<u8>) -> (MockHttpClient, TrackedHttpClientState) {
    let get_calls = Arc::new(Mutex::new(0));
    let get_urls = Arc::new(Mutex::new(Vec::new()));
    let calls = get_calls.clone();
    let urls = get_urls.clone();
    (
        MockHttpClient::with_response(data).on_get(move |url| {
            *calls.lock().unwrap() += 1;
            urls.lock().unwrap().push(url.to_string());
            None
        }),
        TrackedHttpClientState { get_calls, get_urls },
    )
}

pub fn always_failing_http_client() -> (MockHttpClient, TrackedHttpClientState) {
    let get_calls = Arc::new(Mutex::new(0));
    let get_urls = Arc::new(Mutex::new(Vec::new()));
    let calls = get_calls.clone();
    let urls = get_urls.clone();
    (
        MockHttpClient::new().on_get(move |url| {
            *calls.lock().unwrap() += 1;
            urls.lock().unwrap().push(url.to_string());
            Some(Err(anyhow::anyhow!("mock HTTP error")))
        }),
        TrackedHttpClientState { get_calls, get_urls },
    )
}

pub fn fail_up_to_http_client(n: usize) -> (MockHttpClient, TrackedHttpClientState) {
    let get_calls = Arc::new(Mutex::new(0));
    let get_urls = Arc::new(Mutex::new(Vec::new()));
    let calls = get_calls.clone();
    let urls = get_urls.clone();
    (
        MockHttpClient::new().on_get(move |url| {
            let mut count = calls.lock().unwrap();
            *count += 1;
            let current = *count;
            drop(count);
            urls.lock().unwrap().push(url.to_string());
            if current <= n {
                Some(Err(anyhow::anyhow!("mock HTTP error")))
            } else {
                None
            }
        }),
        TrackedHttpClientState { get_calls, get_urls },
    )
}

pub fn panic_on_nth_http_client(n: usize) -> (MockHttpClient, TrackedHttpClientState) {
    let get_calls = Arc::new(Mutex::new(0));
    let get_urls = Arc::new(Mutex::new(Vec::new()));
    let calls = get_calls.clone();
    let urls = get_urls.clone();
    (
        MockHttpClient::new().on_get(move |url| {
            let mut count = calls.lock().unwrap();
            *count += 1;
            let current = *count;
            drop(count);
            urls.lock().unwrap().push(url.to_string());
            if current == n {
                panic!("mock: http_client.get() call #{} reached", current);
            }
            None
        }),
        TrackedHttpClientState { get_calls, get_urls },
    )
}

#[derive(Clone)]
pub struct MockPlatform {
    mac: [u8; 6],
    boot_reason: BootReason,
    boot_reason_hook: Option<PlatformBootHook>,
    mac_address_hook: Option<PlatformMacHook>,
    reboot_hook: Option<PlatformRebootHook>,
}

impl MockPlatform {
    pub fn new(mac: [u8; 6], boot_reason: BootReason) -> Self {
        Self {
            mac,
            boot_reason,
            boot_reason_hook: None,
            mac_address_hook: None,
            reboot_hook: None,
        }
    }

    pub fn on_boot_reason(mut self, hook: impl FnMut() -> Option<BootReason> + Send + 'static) -> Self {
        self.boot_reason_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_mac_address(
        mut self,
        hook: impl FnMut() -> HookResult<[u8; 6]> + Send + 'static,
    ) -> Self {
        self.mac_address_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_reboot(mut self, hook: impl FnMut() + Send + 'static) -> Self {
        self.reboot_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl Platform for MockPlatform {
    fn boot_reason(&self) -> BootReason {
        if let Some(hook) = &self.boot_reason_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        self.boot_reason
    }

    fn mac_address(&self) -> anyhow::Result<[u8; 6]> {
        if let Some(hook) = &self.mac_address_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.mac)
    }

    fn reboot(&self) -> ! {
        if let Some(hook) = &self.reboot_hook {
            (hook.lock().unwrap())();
        }
        panic!("mock reboot")
    }
}

pub fn tracked_platform(mac: [u8; 6], boot_reason: BootReason) -> (MockPlatform, Arc<Mutex<bool>>) {
    let reboot_called = Arc::new(Mutex::new(false));
    let tracked = reboot_called.clone();
    (
        MockPlatform::new(mac, boot_reason).on_reboot(move || {
            *tracked.lock().unwrap() = true;
        }),
        reboot_called,
    )
}

#[derive(Clone)]
pub struct MockClock {
    now: embassy_time::Instant,
    now_hook: Option<ClockNowHook>,
    sleep_hook: Option<ClockSleepHook>,
}

impl MockClock {
    pub fn new(now: embassy_time::Instant) -> Self {
        Self {
            now,
            now_hook: None,
            sleep_hook: None,
        }
    }

    pub fn from_ticks(ticks: &[u64]) -> Self {
        sequenced_clock(ticks).0
    }

    pub fn from_ticks_silent(ticks: &[u64]) -> Self {
        sequenced_clock_silent(ticks)
    }

    pub fn on_now(mut self, hook: impl FnMut() -> Option<embassy_time::Instant> + Send + 'static) -> Self {
        self.now_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_sleep(
        mut self,
        hook: impl FnMut(Duration) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.sleep_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl Clock for MockClock {
    fn now(&self) -> embassy_time::Instant {
        if let Some(hook) = &self.now_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        self.now
    }

    async fn sleep(&self, duration: Duration) {
        if let Some(hook) = &self.sleep_hook {
            if let Some(result) = (hook.lock().unwrap())(duration) {
                result.unwrap();
                return;
            }
        }
    }
}

pub fn tracked_clock(now_ticks: u64) -> (MockClock, Arc<Mutex<Vec<Duration>>>) {
    let sleeps = Arc::new(Mutex::new(Vec::new()));
    let tracked = sleeps.clone();
    (
        MockClock::new(embassy_time::Instant::from_ticks(now_ticks)).on_sleep(move |duration| {
            tracked.lock().unwrap().push(duration);
            None
        }),
        sleeps,
    )
}

pub fn sequenced_clock(ticks: &[u64]) -> (MockClock, Arc<Mutex<Vec<Duration>>>) {
    let sleeps = Arc::new(Mutex::new(Vec::new()));
    let tracked_sleeps = sleeps.clone();
    let queue = Arc::new(Mutex::new(
        ticks
            .iter()
            .copied()
            .map(embassy_time::Instant::from_ticks)
            .collect::<VecDeque<_>>(),
    ));
    let now_value = queue
        .lock()
        .unwrap()
        .front()
        .copied()
        .unwrap_or_else(|| embassy_time::Instant::from_ticks(0));
    let tracked_queue = queue.clone();

    (
        MockClock::new(now_value)
            .on_now(move || {
                let mut guard = tracked_queue.lock().unwrap();
                if guard.len() > 1 {
                    guard.pop_front()
                } else {
                    guard.front().copied()
                }
            })
            .on_sleep(move |duration| {
                tracked_sleeps.lock().unwrap().push(duration);
                None
            }),
        sleeps,
    )
}

pub fn sequenced_clock_silent(ticks: &[u64]) -> MockClock {
    let queue = Arc::new(Mutex::new(
        ticks
            .iter()
            .copied()
            .map(embassy_time::Instant::from_ticks)
            .collect::<VecDeque<_>>(),
    ));
    let now_value = queue
        .lock()
        .unwrap()
        .front()
        .copied()
        .unwrap_or_else(|| embassy_time::Instant::from_ticks(0));
    let tracked_queue = queue.clone();

    MockClock::new(now_value).on_now(move || {
        let mut guard = tracked_queue.lock().unwrap();
        if guard.len() > 1 {
            guard.pop_front()
        } else {
            guard.front().copied()
        }
    })
}

#[derive(Clone)]
pub struct MockWifiBackend {
    started: bool,
    is_connected: bool,
    scan_networks_result: Vec<wifi::FoundNetwork>,
    connection_info: wifi::ConnectionInfo,
    access_point_status: wifi::AccessPointStatus,
    access_point_ip_config: wifi::IpConfig,
    start_hook: Option<WifiStartHook>,
    stop_hook: Option<WifiStopHook>,
    disconnect_hook: Option<WifiDisconnectHook>,
    is_started_hook: Option<WifiIsStartedHook>,
    scan_networks_hook: Option<WifiScanHook>,
    configure_client_hook: Option<WifiConfigureHook>,
    connect_hook: Option<WifiConnectHook>,
    is_connected_hook: Option<WifiIsConnectedHook>,
    connection_info_hook: Option<WifiConnectionInfoHook>,
    start_access_point_hook: Option<WifiStartApHook>,
    stop_access_point_hook: Option<WifiStopApHook>,
    access_point_status_hook: Option<WifiApStatusHook>,
    access_point_ip_config_hook: Option<WifiApIpHook>,
}

impl Default for MockWifiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWifiBackend {
    pub fn new() -> Self {
        Self {
            started: false,
            is_connected: true,
            scan_networks_result: Vec::new(),
            connection_info: wifi::ConnectionInfo::new("0.0.0.0"),
            access_point_status: wifi::AccessPointStatus {
                is_started: false,
                client_count: 0,
            },
            access_point_ip_config: wifi::IpConfig::new(
                "192.168.4.1",
                "192.168.4.1",
                "255.255.255.0",
            ),
            start_hook: None,
            stop_hook: None,
            disconnect_hook: None,
            is_started_hook: None,
            scan_networks_hook: None,
            configure_client_hook: None,
            connect_hook: None,
            is_connected_hook: None,
            connection_info_hook: None,
            start_access_point_hook: None,
            stop_access_point_hook: None,
            access_point_status_hook: None,
            access_point_ip_config_hook: None,
        }
    }

    pub fn with_started(mut self, started: bool) -> Self {
        self.started = started;
        self.access_point_status.is_started = started;
        self
    }

    pub fn with_is_connected(mut self, is_connected: bool) -> Self {
        self.is_connected = is_connected;
        self
    }

    pub fn with_scan_networks_result(mut self, scan_networks_result: Vec<wifi::FoundNetwork>) -> Self {
        self.scan_networks_result = scan_networks_result;
        self
    }

    pub fn with_client_count(mut self, client_count: usize) -> Self {
        self.access_point_status.client_count = client_count;
        self
    }

    pub fn with_connection_info(mut self, connection_info: wifi::ConnectionInfo) -> Self {
        self.connection_info = connection_info;
        self
    }

    pub fn with_access_point_status(mut self, status: wifi::AccessPointStatus) -> Self {
        self.access_point_status = status;
        self
    }

    pub fn with_access_point_ip_config(mut self, ip_config: wifi::IpConfig) -> Self {
        self.access_point_ip_config = ip_config;
        self
    }

    pub fn on_start(mut self, hook: impl FnMut() -> HookResult<()> + Send + 'static) -> Self {
        self.start_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_stop(mut self, hook: impl FnMut() -> HookResult<()> + Send + 'static) -> Self {
        self.stop_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_disconnect(mut self, hook: impl FnMut() -> HookResult<()> + Send + 'static) -> Self {
        self.disconnect_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_is_started(mut self, hook: impl FnMut() -> HookResult<bool> + Send + 'static) -> Self {
        self.is_started_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_scan_networks(
        mut self,
        hook: impl FnMut() -> HookResult<Vec<wifi::FoundNetwork>> + Send + 'static,
    ) -> Self {
        self.scan_networks_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_configure_client(
        mut self,
        hook: impl FnMut(&wifi::WifiCredentials, Option<u8>, wifi::ClientAuth) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.configure_client_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_connect(
        mut self,
        hook: impl FnMut(std::time::Duration) -> HookResult<wifi::ConnectionInfo> + Send + 'static,
    ) -> Self {
        self.connect_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_is_connected(mut self, hook: impl FnMut() -> HookResult<bool> + Send + 'static) -> Self {
        self.is_connected_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_connection_info(
        mut self,
        hook: impl FnMut() -> HookResult<Option<wifi::ConnectionInfo>> + Send + 'static,
    ) -> Self {
        self.connection_info_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_start_access_point(
        mut self,
        hook: impl FnMut(&wifi::AccessPointConfig) -> HookResult<()> + Send + 'static,
    ) -> Self {
        self.start_access_point_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_stop_access_point(mut self, hook: impl FnMut() -> HookResult<()> + Send + 'static) -> Self {
        self.stop_access_point_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_access_point_status(
        mut self,
        hook: impl FnMut() -> HookResult<wifi::AccessPointStatus> + Send + 'static,
    ) -> Self {
        self.access_point_status_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }

    pub fn on_access_point_ip_config(
        mut self,
        hook: impl FnMut() -> HookResult<wifi::IpConfig> + Send + 'static,
    ) -> Self {
        self.access_point_ip_config_hook = Some(Arc::new(Mutex::new(Box::new(hook))));
        self
    }
}

impl wifi::WifiBackend for MockWifiBackend {
    async fn start(&mut self) -> anyhow::Result<()> {
        if let Some(hook) = &self.start_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(hook) = &self.stop_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Some(hook) = &self.disconnect_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(())
    }

    async fn is_started(&mut self) -> anyhow::Result<bool> {
        if let Some(hook) = &self.is_started_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.started)
    }

    async fn scan_networks(&mut self) -> anyhow::Result<Vec<wifi::FoundNetwork>> {
        if let Some(hook) = &self.scan_networks_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.scan_networks_result.clone())
    }

    async fn configure_client(
        &mut self,
        credentials: &wifi::WifiCredentials,
        channel: Option<u8>,
        auth: wifi::ClientAuth,
    ) -> anyhow::Result<()> {
        if let Some(hook) = &self.configure_client_hook {
            if let Some(result) = (hook.lock().unwrap())(credentials, channel, auth) {
                return result;
            }
        }
        Ok(())
    }

    async fn connect(
        &mut self,
        timeout: std::time::Duration,
    ) -> anyhow::Result<wifi::ConnectionInfo> {
        if let Some(hook) = &self.connect_hook {
            if let Some(result) = (hook.lock().unwrap())(timeout) {
                return result;
            }
        }
        Ok(self.connection_info.clone())
    }

    async fn is_connected(&mut self) -> anyhow::Result<bool> {
        if let Some(hook) = &self.is_connected_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.is_connected)
    }

    async fn connection_info(&mut self) -> anyhow::Result<Option<wifi::ConnectionInfo>> {
        if let Some(hook) = &self.connection_info_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(Some(self.connection_info.clone()))
    }

    async fn start_access_point(
        &mut self,
        config: &wifi::AccessPointConfig,
    ) -> anyhow::Result<()> {
        if let Some(hook) = &self.start_access_point_hook {
            if let Some(result) = (hook.lock().unwrap())(config) {
                return result;
            }
        }
        Ok(())
    }

    async fn stop_access_point(&mut self) -> anyhow::Result<()> {
        if let Some(hook) = &self.stop_access_point_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(())
    }

    async fn access_point_status(&mut self) -> anyhow::Result<wifi::AccessPointStatus> {
        if let Some(hook) = &self.access_point_status_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.access_point_status.clone())
    }

    async fn access_point_ip_config(&mut self) -> anyhow::Result<wifi::IpConfig> {
        if let Some(hook) = &self.access_point_ip_config_hook {
            if let Some(result) = (hook.lock().unwrap())() {
                return result;
            }
        }
        Ok(self.access_point_ip_config.clone())
    }
}

pub struct TrackedWifiBackendState {
    pub configured_ssid: Arc<Mutex<Option<String>>>,
    pub configured_password: Arc<Mutex<Option<String>>>,
    pub start_access_point_ssid: Arc<Mutex<Option<String>>>,
    pub access_point_ip: Arc<Mutex<Option<String>>>,
    pub connect_order: Arc<Mutex<Option<u32>>>,
    pub scan_order: Arc<Mutex<Option<u32>>>,
    pub start_ap_order: Arc<Mutex<Option<u32>>>,
    pub started: Arc<AtomicBool>,
}

pub fn tracked_wifi_backend() -> (MockWifiBackend, TrackedWifiBackendState) {
    tracked_wifi_backend_with_counter(Arc::new(AtomicU32::new(1)))
}

pub fn tracked_wifi_backend_with_counter(
    counter: Arc<AtomicU32>,
) -> (MockWifiBackend, TrackedWifiBackendState) {
    let configured_ssid = Arc::new(Mutex::new(None));
    let configured_password = Arc::new(Mutex::new(None));
    let start_access_point_ssid = Arc::new(Mutex::new(None));
    let access_point_ip = Arc::new(Mutex::new(None));
    let connect_order = Arc::new(Mutex::new(None));
    let scan_order = Arc::new(Mutex::new(None));
    let start_ap_order = Arc::new(Mutex::new(None));
    let started = Arc::new(AtomicBool::new(false));

    let configured_ssid_hook = configured_ssid.clone();
    let configured_password_hook = configured_password.clone();
    let connect_order_hook = connect_order.clone();
    let scan_order_hook = scan_order.clone();
    let start_ap_order_hook = start_ap_order.clone();
    let start_access_point_ssid_hook = start_access_point_ssid.clone();
    let access_point_ip_hook = access_point_ip.clone();
    let started_for_start = started.clone();
    let started_for_stop = started.clone();
    let started_for_status = started.clone();
    let counter_for_connect = counter.clone();
    let counter_for_scan = counter.clone();
    let counter_for_start_ap = counter.clone();

    (
        MockWifiBackend::new()
            .on_scan_networks(move || {
                *scan_order_hook.lock().unwrap() =
                    Some(counter_for_scan.fetch_add(1, Ordering::SeqCst));
                None
            })
            .on_configure_client(move |credentials, _channel, _auth| {
                *configured_ssid_hook.lock().unwrap() = Some(credentials.ssid.clone());
                *configured_password_hook.lock().unwrap() = Some(credentials.password.clone());
                None
            })
            .on_connect(move |_timeout| {
                *connect_order_hook.lock().unwrap() =
                    Some(counter_for_connect.fetch_add(1, Ordering::SeqCst));
                None
            })
            .on_start_access_point(move |config| {
                started_for_start.store(true, Ordering::SeqCst);
                *start_access_point_ssid_hook.lock().unwrap() = Some(config.ssid.clone());
                *start_ap_order_hook.lock().unwrap() =
                    Some(counter_for_start_ap.fetch_add(1, Ordering::SeqCst));
                None
            })
            .on_stop_access_point(move || {
                started_for_stop.store(false, Ordering::SeqCst);
                None
            })
            .on_is_started(move || Some(Ok(started_for_status.load(Ordering::SeqCst))))
            .on_access_point_status({
                let started = started.clone();
                move || {
                    Some(Ok(wifi::AccessPointStatus {
                        is_started: started.load(Ordering::SeqCst),
                        client_count: 0,
                    }))
                }
            })
            .on_access_point_ip_config(move || {
                *access_point_ip_hook.lock().unwrap() = Some("192.168.4.1".to_string());
                None
            }),
        TrackedWifiBackendState {
            configured_ssid,
            configured_password,
            start_access_point_ssid,
            access_point_ip,
            connect_order,
            scan_order,
            start_ap_order,
            started,
        },
    )
}
