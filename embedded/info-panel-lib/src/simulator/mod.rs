use anyhow::{anyhow, Result};
use config_portal::{ConfigStore, HttpEndpoint, HttpMethod, HttpRequest, HttpResponse};
use core::future::poll_fn;
use embassy_time::Duration as EmbassyDuration;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use simulator::editor::{TraceItem, TraceRuntime};
use simulator::{
    elapsed_time, AsyncCompletion, AsyncTiming, ElapsedTime, Event, NextEventsSpec, SimBundle,
    SimDriver, TraceStep,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use crate::{BootReason, Clock, Hal, HttpClient, Platform};

mod bundle;
mod codec;
mod forms;
mod replay;
mod runtime;
mod types;

pub use runtime::InfoPanelSimulatorRuntime;

#[cfg(test)]
type SavedTraceItem = TraceItem<
    types::SyncOp,
    types::AsyncOp,
    types::SyncResult,
    types::SyncError,
    types::AsyncResult,
>;
#[cfg(test)]
type RunDocument = Vec<SavedTraceItem>;
