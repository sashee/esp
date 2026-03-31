use serde::{Deserialize, Serialize};

pub const SIMULATOR_RUN_KIND: &str = "simulator-run";
pub const SIMULATOR_RUN_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> {
    OutboundCreateSync { id: String, op: SyncOp },
    OutboundCreateAsync { id: String, op: AsyncOp },
    InboundReturnSync { target: String, result: SyncResult },
    InboundErrorSync { target: String, error: SyncError },
    InboundResolveAsync { target: String, result: AsyncResult },
    InboundAbortAsync { target: String },
    InboundCancelAsync { target: String },
    InboundCreateAsync { id: String, op: AsyncOp },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemSide {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EventKind {
    CreateSync,
    CreateAsync,
    ReturnSync,
    ErrorSync,
    ResolveAsync,
    AbortAsync,
    CancelAsync,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RawTraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> {
    #[serde(rename = "type")]
    side: ItemSide,
    event_type: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op: Option<PayloadOp<SyncOp, AsyncOp>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<PayloadResult<SyncResult, AsyncResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SyncError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum PayloadOp<SyncOp, AsyncOp> {
    Sync(SyncOp),
    Async(AsyncOp),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum PayloadResult<SyncResult, AsyncResult> {
    Sync(SyncResult),
    Async(AsyncResult),
}

impl<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> Serialize
    for TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>
where
    SyncOp: Clone + Serialize,
    AsyncOp: Clone + Serialize,
    SyncResult: Clone + Serialize,
    SyncError: Clone + Serialize,
    AsyncResult: Clone + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = match self {
            TraceItem::OutboundCreateSync { id, op } => RawTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateSync,
                id: Some(id.clone()),
                target: None,
                op: Some(PayloadOp::Sync(op.clone())),
                result: None,
                error: None,
            },
            TraceItem::OutboundCreateAsync { id, op } => RawTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: None,
                op: Some(PayloadOp::Async(op.clone())),
                result: None,
                error: None,
            },
            TraceItem::InboundReturnSync { target, result } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ReturnSync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(PayloadResult::Sync(result.clone())),
                error: None,
            },
            TraceItem::InboundErrorSync { target, error } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ErrorSync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: Some(error.clone()),
            },
            TraceItem::InboundResolveAsync { target, result } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ResolveAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(PayloadResult::Async(result.clone())),
                error: None,
            },
            TraceItem::InboundAbortAsync { target } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::AbortAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            TraceItem::InboundCancelAsync { target } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::CancelAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            TraceItem::InboundCreateAsync { id, op } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: None,
                op: Some(PayloadOp::Async(op.clone())),
                result: None,
                error: None,
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de, SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> Deserialize<'de>
    for TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>
where
    SyncOp: Deserialize<'de>,
    AsyncOp: Deserialize<'de>,
    SyncResult: Deserialize<'de>,
    SyncError: Deserialize<'de>,
    AsyncResult: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawTraceItem::<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult>::deserialize(
            deserializer,
        )?;
        match (raw.side, raw.event_type) {
            (ItemSide::Outbound, EventKind::CreateSync) => match (raw.id, raw.op) {
                (Some(id), Some(PayloadOp::Sync(op))) => {
                    Ok(TraceItem::OutboundCreateSync { id, op })
                }
                _ => Err(serde::de::Error::custom(
                    "invalid outbound create_sync item",
                )),
            },
            (ItemSide::Outbound, EventKind::CreateAsync) => match (raw.id, raw.op) {
                (Some(id), Some(PayloadOp::Async(op))) => {
                    Ok(TraceItem::OutboundCreateAsync { id, op })
                }
                _ => Err(serde::de::Error::custom(
                    "invalid outbound create_async item",
                )),
            },
            (ItemSide::Inbound, EventKind::ReturnSync) => match (raw.target, raw.result) {
                (Some(target), Some(PayloadResult::Sync(result))) => {
                    Ok(TraceItem::InboundReturnSync { target, result })
                }
                _ => Err(serde::de::Error::custom("invalid inbound return_sync item")),
            },
            (ItemSide::Inbound, EventKind::ErrorSync) => match (raw.target, raw.error) {
                (Some(target), Some(error)) => Ok(TraceItem::InboundErrorSync { target, error }),
                _ => Err(serde::de::Error::custom("invalid inbound error_sync item")),
            },
            (ItemSide::Inbound, EventKind::ResolveAsync) => match (raw.target, raw.result) {
                (Some(target), Some(PayloadResult::Async(result))) => {
                    Ok(TraceItem::InboundResolveAsync { target, result })
                }
                _ => Err(serde::de::Error::custom(
                    "invalid inbound resolve_async item",
                )),
            },
            (ItemSide::Inbound, EventKind::AbortAsync) => match raw.target {
                Some(target) => Ok(TraceItem::InboundAbortAsync { target }),
                None => Err(serde::de::Error::custom("invalid inbound abort_async item")),
            },
            (ItemSide::Inbound, EventKind::CancelAsync) => match raw.target {
                Some(target) => Ok(TraceItem::InboundCancelAsync { target }),
                None => Err(serde::de::Error::custom(
                    "invalid inbound cancel_async item",
                )),
            },
            (ItemSide::Inbound, EventKind::CreateAsync) => match (raw.id, raw.op) {
                (Some(id), Some(PayloadOp::Async(op))) => {
                    Ok(TraceItem::InboundCreateAsync { id, op })
                }
                _ => Err(serde::de::Error::custom(
                    "invalid inbound create_async item",
                )),
            },
            _ => Err(serde::de::Error::custom("invalid trace item kind")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEnvelope<T> {
    pub kind: String,
    pub version: u32,
    pub items: Vec<T>,
}

impl<T> Default for RunEnvelope<T> {
    fn default() -> Self {
        Self {
            kind: SIMULATOR_RUN_KIND.to_string(),
            version: SIMULATOR_RUN_VERSION,
            items: Vec::new(),
        }
    }
}

impl<T> RunEnvelope<T> {
    pub fn is_simulator_run(&self) -> bool {
        self.kind == SIMULATOR_RUN_KIND
    }
}
