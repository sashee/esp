use anyhow::{anyhow, Result};
use config_portal::{ConfigStore, HttpEndpoint, HttpMethod, HttpRequest, HttpResponse};
use core::future::poll_fn;
use embassy_time::Duration as EmbassyDuration;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use simulator::editor::{InsertionChoice, RuntimeTarget, TraceItem, TraceRuntime, VisibleRow};
use simulator::{
    elapsed_time, AsyncCompletion, AsyncTiming, ElapsedTime, Event, NewRunWrapper,
    NextEventsSpec, PossibleEvent, SimBundle, SimDriver, TraceStep, possible_next_events,
};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use crate::{BootReason, Clock, Hal, HttpClient, Platform};

mod bundle;
mod forms;
mod replay;
mod runtime;
mod saved;
mod types;

pub use runtime::InfoPanelSimulatorRuntime;

type SavedTraceItem = TraceItem<
    saved::SavedSyncOp,
    saved::SavedAsyncOp,
    saved::SavedSyncResult,
    saved::SavedSyncError,
    saved::SavedAsyncResult,
>;
type RunDocument = Vec<SavedTraceItem>;
type FormTarget = RuntimeTarget;
