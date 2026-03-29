use anyhow::{anyhow, Result};
use config_portal::{ConfigStore, HttpEndpoint, HttpMethod, HttpRequest, HttpResponse};
use core::future::poll_fn;
use crossterm::event::{KeyCode, KeyEvent};
use embassy_time::Duration as EmbassyDuration;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use simulator::editor::{
    FormController, FormResult, FormTarget, InsertionChoice, RunDocument, TraceRuntime, VisibleRow,
};
use simulator::ratatui::layout::Rect;
use simulator::ratatui::style::{Modifier, Style};
use simulator::ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use simulator::{
    possible_next_events, AsyncCompletion, AsyncTiming, Event, NewRunWrapper, NextEventsSpec,
    PossibleEvent, SimBundle, SimDriver, TraceStep,
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
