use super::*;
use super::bundle::*;
use std::panic::resume_unwind;
use std::sync::atomic::AtomicBool;

pub(super) struct UnsafeSendFuture(Pin<Box<dyn Future<Output = ()> + 'static>>);

unsafe impl Send for UnsafeSendFuture {}

impl Future for UnsafeSendFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SyncOp {
    BootReason,
    MacAddress,
    Now,
    TftSetDcLow,
    TftSetDcHigh,
    TftSetRstLow,
    TftSetRstHigh,
    TftWrite { bytes: Vec<u8> },
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
pub(super) enum AsyncOp {
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
pub(super) enum SyncResult {
    BootReason(BootReason),
    MacAddress([u8; 6]),
    Now(u64),
    StoreRead(Result<BTreeMap<String, String>, String>),
    Unit(Result<(), String>),
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AsyncResult {
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

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum InboundAsyncKind {
    PortalHttpRequest,
    PortalClientConnected,
    PortalStopped,
}

pub(super) type BoxHttpHandler = Arc<
    dyn Fn(HttpRequest) -> Pin<Box<dyn Future<Output = Result<HttpResponse>> + Send>> + Send + Sync,
>;

#[derive(Default)]
pub(super) struct PortalState {
    pub(super) http_handler: Option<BoxHttpHandler>,
    pub(super) client_connected_count: usize,
    pub(super) stopped_count: usize,
}

impl PortalState {
    pub(super) fn pop_client_connected(&mut self) -> bool {
        if self.client_connected_count == 0 {
            false
        } else {
            self.client_connected_count -= 1;
            true
        }
    }

    pub(super) fn pop_stopped(&mut self) -> bool {
        if self.stopped_count == 0 {
            false
        } else {
            self.stopped_count -= 1;
            true
        }
    }
}

pub(super) struct InfoPanelSpec;

impl NextEventsSpec<SyncOp, AsyncOp, SyncResult, AsyncResult> for InfoPanelSpec {
    type InboundAsyncKind = InboundAsyncKind;

    fn sync_result_matches(op: &SyncOp, result: &SyncResult) -> bool {
        matches!(
            (op, result),
            (SyncOp::BootReason, SyncResult::BootReason(_))
                | (SyncOp::MacAddress, SyncResult::MacAddress(_))
                | (SyncOp::Now, SyncResult::Now(_))
                | (SyncOp::TftSetDcLow, SyncResult::Unit(_))
                | (SyncOp::TftSetDcHigh, SyncResult::Unit(_))
                | (SyncOp::TftSetRstLow, SyncResult::Unit(_))
                | (SyncOp::TftSetRstHigh, SyncResult::Unit(_))
                | (SyncOp::TftWrite { .. }, SyncResult::Unit(_))
                | (SyncOp::StoreRead { .. }, SyncResult::StoreRead(_))
                | (SyncOp::StoreWrite { .. }, SyncResult::Unit(_))
                | (SyncOp::StoreRemove { .. }, SyncResult::Unit(_))
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
            AsyncOp::Sleep(duration) => AsyncTiming::Delay(Duration::from_millis(duration.as_millis())),
            _ => AsyncTiming::Untimed,
        }
    }

    fn possible_inbound_async(
        trace: &[TraceStep<SyncOp, AsyncOp, SyncResult, AsyncResult>],
    ) -> Vec<Self::InboundAsyncKind> {
        let portal_active = trace.iter().flat_map(|step| step.outbound.iter()).any(|event| {
            matches!(
                event,
                Event::CreateAsync {
                    op: AsyncOp::PortalStartAccessPoint { .. },
                    ..
                }
            )
        });

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

pub(super) struct InfoPanelBundle {
    pub(super) rebooted: Arc<AtomicBool>,
}

impl InfoPanelBundle {
    pub(super) fn new(rebooted: Arc<AtomicBool>) -> Self {
        Self { rebooted }
    }
}

#[derive(Debug)]
pub(super) struct RebootPanic;

pub(super) fn reboot_panic() -> ! {
    resume_unwind(Box::new(RebootPanic))
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
                platform: SimPlatform::new(driver.clone(), self.rebooted.clone()),
                clock: SimClock::new(driver.clone()),
                http_client: SimHttpClient::new(driver.clone()),
                tft_backend: SimTftBackend::new(driver),
                led_backend: SimLedBackend,
            };

            crate::run(hal).await;
        }))
    }

    fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool {
        InfoPanelSpec::sync_result_matches(op, result)
    }

    fn async_result_matches(op: &Self::AsyncOp, result: &Self::AsyncResult) -> bool {
        InfoPanelSpec::async_result_matches(op, result)
    }
}
