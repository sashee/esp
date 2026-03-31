use super::*;

pub type SavedItem = simulator::editor::TraceItem<
    SavedSyncOp,
    SavedAsyncOp,
    SavedSyncResult,
    SavedSyncError,
    SavedAsyncResult,
>;

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedSyncResult {
    BootReason { value: String },
    MacAddress { value: [u8; 6] },
    Now { ticks: u64 },
    StoreReadOk { values: BTreeMap<String, String> },
    UnitOk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedSyncError {
    StoreReadErr { message: String },
    UnitErr { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedAsyncResult {
    SleepDone,
    Unit,
    PortalSignal,
    ScanNetworks {
        ssids: Vec<String>,
    },
    ConnectionInfo {
        ip: String,
    },
    PortalStartAccessPoint {
        ip: String,
        gateway: String,
        netmask: String,
    },
    HttpFrame {
        bytes: Vec<u8>,
    },
    PortalHttpResponse {
        status_code: u16,
        content_type: String,
        body_len: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SavedAsyncOp {
    Sleep {
        duration_ms: u64,
    },
    WifiDisconnect,
    WifiStop,
    WifiStart,
    WifiScanNetworks,
    WifiConfigureClient {
        ssid: String,
        password: String,
        channel: Option<u8>,
        auth: String,
    },
    WifiConnect {
        timeout_ms: u64,
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
