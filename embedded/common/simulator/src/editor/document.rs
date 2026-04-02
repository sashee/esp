use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SIMULATOR_RUN_KIND: &str = "simulator-run";
pub const SIMULATOR_RUN_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> {
    OutboundCreateSync {
        id: String,
        target: Option<String>,
        op: Option<SyncOp>,
    },
    OutboundCreateAsync {
        id: String,
        target: Option<String>,
        op: Option<AsyncOp>,
    },
    OutboundDropResult {
        target: String,
    },
    InboundDropResult {
        target: String,
    },
    InboundReturnSync {
        target: String,
        result: SyncResult,
    },
    InboundErrorSync {
        target: String,
        error: SyncError,
    },
    InboundResolveAsync {
        target: String,
        result: AsyncResult,
    },
    InboundAbortAsync {
        target: String,
    },
    InboundCancelAsync {
        target: String,
    },
    InboundCreateAsync {
        id: String,
        target: Option<String>,
        op: AsyncOp,
    },
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
    DropResult,
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
struct RawEncodedTraceItem {
    #[serde(rename = "type")]
    side: ItemSide,
    event_type: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodedTraceItem {
    OutboundCreateSync {
        id: String,
        target: Option<String>,
    },
    OutboundCreateAsync {
        id: String,
        target: Option<String>,
    },
    OutboundDropResult {
        target: String,
    },
    InboundDropResult {
        target: String,
    },
    InboundReturnSync {
        target: String,
        result: Value,
    },
    InboundErrorSync {
        target: String,
        error: Value,
    },
    InboundResolveAsync {
        target: String,
        result: Value,
    },
    InboundAbortAsync {
        target: String,
    },
    InboundCancelAsync {
        target: String,
    },
    InboundCreateAsync {
        id: String,
        target: Option<String>,
        op: Value,
    },
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
        let raw: RawTraceItem<SyncOp, AsyncOp, SyncResult, SyncError, AsyncResult> = match self {
            TraceItem::OutboundCreateSync { id, target, .. } => RawTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateSync,
                id: Some(id.clone()),
                target: target.clone(),
                op: None,
                result: None,
                error: None,
            },
            TraceItem::OutboundCreateAsync { id, target, .. } => RawTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: target.clone(),
                op: None,
                result: None,
                error: None,
            },
            TraceItem::OutboundDropResult { target } => RawTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::DropResult,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            TraceItem::InboundDropResult { target } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::DropResult,
                id: None,
                target: Some(target.clone()),
                op: None,
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
            TraceItem::InboundCreateAsync { id, target, op } => RawTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: target.clone(),
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
            (ItemSide::Outbound, EventKind::CreateSync) => match raw.id {
                Some(id) => Ok(TraceItem::OutboundCreateSync {
                    id,
                    target: raw.target,
                    op: None,
                }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound create_sync item",
                )),
            },
            (ItemSide::Outbound, EventKind::CreateAsync) => match raw.id {
                Some(id) => Ok(TraceItem::OutboundCreateAsync {
                    id,
                    target: raw.target,
                    op: None,
                }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound create_async item",
                )),
            },
            (ItemSide::Outbound, EventKind::DropResult) => match raw.target {
                Some(target) => Ok(TraceItem::OutboundDropResult { target }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound drop_result item",
                )),
            },
            (ItemSide::Inbound, EventKind::DropResult) => match raw.target {
                Some(target) => Ok(TraceItem::InboundDropResult { target }),
                None => Err(serde::de::Error::custom("invalid inbound drop_result item")),
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
                (Some(id), Some(PayloadOp::Async(op))) => Ok(TraceItem::InboundCreateAsync {
                    id,
                    target: raw.target,
                    op,
                }),
                _ => Err(serde::de::Error::custom(
                    "invalid inbound create_async item",
                )),
            },
            _ => Err(serde::de::Error::custom("invalid trace item kind")),
        }
    }
}

impl Serialize for EncodedTraceItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let raw = match self {
            EncodedTraceItem::OutboundCreateSync { id, target } => RawEncodedTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateSync,
                id: Some(id.clone()),
                target: target.clone(),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::OutboundCreateAsync { id, target } => RawEncodedTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: target.clone(),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::OutboundDropResult { target } => RawEncodedTraceItem {
                side: ItemSide::Outbound,
                event_type: EventKind::DropResult,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::InboundDropResult { target } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::DropResult,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::InboundReturnSync { target, result } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ReturnSync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(result.clone()),
                error: None,
            },
            EncodedTraceItem::InboundErrorSync { target, error } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ErrorSync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: Some(error.clone()),
            },
            EncodedTraceItem::InboundResolveAsync { target, result } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::ResolveAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: Some(result.clone()),
                error: None,
            },
            EncodedTraceItem::InboundAbortAsync { target } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::AbortAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::InboundCancelAsync { target } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::CancelAsync,
                id: None,
                target: Some(target.clone()),
                op: None,
                result: None,
                error: None,
            },
            EncodedTraceItem::InboundCreateAsync { id, target, op } => RawEncodedTraceItem {
                side: ItemSide::Inbound,
                event_type: EventKind::CreateAsync,
                id: Some(id.clone()),
                target: target.clone(),
                op: Some(op.clone()),
                result: None,
                error: None,
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EncodedTraceItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawEncodedTraceItem::deserialize(deserializer)?;
        match (raw.side, raw.event_type) {
            (ItemSide::Outbound, EventKind::CreateSync) => match raw.id {
                Some(id) => Ok(Self::OutboundCreateSync {
                    id,
                    target: raw.target,
                }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound create_sync item",
                )),
            },
            (ItemSide::Outbound, EventKind::CreateAsync) => match raw.id {
                Some(id) => Ok(Self::OutboundCreateAsync {
                    id,
                    target: raw.target,
                }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound create_async item",
                )),
            },
            (ItemSide::Outbound, EventKind::DropResult) => match raw.target {
                Some(target) => Ok(Self::OutboundDropResult { target }),
                None => Err(serde::de::Error::custom(
                    "invalid outbound drop_result item",
                )),
            },
            (ItemSide::Inbound, EventKind::DropResult) => match raw.target {
                Some(target) => Ok(Self::InboundDropResult { target }),
                None => Err(serde::de::Error::custom("invalid inbound drop_result item")),
            },
            (ItemSide::Inbound, EventKind::ReturnSync) => match (raw.target, raw.result) {
                (Some(target), Some(result)) => Ok(Self::InboundReturnSync { target, result }),
                _ => Err(serde::de::Error::custom("invalid inbound return_sync item")),
            },
            (ItemSide::Inbound, EventKind::ErrorSync) => match (raw.target, raw.error) {
                (Some(target), Some(error)) => Ok(Self::InboundErrorSync { target, error }),
                _ => Err(serde::de::Error::custom("invalid inbound error_sync item")),
            },
            (ItemSide::Inbound, EventKind::ResolveAsync) => match (raw.target, raw.result) {
                (Some(target), Some(result)) => Ok(Self::InboundResolveAsync { target, result }),
                _ => Err(serde::de::Error::custom(
                    "invalid inbound resolve_async item",
                )),
            },
            (ItemSide::Inbound, EventKind::AbortAsync) => match raw.target {
                Some(target) => Ok(Self::InboundAbortAsync { target }),
                None => Err(serde::de::Error::custom("invalid inbound abort_async item")),
            },
            (ItemSide::Inbound, EventKind::CancelAsync) => match raw.target {
                Some(target) => Ok(Self::InboundCancelAsync { target }),
                None => Err(serde::de::Error::custom(
                    "invalid inbound cancel_async item",
                )),
            },
            (ItemSide::Inbound, EventKind::CreateAsync) => match (raw.id, raw.op) {
                (Some(id), Some(op)) => Ok(Self::InboundCreateAsync {
                    id,
                    target: raw.target,
                    op,
                }),
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
