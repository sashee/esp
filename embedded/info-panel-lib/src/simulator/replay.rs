use super::codec::*;
use super::types::*;
use super::*;

pub(super) fn format_embassy_duration(duration: EmbassyDuration) -> String {
    let millis = duration.as_millis();
    if millis != 0 && millis % 1000 == 0 {
        format!("{}s", millis / 1000)
    } else if millis > 0 {
        format!("{millis}ms")
    } else {
        format!("{duration:?}")
    }
}

pub(super) fn format_sync_op(op: &SyncOp) -> String {
    match op {
        SyncOp::BootReason => "BootReason".to_string(),
        SyncOp::MacAddress => "MacAddress".to_string(),
        SyncOp::Now => "Now".to_string(),
        SyncOp::TftSetDcLow => "TftSetDcLow".to_string(),
        SyncOp::TftSetDcHigh => "TftSetDcHigh".to_string(),
        SyncOp::TftSetRstLow => "TftSetRstLow".to_string(),
        SyncOp::TftSetRstHigh => "TftSetRstHigh".to_string(),
        SyncOp::TftWrite { bytes } => format!("TftWrite(len={})", bytes.len()),
        SyncOp::StoreRead { namespace, keys } => {
            format!("StoreRead(namespace={namespace}, keys={keys:?})")
        }
        SyncOp::StoreWrite { namespace, values } => {
            format!("StoreWrite(namespace={namespace}, values={values:?})")
        }
        SyncOp::StoreRemove { namespace, keys } => {
            format!("StoreRemove(namespace={namespace}, keys={keys:?})")
        }
        SyncOp::HttpRead { body, max_len } => {
            format!("HttpRead(body={body}, max_len={max_len})")
        }
    }
}

pub(super) fn format_async_op(op: &AsyncOp) -> String {
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
            format!(
                "PortalHttpRequest({method} {path}, body_len={})",
                body.len()
            )
        }
        AsyncOp::PortalClientConnected => "PortalClientConnected".to_string(),
        AsyncOp::PortalStopped => "PortalStopped".to_string(),
    }
}

pub(super) fn format_event(event: &Event<SyncOp, AsyncOp, SyncResult, AsyncResult>) -> String {
    match event {
        Event::CreateSync { op, .. } => format!("CreateSync {}", format_sync_op(op)),
        Event::ReturnSync { result, .. } => format!("ReturnSync {result:?}"),
        Event::CreateAsync { op, .. } => format!("CreateAsync {}", format_async_op(op)),
        Event::ResolveAsync { result, .. } => match result {
            AsyncResult::HttpResponse { body } => {
                format!("ResolveAsync HttpResponse(body={body})")
            }
            AsyncResult::PortalHttpResponse {
                status_code,
                content_type,
                body_len,
            } => format!(
                "ResolveAsync PortalHttpResponse(status={status_code}, type={content_type}, body_len={body_len})"
            ),
            _ => format!("ResolveAsync {result:?}"),
        },
        Event::CancelAsync { .. } => "CancelAsync".to_string(),
        Event::AbortAsync { .. } => "AbortAsync".to_string(),
    }
}

pub(super) fn format_saved_async_op(op: &SavedAsyncOp) -> String {
    match op {
        SavedAsyncOp::Sleep { duration } => {
            format!("Sleep({})", format_embassy_duration(duration.to_runtime()))
        }
        SavedAsyncOp::WifiDisconnect => "WifiDisconnect".to_string(),
        SavedAsyncOp::WifiStop => "WifiStop".to_string(),
        SavedAsyncOp::WifiStart => "WifiStart".to_string(),
        SavedAsyncOp::WifiScanNetworks => "WifiScanNetworks".to_string(),
        SavedAsyncOp::WifiConfigureClient {
            ssid,
            channel,
            auth,
            ..
        } => format!("WifiConfigureClient(ssid={ssid}, channel={channel:?}, auth={auth:?})"),
        SavedAsyncOp::WifiConnect { timeout } => {
            format!("WifiConnect(timeout={:?})", timeout.to_runtime())
        }
        SavedAsyncOp::PortalStartAccessPoint { ssid } => {
            format!("PortalStartAccessPoint(ssid={ssid})")
        }
        SavedAsyncOp::PortalStopAccessPoint => "PortalStopAccessPoint".to_string(),
        SavedAsyncOp::HttpGet { url } => format!("HttpGet(url={url})"),
        SavedAsyncOp::PortalHttpRequest { method, path, body } => {
            format!(
                "PortalHttpRequest({method} {path}, body_len={})",
                body.len()
            )
        }
        SavedAsyncOp::PortalClientConnected => "PortalClientConnected".to_string(),
        SavedAsyncOp::PortalStopped => "PortalStopped".to_string(),
    }
}

pub(super) fn format_saved_item(item: &SavedItem) -> String {
    match item {
        SavedItem::OutboundCreateSync { id, target, .. } => match target {
            Some(target) => format!("OUTBOUND {id} <- CreateSync target={target}"),
            None => format!("OUTBOUND {id} <- CreateSync"),
        },
        SavedItem::OutboundCreateAsync { id, target, .. } => match target {
            Some(target) => format!("OUTBOUND {id} <- CreateAsync target={target}"),
            None => format!("OUTBOUND {id} <- CreateAsync"),
        },
        SavedItem::OutboundDropResult { target } => format!("OUTBOUND DropResult {target}"),
        SavedItem::InboundDropResult { target } => format!("INBOUND DropResult {target}"),
        SavedItem::InboundReturnSync { target, result } => {
            let _ = target;
            format!("INBOUND ReturnSync {result:?}")
        }
        SavedItem::InboundErrorSync { target, error } => {
            let _ = target;
            format!("INBOUND ErrorSync {error:?}")
        }
        SavedItem::InboundResolveAsync { target, result } => {
            let _ = target;
            format!("INBOUND ResolveAsync {result:?}")
        }
        SavedItem::InboundAbortAsync { target } => {
            let _ = target;
            "INBOUND AbortAsync".to_string()
        }
        SavedItem::InboundCancelAsync { target } => {
            let _ = target;
            "INBOUND CancelAsync".to_string()
        }
        SavedItem::InboundCreateAsync { id, target, op } => {
            let _ = id;
            match target {
                Some(target) => format!(
                    "INBOUND CreateAsync {} target={target}",
                    format_saved_async_op(op)
                ),
                None => format!("INBOUND CreateAsync {}", format_saved_async_op(op)),
            }
        }
    }
}

pub(super) fn default_store_values(keys: &[String]) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let defaults = BTreeMap::from([
        ("ssid".to_string(), "test_ssid".to_string()),
        ("pw".to_string(), "test_pw".to_string()),
        (
            "url".to_string(),
            "http://example.com/frame.rgb565".to_string(),
        ),
        ("led_brightness".to_string(), "128".to_string()),
    ]);
    for key in keys {
        if let Some(value) = defaults.get(key) {
            values.insert(key.clone(), value.clone());
        }
    }
    values
}

pub(super) fn default_runtime_sync_result(op: &SyncOp, current_ticks: u64) -> SyncResult {
    match op {
        SyncOp::BootReason => SyncResult::BootReason(BootReason::Software),
        SyncOp::MacAddress => SyncResult::MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x34]),
        SyncOp::Now => SyncResult::Now(current_ticks),
        SyncOp::TftSetDcLow
        | SyncOp::TftSetDcHigh
        | SyncOp::TftSetRstLow
        | SyncOp::TftSetRstHigh
        | SyncOp::TftWrite { .. }
        | SyncOp::StoreWrite { .. }
        | SyncOp::StoreRemove { .. } => SyncResult::Unit(Ok(())),
        SyncOp::StoreRead { keys, .. } => SyncResult::StoreRead(Ok(default_store_values(keys))),
        SyncOp::HttpRead { .. } => SyncResult::HttpRead {
            bytes: vec![0u8; 32],
        },
    }
}

pub(super) fn default_runtime_sync_error(op: &SyncOp) -> Option<SyncError> {
    match op {
        SyncOp::StoreRead { .. } => Some(SyncError::StoreReadErr {
            message: "simulated error".to_string(),
        }),
        SyncOp::TftSetDcLow
        | SyncOp::TftSetDcHigh
        | SyncOp::TftSetRstLow
        | SyncOp::TftSetRstHigh
        | SyncOp::TftWrite { .. }
        | SyncOp::StoreWrite { .. }
        | SyncOp::StoreRemove { .. } => Some(SyncError::UnitErr {
            message: "simulated error".to_string(),
        }),
        SyncOp::BootReason | SyncOp::MacAddress | SyncOp::Now | SyncOp::HttpRead { .. } => None,
    }
}

pub(super) fn current_ticks_from_trace(
    trace: &[TraceStep<SyncOp, AsyncOp, SyncResult, AsyncResult>],
) -> u64 {
    match elapsed_time::<_, _, _, _, InfoPanelSpec>(trace) {
        ElapsedTime::Exact(duration) | ElapsedTime::MoreThan(duration) => {
            EmbassyDuration::from_millis(duration.as_millis() as u64).as_ticks()
        }
    }
}

pub(super) fn default_runtime_async_result(op: &AsyncOp, target: &str) -> AsyncResult {
    match op {
        AsyncOp::Sleep(_) => AsyncResult::SleepDone,
        AsyncOp::WifiDisconnect
        | AsyncOp::WifiStop
        | AsyncOp::WifiStart
        | AsyncOp::WifiConfigureClient { .. }
        | AsyncOp::PortalStopAccessPoint => AsyncResult::Unit,
        AsyncOp::WifiScanNetworks => AsyncResult::ScanNetworks(vec![wifi::FoundNetwork::new(
            "test_ssid",
            Some(6),
            Some(-42),
        )]),
        AsyncOp::WifiConnect { .. } => {
            AsyncResult::ConnectionInfo(wifi::ConnectionInfo::new("192.168.1.23"))
        }
        AsyncOp::PortalStartAccessPoint { .. } => AsyncResult::PortalStartAccessPoint(
            wifi::IpConfig::new("192.168.4.1", "192.168.4.1", "255.255.255.0"),
        ),
        AsyncOp::HttpGet { .. } => AsyncResult::HttpResponse {
            body: format!("{target}_body"),
        },
        AsyncOp::PortalHttpRequest { .. } => AsyncResult::PortalHttpResponse {
            status_code: 200,
            content_type: "text/html",
            body_len: 0,
        },
        AsyncOp::PortalClientConnected | AsyncOp::PortalStopped => AsyncResult::PortalSignal,
    }
}

pub(super) fn default_runtime_inbound_async_op(kind: &InboundAsyncKind) -> AsyncOp {
    match kind {
        InboundAsyncKind::PortalHttpRequest => AsyncOp::PortalHttpRequest {
            method: "GET".to_string(),
            path: "/".to_string(),
            body: Vec::new(),
        },
        InboundAsyncKind::PortalClientConnected => AsyncOp::PortalClientConnected,
        InboundAsyncKind::PortalStopped => AsyncOp::PortalStopped,
    }
}

pub(super) fn inbound_async_kind_name(kind: &InboundAsyncKind) -> &'static str {
    match kind {
        InboundAsyncKind::PortalHttpRequest => "portal_http_request",
        InboundAsyncKind::PortalClientConnected => "portal_client_connected",
        InboundAsyncKind::PortalStopped => "portal_stopped",
    }
}

pub(super) const BOOT_REASON_OPTIONS: &[&str] = &[
    "software",
    "external_pin",
    "watchdog",
    "sdio",
    "panic",
    "interrupt_watchdog",
    "power_on",
    "unknown",
    "brownout",
    "task_watchdog",
    "deep_sleep",
    "usb_peripheral",
    "jtag",
    "efuse_error",
    "power_glitch",
    "cpu_lockup",
];
