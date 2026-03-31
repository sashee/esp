use std::fs;
use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Serialize};

use crate::{Event, NewRunWrapper, NextEventsSpec, PossibleEvent, SimBundle, TraceStep};

use super::{
    form_is_auto_acceptable, missing_form_fields, AppState, Command, DialogTarget, Effect,
    FormFieldKind, FormState, InsertionChoice, RenderedTrace, RunEnvelope, RuntimeTarget,
    TraceItem, TraceViewDialog, VisibleRow,
};

pub type RuntimeTraceItem<R> = TraceItem<
    <R as TraceRuntime>::SyncOp,
    <R as TraceRuntime>::AsyncOp,
    <R as TraceRuntime>::SyncResult,
    <R as TraceRuntime>::SyncError,
    <R as TraceRuntime>::AsyncResult,
>;

pub struct EditorSession<T> {
    pub path: PathBuf,
    pub state: AppState<T>,
}

pub enum ReplayItemAction<S, A, SR, AR> {
    BindOutbound,
    PushInbound(Event<S, A, SR, AR>),
}

pub trait TraceRuntime {
    type SyncOp: Clone + Serialize + DeserializeOwned + PartialEq + Eq;
    type AsyncOp: Clone + Serialize + DeserializeOwned + PartialEq + Eq;
    type SyncResult: Clone + Serialize + DeserializeOwned + PartialEq + Eq;
    type SyncError: Clone + Serialize + DeserializeOwned + PartialEq + Eq;
    type AsyncResult: Clone + Serialize + DeserializeOwned + PartialEq + Eq;
    type ReplaySyncOp: Clone + Send + 'static;
    type ReplayAsyncOp: Clone + Send + 'static;
    type ReplaySyncResult: Clone + Send + 'static;
    type ReplayAsyncResult: Clone + Send + 'static;
    type Bundle: SimBundle<
        SyncOp = Self::ReplaySyncOp,
        AsyncOp = Self::ReplayAsyncOp,
        SyncResult = Self::ReplaySyncResult,
        AsyncResult = Self::ReplayAsyncResult,
    >;
    type ReplaySpec: NextEventsSpec<
        Self::ReplaySyncOp,
        Self::ReplayAsyncOp,
        Self::ReplaySyncResult,
        Self::ReplayAsyncResult,
    >;
    type ReplayState;

    fn insertion_choices(
        &self,
        trace_prefix: &[RuntimeTraceItem<Self>],
    ) -> Result<Vec<InsertionChoice>, String>;
    fn edit_choices(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        item_index: usize,
    ) -> Result<Vec<InsertionChoice>, String>;
    fn form_spec(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        target: &RuntimeTarget,
        choice_index: usize,
    ) -> Result<super::FormSpec, String>;
    fn initial_form_state(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        target: &RuntimeTarget,
        choice_index: usize,
    ) -> Result<FormState, String>;
    fn encode_form_state(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        target: &RuntimeTarget,
        choice_index: usize,
        state: &FormState,
    ) -> Result<Vec<RuntimeTraceItem<Self>>, String>;
    fn apply_form(
        &self,
        trace: &mut Vec<RuntimeTraceItem<Self>>,
        target: &RuntimeTarget,
        items: Vec<RuntimeTraceItem<Self>>,
    ) -> Result<(), String>;
    fn new_replay_state(&self) -> Self::ReplayState;
    fn new_replay_bundle(&self, replay_state: &mut Self::ReplayState) -> Self::Bundle;
    fn record_runtime_outbound(
        &self,
        replay_state: &mut Self::ReplayState,
        events: &[Event<
            Self::ReplaySyncOp,
            Self::ReplayAsyncOp,
            Self::ReplaySyncResult,
            Self::ReplayAsyncResult,
        >],
    );
    fn replay_item_action(
        &self,
        replay_state: &mut Self::ReplayState,
        item: &RuntimeTraceItem<Self>,
    ) -> Result<
        ReplayItemAction<
            Self::ReplaySyncOp,
            Self::ReplayAsyncOp,
            Self::ReplaySyncResult,
            Self::ReplayAsyncResult,
        >,
        String,
    >;
    fn matches_possible_event(
        &self,
        replay_state: &Self::ReplayState,
        candidate: &PossibleEvent<
            Self::ReplaySyncOp,
            Self::ReplayAsyncOp,
            <Self::ReplaySpec as NextEventsSpec<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                Self::ReplaySyncResult,
                Self::ReplayAsyncResult,
            >>::InboundAsyncKind,
        >,
        event: &Event<
            Self::ReplaySyncOp,
            Self::ReplayAsyncOp,
            Self::ReplaySyncResult,
            Self::ReplayAsyncResult,
        >,
    ) -> bool;
    fn format_trace_item(&self, item: &RuntimeTraceItem<Self>) -> String;
    fn format_runtime_event(
        &self,
        event: &Event<
            Self::ReplaySyncOp,
            Self::ReplayAsyncOp,
            Self::ReplaySyncResult,
            Self::ReplayAsyncResult,
        >,
    ) -> String;
    fn replay_terminated_marker(&self, _replay_state: &Self::ReplayState) -> Option<String> {
        None
    }
}

const TRIVIAL_PREVIEW_LIMIT: usize = 5;
const STATUS_HEIGHT: u16 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StepInfo {
    pub start_row: usize,
    pub end_row: usize,
    pub insertion_index: usize,
    pub inbound_item_index: Option<usize>,
    pub outbound_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewSnapshot {
    pub(crate) rows: Vec<VisibleRow>,
    pub(crate) replay_error: Option<String>,
    pub(crate) steps: Vec<StepInfo>,
    pub(crate) trivial_preview: Vec<VisibleRow>,
}

pub fn load_trace<R: TraceRuntime>(path: &Path) -> Result<Vec<RuntimeTraceItem<R>>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let envelope: RunEnvelope<RuntimeTraceItem<R>> = serde_json::from_str(&contents)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    if !envelope.is_simulator_run() {
        return Err(format!("{} is not a simulator run", path.display()));
    }
    Ok(envelope.items)
}

pub fn save_trace<T>(path: &Path, trace: &[T]) -> Result<(), String>
where
    T: Clone + Serialize,
{
    let envelope = RunEnvelope {
        kind: super::SIMULATOR_RUN_KIND.to_string(),
        version: super::SIMULATOR_RUN_VERSION,
        items: trace.to_vec(),
    };
    let contents = serde_json::to_string_pretty(&envelope)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

pub fn create_trace<T>(path: &Path) -> Result<(), String>
where
    T: Clone + Serialize,
{
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    save_trace::<T>(path, &[])
}

pub fn open_trace<R: TraceRuntime>(
    path: &Path,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<EditorSession<RuntimeTraceItem<R>>, String> {
    let trace = load_trace::<R>(path)?;
    Ok(EditorSession {
        path: path.to_path_buf(),
        state: AppState::new(trace, terminal_width, terminal_height),
    })
}

pub fn open_or_create_trace<R: TraceRuntime>(
    path: &Path,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<EditorSession<RuntimeTraceItem<R>>, String> {
    if path.exists() {
        open_trace::<R>(path, terminal_width, terminal_height)
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
        if !parent.exists() {
            return Err(format!(
                "parent directory does not exist: {}",
                parent.display()
            ));
        }
        save_trace::<RuntimeTraceItem<R>>(path, &[])?;
        let mut session = open_trace::<R>(path, terminal_width, terminal_height)?;
        session.state.view.status = Some(format!("saved {}", path.display()));
        Ok(session)
    }
}

fn trace_viewport_height<T>(state: &AppState<T>) -> usize {
    state
        .view
        .terminal_height
        .saturating_sub(STATUS_HEIGHT)
        .saturating_sub(2) as usize
}

fn build_steps(rows: &[VisibleRow], trace_len: usize) -> Vec<StepInfo> {
    let mut steps = Vec::new();
    let mut row_index = 0;
    while row_index < rows.len() {
        let insertion_index = rows[row_index].insertion_index;
        let start_row = row_index;
        let mut end_row = row_index;
        let mut inbound_item_index = None;
        let mut outbound_only = true;
        while end_row + 1 < rows.len() && rows[end_row + 1].insertion_index == insertion_index {
            end_row += 1;
        }
        for row in &rows[start_row..=end_row] {
            if let Some(item_index) = row.script_item_index {
                inbound_item_index = Some(item_index);
                outbound_only = false;
            }
            if row.is_invalid {
                outbound_only = false;
            }
        }
        steps.push(StepInfo {
            start_row,
            end_row,
            insertion_index,
            inbound_item_index,
            outbound_only,
        });
        row_index = end_row + 1;
    }
    if steps.is_empty() && trace_len == 0 {
        steps.push(StepInfo {
            start_row: 0,
            end_row: 0,
            insertion_index: 0,
            inbound_item_index: None,
            outbound_only: false,
        });
    }
    steps
}

#[derive(Clone)]
enum RowTimelineEvent {
    Start(u64),
    End(u64),
}

#[derive(Clone)]
struct ReplayRow {
    text: String,
    insertion_index: usize,
    script_item_index: Option<usize>,
    is_invalid: bool,
    timeline_event: Option<RowTimelineEvent>,
}

fn request_start_id<S, A, SR, AR>(event: &Event<S, A, SR, AR>) -> Option<u64> {
    match event {
        Event::CreateSync { id, .. } | Event::CreateAsync { id, .. } => Some(*id),
        _ => None,
    }
}

fn request_end_id<S, A, SR, AR>(event: &Event<S, A, SR, AR>) -> Option<u64> {
    match event {
        Event::ReturnSync { id, .. }
        | Event::ResolveAsync { id, .. }
        | Event::AbortAsync { id }
        | Event::CancelAsync { id } => Some(*id),
        _ => None,
    }
}

fn build_timeline(rows: Vec<ReplayRow>) -> Vec<VisibleRow> {
    let mut lane_by_request = std::collections::BTreeMap::<u64, usize>::new();
    let mut result = Vec::with_capacity(rows.len());

    for row in rows {
        if row.is_invalid {
            result.push(VisibleRow {
                timeline: String::new(),
                text: row.text,
                insertion_index: row.insertion_index,
                script_item_index: row.script_item_index,
                is_invalid: true,
            });
            continue;
        }

        let mut active = lane_by_request.values().copied().collect::<Vec<_>>();
        active.sort_unstable();
        active.dedup();

        let marker_lane = match row.timeline_event.as_ref() {
            Some(RowTimelineEvent::Start(id)) => {
                let mut lane = 0;
                while active.contains(&lane) {
                    lane += 1;
                }
                Some((lane, true, *id))
            }
            Some(RowTimelineEvent::End(id)) => lane_by_request
                .get(id)
                .copied()
                .map(|lane| (lane, false, *id)),
            None => None,
        };

        let max_lane = active
            .iter()
            .copied()
            .chain(marker_lane.iter().map(|(lane, _, _)| *lane))
            .max();
        let timeline = if let Some(max_lane) = max_lane {
            let mut chars = Vec::new();
            for lane in 0..=max_lane {
                let ch = if let Some((marker_lane, is_start, _)) = marker_lane.as_ref() {
                    if *marker_lane == lane {
                        if *is_start {
                            '┌'
                        } else {
                            '└'
                        }
                    } else if active.contains(&lane) {
                        '│'
                    } else {
                        ' '
                    }
                } else if active.contains(&lane) {
                    '│'
                } else {
                    ' '
                };
                chars.push(ch);
            }
            chars.into_iter().collect::<String>()
        } else {
            String::new()
        };

        if let Some((lane, is_start, id)) = marker_lane {
            if is_start {
                lane_by_request.insert(id, lane);
            } else {
                lane_by_request.remove(&id);
            }
        }

        result.push(VisibleRow {
            timeline,
            text: row.text,
            insertion_index: row.insertion_index,
            script_item_index: row.script_item_index,
            is_invalid: row.is_invalid,
        });
    }

    result
}

pub fn render_trace<R: TraceRuntime>(
    runtime: &R,
    trace: &[RuntimeTraceItem<R>],
) -> Result<RenderedTrace, String> {
    let mut replay_state = runtime.new_replay_state();
    let bundle = runtime.new_replay_bundle(&mut replay_state);
    let (mut wrapper, initial_outbound) = NewRunWrapper::new(bundle).start();
    runtime.record_runtime_outbound(&mut replay_state, &initial_outbound);
    let mut rows = initial_outbound
        .iter()
        .map(|event| ReplayRow {
            text: format!("OUT {}", runtime.format_runtime_event(event)),
            insertion_index: 0,
            script_item_index: None,
            is_invalid: false,
            timeline_event: request_start_id(event).map(RowTimelineEvent::Start),
        })
        .collect::<Vec<_>>();
    let mut replay_trace = vec![TraceStep::start(initial_outbound.clone())];
    let mut replay_error = None;

    for (index, item) in trace.iter().enumerate() {
        let action = match runtime.replay_item_action(&mut replay_state, item) {
            Ok(action) => action,
            Err(err) => {
                replay_error = Some(err);
                rows.push(ReplayRow {
                    text: runtime.format_trace_item(item),
                    insertion_index: index + 1,
                    script_item_index: Some(index),
                    is_invalid: true,
                    timeline_event: None,
                });
                for (tail_index, tail_item) in trace.iter().enumerate().skip(index + 1) {
                    rows.push(ReplayRow {
                        text: runtime.format_trace_item(tail_item),
                        insertion_index: tail_index + 1,
                        script_item_index: Some(tail_index),
                        is_invalid: true,
                        timeline_event: None,
                    });
                }
                break;
            }
        };

        let ReplayItemAction::PushInbound(event) = action else {
            continue;
        };

        let possible = crate::possible_next_events::<_, _, _, _, R::ReplaySpec>(&replay_trace)
            .map_err(|err| format!("failed to compute possible events: {err:?}"))?;
        if !possible
            .iter()
            .any(|candidate| runtime.matches_possible_event(&replay_state, candidate, &event))
        {
            replay_error = Some(format!(
                "saved inbound item is not valid at index {index}: {}",
                runtime.format_trace_item(item)
            ));
            rows.push(ReplayRow {
                text: runtime.format_trace_item(item),
                insertion_index: index + 1,
                script_item_index: Some(index),
                is_invalid: true,
                timeline_event: None,
            });
            for (tail_index, tail_item) in trace.iter().enumerate().skip(index + 1) {
                rows.push(ReplayRow {
                    text: runtime.format_trace_item(tail_item),
                    insertion_index: tail_index + 1,
                    script_item_index: Some(tail_index),
                    is_invalid: true,
                    timeline_event: None,
                });
            }
            break;
        }

        rows.push(ReplayRow {
            text: runtime.format_trace_item(item),
            insertion_index: index + 1,
            script_item_index: Some(index),
            is_invalid: false,
            timeline_event: request_start_id(&event)
                .map(RowTimelineEvent::Start)
                .or_else(|| request_end_id(&event).map(RowTimelineEvent::End)),
        });
        let outbound = wrapper.push(event.clone());
        runtime.record_runtime_outbound(&mut replay_state, &outbound);
        replay_trace.push(TraceStep::push(event, outbound.clone()));
        rows.extend(outbound.iter().map(|event| {
            ReplayRow {
                text: format!("OUT {}", runtime.format_runtime_event(event)),
                insertion_index: index + 1,
                script_item_index: None,
                is_invalid: false,
                timeline_event: request_start_id(event)
                    .map(RowTimelineEvent::Start)
                    .or_else(|| request_end_id(event).map(RowTimelineEvent::End)),
            }
        }));
        if wrapper.is_terminated() {
            if let Some(marker) = runtime.replay_terminated_marker(&replay_state) {
                rows.push(ReplayRow {
                    text: marker,
                    insertion_index: index + 1,
                    script_item_index: None,
                    is_invalid: false,
                    timeline_event: None,
                });
            }
        }
    }

    Ok(RenderedTrace {
        rows: build_timeline(rows),
        replay_error,
    })
}

pub(crate) fn snapshot_for<R: TraceRuntime>(
    state: &AppState<RuntimeTraceItem<R>>,
    runtime: &R,
) -> Result<ViewSnapshot, String> {
    let rendered = render_trace(runtime, &state.view.trace)?;
    let steps = build_steps(&rendered.rows, state.view.trace.len());
    let trivial_preview = preview_insertion_step(&steps, &rendered.rows, state)
        .and_then(|step_index| {
            let insertion_index =
                insertion_index_for_append(&steps, step_index, state.view.trace.len());
            let (trace, inserted_count, has_more) = build_trivial_chain_trace(
                runtime,
                &state.view.trace,
                insertion_index,
                Some(TRIVIAL_PREVIEW_LIMIT),
            )
            .ok()?;
            if inserted_count == 0 {
                return Some(Vec::new());
            }
            let mut preview = render_trace(runtime, &trace).ok()?.rows;
            preview.drain(..rendered.rows.len());
            if has_more {
                preview.push(VisibleRow {
                    timeline: String::new(),
                    text: "... more trivial events".to_string(),
                    insertion_index: trace.len(),
                    script_item_index: None,
                    is_invalid: false,
                });
            }
            Some(preview)
        })
        .unwrap_or_default();

    Ok(ViewSnapshot {
        rows: rendered.rows,
        replay_error: rendered.replay_error,
        steps,
        trivial_preview,
    })
}

fn clamp_cursor_step<T>(state: &mut AppState<T>, snapshot: &ViewSnapshot) {
    state.view.cursor_step_index = state
        .view
        .cursor_step_index
        .min(snapshot.steps.len().saturating_sub(1));
    if let Some(anchor) = state.view.selection_anchor_step_index {
        state.view.selection_anchor_step_index =
            Some(anchor.min(snapshot.steps.len().saturating_sub(1)));
    }
}

fn selected_step_range<T>(state: &AppState<T>) -> (usize, usize) {
    match state.view.selection_anchor_step_index {
        Some(anchor) => (
            anchor.min(state.view.cursor_step_index),
            anchor.max(state.view.cursor_step_index),
        ),
        None => (state.view.cursor_step_index, state.view.cursor_step_index),
    }
}

fn keep_cursor_visible<T>(state: &mut AppState<T>, snapshot: &ViewSnapshot) {
    if snapshot.steps.is_empty() {
        state.view.scroll_offset = 0;
        return;
    }
    let cursor_row = snapshot.steps[state.view.cursor_step_index].start_row;
    let viewport_height = trace_viewport_height(state);
    if viewport_height == 0 {
        return;
    }
    if cursor_row < state.view.scroll_offset {
        state.view.scroll_offset = cursor_row;
        return;
    }
    let last_visible = state
        .view
        .scroll_offset
        .saturating_add(viewport_height.saturating_sub(1));
    if cursor_row > last_visible {
        state.view.scroll_offset = cursor_row.saturating_sub(viewport_height.saturating_sub(1));
    }
}

fn insertion_index_for_append(steps: &[StepInfo], step_index: usize, trace_len: usize) -> usize {
    if steps.is_empty() {
        return 0;
    }
    let current = step_index.min(steps.len() - 1);
    let next_index = current + 1;
    if next_index < steps.len() && steps[next_index].outbound_only {
        if next_index + 1 < steps.len() {
            steps[next_index + 1].insertion_index
        } else {
            trace_len
        }
    } else if next_index < steps.len() {
        steps[next_index].insertion_index
    } else {
        trace_len
    }
}

fn preview_insertion_step<T>(
    steps: &[StepInfo],
    rows: &[VisibleRow],
    state: &AppState<T>,
) -> Option<usize> {
    let last_valid_row = rows.iter().rposition(|row| !row.is_invalid)?;
    let last_valid_step = steps
        .iter()
        .rposition(|step| step.start_row <= last_valid_row && last_valid_row <= step.end_row)?;
    (state.view.cursor_step_index == last_valid_step).then_some(last_valid_step)
}

fn resolve_runtime_target<T>(
    state: &AppState<T>,
    snapshot: &ViewSnapshot,
    target: &DialogTarget,
) -> Result<RuntimeTarget, String> {
    match target {
        DialogTarget::InsertAfterStep { step_index } => Ok(RuntimeTarget::Insert {
            insertion_index: insertion_index_for_append(
                &snapshot.steps,
                *step_index,
                state.view.trace.len(),
            ),
        }),
        DialogTarget::EditInboundOfStep { step_index } => snapshot
            .steps
            .get(*step_index)
            .and_then(|step| step.inbound_item_index)
            .map(|item_index| RuntimeTarget::Edit { item_index })
            .ok_or_else(|| "current step has no editable inbound event".to_string()),
    }
}

fn selected_script_item_indices(
    snapshot: &ViewSnapshot,
    start_step: usize,
    end_step: usize,
) -> Vec<usize> {
    let mut indices = snapshot.steps[start_step..=end_step]
        .iter()
        .filter_map(|step| step.inbound_item_index)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn delete_trace_items<T>(trace: &mut Vec<T>, item_indices: Vec<usize>) -> Result<(), String> {
    let mut item_indices = item_indices;
    item_indices.sort_unstable();
    item_indices.dedup();
    if let Some(index) = item_indices
        .iter()
        .copied()
        .find(|index| *index >= trace.len())
    {
        return Err(format!("invalid item index {index}"));
    }
    for index in item_indices.into_iter().rev() {
        trace.remove(index);
    }
    Ok(())
}

fn build_trivial_chain_trace<R: TraceRuntime>(
    runtime: &R,
    trace: &[RuntimeTraceItem<R>],
    insertion_index: usize,
    preview_limit: Option<usize>,
) -> Result<(Vec<RuntimeTraceItem<R>>, usize, bool), String> {
    let mut trace = trace.to_vec();
    let mut next_insertion_index = insertion_index;
    let mut inserted_count = 0;
    let mut reached_limit_with_more = false;
    for _ in 0..512 {
        if preview_limit.is_some_and(|limit| inserted_count >= limit) {
            let choices = runtime.insertion_choices(&trace[..next_insertion_index])?;
            let target = RuntimeTarget::Insert {
                insertion_index: next_insertion_index,
            };
            let mut complete_choices = 0;
            for (choice_index, _) in choices.iter().enumerate() {
                let spec = runtime.form_spec(&trace, &target, choice_index)?;
                let state = runtime.initial_form_state(&trace, &target, choice_index)?;
                if form_is_auto_acceptable(&spec, &state) {
                    complete_choices += 1;
                    if complete_choices > 1 {
                        break;
                    }
                }
            }
            reached_limit_with_more = complete_choices == 1;
            break;
        }
        let choices = runtime.insertion_choices(&trace[..next_insertion_index])?;
        let target = RuntimeTarget::Insert {
            insertion_index: next_insertion_index,
        };
        let mut trivial_choice = None;
        for (choice_index, _) in choices.iter().enumerate() {
            let spec = runtime.form_spec(&trace, &target, choice_index)?;
            let state = runtime.initial_form_state(&trace, &target, choice_index)?;
            if form_is_auto_acceptable(&spec, &state) {
                if trivial_choice.is_some() {
                    trivial_choice = None;
                    break;
                }
                trivial_choice = Some((choice_index, state));
            }
        }
        let Some((choice_index, state)) = trivial_choice else {
            break;
        };
        let items = runtime.encode_form_state(&trace, &target, choice_index, &state)?;
        if items.is_empty() {
            break;
        }
        let item_count = items.len();
        runtime.apply_form(&mut trace, &target, items)?;
        next_insertion_index += item_count;
        inserted_count += item_count;
    }
    Ok((trace, inserted_count, reached_limit_with_more))
}

fn open_form_dialog<R: TraceRuntime>(
    state: &AppState<RuntimeTraceItem<R>>,
    runtime: &R,
    snapshot: &ViewSnapshot,
    target: DialogTarget,
    choice_index: usize,
) -> Result<TraceViewDialog, String> {
    let runtime_target = resolve_runtime_target(state, snapshot, &target)?;
    let spec = runtime.form_spec(&state.view.trace, &runtime_target, choice_index)?;
    let form_state =
        runtime.initial_form_state(&state.view.trace, &runtime_target, choice_index)?;
    Ok(TraceViewDialog::Form {
        target,
        choice_index,
        spec,
        state: form_state,
        selected_field: 0,
    })
}

fn set_status<T>(state: &mut AppState<T>, message: impl Into<String>) {
    state.view.status = Some(message.into());
}

pub fn update<R: TraceRuntime>(
    mut state: AppState<RuntimeTraceItem<R>>,
    command: Command,
    runtime: &R,
) -> (
    AppState<RuntimeTraceItem<R>>,
    Vec<Effect<RuntimeTraceItem<R>>>,
) {
    if !matches!(command, Command::Char('z')) {
        state.view.last_char = None;
    }
    let snapshot = match snapshot_for(&state, runtime) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            set_status(&mut state, err);
            return (state, Vec::new());
        }
    };
    clamp_cursor_step(&mut state, &snapshot);
    match command {
        Command::Resize { width, height } => {
            state.view.terminal_width = width;
            state.view.terminal_height = height;
            state.view.last_char = None;
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MoveUp => {
            match &mut state.view.dialog {
                TraceViewDialog::Choice { selected, .. } => {
                    *selected = selected.saturating_sub(1);
                }
                TraceViewDialog::Form { .. } | TraceViewDialog::None => {
                    state.view.cursor_step_index = state.view.cursor_step_index.saturating_sub(1);
                    keep_cursor_visible(&mut state, &snapshot);
                }
            }
            (state, Vec::new())
        }
        Command::MoveDown => {
            match &mut state.view.dialog {
                TraceViewDialog::Choice {
                    selected, choices, ..
                } => {
                    if *selected + 1 < choices.len() {
                        *selected += 1;
                    }
                }
                TraceViewDialog::Form { .. } | TraceViewDialog::None => {
                    if state.view.cursor_step_index + 1 < snapshot.steps.len() {
                        state.view.cursor_step_index += 1;
                    }
                    keep_cursor_visible(&mut state, &snapshot);
                }
            }
            (state, Vec::new())
        }
        Command::MoveTop => {
            state.view.cursor_step_index = 0;
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MoveBottom => {
            state.view.cursor_step_index = snapshot.steps.len().saturating_sub(1);
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MovePageUp => {
            let delta = trace_viewport_height(&state).max(1);
            state.view.cursor_step_index = state.view.cursor_step_index.saturating_sub(delta);
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MovePageDown => {
            let delta = trace_viewport_height(&state).max(1);
            state.view.cursor_step_index =
                (state.view.cursor_step_index + delta).min(snapshot.steps.len().saturating_sub(1));
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MoveHalfPageUp => {
            let delta = (trace_viewport_height(&state).max(2) / 2).max(1);
            state.view.cursor_step_index = state.view.cursor_step_index.saturating_sub(delta);
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::MoveHalfPageDown => {
            let delta = (trace_viewport_height(&state).max(2) / 2).max(1);
            state.view.cursor_step_index =
                (state.view.cursor_step_index + delta).min(snapshot.steps.len().saturating_sub(1));
            keep_cursor_visible(&mut state, &snapshot);
            (state, Vec::new())
        }
        Command::CenterCursor => {
            if let Some(step) = snapshot.steps.get(state.view.cursor_step_index) {
                let viewport_height = trace_viewport_height(&state);
                state.view.scroll_offset = step
                    .start_row
                    .saturating_sub(viewport_height.saturating_sub(1) / 2);
            }
            (state, Vec::new())
        }
        Command::StartInsert => {
            let target = DialogTarget::InsertAfterStep {
                step_index: state.view.cursor_step_index,
            };
            let runtime_target = match resolve_runtime_target(&state, &snapshot, &target) {
                Ok(target) => target,
                Err(err) => {
                    set_status(&mut state, err);
                    return (state, Vec::new());
                }
            };
            match runtime.insertion_choices(match runtime_target {
                RuntimeTarget::Insert { insertion_index } => &state.view.trace[..insertion_index],
                RuntimeTarget::Edit { .. } => unreachable!(),
            }) {
                Ok(choices) => {
                    state.view.dialog = TraceViewDialog::Choice {
                        target,
                        choices,
                        selected: 0,
                    };
                    (state, Vec::new())
                }
                Err(err) => {
                    set_status(&mut state, err);
                    (state, Vec::new())
                }
            }
        }
        Command::StartEdit => {
            let target = DialogTarget::EditInboundOfStep {
                step_index: state.view.cursor_step_index,
            };
            let runtime_target = match resolve_runtime_target(&state, &snapshot, &target) {
                Ok(target) => target,
                Err(err) => {
                    set_status(&mut state, err);
                    return (state, Vec::new());
                }
            };
            match runtime.edit_choices(
                &state.view.trace,
                match runtime_target {
                    RuntimeTarget::Edit { item_index } => item_index,
                    RuntimeTarget::Insert { .. } => unreachable!(),
                },
            ) {
                Ok(choices) => {
                    state.view.dialog = TraceViewDialog::Choice {
                        target,
                        choices,
                        selected: 0,
                    };
                    (state, Vec::new())
                }
                Err(err) => {
                    set_status(&mut state, err);
                    (state, Vec::new())
                }
            }
        }
        Command::DialogCancel => {
            state.view.dialog = TraceViewDialog::None;
            (state, Vec::new())
        }
        Command::DialogConfirm => match &state.view.dialog {
            TraceViewDialog::Choice {
                target, selected, ..
            } => match open_form_dialog(&state, runtime, &snapshot, target.clone(), *selected) {
                Ok(dialog) => {
                    state.view.dialog = dialog;
                    (state, Vec::new())
                }
                Err(err) => {
                    set_status(&mut state, err);
                    (state, Vec::new())
                }
            },
            TraceViewDialog::Form { .. } | TraceViewDialog::None => (state, Vec::new()),
        },
        Command::FormCancel
        | Command::FormMoveUp
        | Command::FormMoveDown
        | Command::FormSelectPrev
        | Command::FormSelectNext
        | Command::FormBackspace
        | Command::FormInsertChar(_)
        | Command::FormInsertNewline
        | Command::FormSubmit => {
            let TraceViewDialog::Form {
                target,
                choice_index,
                spec,
                state: form_state,
                selected_field,
            } = &mut state.view.dialog
            else {
                return (state, Vec::new());
            };
            match command {
                Command::FormCancel => {
                    state.view.dialog = TraceViewDialog::None;
                    (state, Vec::new())
                }
                Command::FormMoveUp => {
                    *selected_field = selected_field.saturating_sub(1);
                    (state, Vec::new())
                }
                Command::FormMoveDown => {
                    if *selected_field + 1 < spec.fields.len() {
                        *selected_field += 1;
                    }
                    (state, Vec::new())
                }
                Command::FormSelectPrev => {
                    if let Some(field) = spec.fields.get(*selected_field) {
                        match form_value_for_field(form_state, field) {
                            super::FormValue::Select(value) => {
                                *value = value.saturating_sub(1);
                            }
                            super::FormValue::Toggle(value) => *value = false,
                            super::FormValue::Text(_) => {}
                        }
                    }
                    (state, Vec::new())
                }
                Command::FormSelectNext => {
                    if let Some(field) = spec.fields.get(*selected_field) {
                        match form_value_for_field(form_state, field) {
                            super::FormValue::Select(value) => {
                                if let FormFieldKind::Select { options } = &field.kind {
                                    *value = (*value + 1).min(options.len().saturating_sub(1));
                                }
                            }
                            super::FormValue::Toggle(value) => *value = true,
                            super::FormValue::Text(_) => {}
                        }
                    }
                    (state, Vec::new())
                }
                Command::FormBackspace => {
                    if let Some(field) = spec.fields.get(*selected_field) {
                        if let super::FormValue::Text(value) =
                            form_value_for_field(form_state, field)
                        {
                            value.pop();
                        }
                    }
                    (state, Vec::new())
                }
                Command::FormInsertNewline => {
                    if let Some(field) = spec.fields.get(*selected_field) {
                        if matches!(field.kind, FormFieldKind::Text { multiline: true }) {
                            if let super::FormValue::Text(value) =
                                form_value_for_field(form_state, field)
                            {
                                value.push('\n');
                            }
                        }
                    }
                    (state, Vec::new())
                }
                Command::FormSubmit => {
                    let target = target.clone();
                    let choice_index = *choice_index;
                    let spec = spec.clone();
                    let form_state = form_state.clone();
                    if let Some(missing) = missing_form_fields(&spec, &form_state).first() {
                        set_status(&mut state, format!("missing {missing}"));
                        return (state, Vec::new());
                    }
                    let runtime_target = match resolve_runtime_target(&state, &snapshot, &target) {
                        Ok(target) => target,
                        Err(err) => {
                            set_status(&mut state, err);
                            return (state, Vec::new());
                        }
                    };
                    let items = match runtime.encode_form_state(
                        &state.view.trace,
                        &runtime_target,
                        choice_index,
                        &form_state,
                    ) {
                        Ok(items) => items,
                        Err(err) => {
                            set_status(&mut state, err);
                            return (state, Vec::new());
                        }
                    };
                    match runtime.apply_form(&mut state.view.trace, &runtime_target, items) {
                        Ok(()) => {
                            state.view.dialog = TraceViewDialog::None;
                            match &target {
                                DialogTarget::InsertAfterStep { step_index } => {
                                    state.view.cursor_step_index = *step_index + 1;
                                }
                                DialogTarget::EditInboundOfStep { step_index } => {
                                    state.view.cursor_step_index = *step_index;
                                }
                            }
                            let trace = state.view.trace.clone();
                            (state, vec![Effect::SaveTrace { trace }])
                        }
                        Err(err) => {
                            set_status(&mut state, err);
                            (state, Vec::new())
                        }
                    }
                }
                Command::FormInsertChar(ch) => {
                    if let Some(field) = spec.fields.get(*selected_field) {
                        if let super::FormValue::Text(value) =
                            form_value_for_field(form_state, field)
                        {
                            value.push(ch);
                        }
                    }
                    (state, Vec::new())
                }
                _ => unreachable!(),
            }
        }
        Command::DeleteCurrent => {
            let (start, end) = selected_step_range(&state);
            let indices = selected_script_item_indices(&snapshot, start, end);
            if indices.is_empty() {
                set_status(&mut state, "no editable inbound events selected");
                return (state, Vec::new());
            }
            match delete_trace_items(&mut state.view.trace, indices) {
                Ok(()) => {
                    state.view.selection_anchor_step_index = None;
                    state.view.cursor_step_index = start.min(state.view.cursor_step_index);
                    let trace = state.view.trace.clone();
                    (state, vec![Effect::SaveTrace { trace }])
                }
                Err(err) => {
                    set_status(&mut state, err);
                    (state, Vec::new())
                }
            }
        }
        Command::AcceptTrivialChain => {
            let Some(step_index) = preview_insertion_step(&snapshot.steps, &snapshot.rows, &state)
            else {
                return (state, Vec::new());
            };
            let insertion_index =
                insertion_index_for_append(&snapshot.steps, step_index, state.view.trace.len());
            match build_trivial_chain_trace(runtime, &state.view.trace, insertion_index, None) {
                Ok((trace, inserted_count, _)) if inserted_count > 0 => {
                    state.view.trace = trace;
                    state.view.cursor_step_index = snapshot.steps.len();
                    let trace = state.view.trace.clone();
                    (state, vec![Effect::SaveTrace { trace }])
                }
                Ok(_) => (state, Vec::new()),
                Err(err) => {
                    set_status(&mut state, err);
                    (state, Vec::new())
                }
            }
        }
        Command::ToggleVisual => {
            if state.view.selection_anchor_step_index.is_some() {
                state.view.selection_anchor_step_index = None;
            } else {
                state.view.selection_anchor_step_index = Some(state.view.cursor_step_index);
            }
            (state, Vec::new())
        }
        Command::Char('z') => {
            if state.view.last_char == Some('z') {
                state.view.last_char = None;
                if let Some(step) = snapshot.steps.get(state.view.cursor_step_index) {
                    let viewport_height = trace_viewport_height(&state);
                    state.view.scroll_offset = step
                        .start_row
                        .saturating_sub(viewport_height.saturating_sub(1) / 2);
                }
            } else {
                state.view.last_char = Some('z');
                state.view.status = None;
            }
            (state, Vec::new())
        }
        Command::Char(_) => (state, Vec::new()),
        Command::ClearStatus => {
            state.view.status = None;
            (state, Vec::new())
        }
        Command::Quit => (state, vec![Effect::Quit]),
    }
}

fn form_value_for_field<'a>(
    state: &'a mut FormState,
    field: &super::FormField,
) -> &'a mut super::FormValue {
    state
        .entry(field.id.clone())
        .or_insert_with(|| match &field.kind {
            FormFieldKind::Text { .. } => super::FormValue::Text(String::new()),
            FormFieldKind::Select { .. } => super::FormValue::Select(0),
            FormFieldKind::Toggle { .. } => super::FormValue::Toggle(false),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{AsyncTiming, SimDriver};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestSyncOp {
        BootReason,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestAsyncOp {
        Sleep,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestSyncResult {
        Unit,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestSyncError {
        Unit,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    enum TestAsyncResult {
        Unit,
    }

    struct TestBundle;

    impl SimBundle for TestBundle {
        type SyncOp = TestSyncOp;
        type AsyncOp = TestAsyncOp;
        type SyncResult = TestSyncResult;
        type AsyncResult = TestAsyncResult;
        type RunFuture = std::future::Ready<()>;

        fn build(
            self,
            driver: SimDriver<Self::SyncOp, Self::AsyncOp, Self::SyncResult, Self::AsyncResult>,
        ) -> Self::RunFuture {
            let _ = driver.create_sync(TestSyncOp::BootReason);
            std::future::ready(())
        }

        fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool {
            matches!((op, result), (TestSyncOp::BootReason, TestSyncResult::Unit))
        }

        fn async_result_matches(_op: &Self::AsyncOp, _result: &Self::AsyncResult) -> bool {
            true
        }
    }

    struct TestSpec;

    impl NextEventsSpec<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> for TestSpec {
        type InboundAsyncKind = ();

        fn sync_result_matches(op: &TestSyncOp, result: &TestSyncResult) -> bool {
            TestBundle::sync_result_matches(op, result)
        }

        fn async_result_matches(op: &TestAsyncOp, result: &TestAsyncResult) -> bool {
            TestBundle::async_result_matches(op, result)
        }

        fn async_timing(_op: &TestAsyncOp) -> AsyncTiming {
            AsyncTiming::Untimed
        }
    }

    struct TestRuntime;

    impl TraceRuntime for TestRuntime {
        type SyncOp = TestSyncOp;
        type AsyncOp = TestAsyncOp;
        type SyncResult = TestSyncResult;
        type SyncError = TestSyncError;
        type AsyncResult = TestAsyncResult;
        type ReplaySyncOp = TestSyncOp;
        type ReplayAsyncOp = TestAsyncOp;
        type ReplaySyncResult = TestSyncResult;
        type ReplayAsyncResult = TestAsyncResult;
        type Bundle = TestBundle;
        type ReplaySpec = TestSpec;
        type ReplayState = ();

        fn insertion_choices(
            &self,
            _trace_prefix: &[RuntimeTraceItem<Self>],
        ) -> Result<Vec<InsertionChoice>, String> {
            Ok(vec![
                InsertionChoice {
                    label: "ReturnSync#0 BootReason".into(),
                },
                InsertionChoice {
                    label: "ReturnSync#1 DisplayInit".into(),
                },
            ])
        }

        fn edit_choices(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _item_index: usize,
        ) -> Result<Vec<InsertionChoice>, String> {
            Ok(vec![InsertionChoice {
                label: "ReturnSync#0 BootReason".into(),
            }])
        }

        fn form_spec(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _target: &RuntimeTarget,
            _choice_index: usize,
        ) -> Result<super::super::FormSpec, String> {
            Ok(super::super::FormSpec {
                title: "Edit".into(),
                details: Vec::new(),
                fields: Vec::new(),
                auto_accept_if_complete: true,
            })
        }

        fn initial_form_state(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _target: &RuntimeTarget,
            _choice_index: usize,
        ) -> Result<FormState, String> {
            Ok(FormState::new())
        }

        fn encode_form_state(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _target: &RuntimeTarget,
            _choice_index: usize,
            _state: &FormState,
        ) -> Result<Vec<RuntimeTraceItem<Self>>, String> {
            Ok(vec![TraceItem::InboundReturnSync {
                target: "0".into(),
                result: TestSyncResult::Unit,
            }])
        }

        fn apply_form(
            &self,
            trace: &mut Vec<RuntimeTraceItem<Self>>,
            target: &RuntimeTarget,
            items: Vec<RuntimeTraceItem<Self>>,
        ) -> Result<(), String> {
            match target {
                RuntimeTarget::Insert { insertion_index } => {
                    for (offset, item) in items.into_iter().enumerate() {
                        trace.insert(insertion_index + offset, item);
                    }
                }
                RuntimeTarget::Edit { item_index } => {
                    trace.splice(*item_index..=*item_index, items);
                }
            }
            Ok(())
        }

        fn new_replay_state(&self) -> Self::ReplayState {}

        fn new_replay_bundle(&self, _replay_state: &mut Self::ReplayState) -> Self::Bundle {
            TestBundle
        }

        fn record_runtime_outbound(
            &self,
            _replay_state: &mut Self::ReplayState,
            _events: &[Event<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                Self::ReplaySyncResult,
                Self::ReplayAsyncResult,
            >],
        ) {
        }

        fn replay_item_action(
            &self,
            _replay_state: &mut Self::ReplayState,
            item: &RuntimeTraceItem<Self>,
        ) -> Result<
            ReplayItemAction<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                Self::ReplaySyncResult,
                Self::ReplayAsyncResult,
            >,
            String,
        > {
            match item {
                TraceItem::InboundReturnSync { .. } => {
                    Ok(ReplayItemAction::PushInbound(Event::ReturnSync {
                        id: 0,
                        result: TestSyncResult::Unit,
                    }))
                }
                _ => Err("unsupported test item".to_string()),
            }
        }

        fn matches_possible_event(
            &self,
            _replay_state: &Self::ReplayState,
            candidate: &PossibleEvent<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                <Self::ReplaySpec as NextEventsSpec<
                    Self::ReplaySyncOp,
                    Self::ReplayAsyncOp,
                    Self::ReplaySyncResult,
                    Self::ReplayAsyncResult,
                >>::InboundAsyncKind,
            >,
            event: &Event<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                Self::ReplaySyncResult,
                Self::ReplayAsyncResult,
            >,
        ) -> bool {
            matches!(
                (candidate, event),
                (
                    PossibleEvent::ReturnSync {
                        id: 0,
                        op: TestSyncOp::BootReason,
                    },
                    Event::ReturnSync {
                        id: 0,
                        result: TestSyncResult::Unit,
                    }
                )
            )
        }

        fn format_trace_item(&self, _item: &RuntimeTraceItem<Self>) -> String {
            "row".to_string()
        }

        fn format_runtime_event(
            &self,
            event: &Event<
                Self::ReplaySyncOp,
                Self::ReplayAsyncOp,
                Self::ReplaySyncResult,
                Self::ReplayAsyncResult,
            >,
        ) -> String {
            match event {
                Event::CreateSync { .. } => "CreateSync BootReason".to_string(),
                Event::ReturnSync { .. } => "ReturnSync Unit".to_string(),
                Event::CreateAsync { .. } => "CreateAsync Sleep".to_string(),
                Event::ResolveAsync { .. } => "ResolveAsync Unit".to_string(),
                Event::CancelAsync { .. } => "CancelAsync".to_string(),
                Event::AbortAsync { .. } => "AbortAsync".to_string(),
            }
        }
    }

    fn temp_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("simulator-editor-{unique}.json"));
        path
    }

    #[test]
    fn open_or_create_trace_creates_missing_trace_file() {
        let path = temp_path();
        let session = open_or_create_trace::<TestRuntime>(&path, 120, 40).unwrap();
        assert!(path.exists());
        assert_eq!(session.state.view.trace.len(), 0);
    }

    #[test]
    fn resize_updates_terminal_size() {
        let state = AppState::new(Vec::<RuntimeTraceItem<TestRuntime>>::new(), 80, 24);
        let (state, effects) = update(
            state,
            Command::Resize {
                width: 120,
                height: 40,
            },
            &TestRuntime,
        );
        assert!(effects.is_empty());
        assert_eq!(state.view.terminal_width, 120);
        assert_eq!(state.view.terminal_height, 40);
    }

    #[test]
    fn quit_returns_quit_effect() {
        let state = AppState::new(Vec::<RuntimeTraceItem<TestRuntime>>::new(), 80, 24);
        let (_, effects) = update(state, Command::Quit, &TestRuntime);
        assert_eq!(effects, vec![Effect::Quit]);
    }

    #[test]
    fn move_down_in_choice_dialog_changes_dialog_selection_not_trace_cursor() {
        let state = AppState::new(Vec::<RuntimeTraceItem<TestRuntime>>::new(), 80, 24);
        let (state, effects) = update(state, Command::StartInsert, &TestRuntime);
        assert!(effects.is_empty());

        match &state.view.dialog {
            TraceViewDialog::Choice {
                choices, selected, ..
            } => {
                assert_eq!(choices.len(), 2);
                assert_eq!(*selected, 0);
            }
            dialog => panic!("expected choice dialog, got {dialog:?}"),
        }

        let initial_cursor_step_index = state.view.cursor_step_index;
        let (state, effects) = update(state, Command::MoveDown, &TestRuntime);
        assert!(effects.is_empty());
        assert_eq!(state.view.cursor_step_index, initial_cursor_step_index);
        assert!(matches!(
            state.view.dialog,
            TraceViewDialog::Choice { selected: 1, .. }
        ));
    }
}
