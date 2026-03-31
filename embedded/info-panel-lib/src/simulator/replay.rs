use super::saved::*;
use super::types::*;
use super::*;
use std::sync::atomic::AtomicBool;

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
            AsyncResult::HttpFrame(bytes) => format!("ResolveAsync HttpFrame(len={})", bytes.len()),
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

pub(super) fn format_possible_event(
    event: &PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>,
) -> String {
    match event {
        PossibleEvent::ReturnSync { id, op } => format!("ReturnSync#{id} {}", format_sync_op(op)),
        PossibleEvent::ResolveAsync { id, op, warnings } => {
            if warnings.is_empty() {
                format!("ResolveAsync#{id} {}", format_async_op(op))
            } else {
                format!(
                    "ResolveAsync#{id} {} warnings={warnings:?}",
                    format_async_op(op)
                )
            }
        }
        PossibleEvent::AbortAsync { id, op } => format!("AbortAsync#{id} {}", format_async_op(op)),
        PossibleEvent::CreateInboundAsync { kind } => match kind {
            InboundAsyncKind::PortalHttpRequest => {
                "CreateInboundAsync PortalHttpRequest".to_string()
            }
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

pub(super) fn format_saved_sync_op(op: &SavedSyncOp) -> String {
    match op {
        SavedSyncOp::BootReason => "BootReason".to_string(),
        SavedSyncOp::MacAddress => "MacAddress".to_string(),
        SavedSyncOp::Now => "Now".to_string(),
        SavedSyncOp::TftSetDcLow => "TftSetDcLow".to_string(),
        SavedSyncOp::TftSetDcHigh => "TftSetDcHigh".to_string(),
        SavedSyncOp::TftSetRstLow => "TftSetRstLow".to_string(),
        SavedSyncOp::TftSetRstHigh => "TftSetRstHigh".to_string(),
        SavedSyncOp::TftWrite { bytes } => format!("TftWrite(len={})", bytes.len()),
        SavedSyncOp::StoreRead { namespace, keys } => {
            format!("StoreRead(namespace={namespace}, keys={keys:?})")
        }
        SavedSyncOp::StoreWrite { namespace, values } => {
            format!("StoreWrite(namespace={namespace}, values={values:?})")
        }
        SavedSyncOp::StoreRemove { namespace, keys } => {
            format!("StoreRemove(namespace={namespace}, keys={keys:?})")
        }
    }
}

pub(super) fn format_saved_async_op(op: &SavedAsyncOp) -> String {
    match op {
        SavedAsyncOp::Sleep { duration_ms } => {
            format!(
                "Sleep({})",
                format_embassy_duration(EmbassyDuration::from_millis(*duration_ms))
            )
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
        } => format!("WifiConfigureClient(ssid={ssid}, channel={channel:?}, auth={auth})"),
        SavedAsyncOp::WifiConnect { timeout_ms } => {
            format!("WifiConnect(timeout={}ms)", timeout_ms)
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
        SavedItem::OutboundCreateSync { id, op } => {
            format!("OUTBOUND {id} <- CreateSync {}", format_saved_sync_op(op))
        }
        SavedItem::OutboundCreateAsync { id, op } => {
            format!("OUTBOUND {id} <- CreateAsync {}", format_saved_async_op(op))
        }
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
        SavedItem::InboundCreateAsync { id, op } => {
            let _ = id;
            format!("INBOUND CreateAsync {}", format_saved_async_op(op))
        }
    }
}

pub(super) fn parse_items(document: &[SavedItem]) -> Result<Vec<SavedItem>, String> {
    Ok(document.to_vec())
}

pub(super) fn runtime_sync_op_to_saved(op: &SyncOp) -> SavedSyncOp {
    match op {
        SyncOp::BootReason => SavedSyncOp::BootReason,
        SyncOp::MacAddress => SavedSyncOp::MacAddress,
        SyncOp::Now => SavedSyncOp::Now,
        SyncOp::TftSetDcLow => SavedSyncOp::TftSetDcLow,
        SyncOp::TftSetDcHigh => SavedSyncOp::TftSetDcHigh,
        SyncOp::TftSetRstLow => SavedSyncOp::TftSetRstLow,
        SyncOp::TftSetRstHigh => SavedSyncOp::TftSetRstHigh,
        SyncOp::TftWrite { bytes } => SavedSyncOp::TftWrite {
            bytes: bytes.clone(),
        },
        SyncOp::StoreRead { namespace, keys } => SavedSyncOp::StoreRead {
            namespace: namespace.clone(),
            keys: keys.clone(),
        },
        SyncOp::StoreWrite { namespace, values } => SavedSyncOp::StoreWrite {
            namespace: namespace.clone(),
            values: values.clone(),
        },
        SyncOp::StoreRemove { namespace, keys } => SavedSyncOp::StoreRemove {
            namespace: namespace.clone(),
            keys: keys.clone(),
        },
    }
}

pub(super) fn runtime_async_op_to_saved(op: &AsyncOp) -> SavedAsyncOp {
    match op {
        AsyncOp::Sleep(duration) => SavedAsyncOp::Sleep {
            duration_ms: duration.as_millis(),
        },
        AsyncOp::WifiDisconnect => SavedAsyncOp::WifiDisconnect,
        AsyncOp::WifiStop => SavedAsyncOp::WifiStop,
        AsyncOp::WifiStart => SavedAsyncOp::WifiStart,
        AsyncOp::WifiScanNetworks => SavedAsyncOp::WifiScanNetworks,
        AsyncOp::WifiConfigureClient {
            ssid,
            password,
            channel,
            auth,
        } => SavedAsyncOp::WifiConfigureClient {
            ssid: ssid.clone(),
            password: password.clone(),
            channel: *channel,
            auth: format_client_auth(*auth),
        },
        AsyncOp::WifiConnect { timeout } => SavedAsyncOp::WifiConnect {
            timeout_ms: timeout.as_millis() as u64,
        },
        AsyncOp::PortalStartAccessPoint { ssid } => {
            SavedAsyncOp::PortalStartAccessPoint { ssid: ssid.clone() }
        }
        AsyncOp::PortalStopAccessPoint => SavedAsyncOp::PortalStopAccessPoint,
        AsyncOp::HttpGet { url } => SavedAsyncOp::HttpGet { url: url.clone() },
        AsyncOp::PortalHttpRequest { method, path, body } => SavedAsyncOp::PortalHttpRequest {
            method: method.clone(),
            path: path.clone(),
            body: body.clone(),
        },
        AsyncOp::PortalClientConnected => SavedAsyncOp::PortalClientConnected,
        AsyncOp::PortalStopped => SavedAsyncOp::PortalStopped,
    }
}

pub(super) fn saved_sync_op_matches_runtime(saved: &SavedSyncOp, runtime: &SyncOp) -> bool {
    *saved == runtime_sync_op_to_saved(runtime)
}

pub(super) fn saved_async_op_matches_runtime(saved: &SavedAsyncOp, runtime: &AsyncOp) -> bool {
    *saved == runtime_async_op_to_saved(runtime)
}

pub(super) fn saved_sync_result_to_runtime(result: &SavedSyncResult) -> Result<SyncResult, String> {
    match result {
        SavedSyncResult::BootReason { value } => {
            Ok(SyncResult::BootReason(parse_boot_reason(value)?))
        }
        SavedSyncResult::MacAddress { value } => Ok(SyncResult::MacAddress(*value)),
        SavedSyncResult::Now { ticks } => Ok(SyncResult::Now(*ticks)),
        SavedSyncResult::StoreReadOk { values } => Ok(SyncResult::StoreRead(Ok(values.clone()))),
        SavedSyncResult::UnitOk => Ok(SyncResult::Unit(Ok(()))),
    }
}

pub(super) fn saved_sync_error_to_runtime(error: &SavedSyncError) -> Result<SyncResult, String> {
    match error {
        SavedSyncError::StoreReadErr { message } => Ok(SyncResult::StoreRead(Err(message.clone()))),
        SavedSyncError::UnitErr { message } => Ok(SyncResult::Unit(Err(message.clone()))),
    }
}

pub(super) fn saved_async_result_to_runtime(
    result: &SavedAsyncResult,
) -> Result<AsyncResult, String> {
    match result {
        SavedAsyncResult::SleepDone => Ok(AsyncResult::SleepDone),
        SavedAsyncResult::Unit => Ok(AsyncResult::Unit),
        SavedAsyncResult::PortalSignal => Ok(AsyncResult::PortalSignal),
        SavedAsyncResult::ScanNetworks { ssids } => Ok(AsyncResult::ScanNetworks(
            ssids
                .iter()
                .map(|ssid| wifi::FoundNetwork::new(ssid, Some(6), Some(-42)))
                .collect(),
        )),
        SavedAsyncResult::ConnectionInfo { ip } => {
            Ok(AsyncResult::ConnectionInfo(wifi::ConnectionInfo::new(ip)))
        }
        SavedAsyncResult::PortalStartAccessPoint {
            ip,
            gateway,
            netmask,
        } => Ok(AsyncResult::PortalStartAccessPoint(wifi::IpConfig::new(
            ip, gateway, netmask,
        ))),
        SavedAsyncResult::HttpFrame { bytes } => Ok(AsyncResult::HttpFrame(bytes.clone())),
        SavedAsyncResult::PortalHttpResponse {
            status_code,
            content_type,
            body_len,
        } => {
            let content_type: &'static str = Box::leak(content_type.clone().into_boxed_str());
            Ok(AsyncResult::PortalHttpResponse {
                status_code: *status_code,
                content_type,
                body_len: *body_len,
            })
        }
    }
}

pub(super) fn saved_async_op_to_runtime(op: &SavedAsyncOp) -> Result<AsyncOp, String> {
    match op {
        SavedAsyncOp::Sleep { duration_ms } => {
            Ok(AsyncOp::Sleep(EmbassyDuration::from_millis(*duration_ms)))
        }
        SavedAsyncOp::WifiDisconnect => Ok(AsyncOp::WifiDisconnect),
        SavedAsyncOp::WifiStop => Ok(AsyncOp::WifiStop),
        SavedAsyncOp::WifiStart => Ok(AsyncOp::WifiStart),
        SavedAsyncOp::WifiScanNetworks => Ok(AsyncOp::WifiScanNetworks),
        SavedAsyncOp::WifiConfigureClient {
            ssid,
            password,
            channel,
            auth,
        } => Ok(AsyncOp::WifiConfigureClient {
            ssid: ssid.clone(),
            password: password.clone(),
            channel: *channel,
            auth: parse_client_auth(auth)?,
        }),
        SavedAsyncOp::WifiConnect { timeout_ms } => Ok(AsyncOp::WifiConnect {
            timeout: Duration::from_millis(*timeout_ms),
        }),
        SavedAsyncOp::PortalStartAccessPoint { ssid } => {
            Ok(AsyncOp::PortalStartAccessPoint { ssid: ssid.clone() })
        }
        SavedAsyncOp::PortalStopAccessPoint => Ok(AsyncOp::PortalStopAccessPoint),
        SavedAsyncOp::HttpGet { url } => Ok(AsyncOp::HttpGet { url: url.clone() }),
        SavedAsyncOp::PortalHttpRequest { method, path, body } => Ok(AsyncOp::PortalHttpRequest {
            method: method.clone(),
            path: path.clone(),
            body: body.clone(),
        }),
        SavedAsyncOp::PortalClientConnected => Ok(AsyncOp::PortalClientConnected),
        SavedAsyncOp::PortalStopped => Ok(AsyncOp::PortalStopped),
    }
}

pub(super) fn format_client_auth(auth: wifi::ClientAuth) -> String {
    match auth {
        wifi::ClientAuth::Open => "open".to_string(),
        wifi::ClientAuth::Wpa2Personal => "wpa2_personal".to_string(),
    }
}

pub(super) fn parse_client_auth(value: &str) -> Result<wifi::ClientAuth, String> {
    match value {
        "open" => Ok(wifi::ClientAuth::Open),
        "wpa2_personal" => Ok(wifi::ClientAuth::Wpa2Personal),
        other => Err(format!("unknown wifi auth {other}")),
    }
}

pub(super) fn parse_boot_reason(value: &str) -> Result<BootReason, String> {
    match value {
        "software" => Ok(BootReason::Software),
        "external_pin" => Ok(BootReason::ExternalPin),
        "watchdog" => Ok(BootReason::Watchdog),
        "sdio" => Ok(BootReason::Sdio),
        "panic" => Ok(BootReason::Panic),
        "interrupt_watchdog" => Ok(BootReason::InterruptWatchdog),
        "power_on" => Ok(BootReason::PowerOn),
        "unknown" => Ok(BootReason::Unknown),
        "brownout" => Ok(BootReason::Brownout),
        "task_watchdog" => Ok(BootReason::TaskWatchdog),
        "deep_sleep" => Ok(BootReason::DeepSleep),
        "usb_peripheral" => Ok(BootReason::USBPeripheral),
        "jtag" => Ok(BootReason::JTAG),
        "efuse_error" => Ok(BootReason::EfuseError),
        "power_glitch" => Ok(BootReason::PowerGlitch),
        "cpu_lockup" => Ok(BootReason::CPULockup),
        other => Err(format!("unknown boot reason {other}")),
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

pub(super) fn default_sync_result(op: &SyncOp, current_ticks: u64) -> SavedSyncResult {
    match op {
        SyncOp::BootReason => SavedSyncResult::BootReason {
            value: "software".to_string(),
        },
        SyncOp::MacAddress => SavedSyncResult::MacAddress {
            value: [0x02, 0x00, 0x00, 0x00, 0x12, 0x34],
        },
        SyncOp::Now => SavedSyncResult::Now {
            ticks: current_ticks,
        },
        SyncOp::TftSetDcLow
        | SyncOp::TftSetDcHigh
        | SyncOp::TftSetRstLow
        | SyncOp::TftSetRstHigh
        | SyncOp::TftWrite { .. }
        | SyncOp::StoreWrite { .. }
        | SyncOp::StoreRemove { .. } => SavedSyncResult::UnitOk,
        SyncOp::StoreRead { keys, .. } => SavedSyncResult::StoreReadOk {
            values: default_store_values(keys),
        },
    }
}

pub(super) fn default_sync_error(op: &SyncOp) -> Option<SavedSyncError> {
    match op {
        SyncOp::StoreRead { .. } => Some(SavedSyncError::StoreReadErr {
            message: "simulated error".to_string(),
        }),
        SyncOp::TftSetDcLow
        | SyncOp::TftSetDcHigh
        | SyncOp::TftSetRstLow
        | SyncOp::TftSetRstHigh
        | SyncOp::TftWrite { .. }
        | SyncOp::StoreWrite { .. }
        | SyncOp::StoreRemove { .. } => Some(SavedSyncError::UnitErr {
            message: "simulated error".to_string(),
        }),
        SyncOp::BootReason | SyncOp::MacAddress | SyncOp::Now => None,
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

pub(super) fn default_async_result(op: &AsyncOp) -> SavedAsyncResult {
    match op {
        AsyncOp::Sleep(_) => SavedAsyncResult::SleepDone,
        AsyncOp::WifiDisconnect
        | AsyncOp::WifiStop
        | AsyncOp::WifiStart
        | AsyncOp::WifiConfigureClient { .. }
        | AsyncOp::PortalStopAccessPoint => SavedAsyncResult::Unit,
        AsyncOp::WifiScanNetworks => SavedAsyncResult::ScanNetworks {
            ssids: vec!["test_ssid".to_string()],
        },
        AsyncOp::WifiConnect { .. } => SavedAsyncResult::ConnectionInfo {
            ip: "192.168.1.23".to_string(),
        },
        AsyncOp::PortalStartAccessPoint { .. } => SavedAsyncResult::PortalStartAccessPoint {
            ip: "192.168.4.1".to_string(),
            gateway: "192.168.4.1".to_string(),
            netmask: "255.255.255.0".to_string(),
        },
        AsyncOp::HttpGet { .. } => SavedAsyncResult::HttpFrame {
            bytes: vec![0u8; crate::TFT_WIDTH as usize * crate::TFT_HEIGHT as usize * 2],
        },
        AsyncOp::PortalHttpRequest { .. } => SavedAsyncResult::PortalHttpResponse {
            status_code: 200,
            content_type: "text/html".to_string(),
            body_len: 0,
        },
        AsyncOp::PortalClientConnected | AsyncOp::PortalStopped => SavedAsyncResult::PortalSignal,
    }
}

pub(super) fn default_inbound_async_op(kind: &InboundAsyncKind) -> SavedAsyncOp {
    match kind {
        InboundAsyncKind::PortalHttpRequest => SavedAsyncOp::PortalHttpRequest {
            method: "GET".to_string(),
            path: "/".to_string(),
            body: Vec::new(),
        },
        InboundAsyncKind::PortalClientConnected => SavedAsyncOp::PortalClientConnected,
        InboundAsyncKind::PortalStopped => SavedAsyncOp::PortalStopped,
    }
}

pub(super) fn uniquify_id(used_ids: &BTreeSet<String>, base: &str) -> String {
    if !used_ids.contains(base) {
        return base.to_string();
    }

    let mut index = 2;
    loop {
        let candidate = format!("{base}_{index}");
        if !used_ids.contains(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

pub(super) fn sync_op_name(op: &SyncOp) -> &'static str {
    match op {
        SyncOp::BootReason => "boot_reason",
        SyncOp::MacAddress => "mac_address",
        SyncOp::Now => "now",
        SyncOp::TftSetDcLow => "tft_set_dc_low",
        SyncOp::TftSetDcHigh => "tft_set_dc_high",
        SyncOp::TftSetRstLow => "tft_set_rst_low",
        SyncOp::TftSetRstHigh => "tft_set_rst_high",
        SyncOp::TftWrite { .. } => "tft_write",
        SyncOp::StoreRead { .. } => "store_read",
        SyncOp::StoreWrite { .. } => "store_write",
        SyncOp::StoreRemove { .. } => "store_remove",
    }
}

pub(super) fn async_op_name(op: &AsyncOp) -> String {
    match op {
        AsyncOp::Sleep(duration) => {
            format!(
                "sleep_{}",
                format_embassy_duration(*duration)
                    .replace(['(', ')'], "")
                    .replace('/', "_")
            )
        }
        AsyncOp::WifiDisconnect => "wifi_disconnect".to_string(),
        AsyncOp::WifiStop => "wifi_stop".to_string(),
        AsyncOp::WifiStart => "wifi_start".to_string(),
        AsyncOp::WifiScanNetworks => "wifi_scan_networks".to_string(),
        AsyncOp::WifiConfigureClient { .. } => "wifi_configure_client".to_string(),
        AsyncOp::WifiConnect { .. } => "wifi_connect".to_string(),
        AsyncOp::PortalStartAccessPoint { .. } => "portal_start_access_point".to_string(),
        AsyncOp::PortalStopAccessPoint => "portal_stop_access_point".to_string(),
        AsyncOp::HttpGet { .. } => "http_get".to_string(),
        AsyncOp::PortalHttpRequest { .. } => "portal_http_request".to_string(),
        AsyncOp::PortalClientConnected => "portal_client_connected".to_string(),
        AsyncOp::PortalStopped => "portal_stopped".to_string(),
    }
}

pub(super) fn inbound_async_kind_name(kind: &InboundAsyncKind) -> &'static str {
    match kind {
        InboundAsyncKind::PortalHttpRequest => "portal_http_request",
        InboundAsyncKind::PortalClientConnected => "portal_client_connected",
        InboundAsyncKind::PortalStopped => "portal_stopped",
    }
}

pub(super) fn allows_event(
    possible: &[PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>],
    event: &Event<SyncOp, AsyncOp, SyncResult, AsyncResult>,
) -> bool {
    possible.iter().any(|candidate| match (candidate, event) {
        (
            PossibleEvent::ReturnSync { id, op },
            Event::ReturnSync {
                id: actual_id,
                result,
            },
        ) => *id == *actual_id && InfoPanelSpec::sync_result_matches(op, result),
        (
            PossibleEvent::ResolveAsync { id, op, .. },
            Event::ResolveAsync {
                id: actual_id,
                result,
            },
        ) => *id == *actual_id && InfoPanelSpec::async_result_matches(op, result),
        (PossibleEvent::CreateInboundAsync { kind }, Event::CreateAsync { op, .. }) => matches!(
            (kind, op),
            (
                InboundAsyncKind::PortalHttpRequest,
                AsyncOp::PortalHttpRequest { .. }
            ) | (
                InboundAsyncKind::PortalClientConnected,
                AsyncOp::PortalClientConnected
            ) | (InboundAsyncKind::PortalStopped, AsyncOp::PortalStopped)
        ),
        (PossibleEvent::AbortAsync { id, .. }, Event::AbortAsync { id: actual_id }) => {
            *id == *actual_id
        }
        (PossibleEvent::CancelInboundAsync { id, .. }, Event::CancelAsync { id: actual_id }) => {
            *id == *actual_id
        }
        _ => false,
    })
}

#[derive(Clone)]
pub(super) struct PendingRequest {
    id: u64,
    op: PendingRequestOp,
}

#[derive(Clone)]
pub(super) enum PendingRequestOp {
    Sync(SyncOp),
    Async(AsyncOp),
}

pub(super) struct ReplaySnapshot {
    pub(super) possible: Vec<PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>>,
    pub(super) runtime_to_symbolic: BTreeMap<u64, String>,
    pub(super) used_ids: BTreeSet<String>,
    pub(super) current_ticks: u64,
}

pub(super) fn add_pending_requests(
    target: &mut Vec<PendingRequest>,
    outbound: &[Event<SyncOp, AsyncOp, SyncResult, AsyncResult>],
) {
    target.extend(outbound.iter().filter_map(|event| match event {
        Event::CreateSync { id, op } => Some(PendingRequest {
            id: *id,
            op: PendingRequestOp::Sync(op.clone()),
        }),
        Event::CreateAsync { id, op } => Some(PendingRequest {
            id: *id,
            op: PendingRequestOp::Async(op.clone()),
        }),
        _ => None,
    }));
}

pub(super) fn bind_outbound(
    pending_requests: &mut Vec<PendingRequest>,
    item: &SavedItem,
    symbolic_to_runtime: &mut BTreeMap<String, u64>,
    runtime_to_symbolic: &mut BTreeMap<u64, String>,
    used_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    match item {
        SavedItem::OutboundCreateSync { id, op } => {
            if !used_ids.insert(id.clone()) {
                return Err(format!("duplicate symbolic id {id}"));
            }
            let Some(index) = pending_requests
                .iter()
                .position(|pending| match &pending.op {
                    PendingRequestOp::Sync(runtime_op) => {
                        saved_sync_op_matches_runtime(op, runtime_op)
                    }
                    PendingRequestOp::Async(_) => false,
                })
            else {
                return Err(format!("could not match outbound create_sync for {id}"));
            };
            let pending = pending_requests.remove(index);
            symbolic_to_runtime.insert(id.clone(), pending.id);
            runtime_to_symbolic.insert(pending.id, id.clone());
            Ok(())
        }
        SavedItem::OutboundCreateAsync { id, op } => {
            if !used_ids.insert(id.clone()) {
                return Err(format!("duplicate symbolic id {id}"));
            }
            let Some(index) = pending_requests
                .iter()
                .position(|pending| match &pending.op {
                    PendingRequestOp::Async(runtime_op) => {
                        saved_async_op_matches_runtime(op, runtime_op)
                    }
                    PendingRequestOp::Sync(_) => false,
                })
            else {
                return Err(format!("could not match outbound create_async for {id}"));
            };
            let pending = pending_requests.remove(index);
            symbolic_to_runtime.insert(id.clone(), pending.id);
            runtime_to_symbolic.insert(pending.id, id.clone());
            Ok(())
        }
        _ => Err("expected outbound item".to_string()),
    }
}

pub(super) fn build_inbound_event(
    item: &SavedItem,
    symbolic_to_runtime: &mut BTreeMap<String, u64>,
    runtime_to_symbolic: &mut BTreeMap<u64, String>,
    next_inbound_runtime_id: &mut u64,
) -> Result<Event<SyncOp, AsyncOp, SyncResult, AsyncResult>, String> {
    match item {
        SavedItem::InboundReturnSync { target, result } => {
            let id = symbolic_to_runtime
                .remove(target)
                .ok_or_else(|| format!("unknown symbolic target {target}"))?;
            runtime_to_symbolic.remove(&id);
            Ok(Event::ReturnSync {
                id,
                result: saved_sync_result_to_runtime(result)?,
            })
        }
        SavedItem::InboundErrorSync { target, error } => {
            let id = symbolic_to_runtime
                .remove(target)
                .ok_or_else(|| format!("unknown symbolic target {target}"))?;
            runtime_to_symbolic.remove(&id);
            Ok(Event::ReturnSync {
                id,
                result: saved_sync_error_to_runtime(error)?,
            })
        }
        SavedItem::InboundResolveAsync { target, result } => {
            let id = symbolic_to_runtime
                .remove(target)
                .ok_or_else(|| format!("unknown symbolic target {target}"))?;
            runtime_to_symbolic.remove(&id);
            Ok(Event::ResolveAsync {
                id,
                result: saved_async_result_to_runtime(result)?,
            })
        }
        SavedItem::InboundAbortAsync { target } => {
            let id = symbolic_to_runtime
                .remove(target)
                .ok_or_else(|| format!("unknown symbolic target {target}"))?;
            runtime_to_symbolic.remove(&id);
            Ok(Event::AbortAsync { id })
        }
        SavedItem::InboundCancelAsync { target } => {
            let id = symbolic_to_runtime
                .remove(target)
                .ok_or_else(|| format!("unknown symbolic target {target}"))?;
            runtime_to_symbolic.remove(&id);
            Ok(Event::CancelAsync { id })
        }
        SavedItem::InboundCreateAsync { id, op } => {
            let runtime_id = *next_inbound_runtime_id;
            *next_inbound_runtime_id += 1;
            symbolic_to_runtime.insert(id.clone(), runtime_id);
            runtime_to_symbolic.insert(runtime_id, id.clone());
            Ok(Event::CreateAsync {
                id: runtime_id,
                op: saved_async_op_to_runtime(op)?,
            })
        }
        SavedItem::OutboundCreateSync { .. } | SavedItem::OutboundCreateAsync { .. } => {
            Err("expected inbound item".to_string())
        }
    }
}

pub(super) fn replay_items(items: &[SavedItem]) -> Result<ReplaySnapshot, String> {
    let rebooted = Arc::new(AtomicBool::new(false));
    let (mut wrapper, initial_outbound) =
        NewRunWrapper::new(InfoPanelBundle::new(rebooted.clone())).start();
    let mut trace = vec![TraceStep::start(initial_outbound.clone())];
    let mut pending_requests = Vec::new();
    add_pending_requests(&mut pending_requests, &initial_outbound);
    let mut symbolic_to_runtime = BTreeMap::new();
    let mut runtime_to_symbolic = BTreeMap::new();
    let mut used_ids = BTreeSet::new();
    let mut next_inbound_runtime_id = 1_000_000;
    let mut possible = possible_next_events::<_, _, _, _, InfoPanelSpec>(&trace)
        .map_err(|err| format!("failed to compute possible events: {err:?}"))?;

    for (index, item) in items.iter().enumerate() {
        let result = match item {
            SavedItem::OutboundCreateSync { .. } | SavedItem::OutboundCreateAsync { .. } => {
                bind_outbound(
                    &mut pending_requests,
                    item,
                    &mut symbolic_to_runtime,
                    &mut runtime_to_symbolic,
                    &mut used_ids,
                )
            }
            SavedItem::InboundReturnSync { .. }
            | SavedItem::InboundErrorSync { .. }
            | SavedItem::InboundResolveAsync { .. }
            | SavedItem::InboundAbortAsync { .. }
            | SavedItem::InboundCancelAsync { .. }
            | SavedItem::InboundCreateAsync { .. } => {
                match build_inbound_event(
                    item,
                    &mut symbolic_to_runtime,
                    &mut runtime_to_symbolic,
                    &mut next_inbound_runtime_id,
                ) {
                    Ok(inbound) => {
                        if !allows_event(&possible, &inbound) {
                            Err(format!(
                                "saved inbound item is not valid at index {index}: {}",
                                format_saved_item(item)
                            ))
                        } else {
                            let outbound = wrapper.push(inbound.clone());
                            trace.push(TraceStep::push(inbound, outbound.clone()));
                            add_pending_requests(&mut pending_requests, &outbound);
                            let _ = wrapper.is_terminated()
                                && rebooted.load(std::sync::atomic::Ordering::SeqCst);
                            possible = possible_next_events::<_, _, _, _, InfoPanelSpec>(&trace)
                                .map_err(|err| {
                                    format!("failed to replay trace at index {index}: {err:?}")
                                })?;
                            Ok(())
                        }
                    }
                    Err(err) => Err(err),
                }
            }
        };

        if let Err(err) = result {
            return Err(err);
        }
    }

    let current_ticks = current_ticks_from_trace(&trace);
    Ok(ReplaySnapshot {
        possible,
        runtime_to_symbolic,
        used_ids,
        current_ticks,
    })
}

pub(super) fn choice_to_saved_items(
    used_ids: &BTreeSet<String>,
    runtime_to_symbolic: &BTreeMap<u64, String>,
    current_ticks: u64,
    choice: &PossibleEvent<SyncOp, AsyncOp, InboundAsyncKind>,
) -> Result<Vec<SavedItem>, String> {
    match choice {
        PossibleEvent::ReturnSync { id, op } => {
            let target = runtime_to_symbolic
                .get(id)
                .cloned()
                .unwrap_or_else(|| uniquify_id(used_ids, sync_op_name(op)));
            let mut items = Vec::new();
            if !runtime_to_symbolic.contains_key(id) {
                items.push(SavedItem::OutboundCreateSync {
                    id: target.clone(),
                    op: runtime_sync_op_to_saved(op),
                });
            }
            items.push(SavedItem::InboundReturnSync {
                target,
                result: default_sync_result(op, current_ticks),
            });
            Ok(items)
        }
        PossibleEvent::ResolveAsync { id, op, .. } => {
            let target = runtime_to_symbolic
                .get(id)
                .cloned()
                .unwrap_or_else(|| uniquify_id(used_ids, &async_op_name(op)));
            let mut items = Vec::new();
            if !runtime_to_symbolic.contains_key(id) {
                items.push(SavedItem::OutboundCreateAsync {
                    id: target.clone(),
                    op: runtime_async_op_to_saved(op),
                });
            }
            items.push(SavedItem::InboundResolveAsync {
                target,
                result: default_async_result(op),
            });
            Ok(items)
        }
        PossibleEvent::AbortAsync { id, op } => {
            let target = runtime_to_symbolic
                .get(id)
                .cloned()
                .unwrap_or_else(|| uniquify_id(used_ids, &async_op_name(op)));
            let mut items = Vec::new();
            if !runtime_to_symbolic.contains_key(id) {
                items.push(SavedItem::OutboundCreateAsync {
                    id: target.clone(),
                    op: runtime_async_op_to_saved(op),
                });
            }
            items.push(SavedItem::InboundAbortAsync { target });
            Ok(items)
        }
        PossibleEvent::CreateInboundAsync { kind } => Ok(vec![SavedItem::InboundCreateAsync {
            id: uniquify_id(used_ids, inbound_async_kind_name(kind)),
            op: default_inbound_async_op(kind),
        }]),
        PossibleEvent::CancelInboundAsync { id, .. } => {
            let Some(target) = runtime_to_symbolic.get(id).cloned() else {
                return Err(format!("missing symbolic id for inbound async {id}"));
            };
            Ok(vec![SavedItem::InboundCancelAsync { target }])
        }
    }
}

pub(super) fn inbound_target(item: &SavedItem) -> Option<&str> {
    match item {
        SavedItem::InboundReturnSync { target, .. }
        | SavedItem::InboundErrorSync { target, .. }
        | SavedItem::InboundResolveAsync { target, .. }
        | SavedItem::InboundAbortAsync { target }
        | SavedItem::InboundCancelAsync { target } => Some(target.as_str()),
        SavedItem::InboundCreateAsync { .. }
        | SavedItem::OutboundCreateSync { .. }
        | SavedItem::OutboundCreateAsync { .. } => None,
    }
}

pub(super) fn outbound_id(item: &SavedItem) -> Option<&str> {
    match item {
        SavedItem::OutboundCreateSync { id, .. } | SavedItem::OutboundCreateAsync { id, .. } => {
            Some(id.as_str())
        }
        _ => None,
    }
}

pub(super) fn removal_span(
    items: &[SavedItem],
    item_index: usize,
) -> Result<(usize, usize), String> {
    let Some(item) = items.get(item_index) else {
        return Err(format!("invalid item index {item_index}"));
    };
    match item {
        SavedItem::InboundReturnSync { .. }
        | SavedItem::InboundErrorSync { .. }
        | SavedItem::InboundResolveAsync { .. }
        | SavedItem::InboundAbortAsync { .. }
        | SavedItem::InboundCancelAsync { .. } => {
            if item_index > 0 {
                if let (Some(target), Some(previous_id)) =
                    (inbound_target(item), outbound_id(&items[item_index - 1]))
                {
                    if target == previous_id {
                        return Ok((item_index - 1, item_index + 1));
                    }
                }
            }
            Ok((item_index, item_index + 1))
        }
        SavedItem::InboundCreateAsync { .. } => Ok((item_index, item_index + 1)),
        SavedItem::OutboundCreateSync { .. } | SavedItem::OutboundCreateAsync { .. } => {
            Ok((item_index, item_index + 1))
        }
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
