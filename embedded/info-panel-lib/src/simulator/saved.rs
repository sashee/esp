use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SavedItem {
    OutboundCreateSync {
        id: String,
        op: SavedSyncOp,
    },
    OutboundCreateAsync {
        id: String,
        op: SavedAsyncOp,
    },
    InboundReturnSync {
        target: String,
        result: SavedSyncResult,
    },
    InboundResolveAsync {
        target: String,
        result: SavedAsyncResult,
    },
    InboundAbortAsync {
        target: String,
    },
    InboundCancelAsync {
        target: String,
    },
    InboundCreateAsync {
        id: String,
        op: SavedAsyncOp,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SavedItemType {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SavedEventType {
    CreateSync,
    CreateAsync,
    ReturnSync,
    ResolveAsync,
    AbortAsync,
    CancelAsync,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct RawSavedItem {
    #[serde(rename = "type")]
    item_type: SavedItemType,
    event_type: SavedEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum SavedSyncOp {
    BootReason,
    MacAddress,
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
pub(super) enum SavedSyncResult {
    BootReason { value: String },
    MacAddress { value: [u8; 6] },
    StoreRead { values: BTreeMap<String, String> },
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum SavedAsyncResult {
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
pub(super) enum SavedAsyncOp {
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

impl Serialize for SavedItem {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = match self {
            SavedItem::OutboundCreateSync { id, op } => RawSavedItem {
                item_type: SavedItemType::Outbound,
                event_type: SavedEventType::CreateSync,
                id: Some(id.clone()),
                target: None,
                op: Some(serde_json::to_value(op).map_err(serde::ser::Error::custom)?),
                result: None,
            },
            SavedItem::OutboundCreateAsync { id, op } => RawSavedItem {
                item_type: SavedItemType::Outbound,
                event_type: SavedEventType::CreateAsync,
                id: Some(id.clone()),
                target: None,
                op: Some(serde_json::to_value(op).map_err(serde::ser::Error::custom)?),
                result: None,
            },
            SavedItem::InboundReturnSync { target, result } => RawSavedItem {
                item_type: SavedItemType::Inbound,
                event_type: SavedEventType::ReturnSync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(serde_json::to_value(result).map_err(serde::ser::Error::custom)?),
            },
            SavedItem::InboundResolveAsync { target, result } => RawSavedItem {
                item_type: SavedItemType::Inbound,
                event_type: SavedEventType::ResolveAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(serde_json::to_value(result).map_err(serde::ser::Error::custom)?),
            },
            SavedItem::InboundAbortAsync { target } => RawSavedItem {
                item_type: SavedItemType::Inbound,
                event_type: SavedEventType::AbortAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
            },
            SavedItem::InboundCancelAsync { target } => RawSavedItem {
                item_type: SavedItemType::Inbound,
                event_type: SavedEventType::CancelAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
            },
            SavedItem::InboundCreateAsync { id, op } => RawSavedItem {
                item_type: SavedItemType::Inbound,
                event_type: SavedEventType::CreateAsync,
                id: Some(id.clone()),
                target: None,
                op: Some(serde_json::to_value(op).map_err(serde::ser::Error::custom)?),
                result: None,
            },
        };

        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SavedItem {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSavedItem::deserialize(deserializer)?;

        match (raw.item_type, raw.event_type) {
            (SavedItemType::Outbound, SavedEventType::CreateSync) => {
                Ok(SavedItem::OutboundCreateSync {
                    id: raw
                        .id
                        .ok_or_else(|| serde::de::Error::custom("missing id"))?,
                    op: serde_json::from_value(
                        raw.op
                            .ok_or_else(|| serde::de::Error::custom("missing op"))?,
                    )
                    .map_err(serde::de::Error::custom)?,
                })
            }
            (SavedItemType::Outbound, SavedEventType::CreateAsync) => {
                Ok(SavedItem::OutboundCreateAsync {
                    id: raw
                        .id
                        .ok_or_else(|| serde::de::Error::custom("missing id"))?,
                    op: serde_json::from_value(
                        raw.op
                            .ok_or_else(|| serde::de::Error::custom("missing op"))?,
                    )
                    .map_err(serde::de::Error::custom)?,
                })
            }
            (SavedItemType::Inbound, SavedEventType::ReturnSync) => {
                Ok(SavedItem::InboundReturnSync {
                    target: raw
                        .target
                        .ok_or_else(|| serde::de::Error::custom("missing target"))?,
                    result: serde_json::from_value(
                        raw.result
                            .ok_or_else(|| serde::de::Error::custom("missing result"))?,
                    )
                    .map_err(serde::de::Error::custom)?,
                })
            }
            (SavedItemType::Inbound, SavedEventType::ResolveAsync) => {
                Ok(SavedItem::InboundResolveAsync {
                    target: raw
                        .target
                        .ok_or_else(|| serde::de::Error::custom("missing target"))?,
                    result: serde_json::from_value(
                        raw.result
                            .ok_or_else(|| serde::de::Error::custom("missing result"))?,
                    )
                    .map_err(serde::de::Error::custom)?,
                })
            }
            (SavedItemType::Inbound, SavedEventType::AbortAsync) => {
                Ok(SavedItem::InboundAbortAsync {
                    target: raw
                        .target
                        .ok_or_else(|| serde::de::Error::custom("missing target"))?,
                })
            }
            (SavedItemType::Inbound, SavedEventType::CancelAsync) => {
                Ok(SavedItem::InboundCancelAsync {
                    target: raw
                        .target
                        .ok_or_else(|| serde::de::Error::custom("missing target"))?,
                })
            }
            (SavedItemType::Inbound, SavedEventType::CreateAsync) => {
                Ok(SavedItem::InboundCreateAsync {
                    id: raw
                        .id
                        .ok_or_else(|| serde::de::Error::custom("missing id"))?,
                    op: serde_json::from_value(
                        raw.op
                            .ok_or_else(|| serde::de::Error::custom("missing op"))?,
                    )
                    .map_err(serde::de::Error::custom)?,
                })
            }
            (item_type, event_type) => Err(serde::de::Error::custom(format!(
                "unsupported saved item combination: {item_type:?} {event_type:?}"
            ))),
        }
    }
}
