use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    update, AppState, Command, EncodedTraceItem, RuntimeTraceItem, TraceRuntime, TraceViewState,
};

pub const SIMULATOR_REPLAY_KIND: &str = "simulator-replay";
pub const SIMULATOR_REPLAY_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEnvelope<T> {
    pub kind: String,
    pub version: u32,
    pub initial_state: AppState<T>,
    pub commands: Vec<Command>,
}

impl<T> ReplayEnvelope<T> {
    pub fn new(initial_state: AppState<T>, commands: Vec<Command>) -> Self {
        Self {
            kind: SIMULATOR_REPLAY_KIND.to_string(),
            version: SIMULATOR_REPLAY_VERSION,
            initial_state,
            commands,
        }
    }
}

pub fn save_replay<T>(path: &Path, replay: &ReplayEnvelope<T>) -> Result<(), String>
where
    T: Clone + Serialize,
{
    let contents = serde_json::to_string_pretty(replay).map_err(|err| err.to_string())?;
    fs::write(path, contents).map_err(|err| err.to_string())
}

pub fn load_replay<T>(path: &Path) -> Result<ReplayEnvelope<T>, String>
where
    T: Clone + for<'de> Deserialize<'de>,
{
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let replay: ReplayEnvelope<T> =
        serde_json::from_str(&contents).map_err(|err| err.to_string())?;
    if replay.kind != SIMULATOR_REPLAY_KIND {
        return Err(format!("unsupported replay kind: {}", replay.kind));
    }
    if replay.version != SIMULATOR_REPLAY_VERSION {
        return Err(format!("unsupported replay version: {}", replay.version));
    }
    Ok(replay)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct EncodedReplayEnvelope {
    kind: String,
    version: u32,
    initial_state: AppState<EncodedTraceItem>,
    commands: Vec<Command>,
}

fn encode_item<R: TraceRuntime>(
    runtime: &R,
    item: &RuntimeTraceItem<R>,
) -> Result<EncodedTraceItem, String> {
    Ok(match item {
        super::TraceItem::OutboundCreateSync { id, target, .. } => {
            EncodedTraceItem::OutboundCreateSync {
                id: id.clone(),
                target: target.clone(),
            }
        }
        super::TraceItem::OutboundCreateAsync { id, target, .. } => {
            EncodedTraceItem::OutboundCreateAsync {
                id: id.clone(),
                target: target.clone(),
            }
        }
        super::TraceItem::OutboundDropResult { target } => EncodedTraceItem::OutboundDropResult {
            target: target.clone(),
        },
        super::TraceItem::InboundDropResult { target } => EncodedTraceItem::InboundDropResult {
            target: target.clone(),
        },
        super::TraceItem::InboundReturnSync { target, result } => {
            EncodedTraceItem::InboundReturnSync {
                target: target.clone(),
                result: runtime.sync_result_to_json(result)?,
            }
        }
        super::TraceItem::InboundErrorSync { target, error } => {
            EncodedTraceItem::InboundErrorSync {
                target: target.clone(),
                error: runtime.sync_error_to_json(error)?,
            }
        }
        super::TraceItem::InboundResolveAsync { target, result } => {
            EncodedTraceItem::InboundResolveAsync {
                target: target.clone(),
                result: runtime.async_result_to_json(result)?,
            }
        }
        super::TraceItem::InboundAbortAsync { target } => EncodedTraceItem::InboundAbortAsync {
            target: target.clone(),
        },
        super::TraceItem::InboundCancelAsync { target } => EncodedTraceItem::InboundCancelAsync {
            target: target.clone(),
        },
        super::TraceItem::InboundCreateAsync { id, target, op } => {
            EncodedTraceItem::InboundCreateAsync {
                id: id.clone(),
                target: target.clone(),
                op: runtime.async_op_to_json(op)?,
            }
        }
    })
}

fn decode_item<R: TraceRuntime>(
    runtime: &R,
    item: EncodedTraceItem,
) -> Result<RuntimeTraceItem<R>, String> {
    Ok(match item {
        EncodedTraceItem::OutboundCreateSync { id, target } => {
            super::TraceItem::OutboundCreateSync {
                id,
                target,
                op: None,
            }
        }
        EncodedTraceItem::OutboundCreateAsync { id, target } => {
            super::TraceItem::OutboundCreateAsync {
                id,
                target,
                op: None,
            }
        }
        EncodedTraceItem::OutboundDropResult { target } => {
            super::TraceItem::OutboundDropResult { target }
        }
        EncodedTraceItem::InboundDropResult { target } => {
            super::TraceItem::InboundDropResult { target }
        }
        EncodedTraceItem::InboundReturnSync { target, result } => {
            super::TraceItem::InboundReturnSync {
                target,
                result: runtime.sync_result_from_json(result)?,
            }
        }
        EncodedTraceItem::InboundErrorSync { target, error } => {
            super::TraceItem::InboundErrorSync {
                target,
                error: runtime.sync_error_from_json(error)?,
            }
        }
        EncodedTraceItem::InboundResolveAsync { target, result } => {
            super::TraceItem::InboundResolveAsync {
                target,
                result: runtime.async_result_from_json(result)?,
            }
        }
        EncodedTraceItem::InboundAbortAsync { target } => {
            super::TraceItem::InboundAbortAsync { target }
        }
        EncodedTraceItem::InboundCancelAsync { target } => {
            super::TraceItem::InboundCancelAsync { target }
        }
        EncodedTraceItem::InboundCreateAsync { id, target, op } => {
            super::TraceItem::InboundCreateAsync {
                id,
                target,
                op: runtime.async_op_from_json(op)?,
            }
        }
    })
}

pub fn save_runtime_replay<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
    replay: &ReplayEnvelope<RuntimeTraceItem<R>>,
) -> Result<(), String> {
    let encoded = EncodedReplayEnvelope {
        kind: replay.kind.clone(),
        version: replay.version,
        initial_state: AppState {
            view: TraceViewState {
                trace: replay
                    .initial_state
                    .view
                    .trace
                    .iter()
                    .map(|item| encode_item(runtime, item))
                    .collect::<Result<Vec<_>, _>>()?,
                cursor_step_index: replay.initial_state.view.cursor_step_index,
                selection_anchor_step_index: replay.initial_state.view.selection_anchor_step_index,
                scroll_offset: replay.initial_state.view.scroll_offset,
                dialog: replay.initial_state.view.dialog.clone(),
                status: replay.initial_state.view.status.clone(),
                last_char: replay.initial_state.view.last_char,
                terminal_width: replay.initial_state.view.terminal_width,
                terminal_height: replay.initial_state.view.terminal_height,
            },
        },
        commands: replay.commands.clone(),
    };
    let contents = serde_json::to_string_pretty(&encoded).map_err(|err| err.to_string())?;
    fs::write(path, contents).map_err(|err| err.to_string())
}

pub fn load_runtime_replay<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
) -> Result<ReplayEnvelope<RuntimeTraceItem<R>>, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let replay: EncodedReplayEnvelope =
        serde_json::from_str(&contents).map_err(|err| err.to_string())?;
    if replay.kind != SIMULATOR_REPLAY_KIND {
        return Err(format!("unsupported replay kind: {}", replay.kind));
    }
    if replay.version != SIMULATOR_REPLAY_VERSION {
        return Err(format!("unsupported replay version: {}", replay.version));
    }
    Ok(ReplayEnvelope {
        kind: replay.kind,
        version: replay.version,
        initial_state: AppState {
            view: TraceViewState {
                trace: replay
                    .initial_state
                    .view
                    .trace
                    .into_iter()
                    .map(|item| decode_item(runtime, item))
                    .collect::<Result<Vec<_>, _>>()?,
                cursor_step_index: replay.initial_state.view.cursor_step_index,
                selection_anchor_step_index: replay.initial_state.view.selection_anchor_step_index,
                scroll_offset: replay.initial_state.view.scroll_offset,
                dialog: replay.initial_state.view.dialog,
                status: replay.initial_state.view.status,
                last_char: replay.initial_state.view.last_char,
                terminal_width: replay.initial_state.view.terminal_width,
                terminal_height: replay.initial_state.view.terminal_height,
            },
        },
        commands: replay.commands,
    })
}

pub fn is_replay_file(path: &Path) -> Result<bool, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(&contents).map_err(|err| err.to_string())?;
    Ok(value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == SIMULATOR_REPLAY_KIND))
}

pub fn replay_state_at<R: TraceRuntime>(
    runtime: &R,
    initial_state: &AppState<RuntimeTraceItem<R>>,
    commands: &[Command],
    state_index: usize,
) -> Result<AppState<RuntimeTraceItem<R>>, String> {
    if state_index > commands.len() {
        return Err(format!(
            "action index {state_index} out of range; max is {}",
            commands.len()
        ));
    }
    let mut state = initial_state.clone();
    for command in commands.iter().take(state_index) {
        let (next_state, _) = update(state, command.clone(), runtime);
        state = next_state;
    }
    Ok(state)
}
