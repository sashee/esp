# Next Events Specification

## Goal

Add a simulator-facing "next events" module that can inspect a replayable trace and report which inbound events are currently valid.

This module is separate from the wrapper runtime. The wrapper remains unchanged and continues to operate on concrete `Event<S, A, SR, AR>` values.

## Location

Create a dedicated subdirectory:

- `embedded/common/sim/src/next_events/`

This subdirectory will contain the next-events implementation and its tests so the feature is easy to discover and maintain.

## Purpose

The next-events module is used by higher-level simulators and UIs to:

- determine which inbound events can legally happen next.
- omit impossible protocol actions entirely.
- attach warnings to valid-but-suspicious actions.
- support trace validation by checking whether a candidate event appears in the possible-next set for the prior trace prefix.

## Non-Goals

- do not change the wrapper runtime API.
- do not move runtime mechanics out of `src/lib.rs`.
- do not require explicit simulated elapsed time such as `AdvanceTime`.
- do not invent concrete payloads for inbound-created operations such as HTTP requests.

## Core Model

The module will analyze a full trace of wrapper interaction.

The trace should preserve wrapper turns, so each step records:

- the single inbound event supplied to `push()`, if any.
- the outbound events returned by `start()` or `push()`.

Planned shape:

```rust
pub struct TraceStep<S, A, SR, AR> {
    pub inbound: Option<Event<S, A, SR, AR>>,
    pub outbound: Vec<Event<S, A, SR, AR>>,
}
```

The module will expose a function conceptually like:

```rust
pub fn possible_next_events<S, A, SR, AR, Spec>(
    trace: &[TraceStep<S, A, SR, AR>],
) -> Result<Vec<PossibleEvent<S, A, Spec::InboundAsyncKind>>, ReplayError>
where
    Spec: NextEventsSpec<S, A, SR, AR>;
```

This function returns only valid next actions for a valid replay trace.

- if an action is invalid, it is omitted.
- if an action is valid but suspicious, it is included with warnings.
- if the replay trace itself is invalid, the function fails.

The function does not reason about run termination. It operates only on the replayed event series.

## Possible Event Shapes

The returned values represent event kinds, not always fully materialized `Event` values, because some valid next actions still require user-provided payloads.

Planned shape:

```rust
pub enum PossibleEvent<S, A, K> {
    ReturnSync { id: OpId, op: S },
    ResolveAsync { id: OpId, op: A, warnings: Vec<Warning> },
    AbortAsync { id: OpId, op: A },
    CreateInboundAsync { kind: K },
    CancelInboundAsync { id: OpId, op: A },
}
```

Notes:

- `ReturnSync` is returned only when the wrapper is blocked on a sync op.
- `ResolveAsync` is returned only for currently pending outbound async ops.
- `AbortAsync` is returned only for currently pending outbound async ops.
- `CreateInboundAsync` represents that an inbound-created async operation of a given kind is allowed now.
- `CancelInboundAsync` is returned only for currently active/queued inbound async ops.

`CreateInboundAsync` intentionally names a kind rather than a full operation instance. For example, the module can report that an HTTP request is possible without guessing its path/body.

## Spec Hook

The next-events logic needs a small simulator-facing trait layer that is independent from `SimBundle`.

Planned shape:

```rust
pub trait NextEventsSpec<S, A, SR, AR> {
    type InboundAsyncKind;

    fn sync_result_matches(op: &S, result: &SR) -> bool;
    fn async_result_matches(op: &A, result: &AR) -> bool;

    fn async_timing(op: &A) -> AsyncTiming {
        AsyncTiming::Untimed
    }

    fn possible_inbound_async(
        trace: &[TraceStep<S, A, SR, AR>],
    ) -> Vec<Self::InboundAsyncKind> {
        Vec::new()
    }
}
```

Responsibilities:

- define result/op matching for replay validation.
- classify async ops for timing analysis.
- report which inbound-created async operation kinds are allowed at the current trace prefix.

This keeps wrapper-specific semantics out of the runtime while giving the simulator enough information to drive a UI.

## Timing Classification

Async operations fall into two groups:

- untimed async ops such as wifi scanning, where only "later than creation" is known.
- delay-like async ops, where the requested duration is known.

Planned shape:

```rust
pub enum AsyncTiming {
    Untimed,
    Delay(std::time::Duration),
}
```

For v1, only `Delay(Duration)` receives timing analysis.

## Warning Model

Hard-invalid protocol actions are omitted from the result entirely.

Warnings are only attached to valid actions. Initially, warnings cover timing issues for delay-like async ops.

Planned shape:

```rust
pub enum Warning {
    Timing(TimingWarning),
}

pub enum TimingWarning {
    EarlierDelayStillPending {
        pending_id: OpId,
        pending_duration: std::time::Duration,
    },
}
```

The exact warning payload may evolve, but the initial intent is:

- if resolving one delay now would overtake an earlier-created shorter-or-equal delay that is still pending, keep the action visible but attach a timing warning.

## Replay Errors

Invalid replay input is treated as a bug and causes `possible_next_events()` to fail.

Planned shape:

```rust
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
```

The exact variants may evolve, but the important behavior is:

- invalid prior trace data causes an error.
- omission is only for invalid next actions, not for malformed replay history.
- wrong-domain events for known ids are replay errors.
- unsupported event directions such as inbound `CreateSync` or outbound `ReturnSync` are replay errors.

## Validation Semantics

The module reconstructs protocol state from the trace prefix.

It should track at least:

- the pending sync op, if any.
- pending outbound async ops created by the run.
- pending inbound async ops created by the simulator.
- enough creation-order information for timed async warning checks.

Rules:

- while blocked on sync, only the matching `ReturnSync` is possible.
- a resolved, aborted, or canceled async id is no longer pending.
- a duplicate completion or completion of an unknown id in the replay history is a replay error.
- applying an outbound-only async event to an inbound id, or an inbound-only async event to an outbound id, is a replay error.
- inbound-created async kinds are offered only if the spec says they are possible at this point.
- duplicate ids or id collisions in the replay history are replay errors.
- wrong result/op pairings in the replay history are replay errors.

## Delay Warning Rule

Given the agreed semantics for delay operations:

- `CreateAsync Delay(d)` starts the delay.
- `ResolveAsync` means the delay actually ended.

This allows ordering constraints without explicit timestamps.

For v1, the module should warn when:

- an earlier-created pending delay has duration less than or equal to the delay being resolved.

Example warning case:

- `CreateAsync Delay(10)`
- `CreateAsync Delay(100)`
- possible next includes `ResolveAsync(100)` with a timing warning because `Delay(10)` is still pending.

This remains a warning rather than a hard error in the next-events API.

## HTTP / Inbound Service Example

The generic module does not know what an HTTP server is.

Instead, the simulator-specific `NextEventsSpec` implementation can inspect the trace and decide whether an inbound async kind such as `HttpRequest` is currently possible.

For example:

- if prior events show that the server has started and not stopped, `possible_inbound_async()` can return `HttpRequest`.
- otherwise it returns nothing.

## Integration Plan

Add a new module tree:

- `embedded/common/sim/src/next_events/mod.rs`
- `embedded/common/sim/src/next_events/tests.rs`

And re-export the public API from `embedded/common/sim/src/lib.rs` so downstream simulators can import it from the `sim` crate directly.

## Status

This file is a planning document for the next-events module only. No runtime changes are implied beyond adding the new module and re-exports.
