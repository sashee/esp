use anyhow::{anyhow, Result};
use config_portal::{ConfigStore, HttpEndpoint, HttpMethod, HttpRequest, HttpResponse};
use core::future::poll_fn;
use embassy_time::Duration as EmbassyDuration;
use info_panel_lib::{BootReason, Clock, Hal, HttpClient, Platform, TFT_HEIGHT, TFT_WIDTH};
use sim::{
    possible_next_events, AsyncCompletion, AsyncTiming, Event, NewRunWrapper, NextEventsSpec,
    PossibleEvent, SimBundle, SimDriver, TraceStep,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

struct UnsafeSendFuture(Pin<Box<dyn Future<Output = ()> + 'static>>);

unsafe impl Send for UnsafeSendFuture {}

impl Future for UnsafeSendFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum SyncOp {
    BootReason,
    MacAddress,
    StoreRead {
        namespace: String,
        keys: Vec<String>,
    },
    StoreWrite {
        namespace: String,
        values: BTreeMap<String, String>,
    },
    StoreRemove {
        namespace: String,
        keys: Vec<String>,
    },
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum AsyncOp {
    Sleep(EmbassyDuration),
    WifiDisconnect,
    WifiStop,
    WifiStart,
    WifiScanNetworks,
    WifiConfigureClient {
        ssid: String,
        password: String,
        channel: Option<u8>,
        auth: wifi::ClientAuth,
    },
    WifiConnect {
        timeout: Duration,
    },
    PortalStartAccessPoint {
        ssid: String,
    },
    PortalStopAccessPoint,
    HttpGet {
        url: String,
    },
    PortalHttpRequest {
        method: String,
        path: String,
        body: Vec<u8>,
    },
    PortalClientConnected,
    PortalStopped,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum SyncResult {
    BootReason(BootReason),
    MacAddress([u8; 6]),
    StoreRead(BTreeMap<String, String>),
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AsyncResult {
    SleepDone,
    Unit,
    PortalSignal,
    ScanNetworks(Vec<wifi::FoundNetwork>),
    ConnectionInfo(wifi::ConnectionInfo),
    PortalStartAccessPoint(wifi::IpConfig),
    HttpFrame(Vec<u8>),
    PortalHttpResponse {
        status_code: u16,
        content_type: &'static str,
        body_len: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InboundAsyncKind {
    PortalHttpRequest,
    PortalClientConnected,
    PortalStopped,
}

type BoxHttpHandler = Arc<
    dyn Fn(HttpRequest) -> Pin<Box<dyn Future<Output = Result<HttpResponse>> + Send>> + Send + Sync,
>;

#[derive(Default)]
struct PortalState {
    http_handler: Option<BoxHttpHandler>,
    client_connected_count: usize,
    stopped_count: usize,
}

impl PortalState {
    fn pop_client_connected(&mut self) -> bool {
        if self.client_connected_count == 0 {
            false
        } else {
            self.client_connected_count -= 1;
            true
        }
    }

    fn pop_stopped(&mut self) -> bool {
        if self.stopped_count == 0 {
            false
        } else {
            self.stopped_count -= 1;
            true
        }
    }
}

struct InfoPanelSpec;

impl NextEventsSpec<SyncOp, AsyncOp, SyncResult, AsyncResult> for InfoPanelSpec {
    type InboundAsyncKind = InboundAsyncKind;

    fn sync_result_matches(op: &SyncOp, result: &SyncResult) -> bool {
        matches!(
            (op, result),
            (SyncOp::BootReason, SyncResult::BootReason(_))
                | (SyncOp::MacAddress, SyncResult::MacAddress(_))
                | (SyncOp::StoreRead { .. }, SyncResult::StoreRead(_))
                | (SyncOp::StoreWrite { .. }, SyncResult::Unit)
                | (SyncOp::StoreRemove { .. }, SyncResult::Unit)
        )
    }

    fn async_result_matches(op: &AsyncOp, result: &AsyncResult) -> bool {
        matches!(
            (op, result),
            (AsyncOp::Sleep(_), AsyncResult::SleepDone)
                | (AsyncOp::WifiDisconnect, AsyncResult::Unit)
                | (AsyncOp::WifiStop, AsyncResult::Unit)
                | (AsyncOp::WifiStart, AsyncResult::Unit)
                | (AsyncOp::WifiScanNetworks, AsyncResult::ScanNetworks(_))
                | (AsyncOp::WifiConfigureClient { .. }, AsyncResult::Unit)
                | (AsyncOp::WifiConnect { .. }, AsyncResult::ConnectionInfo(_))
                | (AsyncOp::PortalStartAccessPoint { .. }, AsyncResult::PortalStartAccessPoint(_))
                | (AsyncOp::PortalStopAccessPoint, AsyncResult::Unit)
                | (AsyncOp::HttpGet { .. }, AsyncResult::HttpFrame(_))
                | (AsyncOp::PortalHttpRequest { .. }, AsyncResult::PortalHttpResponse { .. })
                | (AsyncOp::PortalClientConnected, AsyncResult::PortalSignal)
                | (AsyncOp::PortalStopped, AsyncResult::PortalSignal)
        )
    }

    fn async_timing(op: &AsyncOp) -> AsyncTiming {
        match op {
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(10) => {
                AsyncTiming::Delay(Duration::from_millis(10))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(20) => {
                AsyncTiming::Delay(Duration::from_millis(20))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(100) => {
                AsyncTiming::Delay(Duration::from_millis(100))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(150) => {
                AsyncTiming::Delay(Duration::from_millis(150))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(250) => {
                AsyncTiming::Delay(Duration::from_millis(250))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_millis(500) => {
                AsyncTiming::Delay(Duration::from_millis(500))
            }
            AsyncOp::Sleep(duration) if *duration == EmbassyDuration::from_secs(30) => {
                AsyncTiming::Delay(Duration::from_secs(30))
            }
            _ => AsyncTiming::Untimed,
        }
    }

    fn possible_inbound_async(
        trace: &[TraceStep<SyncOp, AsyncOp, SyncResult, AsyncResult>],
    ) -> Vec<Self::InboundAsyncKind> {
        let portal_active = trace
            .iter()
            .flat_map(|step| step.outbound.iter())
            .any(|event| matches!(event, Event::CreateAsync { op: AsyncOp::PortalStartAccessPoint { .. }, .. }));

        if portal_active {
            vec![
                InboundAsyncKind::PortalHttpRequest,
                InboundAsyncKind::PortalClientConnected,
                InboundAsyncKind::PortalStopped,
            ]
        } else {
            Vec::new()
        }
    }
}

struct InfoPanelBundle {
    config: BTreeMap<String, String>,
    frame_bytes: Vec<u8>,
    boot_reason: BootReason,
}

impl InfoPanelBundle {
    fn new() -> Self {
        Self::with_boot_reason(BootReason::Software)
    }

    fn with_boot_reason(boot_reason: BootReason) -> Self {
        let mut config = BTreeMap::new();
        config.insert("ssid".to_string(), "test_ssid".to_string());
        config.insert("pw".to_string(), "test_pw".to_string());
        config.insert("url".to_string(), "http://example.com/frame.rgb565".to_string());
        config.insert("led_brightness".to_string(), "128".to_string());

        Self {
            config,
            frame_bytes: vec![0u8; TFT_WIDTH as usize * TFT_HEIGHT as usize * 2],
            boot_reason,
        }
    }
}

impl SimBundle for InfoPanelBundle {
    type SyncOp = SyncOp;
    type AsyncOp = AsyncOp;
    type SyncResult = SyncResult;
    type AsyncResult = AsyncResult;
    type RunFuture = UnsafeSendFuture;

    fn build(
        self,
        driver: SimDriver<Self::SyncOp, Self::AsyncOp, Self::SyncResult, Self::AsyncResult>,
    ) -> Self::RunFuture {
        UnsafeSendFuture(Box::pin(async move {
            let portal_state = Arc::new(Mutex::new(PortalState::default()));
            let hal = Hal {
                wifi_backend: SimWifiBackend::new(driver.clone(), portal_state.clone()),
                store: SimStore::new(driver.clone()),
                http_backend: SimConfigHttpBackend::new(driver.clone(), portal_state.clone()),
                platform: SimPlatform::new(driver.clone(), self.boot_reason),
                clock: SimClock::new(driver.clone()),
                http_client: SimHttpClient::new(driver),
                tft_backend: SimTftBackend,
                led_backend: SimLedBackend,
            };

            info_panel_lib::run(hal).await;
        }))
    }

    fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool {
        InfoPanelSpec::sync_result_matches(op, result)
    }

    fn async_result_matches(op: &Self::AsyncOp, result: &Self::AsyncResult) -> bool {
        InfoPanelSpec::async_result_matches(op, result)
    }
}

#[derive(Clone)]
struct SimPlatform {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    boot_reason: BootReason,
}

impl SimPlatform {
    fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>, boot_reason: BootReason) -> Self {
        Self { driver, boot_reason }
    }
}

impl Platform for SimPlatform {
    fn boot_reason(&self) -> BootReason {
        match self.driver.create_sync(SyncOp::BootReason) {
            SyncResult::BootReason(reason) => {
                assert_eq!(reason, self.boot_reason, "scripted boot reason should match bundle boot reason");
                reason
            }
            _ => panic!("unexpected boot reason result"),
        }
    }

    fn mac_address(&self) -> Result<[u8; 6]> {
        match self.driver.create_sync(SyncOp::MacAddress) {
            SyncResult::MacAddress(mac) => Ok(mac),
            _ => panic!("unexpected mac address result"),
        }
    }

    fn reboot(&self) -> ! {
        panic!("unexpected reboot")
    }
}

#[derive(Clone)]
struct SimClock {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimClock {
    fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
        Self { driver }
    }
}

impl Clock for SimClock {
    fn now(&self) -> embassy_time::Instant {
        embassy_time::Instant::from_ticks(0)
    }

    async fn sleep(&self, duration: EmbassyDuration) {
        match self.driver.create_async(AsyncOp::Sleep(duration)).await {
            AsyncCompletion::Resolved(AsyncResult::SleepDone) => {}
            _ => panic!("unexpected sleep completion"),
        }
    }
}

#[derive(Clone)]
struct SimStore {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimStore {
    fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
        Self { driver }
    }
}

impl ConfigStore for SimStore {
    fn read(&self, namespace: &str, keys: &[&str]) -> Result<BTreeMap<String, String>> {
        let op = SyncOp::StoreRead {
            namespace: namespace.to_string(),
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
        };
        match self.driver.create_sync(op) {
            SyncResult::StoreRead(values) => Ok(values),
            _ => panic!("unexpected store read result"),
        }
    }

    fn write(&self, _namespace: &str, _values: &BTreeMap<String, String>) -> Result<()> {
        let op = SyncOp::StoreWrite {
            namespace: _namespace.to_string(),
            values: _values.clone(),
        };
        match self.driver.create_sync(op) {
            SyncResult::Unit => Ok(()),
            _ => panic!("unexpected store write result"),
        }
    }

    fn remove(&self, _namespace: &str, _keys: &[&str]) -> Result<()> {
        let op = SyncOp::StoreRemove {
            namespace: _namespace.to_string(),
            keys: _keys.iter().map(|key| (*key).to_string()).collect(),
        };
        match self.driver.create_sync(op) {
            SyncResult::Unit => Ok(()),
            _ => panic!("unexpected store remove result"),
        }
    }
}

#[derive(Clone)]
struct SimConfigHttpBackend {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    portal_state: Arc<Mutex<PortalState>>,
}

impl SimConfigHttpBackend {
    fn new(
        driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
        portal_state: Arc<Mutex<PortalState>>,
    ) -> Self {
        Self { driver, portal_state }
    }
}

impl config_portal::ConfigHttpBackend for SimConfigHttpBackend {
    type Server = SimConfigServer;

    fn start<H, Fut>(self, _endpoints: &'static [HttpEndpoint], handler: H) -> Result<Self::Server>
    where
        H: Fn(HttpRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<HttpResponse>> + Send,
    {
        let handler = Arc::new(handler);
        let boxed: BoxHttpHandler = Arc::new(move |request| {
            let handler = handler.clone();
            Box::pin(async move { handler(request).await })
        });
        self.portal_state.lock().unwrap().http_handler = Some(boxed);

        let driver = self.driver.clone();
        let portal_state = self.portal_state.clone();
        self.driver.spawn(async move {
            loop {
                let inbound = driver.next_inbound_async().await;
                match inbound.op {
                    AsyncOp::PortalHttpRequest { method, path, body } => {
                        let handler = portal_state
                            .lock()
                            .unwrap()
                            .http_handler
                            .clone()
                            .expect("portal HTTP handler should be installed");
                        let driver = driver.clone();
                        let resolve_driver = driver.clone();
                        driver.spawn(async move {
                            let request = HttpRequest {
                                method: match method.as_str() {
                                    "GET" => HttpMethod::Get,
                                    "POST" => HttpMethod::Post,
                                    other => HttpMethod::Other(other.to_string()),
                                },
                                path,
                                headers: BTreeMap::new(),
                                body,
                            };
                            let response = handler(request).await.expect("portal HTTP request should succeed");
                            resolve_driver.resolve_inbound_async(
                                inbound.id,
                                AsyncResult::PortalHttpResponse {
                                    status_code: response.status_code,
                                    content_type: response.content_type,
                                    body_len: response.body.len(),
                                },
                            );
                        });
                    }
                    AsyncOp::PortalClientConnected => {
                        portal_state.lock().unwrap().client_connected_count += 1;
                        driver.resolve_inbound_async(inbound.id, AsyncResult::PortalSignal);
                    }
                    AsyncOp::PortalStopped => {
                        portal_state.lock().unwrap().stopped_count += 1;
                        driver.resolve_inbound_async(inbound.id, AsyncResult::PortalSignal);
                    }
                    other => panic!("unexpected inbound async op in portal router: {other:?}"),
                }
            }
        });

        Ok(SimConfigServer)
    }
}

struct SimConfigServer;

#[derive(Clone)]
struct SimWifiBackend {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    portal_state: Arc<Mutex<PortalState>>,
}

impl SimWifiBackend {
    fn new(
        driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
        portal_state: Arc<Mutex<PortalState>>,
    ) -> Self {
        Self { driver, portal_state }
    }
}

impl wifi::WifiBackend for SimWifiBackend {
    type AccessPointClientConnectedSubscription = SimPortalSubscription;
    type AccessPointStoppedSubscription = SimPortalSubscription;

    async fn start(&mut self) -> Result<()> {
        match self.driver.create_async(AsyncOp::WifiStart).await {
            AsyncCompletion::Resolved(AsyncResult::Unit) => Ok(()),
            _ => Err(anyhow!("unexpected wifi start completion")),
        }
    }

    async fn stop(&mut self) -> Result<()> {
        match self.driver.create_async(AsyncOp::WifiStop).await {
            AsyncCompletion::Resolved(AsyncResult::Unit) => Ok(()),
            _ => Err(anyhow!("unexpected wifi stop completion")),
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        match self.driver.create_async(AsyncOp::WifiDisconnect).await {
            AsyncCompletion::Resolved(AsyncResult::Unit) => Ok(()),
            _ => Err(anyhow!("unexpected wifi disconnect completion")),
        }
    }

    async fn scan_networks(&mut self) -> Result<Vec<wifi::FoundNetwork>> {
        match self.driver.create_async(AsyncOp::WifiScanNetworks).await {
            AsyncCompletion::Resolved(AsyncResult::ScanNetworks(networks)) => Ok(networks),
            _ => Err(anyhow!("unexpected wifi scan completion")),
        }
    }

    async fn configure_client(
        &mut self,
        credentials: &wifi::WifiCredentials,
        channel: Option<u8>,
        auth: wifi::ClientAuth,
    ) -> Result<()> {
        match self
            .driver
            .create_async(AsyncOp::WifiConfigureClient {
                ssid: credentials.ssid.clone(),
                password: credentials.password.clone(),
                channel,
                auth,
            })
            .await
        {
            AsyncCompletion::Resolved(AsyncResult::Unit) => Ok(()),
            _ => Err(anyhow!("unexpected wifi configure completion")),
        }
    }

    async fn connect(&mut self, timeout: Duration) -> Result<wifi::ConnectionInfo> {
        match self.driver.create_async(AsyncOp::WifiConnect { timeout }).await {
            AsyncCompletion::Resolved(AsyncResult::ConnectionInfo(info)) => Ok(info),
            _ => Err(anyhow!("unexpected wifi connect completion")),
        }
    }

    async fn is_connected(&mut self) -> Result<bool> {
        unreachable!("wifi.is_connected is not expected before the scripted stop point")
    }

    async fn start_access_point(&mut self, _config: &wifi::AccessPointConfig) -> Result<wifi::IpConfig> {
        match self
            .driver
            .create_async(AsyncOp::PortalStartAccessPoint {
                ssid: _config.ssid.clone(),
            })
            .await
        {
            AsyncCompletion::Resolved(AsyncResult::PortalStartAccessPoint(ip_config)) => Ok(ip_config),
            _ => Err(anyhow!("unexpected access point start completion")),
        }
    }

    async fn stop_access_point(&mut self) -> Result<()> {
        match self.driver.create_async(AsyncOp::PortalStopAccessPoint).await {
            AsyncCompletion::Resolved(AsyncResult::Unit) => Ok(()),
            _ => Err(anyhow!("unexpected access point stop completion")),
        }
    }

    fn subscribe_access_point_client_connected(
        &self,
    ) -> Result<Self::AccessPointClientConnectedSubscription> {
        Ok(SimPortalSubscription::new(
            self.portal_state.clone(),
            PortalSignalKind::ClientConnected,
        ))
    }

    fn subscribe_access_point_stopped(&self) -> Result<Self::AccessPointStoppedSubscription> {
        Ok(SimPortalSubscription::new(
            self.portal_state.clone(),
            PortalSignalKind::Stopped,
        ))
    }
}

#[derive(Clone, Copy)]
enum PortalSignalKind {
    ClientConnected,
    Stopped,
}

#[derive(Clone)]
struct SimPortalSubscription {
    portal_state: Arc<Mutex<PortalState>>,
    kind: PortalSignalKind,
}

impl SimPortalSubscription {
    fn new(portal_state: Arc<Mutex<PortalState>>, kind: PortalSignalKind) -> Self {
        Self { portal_state, kind }
    }
}

impl wifi::AccessPointClientConnectedSubscription for SimPortalSubscription {
    async fn next(&mut self) -> Result<()> {
        poll_fn(|_| {
            let mut state = self.portal_state.lock().unwrap();
            if matches!(self.kind, PortalSignalKind::ClientConnected) && state.pop_client_connected() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

impl wifi::AccessPointStoppedSubscription for SimPortalSubscription {
    async fn next(&mut self) -> Result<()> {
        poll_fn(|_| {
            let mut state = self.portal_state.lock().unwrap();
            if matches!(self.kind, PortalSignalKind::Stopped) && state.pop_stopped() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

struct SimHttpClient {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimHttpClient {
    fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
        Self { driver }
    }
}

impl HttpClient for SimHttpClient {
    async fn get(&mut self, url: &str) -> Result<Box<dyn tft_display::FrameSource<Error = anyhow::Error>>> {
        match self
            .driver
            .create_async(AsyncOp::HttpGet {
                url: url.to_string(),
            })
            .await
        {
            AsyncCompletion::Resolved(AsyncResult::HttpFrame(bytes)) => {
                Ok(Box::new(ByteFrameSource::new(bytes)))
            }
            _ => Err(anyhow!("unexpected http get completion")),
        }
    }
}

struct ByteFrameSource {
    bytes: Vec<u8>,
    offset: usize,
}

impl ByteFrameSource {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, offset: 0 }
    }
}

impl tft_display::FrameSource for ByteFrameSource {
    type Error = anyhow::Error;

    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, Self::Error> {
        if self.offset >= self.bytes.len() {
            return Ok(0);
        }

        let read = (self.bytes.len() - self.offset).min(buf.len());
        buf[..read].copy_from_slice(&self.bytes[self.offset..self.offset + read]);
        self.offset += read;
        Ok(read)
    }
}

struct SimTftBackend;

impl tft_display::TftBackend for SimTftBackend {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_dc_high(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_rst_low(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_rst_high(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        Ok(())
    }
}

struct SimLedBackend;

impl rgb_led::RgbLedBackend for SimLedBackend {
    type Error = anyhow::Error;

    fn color_order(&self) -> rgb_led::ColorOrder {
        rgb_led::ColorOrder::RGB
    }

    fn set_pixel_bytes(&mut self, _bytes: [u8; 3]) -> Result<()> {
        Ok(())
    }
}

fn scripted_inputs(frame_bytes: &[u8], config: &BTreeMap<String, String>) -> Vec<Event<SyncOp, AsyncOp, SyncResult, AsyncResult>> {
    vec![
        Event::ResolveAsync {
            id: 0,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 1,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 2,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 3,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 4,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 5,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 6,
            result: AsyncResult::SleepDone,
        },
        Event::ReturnSync {
            id: 7,
            result: SyncResult::BootReason(BootReason::Software),
        },
        Event::ReturnSync {
            id: 8,
            result: SyncResult::StoreRead(config.clone()),
        },
        Event::ResolveAsync {
            id: 9,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 10,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 11,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 12,
            result: AsyncResult::ScanNetworks(vec![wifi::FoundNetwork::new(
                "test_ssid",
                Some(6),
                Some(-42),
            )]),
        },
        Event::ResolveAsync {
            id: 13,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 14,
            result: AsyncResult::ConnectionInfo(wifi::ConnectionInfo::new("192.168.1.23")),
        },
        Event::ResolveAsync {
            id: 15,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 16,
            result: AsyncResult::HttpFrame(frame_bytes.to_vec()),
        },
    ]
}

fn scripted_portal_inputs(config: &BTreeMap<String, String>) -> Vec<Event<SyncOp, AsyncOp, SyncResult, AsyncResult>> {
    vec![
        Event::ResolveAsync {
            id: 0,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 1,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 2,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 3,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 4,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 5,
            result: AsyncResult::SleepDone,
        },
        Event::ResolveAsync {
            id: 6,
            result: AsyncResult::SleepDone,
        },
        Event::ReturnSync {
            id: 7,
            result: SyncResult::BootReason(BootReason::PowerOn),
        },
        Event::ReturnSync {
            id: 8,
            result: SyncResult::StoreRead(config.clone()),
        },
        Event::ResolveAsync {
            id: 9,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 10,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 11,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 12,
            result: AsyncResult::ScanNetworks(vec![wifi::FoundNetwork::new(
                "test_ssid",
                Some(6),
                Some(-42),
            )]),
        },
        Event::ResolveAsync {
            id: 13,
            result: AsyncResult::Unit,
        },
        Event::ReturnSync {
            id: 14,
            result: SyncResult::MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x34]),
        },
        Event::ResolveAsync {
            id: 15,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 16,
            result: AsyncResult::Unit,
        },
        Event::ResolveAsync {
            id: 17,
            result: AsyncResult::PortalStartAccessPoint(wifi::IpConfig::new(
                "192.168.4.1",
                "192.168.4.1",
                "255.255.255.0",
            )),
        },
        Event::CreateAsync {
            id: 100,
            op: AsyncOp::PortalHttpRequest {
                method: "GET".to_string(),
                path: "/".to_string(),
                body: Vec::new(),
            },
        },
        Event::ResolveAsync {
            id: 18,
            result: AsyncResult::SleepDone,
        },
        Event::ReturnSync {
            id: 20,
            result: SyncResult::StoreRead(config.clone()),
        },
    ]
}

fn format_embassy_duration(duration: EmbassyDuration) -> String {
    if duration == EmbassyDuration::from_millis(10) {
        "10ms".to_string()
    } else if duration == EmbassyDuration::from_millis(20) {
        "20ms".to_string()
    } else if duration == EmbassyDuration::from_millis(100) {
        "100ms".to_string()
    } else if duration == EmbassyDuration::from_millis(150) {
        "150ms".to_string()
    } else if duration == EmbassyDuration::from_millis(250) {
        "250ms".to_string()
    } else if duration == EmbassyDuration::from_millis(500) {
        "500ms".to_string()
    } else if duration == EmbassyDuration::from_secs(30) {
        "30s".to_string()
    } else {
        format!("{duration:?}")
    }
}

fn format_sync_op(op: &SyncOp) -> String {
    match op {
        SyncOp::BootReason => "BootReason".to_string(),
        SyncOp::MacAddress => "MacAddress".to_string(),
        SyncOp::StoreRead { namespace, keys } => {
            format!("StoreRead(namespace={namespace}, keys={keys:?})")
        }
        SyncOp::StoreWrite { namespace, values } => {
            format!("StoreWrite(namespace={namespace}, values={values:?})")
        }
        SyncOp::StoreRemove { namespace, keys } => {
            format!("StoreRemove(namespace={namespace}, keys={keys:?})")
        }
    }
}

fn format_async_op(op: &AsyncOp) -> String {
    match op {
        AsyncOp::Sleep(duration) => format!("Sleep({})", format_embassy_duration(*duration)),
        AsyncOp::WifiDisconnect => "WifiDisconnect".to_string(),
        AsyncOp::WifiStop => "WifiStop".to_string(),
        AsyncOp::WifiStart => "WifiStart".to_string(),
        AsyncOp::WifiScanNetworks => "WifiScanNetworks".to_string(),
        AsyncOp::WifiConfigureClient {
            ssid,
            password: _,
            channel,
            auth,
        } => format!("WifiConfigureClient(ssid={ssid}, channel={channel:?}, auth={auth:?})"),
        AsyncOp::WifiConnect { timeout } => format!("WifiConnect(timeout={timeout:?})"),
        AsyncOp::PortalStartAccessPoint { ssid } => format!("PortalStartAccessPoint(ssid={ssid})"),
        AsyncOp::PortalStopAccessPoint => "PortalStopAccessPoint".to_string(),
        AsyncOp::HttpGet { url } => format!("HttpGet(url={url})"),
        AsyncOp::PortalHttpRequest { method, path, body } => {
            format!("PortalHttpRequest({method} {path}, body_len={})", body.len())
        }
        AsyncOp::PortalClientConnected => "PortalClientConnected".to_string(),
        AsyncOp::PortalStopped => "PortalStopped".to_string(),
    }
}

fn format_event(event: &Event<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> String {
    match event {
        Event::CreateSync { id, op } => format!("CreateSync#{id} {}", format_sync_op(op)),
        Event::ReturnSync { id, result } => format!("ReturnSync#{id} {result:?}"),
        Event::CreateAsync { id, op } => format!("CreateAsync#{id} {}", format_async_op(op)),
        Event::ResolveAsync { id, result } => match result {
            AsyncResult::HttpFrame(bytes) => format!("ResolveAsync#{id} HttpFrame(len={})", bytes.len()),
            AsyncResult::PortalHttpResponse {
                status_code,
                content_type,
                body_len,
            } => format!(
                "ResolveAsync#{id} PortalHttpResponse(status={status_code}, type={content_type}, body_len={body_len})"
            ),
            _ => format!("ResolveAsync#{id} {result:?}"),
        },
        Event::CancelAsync { id } => format!("CancelAsync#{id}"),
        Event::AbortAsync { id } => format!("AbortAsync#{id}"),
    }
}

fn format_possible_event(event: &PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>) -> String {
    match event {
        PossibleEvent::ReturnSync { id, op } => format!("ReturnSync#{id} {}", format_sync_op(op)),
        PossibleEvent::ResolveAsync { id, op, warnings } => {
            if warnings.is_empty() {
                format!("ResolveAsync#{id} {}", format_async_op(op))
            } else {
                format!("ResolveAsync#{id} {} warnings={warnings:?}", format_async_op(op))
            }
        }
        PossibleEvent::AbortAsync { id, op } => format!("AbortAsync#{id} {}", format_async_op(op)),
        PossibleEvent::CreateInboundAsync { kind } => match kind {
            InboundAsyncKind::PortalHttpRequest => "CreateInboundAsync PortalHttpRequest".to_string(),
            InboundAsyncKind::PortalClientConnected => {
                "CreateInboundAsync PortalClientConnected".to_string()
            }
            InboundAsyncKind::PortalStopped => "CreateInboundAsync PortalStopped".to_string(),
        },
        PossibleEvent::CancelInboundAsync { id, op } => {
            format!("CancelInboundAsync#{id} {}", format_async_op(op))
        }
    }
}

fn describe_step(
    index: usize,
    inbound: Option<&Event<SyncOp, AsyncOp, SyncResult, AsyncResult>>,
    outbound: &[Event<SyncOp, AsyncOp, SyncResult, AsyncResult>],
    possible: &[PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>],
) -> String {
    let outbound = outbound.iter().map(format_event).collect::<Vec<_>>().join(", ");
    let possible = possible
        .iter()
        .map(format_possible_event)
        .collect::<Vec<_>>()
        .join(", ");
    let inbound = inbound
        .map(format_event)
        .unwrap_or_else(|| "<start>".to_string());
    format!(
        "Step {index}\n  sent in: {inbound}\n  wrapper out: [{outbound}]\n  possible next: [{possible}]\n"
    )
}

fn allows_event(
    possible: &[PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>],
    event: &Event<SyncOp, AsyncOp, SyncResult, AsyncResult>,
) -> bool {
    possible.iter().any(|candidate| match (candidate, event) {
        (PossibleEvent::ReturnSync { id, op }, Event::ReturnSync { id: actual_id, result }) => {
            *id == *actual_id && InfoPanelSpec::sync_result_matches(op, result)
        }
        (
            PossibleEvent::ResolveAsync { id, op, .. },
            Event::ResolveAsync {
                id: actual_id,
                result,
            },
        ) => *id == *actual_id && InfoPanelSpec::async_result_matches(op, result),
        (PossibleEvent::CreateInboundAsync { kind }, Event::CreateAsync { op, .. }) => {
            matches!(
                (kind, op),
                (InboundAsyncKind::PortalHttpRequest, AsyncOp::PortalHttpRequest { .. })
                    | (InboundAsyncKind::PortalClientConnected, AsyncOp::PortalClientConnected)
                    | (InboundAsyncKind::PortalStopped, AsyncOp::PortalStopped)
            )
        }
        (PossibleEvent::AbortAsync { id, .. }, Event::AbortAsync { id: actual_id }) => *id == *actual_id,
        (PossibleEvent::CancelInboundAsync { id, .. }, Event::CancelAsync { id: actual_id }) => {
            *id == *actual_id
        }
        _ => false,
    })
}

fn drive_script(
    bundle: InfoPanelBundle,
    scripted_inputs: Vec<Event<SyncOp, AsyncOp, SyncResult, AsyncResult>>,
) -> (
    String,
    Vec<TraceStep<SyncOp, AsyncOp, SyncResult, AsyncResult>>,
    Vec<PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>>,
) {
    let (mut wrapper, initial_outbound) = NewRunWrapper::new(bundle).start();

    let mut trace = vec![TraceStep::start(initial_outbound.clone())];
    let mut report = String::new();

    let mut possible =
        possible_next_events::<_, _, _, _, InfoPanelSpec>(&trace).expect("initial trace should replay");
    report.push_str(&describe_step(0, None, &initial_outbound, &possible));

    for (index, inbound) in scripted_inputs.into_iter().enumerate() {
        assert!(
            allows_event(&possible, &inbound),
            "scripted inbound event is not currently possible at step {}: {inbound:#?}\n{report}",
            index + 1,
        );

        let outbound = wrapper.push(inbound.clone());
        trace.push(TraceStep::push(inbound, outbound.clone()));
        possible =
            possible_next_events::<_, _, _, _, InfoPanelSpec>(&trace).expect("trace should replay");
        report.push_str(&describe_step(index + 1, trace.last().unwrap().inbound.as_ref(), &outbound, &possible));
    }

    (report, trace, possible)
}

#[test]
fn test_sim_reports_startup_outputs_and_next_events() {
    let bundle = InfoPanelBundle::new();
    let scripted_inputs = scripted_inputs(&bundle.frame_bytes, &bundle.config);
    let (report, trace, possible) = drive_script(bundle, scripted_inputs);

    println!("{report}");

    assert!(
        report.contains("BootReason")
            && report.contains("StoreRead")
            && report.contains("WifiConfigureClient")
            && report.contains("HttpGet")
            && report.contains("Sleep("),
        "report should contain useful startup milestones:\n{report}"
    );

    assert_eq!(
        trace.last().unwrap().outbound,
        vec![Event::CreateAsync {
            id: 17,
            op: AsyncOp::Sleep(EmbassyDuration::from_secs(30)),
        }],
        "after the initial fetch, the app should enter the runtime sleep"
    );

    assert!(
        allows_event(
            &possible,
            &Event::ResolveAsync {
                id: 17,
                result: AsyncResult::SleepDone,
            }
        ),
        "the final possible-next list should offer resolving the first runtime sleep:\n{report}"
    );
}

#[test]
fn test_sim_reports_config_portal_and_http_request() {
    let bundle = InfoPanelBundle::with_boot_reason(BootReason::PowerOn);
    let scripted_inputs = scripted_portal_inputs(&bundle.config);
    let (report, trace, possible) = drive_script(bundle, scripted_inputs);

    println!("{report}");

    assert!(
        report.contains("BootReason(PowerOn)")
            && report.contains("MacAddress")
            && report.contains("PortalStartAccessPoint")
            && report.contains("CreateInboundAsync PortalHttpRequest")
            && report.contains("sent in: CreateAsync#100 PortalHttpRequest(GET /")
            && report.contains("CreateSync#20 StoreRead")
            && report.contains("ResolveAsync#100 PortalHttpResponse(status=200"),
        "report should show the config portal request flow:\n{report}"
    );

    assert!(
        matches!(
            trace.last().unwrap().outbound.as_slice(),
            [Event::ResolveAsync {
                id: 100,
                result: AsyncResult::PortalHttpResponse {
                    status_code: 200,
                    content_type: "text/html; charset=utf-8",
                    body_len: _,
                },
            }]
        ),
        "after the simulated POST /save, the portal should emit an HTTP response"
    );

    assert!(
        possible.iter().any(|event| matches!(
            event,
            PossibleEvent::ResolveAsync {
                id: 19,
                op: AsyncOp::Sleep(duration),
                ..
            } if *duration == EmbassyDuration::from_millis(250)
        )),
        "after the request, the next portal wait sleep should be pending:\n{report}"
    );
}
