use std::fs;
use std::path::Path;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use super::{update, AppState, Command, RuntimeTraceItem, TraceRuntime};

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
    T: Clone + DeserializeOwned,
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
