use super::types::{AsyncOp, AsyncResult, SyncError, SyncOp, SyncResult};
use super::*;

pub type SavedItem = simulator::editor::TraceItem<
    SavedSyncOp,
    SavedAsyncOp,
    SavedSyncResult,
    SavedSyncError,
    SavedAsyncResult,
>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedClientAuth {
    Open,
    Wpa2Personal,
}

impl SavedClientAuth {
    pub fn from_runtime(value: wifi::ClientAuth) -> Self {
        match value {
            wifi::ClientAuth::Open => Self::Open,
            wifi::ClientAuth::Wpa2Personal => Self::Wpa2Personal,
        }
    }

    pub fn to_runtime(&self) -> wifi::ClientAuth {
        match self {
            Self::Open => wifi::ClientAuth::Open,
            Self::Wpa2Personal => wifi::ClientAuth::Wpa2Personal,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedEmbassyDuration {
    pub ticks: u64,
}

impl SavedEmbassyDuration {
    pub fn from_runtime(value: EmbassyDuration) -> Self {
        Self {
            ticks: value.as_ticks(),
        }
    }

    pub fn to_runtime(&self) -> EmbassyDuration {
        EmbassyDuration::from_ticks(self.ticks)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedStdDuration {
    pub secs: u64,
    pub nanos: u32,
}

impl SavedStdDuration {
    pub fn from_runtime(value: Duration) -> Self {
        Self {
            secs: value.as_secs(),
            nanos: value.subsec_nanos(),
        }
    }

    pub fn to_runtime(&self) -> Duration {
        Duration::new(self.secs, self.nanos)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFoundNetwork {
    pub ssid: String,
    pub channel: Option<u8>,
    pub signal_strength: Option<i8>,
}

impl SavedFoundNetwork {
    pub fn from_runtime(value: &wifi::FoundNetwork) -> Self {
        Self {
            ssid: value.ssid.clone(),
            channel: value.channel,
            signal_strength: value.signal_strength,
        }
    }

    pub fn to_runtime(&self) -> wifi::FoundNetwork {
        wifi::FoundNetwork::new(self.ssid.clone(), self.channel, self.signal_strength)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedConnectionInfo {
    pub ip: String,
}

impl SavedConnectionInfo {
    pub fn from_runtime(value: &wifi::ConnectionInfo) -> Self {
        Self {
            ip: value.ip.clone(),
        }
    }

    pub fn to_runtime(&self) -> wifi::ConnectionInfo {
        wifi::ConnectionInfo::new(self.ip.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedIpConfig {
    pub ip: String,
    pub gateway: String,
    pub netmask: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedRef {
    #[serde(rename = "ref")]
    pub id: String,
}

impl SavedIpConfig {
    pub fn from_runtime(value: &wifi::IpConfig) -> Self {
        Self {
            ip: value.ip.clone(),
            gateway: value.gateway.clone(),
            netmask: value.netmask.clone(),
        }
    }

    pub fn to_runtime(&self) -> wifi::IpConfig {
        wifi::IpConfig::new(self.ip.clone(), self.gateway.clone(), self.netmask.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedSyncOp {
    BootReason,
    MacAddress,
    Now,
    TftSetDcLow,
    TftSetDcHigh,
    TftSetRstLow,
    TftSetRstHigh,
    TftWrite {
        bytes: Vec<u8>,
    },
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
    HttpRead {
        body: String,
        max_len: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawSavedSyncOp {
    BootReason,
    MacAddress,
    Now,
    TftSetDcLow,
    TftSetDcHigh,
    TftSetRstLow,
    TftSetRstHigh,
    TftWrite {
        bytes_hex: String,
    },
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
    HttpRead {
        body: String,
        max_len: usize,
    },
}

impl SavedSyncOp {
    pub fn from_runtime(value: &SyncOp) -> Self {
        match value {
            SyncOp::BootReason => Self::BootReason,
            SyncOp::MacAddress => Self::MacAddress,
            SyncOp::Now => Self::Now,
            SyncOp::TftSetDcLow => Self::TftSetDcLow,
            SyncOp::TftSetDcHigh => Self::TftSetDcHigh,
            SyncOp::TftSetRstLow => Self::TftSetRstLow,
            SyncOp::TftSetRstHigh => Self::TftSetRstHigh,
            SyncOp::TftWrite { bytes } => Self::TftWrite {
                bytes: bytes.clone(),
            },
            SyncOp::StoreRead { namespace, keys } => Self::StoreRead {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            SyncOp::StoreWrite { namespace, values } => Self::StoreWrite {
                namespace: namespace.clone(),
                values: values.clone(),
            },
            SyncOp::StoreRemove { namespace, keys } => Self::StoreRemove {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            SyncOp::HttpRead { body, max_len } => Self::HttpRead {
                body: body.clone(),
                max_len: *max_len,
            },
        }
    }

    #[cfg(test)]
    pub fn to_runtime(&self) -> SyncOp {
        match self {
            Self::BootReason => SyncOp::BootReason,
            Self::MacAddress => SyncOp::MacAddress,
            Self::Now => SyncOp::Now,
            Self::TftSetDcLow => SyncOp::TftSetDcLow,
            Self::TftSetDcHigh => SyncOp::TftSetDcHigh,
            Self::TftSetRstLow => SyncOp::TftSetRstLow,
            Self::TftSetRstHigh => SyncOp::TftSetRstHigh,
            Self::TftWrite { bytes } => SyncOp::TftWrite {
                bytes: bytes.clone(),
            },
            Self::StoreRead { namespace, keys } => SyncOp::StoreRead {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            Self::StoreWrite { namespace, values } => SyncOp::StoreWrite {
                namespace: namespace.clone(),
                values: values.clone(),
            },
            Self::StoreRemove { namespace, keys } => SyncOp::StoreRemove {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            Self::HttpRead { body, max_len } => SyncOp::HttpRead {
                body: body.clone(),
                max_len: *max_len,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedSyncResult {
    BootReason { value: String },
    MacAddress { value: [u8; 6] },
    Now { ticks: u64 },
    StoreReadOk { values: BTreeMap<String, String> },
    UnitOk,
    HttpRead { bytes: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawSavedSyncResult {
    BootReason { value: String },
    MacAddress { value: [u8; 6] },
    Now { ticks: u64 },
    StoreReadOk { values: BTreeMap<String, String> },
    UnitOk,
    HttpRead { bytes_hex: String },
}

impl SavedSyncResult {
    pub fn from_runtime(value: &SyncResult) -> Result<Self, String> {
        match value {
            SyncResult::BootReason(reason) => Ok(Self::BootReason {
                value: format_boot_reason(*reason),
            }),
            SyncResult::MacAddress(value) => Ok(Self::MacAddress { value: *value }),
            SyncResult::Now(ticks) => Ok(Self::Now { ticks: *ticks }),
            SyncResult::StoreRead(Ok(values)) => Ok(Self::StoreReadOk {
                values: values.clone(),
            }),
            SyncResult::Unit(Ok(())) => Ok(Self::UnitOk),
            SyncResult::HttpRead { bytes } => Ok(Self::HttpRead {
                bytes: bytes.clone(),
            }),
            SyncResult::StoreRead(Err(_)) | SyncResult::Unit(Err(_)) => {
                Err("sync error result must use error channel".to_string())
            }
        }
    }

    pub fn to_runtime(&self) -> Result<SyncResult, String> {
        match self {
            Self::BootReason { value } => Ok(SyncResult::BootReason(parse_boot_reason(value)?)),
            Self::MacAddress { value } => Ok(SyncResult::MacAddress(*value)),
            Self::Now { ticks } => Ok(SyncResult::Now(*ticks)),
            Self::StoreReadOk { values } => Ok(SyncResult::StoreRead(Ok(values.clone()))),
            Self::UnitOk => Ok(SyncResult::Unit(Ok(()))),
            Self::HttpRead { bytes } => Ok(SyncResult::HttpRead {
                bytes: bytes.clone(),
            }),
        }
    }
}

impl Serialize for SavedSyncResult {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = match self {
            SavedSyncResult::BootReason { value } => RawSavedSyncResult::BootReason {
                value: value.clone(),
            },
            SavedSyncResult::MacAddress { value } => {
                RawSavedSyncResult::MacAddress { value: *value }
            }
            SavedSyncResult::Now { ticks } => RawSavedSyncResult::Now { ticks: *ticks },
            SavedSyncResult::StoreReadOk { values } => RawSavedSyncResult::StoreReadOk {
                values: values.clone(),
            },
            SavedSyncResult::UnitOk => RawSavedSyncResult::UnitOk,
            SavedSyncResult::HttpRead { bytes } => RawSavedSyncResult::HttpRead {
                bytes_hex: encode_hex(bytes),
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SavedSyncResult {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSavedSyncResult::deserialize(deserializer)?;
        Ok(match raw {
            RawSavedSyncResult::BootReason { value } => SavedSyncResult::BootReason { value },
            RawSavedSyncResult::MacAddress { value } => SavedSyncResult::MacAddress { value },
            RawSavedSyncResult::Now { ticks } => SavedSyncResult::Now { ticks },
            RawSavedSyncResult::StoreReadOk { values } => SavedSyncResult::StoreReadOk { values },
            RawSavedSyncResult::UnitOk => SavedSyncResult::UnitOk,
            RawSavedSyncResult::HttpRead { bytes_hex } => SavedSyncResult::HttpRead {
                bytes: decode_hex(&bytes_hex).map_err(serde::de::Error::custom)?,
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedSyncError {
    StoreReadErr { message: String },
    UnitErr { message: String },
}

impl SavedSyncError {
    pub fn to_runtime_error(&self) -> SyncError {
        match self {
            Self::StoreReadErr { message } => SyncError::StoreReadErr {
                message: message.clone(),
            },
            Self::UnitErr { message } => SyncError::UnitErr {
                message: message.clone(),
            },
        }
    }

    pub fn from_runtime_error(value: &SyncError) -> Self {
        match value {
            SyncError::StoreReadErr { message } => Self::StoreReadErr {
                message: message.clone(),
            },
            SyncError::UnitErr { message } => Self::UnitErr {
                message: message.clone(),
            },
        }
    }

    pub fn to_runtime_result(&self) -> SyncResult {
        match self {
            Self::StoreReadErr { message } => SyncResult::StoreRead(Err(message.clone())),
            Self::UnitErr { message } => SyncResult::Unit(Err(message.clone())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedAsyncResult {
    SleepDone,
    Unit,
    PortalSignal,
    ScanNetworks {
        networks: Vec<SavedFoundNetwork>,
    },
    ConnectionInfo {
        info: SavedConnectionInfo,
    },
    PortalStartAccessPoint {
        config: SavedIpConfig,
    },
    HttpResponse {
        body: SavedRef,
    },
    PortalHttpResponse {
        status_code: u16,
        content_type: String,
        body_len: usize,
    },
}

impl SavedAsyncResult {
    pub fn from_runtime(value: &AsyncResult) -> Self {
        match value {
            AsyncResult::SleepDone => Self::SleepDone,
            AsyncResult::Unit => Self::Unit,
            AsyncResult::PortalSignal => Self::PortalSignal,
            AsyncResult::ScanNetworks(networks) => Self::ScanNetworks {
                networks: networks
                    .iter()
                    .map(SavedFoundNetwork::from_runtime)
                    .collect(),
            },
            AsyncResult::ConnectionInfo(info) => Self::ConnectionInfo {
                info: SavedConnectionInfo::from_runtime(info),
            },
            AsyncResult::PortalStartAccessPoint(config) => Self::PortalStartAccessPoint {
                config: SavedIpConfig::from_runtime(config),
            },
            AsyncResult::HttpResponse { body } => Self::HttpResponse {
                body: SavedRef { id: body.clone() },
            },
            AsyncResult::PortalHttpResponse {
                status_code,
                content_type,
                body_len,
            } => Self::PortalHttpResponse {
                status_code: *status_code,
                content_type: (*content_type).to_string(),
                body_len: *body_len,
            },
        }
    }

    pub fn to_runtime(&self) -> AsyncResult {
        match self {
            Self::SleepDone => AsyncResult::SleepDone,
            Self::Unit => AsyncResult::Unit,
            Self::PortalSignal => AsyncResult::PortalSignal,
            Self::ScanNetworks { networks } => AsyncResult::ScanNetworks(
                networks.iter().map(SavedFoundNetwork::to_runtime).collect(),
            ),
            Self::ConnectionInfo { info } => AsyncResult::ConnectionInfo(info.to_runtime()),
            Self::PortalStartAccessPoint { config } => {
                AsyncResult::PortalStartAccessPoint(config.to_runtime())
            }
            Self::HttpResponse { body } => AsyncResult::HttpResponse {
                body: body.id.clone(),
            },
            Self::PortalHttpResponse {
                status_code,
                content_type,
                body_len,
            } => AsyncResult::PortalHttpResponse {
                status_code: *status_code,
                content_type: Box::leak(content_type.clone().into_boxed_str()),
                body_len: *body_len,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedAsyncOp {
    Sleep {
        duration: SavedEmbassyDuration,
    },
    WifiDisconnect,
    WifiStop,
    WifiStart,
    WifiScanNetworks,
    WifiConfigureClient {
        ssid: String,
        password: String,
        channel: Option<u8>,
        auth: SavedClientAuth,
    },
    WifiConnect {
        timeout: SavedStdDuration,
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

impl SavedAsyncOp {
    pub fn from_runtime(value: &AsyncOp) -> Self {
        match value {
            AsyncOp::Sleep(duration) => Self::Sleep {
                duration: SavedEmbassyDuration::from_runtime(*duration),
            },
            AsyncOp::WifiDisconnect => Self::WifiDisconnect,
            AsyncOp::WifiStop => Self::WifiStop,
            AsyncOp::WifiStart => Self::WifiStart,
            AsyncOp::WifiScanNetworks => Self::WifiScanNetworks,
            AsyncOp::WifiConfigureClient {
                ssid,
                password,
                channel,
                auth,
            } => Self::WifiConfigureClient {
                ssid: ssid.clone(),
                password: password.clone(),
                channel: *channel,
                auth: SavedClientAuth::from_runtime(*auth),
            },
            AsyncOp::WifiConnect { timeout } => Self::WifiConnect {
                timeout: SavedStdDuration::from_runtime(*timeout),
            },
            AsyncOp::PortalStartAccessPoint { ssid } => {
                Self::PortalStartAccessPoint { ssid: ssid.clone() }
            }
            AsyncOp::PortalStopAccessPoint => Self::PortalStopAccessPoint,
            AsyncOp::HttpGet { url } => Self::HttpGet { url: url.clone() },
            AsyncOp::PortalHttpRequest { method, path, body } => Self::PortalHttpRequest {
                method: method.clone(),
                path: path.clone(),
                body: body.clone(),
            },
            AsyncOp::PortalClientConnected => Self::PortalClientConnected,
            AsyncOp::PortalStopped => Self::PortalStopped,
        }
    }

    pub fn to_runtime(&self) -> AsyncOp {
        match self {
            Self::Sleep { duration } => AsyncOp::Sleep(duration.to_runtime()),
            Self::WifiDisconnect => AsyncOp::WifiDisconnect,
            Self::WifiStop => AsyncOp::WifiStop,
            Self::WifiStart => AsyncOp::WifiStart,
            Self::WifiScanNetworks => AsyncOp::WifiScanNetworks,
            Self::WifiConfigureClient {
                ssid,
                password,
                channel,
                auth,
            } => AsyncOp::WifiConfigureClient {
                ssid: ssid.clone(),
                password: password.clone(),
                channel: *channel,
                auth: auth.to_runtime(),
            },
            Self::WifiConnect { timeout } => AsyncOp::WifiConnect {
                timeout: timeout.to_runtime(),
            },
            Self::PortalStartAccessPoint { ssid } => {
                AsyncOp::PortalStartAccessPoint { ssid: ssid.clone() }
            }
            Self::PortalStopAccessPoint => AsyncOp::PortalStopAccessPoint,
            Self::HttpGet { url } => AsyncOp::HttpGet { url: url.clone() },
            Self::PortalHttpRequest { method, path, body } => AsyncOp::PortalHttpRequest {
                method: method.clone(),
                path: path.clone(),
                body: body.clone(),
            },
            Self::PortalClientConnected => AsyncOp::PortalClientConnected,
            Self::PortalStopped => AsyncOp::PortalStopped,
        }
    }
}

impl Serialize for SavedSyncOp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = match self {
            SavedSyncOp::BootReason => RawSavedSyncOp::BootReason,
            SavedSyncOp::MacAddress => RawSavedSyncOp::MacAddress,
            SavedSyncOp::Now => RawSavedSyncOp::Now,
            SavedSyncOp::TftSetDcLow => RawSavedSyncOp::TftSetDcLow,
            SavedSyncOp::TftSetDcHigh => RawSavedSyncOp::TftSetDcHigh,
            SavedSyncOp::TftSetRstLow => RawSavedSyncOp::TftSetRstLow,
            SavedSyncOp::TftSetRstHigh => RawSavedSyncOp::TftSetRstHigh,
            SavedSyncOp::TftWrite { bytes } => RawSavedSyncOp::TftWrite {
                bytes_hex: encode_hex(bytes),
            },
            SavedSyncOp::StoreRead { namespace, keys } => RawSavedSyncOp::StoreRead {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            SavedSyncOp::StoreWrite { namespace, values } => RawSavedSyncOp::StoreWrite {
                namespace: namespace.clone(),
                values: values.clone(),
            },
            SavedSyncOp::StoreRemove { namespace, keys } => RawSavedSyncOp::StoreRemove {
                namespace: namespace.clone(),
                keys: keys.clone(),
            },
            SavedSyncOp::HttpRead { body, max_len } => RawSavedSyncOp::HttpRead {
                body: body.clone(),
                max_len: *max_len,
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SavedSyncOp {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSavedSyncOp::deserialize(deserializer)?;
        Ok(match raw {
            RawSavedSyncOp::BootReason => SavedSyncOp::BootReason,
            RawSavedSyncOp::MacAddress => SavedSyncOp::MacAddress,
            RawSavedSyncOp::Now => SavedSyncOp::Now,
            RawSavedSyncOp::TftSetDcLow => SavedSyncOp::TftSetDcLow,
            RawSavedSyncOp::TftSetDcHigh => SavedSyncOp::TftSetDcHigh,
            RawSavedSyncOp::TftSetRstLow => SavedSyncOp::TftSetRstLow,
            RawSavedSyncOp::TftSetRstHigh => SavedSyncOp::TftSetRstHigh,
            RawSavedSyncOp::TftWrite { bytes_hex } => SavedSyncOp::TftWrite {
                bytes: decode_hex(&bytes_hex).map_err(serde::de::Error::custom)?,
            },
            RawSavedSyncOp::StoreRead { namespace, keys } => {
                SavedSyncOp::StoreRead { namespace, keys }
            }
            RawSavedSyncOp::StoreWrite { namespace, values } => {
                SavedSyncOp::StoreWrite { namespace, values }
            }
            RawSavedSyncOp::StoreRemove { namespace, keys } => {
                SavedSyncOp::StoreRemove { namespace, keys }
            }
            RawSavedSyncOp::HttpRead { body, max_len } => SavedSyncOp::HttpRead { body, max_len },
        })
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err("hex string must have even length".to_string());
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes();
    let decode_nibble = |ch: u8| -> Result<u8, String> {
        match ch {
            b'0'..=b'9' => Ok(ch - b'0'),
            b'a'..=b'f' => Ok(ch - b'a' + 10),
            b'A'..=b'F' => Ok(ch - b'A' + 10),
            _ => Err(format!("invalid hex character '{}'", ch as char)),
        }
    };

    for index in (0..chars.len()).step_by(2) {
        let hi = decode_nibble(chars[index])?;
        let lo = decode_nibble(chars[index + 1])?;
        bytes.push((hi << 4) | lo);
    }

    Ok(bytes)
}

pub fn saved_item_from_runtime_item(
    item: &TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>,
) -> SavedItem {
    match item {
        TraceItem::OutboundCreateSync { id, target, op } => SavedItem::OutboundCreateSync {
            id: id.clone(),
            target: target.clone(),
            op: op.as_ref().map(SavedSyncOp::from_runtime),
        },
        TraceItem::OutboundCreateAsync { id, target, op } => SavedItem::OutboundCreateAsync {
            id: id.clone(),
            target: target.clone(),
            op: op.as_ref().map(SavedAsyncOp::from_runtime),
        },
        TraceItem::OutboundDropResult { target } => SavedItem::OutboundDropResult {
            target: target.clone(),
        },
        TraceItem::InboundDropResult { target } => SavedItem::InboundDropResult {
            target: target.clone(),
        },
        TraceItem::InboundReturnSync { target, result } => SavedItem::InboundReturnSync {
            target: target.clone(),
            result: SavedSyncResult::from_runtime(result)
                .expect("sync success result should stay on success channel"),
        },
        TraceItem::InboundErrorSync { target, error } => SavedItem::InboundErrorSync {
            target: target.clone(),
            error: SavedSyncError::from_runtime_error(error),
        },
        TraceItem::InboundResolveAsync { target, result } => SavedItem::InboundResolveAsync {
            target: target.clone(),
            result: SavedAsyncResult::from_runtime(result),
        },
        TraceItem::InboundAbortAsync { target } => SavedItem::InboundAbortAsync {
            target: target.clone(),
        },
        TraceItem::InboundCancelAsync { target } => SavedItem::InboundCancelAsync {
            target: target.clone(),
        },
        TraceItem::InboundCreateAsync { id, target, op } => SavedItem::InboundCreateAsync {
            id: id.clone(),
            target: target.clone(),
            op: SavedAsyncOp::from_runtime(op),
        },
    }
}

#[cfg(test)]
pub fn runtime_item_from_saved_item(
    item: &SavedItem,
) -> Result<TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>, String> {
    Ok(match item {
        SavedItem::OutboundCreateSync { id, target, op } => TraceItem::OutboundCreateSync {
            id: id.clone(),
            target: target.clone(),
            op: op.as_ref().map(SavedSyncOp::to_runtime),
        },
        SavedItem::OutboundCreateAsync { id, target, op } => TraceItem::OutboundCreateAsync {
            id: id.clone(),
            target: target.clone(),
            op: op.as_ref().map(SavedAsyncOp::to_runtime),
        },
        SavedItem::OutboundDropResult { target } => TraceItem::OutboundDropResult {
            target: target.clone(),
        },
        SavedItem::InboundDropResult { target } => TraceItem::InboundDropResult {
            target: target.clone(),
        },
        SavedItem::InboundReturnSync { target, result } => TraceItem::InboundReturnSync {
            target: target.clone(),
            result: result.to_runtime()?,
        },
        SavedItem::InboundErrorSync { target, error } => TraceItem::InboundErrorSync {
            target: target.clone(),
            error: error.to_runtime_error(),
        },
        SavedItem::InboundResolveAsync { target, result } => TraceItem::InboundResolveAsync {
            target: target.clone(),
            result: result.to_runtime(),
        },
        SavedItem::InboundAbortAsync { target } => TraceItem::InboundAbortAsync {
            target: target.clone(),
        },
        SavedItem::InboundCancelAsync { target } => TraceItem::InboundCancelAsync {
            target: target.clone(),
        },
        SavedItem::InboundCreateAsync { id, target, op } => TraceItem::InboundCreateAsync {
            id: id.clone(),
            target: target.clone(),
            op: op.to_runtime(),
        },
    })
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

pub(super) fn format_boot_reason(reason: BootReason) -> String {
    match reason {
        BootReason::Software => "software",
        BootReason::ExternalPin => "external_pin",
        BootReason::Watchdog => "watchdog",
        BootReason::Sdio => "sdio",
        BootReason::Panic => "panic",
        BootReason::InterruptWatchdog => "interrupt_watchdog",
        BootReason::PowerOn => "power_on",
        BootReason::Unknown => "unknown",
        BootReason::Brownout => "brownout",
        BootReason::TaskWatchdog => "task_watchdog",
        BootReason::DeepSleep => "deep_sleep",
        BootReason::USBPeripheral => "usb_peripheral",
        BootReason::JTAG => "jtag",
        BootReason::EfuseError => "efuse_error",
        BootReason::PowerGlitch => "power_glitch",
        BootReason::CPULockup => "cpu_lockup",
    }
    .to_string()
}
