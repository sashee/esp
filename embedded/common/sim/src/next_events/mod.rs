use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use crate::{Event, OpId};

#[derive(Clone, Debug, PartialEq)]
pub struct TraceStep<S, A, SR, AR> {
    pub inbound: Option<Event<S, A, SR, AR>>,
    pub outbound: Vec<Event<S, A, SR, AR>>,
}

impl<S, A, SR, AR> TraceStep<S, A, SR, AR> {
    pub fn start(outbound: Vec<Event<S, A, SR, AR>>) -> Self {
        Self {
            inbound: None,
            outbound,
        }
    }

    pub fn push(inbound: Event<S, A, SR, AR>, outbound: Vec<Event<S, A, SR, AR>>) -> Self {
        Self {
            inbound: Some(inbound),
            outbound,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncTiming {
    Untimed,
    Delay(Duration),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    Timing(TimingWarning),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimingWarning {
    EarlierDelayStillPending {
        pending_id: OpId,
        pending_duration: Duration,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PossibleEvent<S, A, K> {
    ReturnSync {
        id: OpId,
        op: S,
    },
    ResolveAsync {
        id: OpId,
        op: A,
        warnings: Vec<Warning>,
    },
    AbortAsync {
        id: OpId,
        op: A,
    },
    CreateInboundAsync {
        kind: K,
    },
    CancelInboundAsync {
        id: OpId,
        op: A,
    },
}

pub trait NextEventsSpec<S, A, SR, AR> {
    type InboundAsyncKind: Clone + Eq;

    fn sync_result_matches(op: &S, result: &SR) -> bool;
    fn async_result_matches(op: &A, result: &AR) -> bool;

    fn async_timing(op: &A) -> AsyncTiming {
        let _ = op;
        AsyncTiming::Untimed
    }

    fn possible_inbound_async(_trace: &[TraceStep<S, A, SR, AR>]) -> Vec<Self::InboundAsyncKind> {
        Vec::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    UnknownSyncId(OpId),
    SyncIdMismatch { expected: OpId, actual: OpId },
    UnexpectedEventWhileSyncBlocked,
    UnknownOutboundAsyncId(OpId),
    UnknownInboundAsyncId(OpId),
    DuplicateAsyncId(OpId),
    AsyncIdCollision(OpId),
    AsyncAlreadyCompleted(OpId),
    OutboundAsyncWrongEventKind(OpId),
    InboundAsyncWrongEventKind(OpId),
    WrongSyncResultType(OpId),
    WrongAsyncResultType(OpId),
    InboundCreateSyncUnsupported,
    OutboundReturnSyncUnsupported,
    OutboundCreateSyncWhileSyncBlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsyncOrigin {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug)]
struct PendingSync<S> {
    id: OpId,
    op: S,
}

#[derive(Clone, Debug)]
struct PendingAsync<A> {
    op: A,
    creation_order: u64,
}

struct ReplayState<S, A> {
    next_creation_order: u64,
    pending_sync: Option<PendingSync<S>>,
    pending_outbound_async: BTreeMap<OpId, PendingAsync<A>>,
    pending_inbound_async: BTreeMap<OpId, PendingAsync<A>>,
    async_origins: BTreeMap<OpId, AsyncOrigin>,
    completed_async: BTreeSet<OpId>,
}

impl<S, A> Default for ReplayState<S, A> {
    fn default() -> Self {
        Self {
            next_creation_order: 0,
            pending_sync: None,
            pending_outbound_async: BTreeMap::new(),
            pending_inbound_async: BTreeMap::new(),
            async_origins: BTreeMap::new(),
            completed_async: BTreeSet::new(),
        }
    }
}

pub fn possible_next_events<S, A, SR, AR, Spec>(
    trace: &[TraceStep<S, A, SR, AR>],
) -> Result<Vec<PossibleEvent<S, A, Spec::InboundAsyncKind>>, ReplayError>
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    let mut state = ReplayState::default();
    for step in trace {
        replay_step::<S, A, SR, AR, Spec>(&mut state, step)?;
    }

    if let Some(pending) = state.pending_sync {
        return Ok(vec![PossibleEvent::ReturnSync {
            id: pending.id,
            op: pending.op,
        }]);
    }

    let mut events = Vec::new();

    for (&id, pending) in &state.pending_outbound_async {
        events.push(PossibleEvent::ResolveAsync {
            id,
            op: pending.op.clone(),
            warnings: resolve_warnings::<S, A, SR, AR, Spec>(&state, id),
        });
        events.push(PossibleEvent::AbortAsync {
            id,
            op: pending.op.clone(),
        });
    }

    for (&id, pending) in &state.pending_inbound_async {
        events.push(PossibleEvent::CancelInboundAsync {
            id,
            op: pending.op.clone(),
        });
    }

    let mut seen_kinds = Vec::new();
    for kind in Spec::possible_inbound_async(trace) {
        if seen_kinds.contains(&kind) {
            continue;
        }
        seen_kinds.push(kind.clone());
        events.push(PossibleEvent::CreateInboundAsync { kind });
    }

    Ok(events)
}

fn resolve_warnings<S, A, SR, AR, Spec>(state: &ReplayState<S, A>, id: OpId) -> Vec<Warning>
where
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    let Some(current) = state.pending_outbound_async.get(&id) else {
        return Vec::new();
    };

    let AsyncTiming::Delay(current_duration) = Spec::async_timing(&current.op) else {
        return Vec::new();
    };

    let mut warnings = Vec::new();
    for (&other_id, other) in &state.pending_outbound_async {
        if other_id == id || other.creation_order >= current.creation_order {
            continue;
        }

        let AsyncTiming::Delay(other_duration) = Spec::async_timing(&other.op) else {
            continue;
        };

        if other_duration <= current_duration {
            warnings.push(Warning::Timing(TimingWarning::EarlierDelayStillPending {
                pending_id: other_id,
                pending_duration: other_duration,
            }));
        }
    }

    warnings
}

fn replay_step<S, A, SR, AR, Spec>(
    state: &mut ReplayState<S, A>,
    step: &TraceStep<S, A, SR, AR>,
) -> Result<(), ReplayError>
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    if let Some(inbound) = &step.inbound {
        apply_inbound::<S, A, SR, AR, Spec>(state, inbound)?;
    }

    for (index, outbound) in step.outbound.iter().enumerate() {
        apply_outbound::<S, A, SR, AR, Spec>(state, outbound)?;
        if state.pending_sync.is_some() && index + 1 < step.outbound.len() {
            return Err(ReplayError::OutboundCreateSyncWhileSyncBlocked);
        }
    }

    Ok(())
}

fn apply_inbound<S, A, SR, AR, Spec>(
    state: &mut ReplayState<S, A>,
    event: &Event<S, A, SR, AR>,
) -> Result<(), ReplayError>
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    if let Some(pending_sync) = state.pending_sync.as_ref() {
        return match event {
            Event::ReturnSync { id, result } => {
                if *id != pending_sync.id {
                    Err(ReplayError::SyncIdMismatch {
                        expected: pending_sync.id,
                        actual: *id,
                    })
                } else if !Spec::sync_result_matches(&pending_sync.op, result) {
                    Err(ReplayError::WrongSyncResultType(*id))
                } else {
                    state.pending_sync = None;
                    Ok(())
                }
            }
            _ => Err(ReplayError::UnexpectedEventWhileSyncBlocked),
        };
    }

    match event {
        Event::CreateSync { .. } => Err(ReplayError::InboundCreateSyncUnsupported),
        Event::ReturnSync { id, .. } => Err(ReplayError::UnknownSyncId(*id)),
        Event::CreateAsync { id, op } => create_async(state, *id, op.clone(), AsyncOrigin::Inbound),
        Event::ResolveAsync { id, result } => {
            resolve_outbound_async::<S, A, SR, AR, Spec>(state, *id, result)
        }
        Event::AbortAsync { id } => abort_outbound_async(state, *id),
        Event::CancelAsync { id } => cancel_inbound_async(state, *id),
    }
}

fn apply_outbound<S, A, SR, AR, Spec>(
    state: &mut ReplayState<S, A>,
    event: &Event<S, A, SR, AR>,
) -> Result<(), ReplayError>
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    match event {
        Event::CreateSync { id, op } => {
            if state.pending_sync.is_some() {
                return Err(ReplayError::OutboundCreateSyncWhileSyncBlocked);
            }
            state.pending_sync = Some(PendingSync {
                id: *id,
                op: op.clone(),
            });
            Ok(())
        }
        Event::ReturnSync { .. } => Err(ReplayError::OutboundReturnSyncUnsupported),
        Event::CreateAsync { id, op } => {
            create_async(state, *id, op.clone(), AsyncOrigin::Outbound)
        }
        Event::ResolveAsync { id, result } => {
            resolve_inbound_async::<S, A, SR, AR, Spec>(state, *id, result)
        }
        Event::AbortAsync { id } => abort_inbound_async(state, *id),
        Event::CancelAsync { id } => cancel_outbound_async(state, *id),
    }
}

fn create_async<S, A>(
    state: &mut ReplayState<S, A>,
    id: OpId,
    op: A,
    origin: AsyncOrigin,
) -> Result<(), ReplayError> {
    if let Some(existing) = state.async_origins.get(&id) {
        return Err(match existing {
            AsyncOrigin::Outbound if origin == AsyncOrigin::Outbound => {
                ReplayError::DuplicateAsyncId(id)
            }
            AsyncOrigin::Inbound if origin == AsyncOrigin::Inbound => {
                ReplayError::DuplicateAsyncId(id)
            }
            _ => ReplayError::AsyncIdCollision(id),
        });
    }

    state.async_origins.insert(id, origin);
    let pending = PendingAsync {
        op,
        creation_order: state.next_creation_order,
    };
    state.next_creation_order += 1;
    match origin {
        AsyncOrigin::Outbound => {
            state.pending_outbound_async.insert(id, pending);
        }
        AsyncOrigin::Inbound => {
            state.pending_inbound_async.insert(id, pending);
        }
    }
    Ok(())
}

fn resolve_outbound_async<S, A, SR, AR, Spec>(
    state: &mut ReplayState<S, A>,
    id: OpId,
    result: &AR,
) -> Result<(), ReplayError>
where
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    let Some(pending) = state.pending_outbound_async.remove(&id) else {
        return missing_outbound_async_error(state, id);
    };
    if !Spec::async_result_matches(&pending.op, result) {
        state.pending_outbound_async.insert(id, pending);
        return Err(ReplayError::WrongAsyncResultType(id));
    }
    state.completed_async.insert(id);
    Ok(())
}

fn abort_outbound_async<S, A>(state: &mut ReplayState<S, A>, id: OpId) -> Result<(), ReplayError> {
    if state.pending_outbound_async.remove(&id).is_some() {
        state.completed_async.insert(id);
        Ok(())
    } else {
        missing_outbound_async_error(state, id)
    }
}

fn cancel_outbound_async<S, A>(state: &mut ReplayState<S, A>, id: OpId) -> Result<(), ReplayError> {
    if state.pending_outbound_async.remove(&id).is_some() {
        state.completed_async.insert(id);
        Ok(())
    } else {
        missing_outbound_async_error(state, id)
    }
}

fn resolve_inbound_async<S, A, SR, AR, Spec>(
    state: &mut ReplayState<S, A>,
    id: OpId,
    result: &AR,
) -> Result<(), ReplayError>
where
    Spec: NextEventsSpec<S, A, SR, AR>,
{
    let Some(pending) = state.pending_inbound_async.remove(&id) else {
        return missing_inbound_async_error(state, id);
    };
    if !Spec::async_result_matches(&pending.op, result) {
        state.pending_inbound_async.insert(id, pending);
        return Err(ReplayError::WrongAsyncResultType(id));
    }
    state.completed_async.insert(id);
    Ok(())
}

fn abort_inbound_async<S, A>(state: &mut ReplayState<S, A>, id: OpId) -> Result<(), ReplayError> {
    if state.pending_inbound_async.remove(&id).is_some() {
        state.completed_async.insert(id);
        Ok(())
    } else {
        missing_inbound_async_error(state, id)
    }
}

fn cancel_inbound_async<S, A>(state: &mut ReplayState<S, A>, id: OpId) -> Result<(), ReplayError> {
    if state.pending_inbound_async.remove(&id).is_some() {
        state.completed_async.insert(id);
        Ok(())
    } else {
        missing_inbound_async_error(state, id)
    }
}

fn missing_outbound_async_error<S, A>(
    state: &ReplayState<S, A>,
    id: OpId,
) -> Result<(), ReplayError> {
    Err(match state.async_origins.get(&id) {
        Some(AsyncOrigin::Inbound) => ReplayError::InboundAsyncWrongEventKind(id),
        Some(AsyncOrigin::Outbound) if state.completed_async.contains(&id) => {
            ReplayError::AsyncAlreadyCompleted(id)
        }
        Some(AsyncOrigin::Outbound) => ReplayError::UnknownOutboundAsyncId(id),
        None => ReplayError::UnknownOutboundAsyncId(id),
    })
}

fn missing_inbound_async_error<S, A>(
    state: &ReplayState<S, A>,
    id: OpId,
) -> Result<(), ReplayError> {
    Err(match state.async_origins.get(&id) {
        Some(AsyncOrigin::Outbound) => ReplayError::OutboundAsyncWrongEventKind(id),
        Some(AsyncOrigin::Inbound) if state.completed_async.contains(&id) => {
            ReplayError::AsyncAlreadyCompleted(id)
        }
        Some(AsyncOrigin::Inbound) => ReplayError::UnknownInboundAsyncId(id),
        None => ReplayError::UnknownInboundAsyncId(id),
    })
}

#[cfg(test)]
mod tests;
