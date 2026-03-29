use super::*;
use super::types::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub(super) struct SimPlatform {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    rebooted: Arc<AtomicBool>,
}

impl SimPlatform {
    pub(super) fn new(
        driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
        rebooted: Arc<AtomicBool>,
    ) -> Self {
        Self { driver, rebooted }
    }
}

impl Platform for SimPlatform {
    fn boot_reason(&self) -> BootReason {
        match self.driver.create_sync(SyncOp::BootReason) {
            SyncResult::BootReason(reason) => reason,
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
        self.rebooted.store(true, Ordering::SeqCst);
        reboot_panic()
    }
}

#[derive(Clone)]
pub(super) struct SimClock {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimClock {
    pub(super) fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
        Self { driver }
    }
}

impl Clock for SimClock {
    fn now(&self) -> embassy_time::Instant {
        match self.driver.create_sync(SyncOp::Now) {
            SyncResult::Now(ticks) => embassy_time::Instant::from_ticks(ticks),
            _ => panic!("unexpected now result"),
        }
    }

    async fn sleep(&self, duration: EmbassyDuration) {
        match self.driver.create_async(AsyncOp::Sleep(duration)).await {
            AsyncCompletion::Resolved(AsyncResult::SleepDone) => {}
            _ => panic!("unexpected sleep completion"),
        }
    }
}

#[derive(Clone)]
pub(super) struct SimStore {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimStore {
    pub(super) fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
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
            SyncResult::StoreRead(Ok(values)) => Ok(values),
            SyncResult::StoreRead(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected store read result"),
        }
    }

    fn write(&self, namespace: &str, values: &BTreeMap<String, String>) -> Result<()> {
        let op = SyncOp::StoreWrite {
            namespace: namespace.to_string(),
            values: values.clone(),
        };
        match self.driver.create_sync(op) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected store write result"),
        }
    }

    fn remove(&self, namespace: &str, keys: &[&str]) -> Result<()> {
        let op = SyncOp::StoreRemove {
            namespace: namespace.to_string(),
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
        };
        match self.driver.create_sync(op) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected store remove result"),
        }
    }
}

#[derive(Clone)]
pub(super) struct SimConfigHttpBackend {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    portal_state: Arc<Mutex<PortalState>>,
}

impl SimConfigHttpBackend {
    pub(super) fn new(
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

pub(super) struct SimConfigServer;

#[derive(Clone)]
pub(super) struct SimWifiBackend {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
    portal_state: Arc<Mutex<PortalState>>,
}

impl SimWifiBackend {
    pub(super) fn new(
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

    async fn start_access_point(&mut self, config: &wifi::AccessPointConfig) -> Result<wifi::IpConfig> {
        match self
            .driver
            .create_async(AsyncOp::PortalStartAccessPoint {
                ssid: config.ssid.clone(),
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
pub(super) enum PortalSignalKind {
    ClientConnected,
    Stopped,
}

#[derive(Clone)]
pub(super) struct SimPortalSubscription {
    portal_state: Arc<Mutex<PortalState>>,
    kind: PortalSignalKind,
}

impl SimPortalSubscription {
    pub(super) fn new(portal_state: Arc<Mutex<PortalState>>, kind: PortalSignalKind) -> Self {
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

pub(super) struct SimHttpClient {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimHttpClient {
    pub(super) fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
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

pub(super) struct ByteFrameSource {
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

pub(super) struct SimTftBackend {
    driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>,
}

impl SimTftBackend {
    pub(super) fn new(driver: SimDriver<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> Self {
        Self { driver }
    }
}

impl tft_display::TftBackend for SimTftBackend {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> Result<()> {
        match self.driver.create_sync(SyncOp::TftSetDcLow) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected tft set_dc_low result"),
        }
    }

    fn set_dc_high(&mut self) -> Result<()> {
        match self.driver.create_sync(SyncOp::TftSetDcHigh) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected tft set_dc_high result"),
        }
    }

    fn set_rst_low(&mut self) -> Result<()> {
        match self.driver.create_sync(SyncOp::TftSetRstLow) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected tft set_rst_low result"),
        }
    }

    fn set_rst_high(&mut self) -> Result<()> {
        match self.driver.create_sync(SyncOp::TftSetRstHigh) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected tft set_rst_high result"),
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        match self.driver.create_sync(SyncOp::TftWrite { bytes: data.to_vec() }) {
            SyncResult::Unit(Ok(())) => Ok(()),
            SyncResult::Unit(Err(message)) => Err(anyhow!(message)),
            _ => panic!("unexpected tft write result"),
        }
    }
}

pub(super) struct SimLedBackend;

impl rgb_led::RgbLedBackend for SimLedBackend {
    type Error = anyhow::Error;

    fn color_order(&self) -> rgb_led::ColorOrder { rgb_led::ColorOrder::RGB }
    fn set_pixel_bytes(&mut self, _bytes: [u8; 3]) -> Result<()> { Ok(()) }
}
