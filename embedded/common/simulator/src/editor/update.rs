use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{Event, NewRunWrapper, NextEventsSpec, PossibleEvent, SimBundle, TraceStep, Warning};

use super::{
    form_is_auto_acceptable, form_state_from_spec, missing_form_fields, AppState, Command,
    DialogTarget, Effect, EncodedTraceItem, FormFieldKind, FormSpec, FormState, InsertionChoice,
    RenderedTrace, RunEnvelope, RuntimeTarget, TraceItem, TraceViewDialog, VisibleRow,
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

type RuntimeSyncOpOf<R> = <<R as TraceRuntime>::Bundle as SimBundle>::SyncOp;
type RuntimeAsyncOpOf<R> = <<R as TraceRuntime>::Bundle as SimBundle>::AsyncOp;
type RuntimeSyncResultOf<R> = <<R as TraceRuntime>::Bundle as SimBundle>::SyncResult;
type RuntimeAsyncResultOf<R> = <<R as TraceRuntime>::Bundle as SimBundle>::AsyncResult;
type RuntimeEventOf<R> =
    Event<RuntimeSyncOpOf<R>, RuntimeAsyncOpOf<R>, RuntimeSyncResultOf<R>, RuntimeAsyncResultOf<R>>;
type RuntimeInboundAsyncKindOf<R> = <<R as TraceRuntime>::ReplaySpec as NextEventsSpec<
    RuntimeSyncOpOf<R>,
    RuntimeAsyncOpOf<R>,
    RuntimeSyncResultOf<R>,
    RuntimeAsyncResultOf<R>,
>>::InboundAsyncKind;
type RuntimePossibleEventOf<R> =
    PossibleEvent<RuntimeSyncOpOf<R>, RuntimeAsyncOpOf<R>, RuntimeInboundAsyncKindOf<R>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorChoice<S, A, K> {
    ReturnSyncSuccess {
        target: String,
        op: S,
        include_outbound: bool,
    },
    ReturnSyncError {
        target: String,
        op: S,
        include_outbound: bool,
    },
    ResolveAsync {
        target: String,
        op: A,
        include_outbound: bool,
        warnings: Vec<Warning>,
    },
    AbortAsync {
        target: String,
        op: A,
        include_outbound: bool,
    },
    CreateInboundAsync {
        id: String,
        kind: K,
    },
    CancelInboundAsync {
        target: String,
        op: A,
    },
    DropResult {
        target: String,
        outbound: bool,
    },
}

type RuntimeEditorChoiceOf<R> =
    EditorChoice<RuntimeSyncOpOf<R>, RuntimeAsyncOpOf<R>, RuntimeInboundAsyncKindOf<R>>;

pub trait TraceRuntime {
    type SyncOp: Clone + PartialEq + Eq;
    type AsyncOp: Clone + PartialEq + Eq;
    type SyncResult: Clone + PartialEq + Eq;
    type SyncError: Clone + PartialEq + Eq;
    type AsyncResult: Clone + PartialEq + Eq;
    type Bundle: SimBundle<
        SyncOp = Self::SyncOp,
        AsyncOp = Self::AsyncOp,
        SyncResult = Self::SyncResult,
        AsyncResult = Self::AsyncResult,
    >;
    type ReplaySpec: NextEventsSpec<
        Self::SyncOp,
        Self::AsyncOp,
        Self::SyncResult,
        Self::AsyncResult,
    >;

    fn form_schema(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        target: &RuntimeTarget,
        choice: &EditorChoice<
            Self::SyncOp,
            Self::AsyncOp,
            <Self::ReplaySpec as NextEventsSpec<
                Self::SyncOp,
                Self::AsyncOp,
                Self::SyncResult,
                Self::AsyncResult,
            >>::InboundAsyncKind,
        >,
    ) -> Result<FormSpec, String>;
    fn decode_form_state(
        &self,
        trace: &[RuntimeTraceItem<Self>],
        target: &RuntimeTarget,
        choice: &EditorChoice<
            Self::SyncOp,
            Self::AsyncOp,
            <Self::ReplaySpec as NextEventsSpec<
                Self::SyncOp,
                Self::AsyncOp,
                Self::SyncResult,
                Self::AsyncResult,
            >>::InboundAsyncKind,
        >,
        state: &FormState,
    ) -> Result<Vec<RuntimeTraceItem<Self>>, String>;
    fn format_editor_choice(
        &self,
        choice: &EditorChoice<
            Self::SyncOp,
            Self::AsyncOp,
            <Self::ReplaySpec as NextEventsSpec<
                Self::SyncOp,
                Self::AsyncOp,
                Self::SyncResult,
                Self::AsyncResult,
            >>::InboundAsyncKind,
        >,
    ) -> String;
    fn default_sync_error(&self, op: &Self::SyncOp) -> Option<Self::SyncError>;
    fn new_replay_bundle(&self) -> Self::Bundle;
    fn sync_error_to_result(&self, error: &Self::SyncError) -> Self::SyncResult;
    fn inbound_async_kind(
        &self,
        op: &Self::AsyncOp,
    ) -> Option<
        <Self::ReplaySpec as NextEventsSpec<
            Self::SyncOp,
            Self::AsyncOp,
            Self::SyncResult,
            Self::AsyncResult,
        >>::InboundAsyncKind,
    >;
    fn format_trace_item(&self, item: &RuntimeTraceItem<Self>) -> String;
    fn format_runtime_event(&self, event: &RuntimeEventOf<Self>) -> String;
    fn sync_op_result_target(&self, op: &Self::SyncOp) -> Option<String>;
    fn async_op_result_target(&self, op: &Self::AsyncOp) -> Option<String>;
    fn async_op_to_json(&self, value: &Self::AsyncOp) -> Result<Value, String>;
    fn async_op_from_json(&self, value: Value) -> Result<Self::AsyncOp, String>;
    fn sync_result_to_json(&self, value: &Self::SyncResult) -> Result<Value, String>;
    fn sync_result_from_json(&self, value: Value) -> Result<Self::SyncResult, String>;
    fn sync_error_to_json(&self, value: &Self::SyncError) -> Result<Value, String>;
    fn sync_error_from_json(&self, value: Value) -> Result<Self::SyncError, String>;
    fn async_result_to_json(&self, value: &Self::AsyncResult) -> Result<Value, String>;
    fn async_result_from_json(&self, value: Value) -> Result<Self::AsyncResult, String>;
    fn sync_result_refs(&self, value: &Self::SyncResult) -> Result<Vec<String>, String> {
        collect_result_refs(&self.sync_result_to_json(value)?)
    }
    fn async_result_refs(&self, value: &Self::AsyncResult) -> Result<Vec<String>, String> {
        collect_result_refs(&self.async_result_to_json(value)?)
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingRequest<S, A> {
    Sync { id: u64, op: S },
    Async { id: u64, op: A },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BoundRequest<S, A> {
    Sync { id: u64, op: S },
    Async { id: u64, op: A },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReplayBindings<S, A> {
    pending_requests: Vec<PendingRequest<S, A>>,
    bound_requests: BTreeMap<String, BoundRequest<S, A>>,
    runtime_to_symbolic: BTreeMap<u64, String>,
    used_ids: BTreeSet<String>,
    live_result_refs: BTreeSet<String>,
    dropped_result_refs: BTreeSet<String>,
    next_inbound_async_id: u64,
}

impl<S, A> Default for ReplayBindings<S, A> {
    fn default() -> Self {
        Self {
            pending_requests: Vec::new(),
            bound_requests: BTreeMap::new(),
            runtime_to_symbolic: BTreeMap::new(),
            used_ids: BTreeSet::new(),
            live_result_refs: BTreeSet::new(),
            dropped_result_refs: BTreeSet::new(),
            next_inbound_async_id: u64::MAX,
        }
    }
}

fn collect_result_refs(value: &Value) -> Result<Vec<String>, String> {
    fn visit(value: &Value, refs: &mut Vec<String>) -> Result<(), String> {
        match value {
            Value::Array(values) => values.iter().try_for_each(|value| visit(value, refs)),
            Value::Object(map) => {
                if let Some(reference) = map.get("ref") {
                    let reference = reference
                        .as_str()
                        .ok_or_else(|| "result ref must be a string".to_string())?;
                    refs.push(reference.to_string());
                }
                map.values().try_for_each(|value| visit(value, refs))
            }
            _ => Ok(()),
        }
    }

    let mut refs = Vec::new();
    visit(value, &mut refs)?;
    Ok(refs)
}

fn validate_request_target(
    known_refs: &BTreeSet<String>,
    dropped_refs: &BTreeSet<String>,
    item_kind: &str,
    item_id: &str,
    saved_target: Option<&str>,
    runtime_target: Option<&str>,
) -> Result<(), String> {
    match (saved_target, runtime_target) {
        (Some(saved), Some(runtime)) if saved == runtime => {}
        (Some(saved), Some(runtime)) => {
            return Err(format!(
                "{item_kind} {item_id} targets {saved}, but runtime request targets {runtime}"
            ));
        }
        (Some(saved), None) => {
            return Err(format!(
                "{item_kind} {item_id} targets {saved}, but runtime request has no target"
            ));
        }
        (None, Some(runtime)) => {
            return Err(format!(
                "{item_kind} {item_id} is missing target for runtime request target {runtime}"
            ));
        }
        (None, None) => return Ok(()),
    }

    let target = saved_target.expect("target checked above");
    if dropped_refs.contains(target) {
        return Err(format!(
            "{item_kind} {item_id} targets dropped result {target}"
        ));
    }
    if !known_refs.contains(target) {
        return Err(format!(
            "{item_kind} {item_id} targets unknown result {target}"
        ));
    }
    Ok(())
}

fn register_result_refs<S, A>(
    bindings: &mut ReplayBindings<S, A>,
    refs: Vec<String>,
) -> Result<(), String> {
    for reference in refs {
        if bindings.live_result_refs.contains(&reference)
            || bindings.dropped_result_refs.contains(&reference)
        {
            return Err(format!("duplicate symbolic id {reference}"));
        }
        if !bindings.used_ids.insert(reference.clone()) {
            return Err(format!("duplicate symbolic id {reference}"));
        }
        bindings.live_result_refs.insert(reference);
    }
    Ok(())
}

fn drop_result_ref<S, A>(bindings: &mut ReplayBindings<S, A>, target: &str) -> Result<(), String> {
    if bindings.dropped_result_refs.contains(target) {
        return Err(format!("result {target} is already dropped"));
    }
    if !bindings.live_result_refs.remove(target) {
        return Err(format!("unknown result target {target}"));
    }
    bindings.dropped_result_refs.insert(target.to_string());
    Ok(())
}

fn add_pending_requests<S: Clone, A: Clone, SR, AR>(
    target: &mut Vec<PendingRequest<S, A>>,
    outbound: &[Event<S, A, SR, AR>],
) {
    target.extend(outbound.iter().filter_map(|event| match event {
        Event::CreateSync { id, op } => Some(PendingRequest::Sync {
            id: *id,
            op: op.clone(),
        }),
        Event::CreateAsync { id, op } => Some(PendingRequest::Async {
            id: *id,
            op: op.clone(),
        }),
        _ => None,
    }));
}

fn bind_outbound_item<R: TraceRuntime>(
    runtime: &R,
    bindings: &mut ReplayBindings<R::SyncOp, R::AsyncOp>,
    item: &RuntimeTraceItem<R>,
) -> Result<(), String> {
    match item {
        TraceItem::OutboundCreateSync { id, target, .. } => {
            if !bindings.used_ids.insert(id.clone()) {
                return Err(format!("duplicate symbolic id {id}"));
            }
            let Some(index) = bindings
                .pending_requests
                .iter()
                .position(|pending| matches!(pending, PendingRequest::Sync { .. }))
            else {
                return Err(format!("could not match outbound create_sync for {id}"));
            };
            let PendingRequest::Sync { id: runtime_id, op } =
                bindings.pending_requests.remove(index)
            else {
                unreachable!();
            };
            validate_request_target(
                &bindings.live_result_refs,
                &bindings.dropped_result_refs,
                "create_sync",
                id,
                target.as_deref(),
                runtime.sync_op_result_target(&op).as_deref(),
            )?;
            bindings.runtime_to_symbolic.insert(runtime_id, id.clone());
            bindings
                .bound_requests
                .insert(id.clone(), BoundRequest::Sync { id: runtime_id, op });
            Ok(())
        }
        TraceItem::OutboundCreateAsync { id, target, .. } => {
            if !bindings.used_ids.insert(id.clone()) {
                return Err(format!("duplicate symbolic id {id}"));
            }
            let Some(index) = bindings
                .pending_requests
                .iter()
                .position(|pending| matches!(pending, PendingRequest::Async { .. }))
            else {
                return Err(format!("could not match outbound create_async for {id}"));
            };
            let PendingRequest::Async { id: runtime_id, op } =
                bindings.pending_requests.remove(index)
            else {
                unreachable!();
            };
            validate_request_target(
                &bindings.live_result_refs,
                &bindings.dropped_result_refs,
                "create_async",
                id,
                target.as_deref(),
                runtime.async_op_result_target(&op).as_deref(),
            )?;
            bindings.runtime_to_symbolic.insert(runtime_id, id.clone());
            bindings
                .bound_requests
                .insert(id.clone(), BoundRequest::Async { id: runtime_id, op });
            Ok(())
        }
        TraceItem::OutboundDropResult { target } => drop_result_ref(bindings, target),
        _ => Err("expected outbound item".to_string()),
    }
}

fn apply_inbound_drop_result<S, A>(
    bindings: &mut ReplayBindings<S, A>,
    target: &str,
) -> Result<(), String> {
    drop_result_ref(bindings, target)
}

fn next_inbound_async_id<S, A>(bindings: &mut ReplayBindings<S, A>) -> Result<u64, String> {
    let id = bindings.next_inbound_async_id;
    bindings.next_inbound_async_id = bindings
        .next_inbound_async_id
        .checked_sub(1)
        .ok_or_else(|| "exhausted inbound async ids".to_string())?;
    Ok(id)
}

fn inbound_event_for_item<R: TraceRuntime>(
    runtime: &R,
    bindings: &mut ReplayBindings<R::SyncOp, R::AsyncOp>,
    item: &RuntimeTraceItem<R>,
) -> Result<RuntimeEventOf<R>, String> {
    match item {
        TraceItem::InboundReturnSync { target, result } => {
            let Some(BoundRequest::Sync { id, .. }) = bindings.bound_requests.remove(target) else {
                return Err(format!("unknown sync target {target}"));
            };
            bindings.runtime_to_symbolic.remove(&id);
            register_result_refs(bindings, runtime.sync_result_refs(result)?)?;
            Ok(Event::ReturnSync {
                id,
                result: result.clone(),
            })
        }
        TraceItem::InboundErrorSync { target, error } => {
            let Some(BoundRequest::Sync { id, .. }) = bindings.bound_requests.remove(target) else {
                return Err(format!("unknown sync target {target}"));
            };
            bindings.runtime_to_symbolic.remove(&id);
            Ok(Event::ReturnSync {
                id,
                result: runtime.sync_error_to_result(error),
            })
        }
        TraceItem::InboundResolveAsync { target, result } => {
            let Some(BoundRequest::Async { id, .. }) = bindings.bound_requests.remove(target)
            else {
                return Err(format!("unknown async target {target}"));
            };
            bindings.runtime_to_symbolic.remove(&id);
            register_result_refs(bindings, runtime.async_result_refs(result)?)?;
            Ok(Event::ResolveAsync {
                id,
                result: result.clone(),
            })
        }
        TraceItem::InboundAbortAsync { target } => {
            let Some(BoundRequest::Async { id, .. }) = bindings.bound_requests.remove(target)
            else {
                return Err(format!("unknown async target {target}"));
            };
            bindings.runtime_to_symbolic.remove(&id);
            Ok(Event::AbortAsync { id })
        }
        TraceItem::InboundCancelAsync { target } => {
            let Some(BoundRequest::Async { id, .. }) = bindings.bound_requests.remove(target)
            else {
                return Err(format!("unknown async target {target}"));
            };
            bindings.runtime_to_symbolic.remove(&id);
            Ok(Event::CancelAsync { id })
        }
        TraceItem::InboundCreateAsync { id, target, op } => {
            if !bindings.used_ids.insert(id.clone()) {
                return Err(format!("duplicate symbolic id {id}"));
            }
            validate_request_target(
                &bindings.live_result_refs,
                &bindings.dropped_result_refs,
                "inbound create_async",
                id,
                target.as_deref(),
                runtime.async_op_result_target(op).as_deref(),
            )?;
            let runtime_id = next_inbound_async_id(bindings)?;
            bindings.runtime_to_symbolic.insert(runtime_id, id.clone());
            bindings.bound_requests.insert(
                id.clone(),
                BoundRequest::Async {
                    id: runtime_id,
                    op: op.clone(),
                },
            );
            Ok(Event::CreateAsync {
                id: runtime_id,
                op: op.clone(),
            })
        }
        TraceItem::OutboundCreateSync { .. }
        | TraceItem::OutboundCreateAsync { .. }
        | TraceItem::OutboundDropResult { .. }
        | TraceItem::InboundDropResult { .. } => Err("expected inbound item".to_string()),
    }
}

fn event_matches_possible<R: TraceRuntime>(
    runtime: &R,
    candidate: &RuntimePossibleEventOf<R>,
    event: &RuntimeEventOf<R>,
) -> bool {
    match (candidate, event) {
        (
            PossibleEvent::ReturnSync { id, op },
            Event::ReturnSync {
                id: event_id,
                result,
            },
        ) => id == event_id && R::ReplaySpec::sync_result_matches(op, result),
        (
            PossibleEvent::ResolveAsync { id, op, .. },
            Event::ResolveAsync {
                id: event_id,
                result,
            },
        ) => id == event_id && R::ReplaySpec::async_result_matches(op, result),
        (PossibleEvent::AbortAsync { id, .. }, Event::AbortAsync { id: event_id }) => {
            id == event_id
        }
        (PossibleEvent::CreateInboundAsync { kind }, Event::CreateAsync { op, .. }) => {
            runtime.inbound_async_kind(op).as_ref() == Some(kind)
        }
        (PossibleEvent::CancelInboundAsync { id, .. }, Event::CancelAsync { id: event_id }) => {
            id == event_id
        }
        _ => false,
    }
}

fn generated_symbolic_id(used_ids: &BTreeSet<String>, prefix: &str) -> String {
    let mut index = 1;
    loop {
        let candidate = format!("{prefix}_{index}");
        if !used_ids.contains(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn outbound_id<SO, AO, SR, SE, AR>(item: &TraceItem<SO, AO, SR, SE, AR>) -> Option<&str> {
    match item {
        TraceItem::OutboundCreateSync { id, .. } | TraceItem::OutboundCreateAsync { id, .. } => {
            Some(id.as_str())
        }
        _ => None,
    }
}

fn inbound_target<SO, AO, SR, SE, AR>(item: &TraceItem<SO, AO, SR, SE, AR>) -> Option<&str> {
    match item {
        TraceItem::InboundReturnSync { target, .. }
        | TraceItem::InboundErrorSync { target, .. }
        | TraceItem::InboundResolveAsync { target, .. }
        | TraceItem::InboundAbortAsync { target }
        | TraceItem::InboundCancelAsync { target } => Some(target.as_str()),
        TraceItem::InboundCreateAsync { .. }
        | TraceItem::InboundDropResult { .. }
        | TraceItem::OutboundCreateSync { .. }
        | TraceItem::OutboundCreateAsync { .. }
        | TraceItem::OutboundDropResult { .. } => None,
    }
}

fn removal_span<SO, AO, SR, SE, AR>(
    trace: &[TraceItem<SO, AO, SR, SE, AR>],
    item_index: usize,
) -> Result<(usize, usize), String> {
    let Some(item) = trace.get(item_index) else {
        return Err(format!("invalid item index {item_index}"));
    };
    match item {
        TraceItem::InboundReturnSync { .. }
        | TraceItem::InboundErrorSync { .. }
        | TraceItem::InboundResolveAsync { .. }
        | TraceItem::InboundAbortAsync { .. }
        | TraceItem::InboundCancelAsync { .. } => {
            if item_index > 0 {
                if let (Some(target), Some(previous_id)) =
                    (inbound_target(item), outbound_id(&trace[item_index - 1]))
                {
                    if target == previous_id {
                        return Ok((item_index - 1, item_index + 1));
                    }
                }
            }
            Ok((item_index, item_index + 1))
        }
        TraceItem::InboundDropResult { .. }
        | TraceItem::InboundCreateAsync { .. }
        | TraceItem::OutboundDropResult { .. }
        | TraceItem::OutboundCreateSync { .. }
        | TraceItem::OutboundCreateAsync { .. } => Ok((item_index, item_index + 1)),
    }
}

struct ChoiceSnapshot<R: TraceRuntime> {
    possible: Vec<RuntimePossibleEventOf<R>>,
    runtime_to_symbolic: BTreeMap<u64, String>,
    used_ids: BTreeSet<String>,
    live_result_refs: BTreeSet<String>,
}

fn choice_snapshot_for_prefix<R: TraceRuntime>(
    runtime: &R,
    trace_prefix: &[RuntimeTraceItem<R>],
) -> Result<ChoiceSnapshot<R>, String> {
    let mut bindings = ReplayBindings::default();
    let bundle = runtime.new_replay_bundle();
    let (mut wrapper, initial_outbound) = NewRunWrapper::new(bundle).start();
    add_pending_requests(&mut bindings.pending_requests, &initial_outbound);
    let mut replay_trace = vec![TraceStep::start(initial_outbound.clone())];

    for (index, item) in trace_prefix.iter().enumerate() {
        match item {
            TraceItem::OutboundCreateSync { .. }
            | TraceItem::OutboundCreateAsync { .. }
            | TraceItem::OutboundDropResult { .. } => {
                bind_outbound_item::<R>(runtime, &mut bindings, item)?;
            }
            TraceItem::InboundDropResult { target } => {
                apply_inbound_drop_result(&mut bindings, target)?;
            }
            _ => {
                let event = inbound_event_for_item::<R>(runtime, &mut bindings, item)?;
                let possible =
                    crate::possible_next_events::<_, _, _, _, R::ReplaySpec>(&replay_trace)
                        .map_err(|err| format!("failed to compute possible events: {err:?}"))?;
                if !possible
                    .iter()
                    .any(|candidate| event_matches_possible(runtime, candidate, &event))
                {
                    return Err(format!(
                        "saved inbound item is not valid at index {index}: {}",
                        runtime.format_trace_item(item)
                    ));
                }
                let outbound = wrapper.push(event.clone());
                add_pending_requests(&mut bindings.pending_requests, &outbound);
                replay_trace.push(TraceStep::push(event, outbound));
            }
        }
    }

    let possible = crate::possible_next_events::<_, _, _, _, R::ReplaySpec>(&replay_trace)
        .map_err(|err| format!("failed to compute possible events: {err:?}"))?;
    Ok(ChoiceSnapshot {
        possible,
        runtime_to_symbolic: bindings.runtime_to_symbolic,
        used_ids: bindings.used_ids,
        live_result_refs: bindings.live_result_refs,
    })
}

pub fn replay_steps_for_trace<R: TraceRuntime>(
    runtime: &R,
    trace_prefix: &[RuntimeTraceItem<R>],
) -> Result<Vec<TraceStep<R::SyncOp, R::AsyncOp, R::SyncResult, R::AsyncResult>>, String> {
    let mut bindings = ReplayBindings::default();
    let bundle = runtime.new_replay_bundle();
    let (mut wrapper, initial_outbound) = NewRunWrapper::new(bundle).start();
    add_pending_requests(&mut bindings.pending_requests, &initial_outbound);
    let mut replay_trace = vec![TraceStep::start(initial_outbound.clone())];

    for (index, item) in trace_prefix.iter().enumerate() {
        match item {
            TraceItem::OutboundCreateSync { .. }
            | TraceItem::OutboundCreateAsync { .. }
            | TraceItem::OutboundDropResult { .. } => {
                bind_outbound_item::<R>(runtime, &mut bindings, item)?;
            }
            TraceItem::InboundDropResult { target } => {
                apply_inbound_drop_result(&mut bindings, target)?;
            }
            _ => {
                let event = inbound_event_for_item::<R>(runtime, &mut bindings, item)?;
                let possible =
                    crate::possible_next_events::<_, _, _, _, R::ReplaySpec>(&replay_trace)
                        .map_err(|err| format!("failed to compute possible events: {err:?}"))?;
                if !possible
                    .iter()
                    .any(|candidate| event_matches_possible(runtime, candidate, &event))
                {
                    return Err(format!(
                        "saved inbound item is not valid at index {index}: {}",
                        runtime.format_trace_item(item)
                    ));
                }
                let outbound = wrapper.push(event.clone());
                add_pending_requests(&mut bindings.pending_requests, &outbound);
                replay_trace.push(TraceStep::push(event, outbound));
            }
        }
    }

    Ok(replay_trace)
}

fn editor_choices_from_snapshot<R: TraceRuntime>(
    runtime: &R,
    snapshot: ChoiceSnapshot<R>,
) -> Result<Vec<RuntimeEditorChoiceOf<R>>, String> {
    let ChoiceSnapshot {
        possible,
        runtime_to_symbolic,
        used_ids,
        live_result_refs,
    } = snapshot;
    let mut choices = Vec::new();
    for possible in possible {
        match possible {
            PossibleEvent::ReturnSync { id, op } => {
                let target = runtime_to_symbolic
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| generated_symbolic_id(&used_ids, "sync"));
                let include_outbound = !runtime_to_symbolic.contains_key(&id);
                choices.push(EditorChoice::ReturnSyncSuccess {
                    target: target.clone(),
                    op: op.clone(),
                    include_outbound,
                });
                if runtime.default_sync_error(&op).is_some() {
                    choices.push(EditorChoice::ReturnSyncError {
                        target,
                        op,
                        include_outbound,
                    });
                }
            }
            PossibleEvent::ResolveAsync { id, op, warnings } => {
                let target = runtime_to_symbolic
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| generated_symbolic_id(&used_ids, "async"));
                let include_outbound = !runtime_to_symbolic.contains_key(&id);
                choices.push(EditorChoice::ResolveAsync {
                    target,
                    op,
                    include_outbound,
                    warnings,
                });
            }
            PossibleEvent::AbortAsync { id, op } => {
                let target = runtime_to_symbolic
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| generated_symbolic_id(&used_ids, "async"));
                let include_outbound = !runtime_to_symbolic.contains_key(&id);
                choices.push(EditorChoice::AbortAsync {
                    target,
                    op,
                    include_outbound,
                });
            }
            PossibleEvent::CreateInboundAsync { kind } => {
                choices.push(EditorChoice::CreateInboundAsync {
                    id: generated_symbolic_id(&used_ids, "inbound_async"),
                    kind,
                });
            }
            PossibleEvent::CancelInboundAsync { id, op } => {
                let Some(target) = runtime_to_symbolic.get(&id).cloned() else {
                    return Err(format!("missing symbolic id for inbound async {id}"));
                };
                choices.push(EditorChoice::CancelInboundAsync { target, op });
            }
        }
    }
    for target in &live_result_refs {
        choices.push(EditorChoice::DropResult {
            target: target.clone(),
            outbound: true,
        });
        choices.push(EditorChoice::DropResult {
            target: target.clone(),
            outbound: false,
        });
    }
    Ok(choices)
}

pub fn editor_choices_for_target<R: TraceRuntime>(
    runtime: &R,
    trace: &[RuntimeTraceItem<R>],
    target: &RuntimeTarget,
) -> Result<Vec<RuntimeEditorChoiceOf<R>>, String> {
    match target {
        RuntimeTarget::Insert { insertion_index } => {
            if *insertion_index > trace.len() {
                return Err(format!("invalid insertion index {insertion_index}"));
            }
            let snapshot = choice_snapshot_for_prefix(runtime, &trace[..*insertion_index])?;
            editor_choices_from_snapshot(runtime, snapshot)
        }
        RuntimeTarget::Edit { item_index } => {
            let (start, end) = removal_span(trace, *item_index)?;
            let mut reduced = trace.to_vec();
            reduced.drain(start..end);
            let snapshot = choice_snapshot_for_prefix(runtime, &reduced[..start])?;
            editor_choices_from_snapshot(runtime, snapshot)
        }
    }
}

fn decode_trace_item<R: TraceRuntime>(
    runtime: &R,
    item: EncodedTraceItem,
) -> Result<RuntimeTraceItem<R>, String> {
    match item {
        EncodedTraceItem::OutboundCreateSync { id, target } => Ok(TraceItem::OutboundCreateSync {
            id,
            target,
            op: None,
        }),
        EncodedTraceItem::OutboundCreateAsync { id, target } => {
            Ok(TraceItem::OutboundCreateAsync {
                id,
                target,
                op: None,
            })
        }
        EncodedTraceItem::OutboundDropResult { target } => {
            Ok(TraceItem::OutboundDropResult { target })
        }
        EncodedTraceItem::InboundDropResult { target } => {
            Ok(TraceItem::InboundDropResult { target })
        }
        EncodedTraceItem::InboundReturnSync { target, result } => {
            Ok(TraceItem::InboundReturnSync {
                target,
                result: runtime.sync_result_from_json(result)?,
            })
        }
        EncodedTraceItem::InboundErrorSync { target, error } => Ok(TraceItem::InboundErrorSync {
            target,
            error: runtime.sync_error_from_json(error)?,
        }),
        EncodedTraceItem::InboundResolveAsync { target, result } => {
            Ok(TraceItem::InboundResolveAsync {
                target,
                result: runtime.async_result_from_json(result)?,
            })
        }
        EncodedTraceItem::InboundAbortAsync { target } => {
            Ok(TraceItem::InboundAbortAsync { target })
        }
        EncodedTraceItem::InboundCancelAsync { target } => {
            Ok(TraceItem::InboundCancelAsync { target })
        }
        EncodedTraceItem::InboundCreateAsync { id, target, op } => {
            Ok(TraceItem::InboundCreateAsync {
                id,
                target,
                op: runtime.async_op_from_json(op)?,
            })
        }
    }
}

fn encode_trace_item<R: TraceRuntime>(
    runtime: &R,
    item: &RuntimeTraceItem<R>,
) -> Result<EncodedTraceItem, String> {
    match item {
        TraceItem::OutboundCreateSync { id, target, .. } => {
            Ok(EncodedTraceItem::OutboundCreateSync {
                id: id.clone(),
                target: target.clone(),
            })
        }
        TraceItem::OutboundCreateAsync { id, target, .. } => {
            Ok(EncodedTraceItem::OutboundCreateAsync {
                id: id.clone(),
                target: target.clone(),
            })
        }
        TraceItem::OutboundDropResult { target } => Ok(EncodedTraceItem::OutboundDropResult {
            target: target.clone(),
        }),
        TraceItem::InboundDropResult { target } => Ok(EncodedTraceItem::InboundDropResult {
            target: target.clone(),
        }),
        TraceItem::InboundReturnSync { target, result } => {
            Ok(EncodedTraceItem::InboundReturnSync {
                target: target.clone(),
                result: runtime.sync_result_to_json(result)?,
            })
        }
        TraceItem::InboundErrorSync { target, error } => Ok(EncodedTraceItem::InboundErrorSync {
            target: target.clone(),
            error: runtime.sync_error_to_json(error)?,
        }),
        TraceItem::InboundResolveAsync { target, result } => {
            Ok(EncodedTraceItem::InboundResolveAsync {
                target: target.clone(),
                result: runtime.async_result_to_json(result)?,
            })
        }
        TraceItem::InboundAbortAsync { target } => Ok(EncodedTraceItem::InboundAbortAsync {
            target: target.clone(),
        }),
        TraceItem::InboundCancelAsync { target } => Ok(EncodedTraceItem::InboundCancelAsync {
            target: target.clone(),
        }),
        TraceItem::InboundCreateAsync { id, target, op } => {
            Ok(EncodedTraceItem::InboundCreateAsync {
                id: id.clone(),
                target: target.clone(),
                op: runtime.async_op_to_json(op)?,
            })
        }
    }
}

pub fn load_trace<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
) -> Result<Vec<RuntimeTraceItem<R>>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let envelope: RunEnvelope<EncodedTraceItem> = serde_json::from_str(&contents)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    if !envelope.is_simulator_run() {
        return Err(format!("{} is not a simulator run", path.display()));
    }
    envelope
        .items
        .into_iter()
        .map(|item| decode_trace_item(runtime, item))
        .collect()
}

pub fn save_trace<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
    trace: &[RuntimeTraceItem<R>],
) -> Result<(), String> {
    let envelope = RunEnvelope {
        kind: super::SIMULATOR_RUN_KIND.to_string(),
        version: super::SIMULATOR_RUN_VERSION,
        items: trace
            .iter()
            .map(|item| encode_trace_item(runtime, item))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let contents = serde_json::to_string_pretty(&envelope)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

pub fn create_trace<R: TraceRuntime>(runtime: &R, path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    save_trace::<R>(runtime, path, &[])
}

pub fn open_trace<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<EditorSession<RuntimeTraceItem<R>>, String> {
    let trace = load_trace(runtime, path)?;
    Ok(EditorSession {
        path: path.to_path_buf(),
        state: AppState::new(trace, terminal_width, terminal_height),
    })
}

pub fn open_or_create_trace<R: TraceRuntime>(
    runtime: &R,
    path: &Path,
    terminal_width: u16,
    terminal_height: u16,
) -> Result<EditorSession<RuntimeTraceItem<R>>, String> {
    if path.exists() {
        open_trace(runtime, path, terminal_width, terminal_height)
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
        save_trace(runtime, path, &[])?;
        let mut session = open_trace(runtime, path, terminal_width, terminal_height)?;
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
    let mut bindings = ReplayBindings::default();
    let bundle = runtime.new_replay_bundle();
    let (mut wrapper, initial_outbound) = NewRunWrapper::new(bundle).start();
    add_pending_requests(&mut bindings.pending_requests, &initial_outbound);
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
        if matches!(
            item,
            TraceItem::OutboundCreateSync { .. } | TraceItem::OutboundCreateAsync { .. }
        ) {
            if let Err(err) = bind_outbound_item::<R>(runtime, &mut bindings, item) {
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
            continue;
        }

        if let TraceItem::OutboundDropResult { .. } = item {
            if let Err(err) = bind_outbound_item::<R>(runtime, &mut bindings, item) {
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
            rows.push(ReplayRow {
                text: runtime.format_trace_item(item),
                insertion_index: index + 1,
                script_item_index: Some(index),
                is_invalid: false,
                timeline_event: None,
            });
            continue;
        }

        if let TraceItem::InboundDropResult { target } = item {
            if let Err(err) = apply_inbound_drop_result(&mut bindings, target) {
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
            rows.push(ReplayRow {
                text: runtime.format_trace_item(item),
                insertion_index: index + 1,
                script_item_index: Some(index),
                is_invalid: false,
                timeline_event: None,
            });
            continue;
        }

        let event = match inbound_event_for_item::<R>(runtime, &mut bindings, item) {
            Ok(event) => event,
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

        let possible = crate::possible_next_events::<_, _, _, _, R::ReplaySpec>(&replay_trace)
            .map_err(|err| format!("failed to compute possible events: {err:?}"))?;
        if !possible
            .iter()
            .any(|candidate| event_matches_possible(runtime, candidate, &event))
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
        add_pending_requests(&mut bindings.pending_requests, &outbound);
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
            rows.push(ReplayRow {
                text: "RUN terminated".to_string(),
                insertion_index: index + 1,
                script_item_index: None,
                is_invalid: false,
                timeline_event: None,
            });
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
            let target = RuntimeTarget::Insert {
                insertion_index: next_insertion_index,
            };
            let choices = editor_choices_for_target(runtime, &trace, &target)?;
            let mut complete_choices = 0;
            for choice in &choices {
                let spec = runtime.form_schema(&trace, &target, choice)?;
                let state = form_state_from_spec(&spec);
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
        let target = RuntimeTarget::Insert {
            insertion_index: next_insertion_index,
        };
        let choices = editor_choices_for_target(runtime, &trace, &target)?;
        let mut trivial_choice = None;
        for choice in &choices {
            let spec = runtime.form_schema(&trace, &target, choice)?;
            let state = form_state_from_spec(&spec);
            if form_is_auto_acceptable(&spec, &state) {
                if trivial_choice.is_some() {
                    trivial_choice = None;
                    break;
                }
                trivial_choice = Some((choice.clone(), state));
            }
        }
        let Some((choice, state)) = trivial_choice else {
            break;
        };
        let items = runtime.decode_form_state(&trace, &target, &choice, &state)?;
        if items.is_empty() {
            break;
        }
        let item_count = items.len();
        apply_form_items::<R>(&mut trace, &target, items)?;
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
    let choices = editor_choices_for_target(runtime, &state.view.trace, &runtime_target)?;
    let choice = choices
        .get(choice_index)
        .ok_or_else(|| format!("invalid choice index {choice_index}"))?;
    let spec = runtime.form_schema(&state.view.trace, &runtime_target, choice)?;
    let form_state = form_state_from_spec(&spec);
    Ok(TraceViewDialog::Form {
        target,
        choice_index,
        spec,
        state: form_state,
        selected_field: 0,
    })
}

fn apply_form_items<R: TraceRuntime>(
    trace: &mut Vec<RuntimeTraceItem<R>>,
    target: &RuntimeTarget,
    items: Vec<RuntimeTraceItem<R>>,
) -> Result<(), String> {
    match target {
        RuntimeTarget::Insert { insertion_index } => {
            if *insertion_index > trace.len() {
                return Err(format!("invalid insertion index {insertion_index}"));
            }
            for (offset, item) in items.into_iter().enumerate() {
                trace.insert(insertion_index + offset, item);
            }
        }
        RuntimeTarget::Edit { item_index } => {
            let (start, end) = removal_span(trace, *item_index)?;
            trace.splice(start..end, items);
        }
    }
    Ok(())
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
            match editor_choices_for_target(runtime, &state.view.trace, &runtime_target) {
                Ok(choices) => {
                    state.view.dialog = TraceViewDialog::Choice {
                        target,
                        choices: choices
                            .iter()
                            .map(|choice| InsertionChoice {
                                label: runtime.format_editor_choice(choice),
                            })
                            .collect(),
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
            match editor_choices_for_target(runtime, &state.view.trace, &runtime_target) {
                Ok(choices) => {
                    state.view.dialog = TraceViewDialog::Choice {
                        target,
                        choices: choices
                            .iter()
                            .map(|choice| InsertionChoice {
                                label: runtime.format_editor_choice(choice),
                            })
                            .collect(),
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
                    let choices = match editor_choices_for_target(
                        runtime,
                        &state.view.trace,
                        &runtime_target,
                    ) {
                        Ok(choices) => choices,
                        Err(err) => {
                            set_status(&mut state, err);
                            return (state, Vec::new());
                        }
                    };
                    let Some(choice) = choices.get(choice_index) else {
                        set_status(&mut state, format!("invalid choice index {choice_index}"));
                        return (state, Vec::new());
                    };
                    let items = match runtime.decode_form_state(
                        &state.view.trace,
                        &runtime_target,
                        choice,
                        &form_state,
                    ) {
                        Ok(items) => items,
                        Err(err) => {
                            set_status(&mut state, err);
                            return (state, Vec::new());
                        }
                    };
                    match apply_form_items::<R>(&mut state.view.trace, &runtime_target, items) {
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
    use serde::{Deserialize, Serialize};
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
        type Bundle = TestBundle;
        type ReplaySpec = TestSpec;

        fn form_schema(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _target: &RuntimeTarget,
            _choice: &EditorChoice<Self::SyncOp, Self::AsyncOp, ()>,
        ) -> Result<FormSpec, String> {
            Ok(super::super::FormSpec {
                title: "Edit".into(),
                details: Vec::new(),
                fields: Vec::new(),
                auto_accept_if_complete: true,
            })
        }

        fn decode_form_state(
            &self,
            _trace: &[RuntimeTraceItem<Self>],
            _target: &RuntimeTarget,
            _choice: &EditorChoice<Self::SyncOp, Self::AsyncOp, ()>,
            _state: &FormState,
        ) -> Result<Vec<RuntimeTraceItem<Self>>, String> {
            Ok(vec![TraceItem::InboundReturnSync {
                target: "0".into(),
                result: TestSyncResult::Unit,
            }])
        }

        fn format_editor_choice(
            &self,
            choice: &EditorChoice<Self::SyncOp, Self::AsyncOp, ()>,
        ) -> String {
            match choice {
                EditorChoice::ReturnSyncSuccess { target, .. } => {
                    format!("ReturnSync#{target} BootReason")
                }
                EditorChoice::ReturnSyncError { target, .. } => {
                    format!("ErrorSync#{target} BootReason")
                }
                EditorChoice::ResolveAsync { target, .. } => {
                    format!("ResolveAsync#{target} Sleep")
                }
                EditorChoice::AbortAsync { target, .. } => format!("AbortAsync#{target} Sleep"),
                EditorChoice::CreateInboundAsync { id, .. } => {
                    format!("CreateInboundAsync {id}")
                }
                EditorChoice::CancelInboundAsync { target, .. } => {
                    format!("CancelInboundAsync#{target} Sleep")
                }
                EditorChoice::DropResult { target, outbound } => {
                    if *outbound {
                        format!("DropResult#{target}")
                    } else {
                        format!("InboundDropResult#{target}")
                    }
                }
            }
        }

        fn default_sync_error(&self, _op: &Self::SyncOp) -> Option<Self::SyncError> {
            Some(TestSyncError::Unit)
        }

        fn new_replay_bundle(&self) -> Self::Bundle {
            TestBundle
        }

        fn sync_error_to_result(&self, _error: &Self::SyncError) -> Self::SyncResult {
            TestSyncResult::Unit
        }

        fn inbound_async_kind(
            &self,
            _op: &Self::AsyncOp,
        ) -> Option<
            <Self::ReplaySpec as NextEventsSpec<
                Self::SyncOp,
                Self::AsyncOp,
                Self::SyncResult,
                Self::AsyncResult,
            >>::InboundAsyncKind,
        > {
            None
        }

        fn format_trace_item(&self, _item: &RuntimeTraceItem<Self>) -> String {
            "row".to_string()
        }

        fn format_runtime_event(&self, event: &RuntimeEventOf<Self>) -> String {
            match event {
                Event::CreateSync { .. } => "CreateSync BootReason".to_string(),
                Event::ReturnSync { .. } => "ReturnSync Unit".to_string(),
                Event::CreateAsync { .. } => "CreateAsync Sleep".to_string(),
                Event::ResolveAsync { .. } => "ResolveAsync Unit".to_string(),
                Event::CancelAsync { .. } => "CancelAsync".to_string(),
                Event::AbortAsync { .. } => "AbortAsync".to_string(),
            }
        }

        fn sync_op_result_target(&self, _op: &Self::SyncOp) -> Option<String> {
            None
        }

        fn async_op_result_target(&self, _op: &Self::AsyncOp) -> Option<String> {
            None
        }

        fn async_op_to_json(&self, value: &Self::AsyncOp) -> Result<Value, String> {
            serde_json::to_value(value).map_err(|err| err.to_string())
        }

        fn async_op_from_json(&self, value: Value) -> Result<Self::AsyncOp, String> {
            serde_json::from_value(value).map_err(|err| err.to_string())
        }

        fn sync_result_to_json(&self, value: &Self::SyncResult) -> Result<Value, String> {
            serde_json::to_value(value).map_err(|err| err.to_string())
        }

        fn sync_result_from_json(&self, value: Value) -> Result<Self::SyncResult, String> {
            serde_json::from_value(value).map_err(|err| err.to_string())
        }

        fn sync_error_to_json(&self, value: &Self::SyncError) -> Result<Value, String> {
            serde_json::to_value(value).map_err(|err| err.to_string())
        }

        fn sync_error_from_json(&self, value: Value) -> Result<Self::SyncError, String> {
            serde_json::from_value(value).map_err(|err| err.to_string())
        }

        fn async_result_to_json(&self, value: &Self::AsyncResult) -> Result<Value, String> {
            serde_json::to_value(value).map_err(|err| err.to_string())
        }

        fn async_result_from_json(&self, value: Value) -> Result<Self::AsyncResult, String> {
            serde_json::from_value(value).map_err(|err| err.to_string())
        }

        fn sync_result_refs(&self, value: &Self::SyncResult) -> Result<Vec<String>, String> {
            match value {
                TestSyncResult::Unit => Ok(vec!["ref_1".to_string()]),
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
        let session = open_or_create_trace(&TestRuntime, &path, 120, 40).unwrap();
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

    #[test]
    fn editor_choices_include_drop_result_for_live_refs() {
        let trace = vec![
            TraceItem::OutboundCreateSync {
                id: "boot_reason".into(),
                target: None,
                op: None,
            },
            TraceItem::InboundReturnSync {
                target: "boot_reason".into(),
                result: TestSyncResult::Unit,
            },
        ];

        let choices = editor_choices_for_target(
            &TestRuntime,
            &trace,
            &RuntimeTarget::Insert {
                insertion_index: trace.len(),
            },
        )
        .unwrap();

        assert!(choices.iter().any(|choice| matches!(
            choice,
            EditorChoice::DropResult {
                target,
                outbound: true,
            } if target == "ref_1"
        )));
        assert!(choices.iter().any(|choice| matches!(
            choice,
            EditorChoice::DropResult {
                target,
                outbound: false,
            } if target == "ref_1"
        )));
    }

    #[test]
    fn inbound_drop_result_renders_as_valid_script_row() {
        let trace = vec![
            TraceItem::OutboundCreateSync {
                id: "boot_reason".into(),
                target: None,
                op: None,
            },
            TraceItem::InboundReturnSync {
                target: "boot_reason".into(),
                result: TestSyncResult::Unit,
            },
            TraceItem::InboundDropResult {
                target: "ref_1".into(),
            },
        ];

        let rendered = render_trace(&TestRuntime, &trace).unwrap();
        assert!(rendered.replay_error.is_none());
        assert!(rendered
            .rows
            .iter()
            .any(|row| row.script_item_index == Some(2) && !row.is_invalid));
    }
}
