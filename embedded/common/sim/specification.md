# Simulator Wrapper Implementation Plan

## Goal

Build a generic, event-driven simulator wrapper core in `embedded/common/sim` that can drive a library's `run()` future through typed sync and async operations without hardcoding one concrete application into the core.

The core owns the runtime mechanics only. Application-visible values such as boot reason, MAC address, store contents, wifi results, and inbound HTTP requests must stay protocol-visible and flow through events rather than through wrapper construction.

## Public Model

Use a generic wrapper split into pre-start and started states:

```rust
pub struct NewRunWrapper<B: SimBundle> { ... }
pub struct RunWrapper<B: SimBundle> { ... }
```

Startup is one-shot and consumes the bundle-backed pre-start wrapper:

```rust
impl<B: SimBundle> NewRunWrapper<B> {
    pub fn new(bundle: B) -> Self;
    pub fn start(self) -> (RunWrapper<B>, Vec<Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>>);
}
```

The started wrapper is driven by one inbound event at a time:

```rust
impl<B: SimBundle> RunWrapper<B> {
    pub fn push(
        &mut self,
        event: Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>,
    ) -> Vec<Event<B::SyncOp, B::AsyncOp, B::SyncResult, B::AsyncResult>>;
}
```

## Event Protocol

Use separate sync and async request/result domains, with validation by `op_id` against stored pending ops.

```rust
pub enum Event<S, A, SR, AR> {
    CreateSync { id: OpId, op: S },
    ReturnSync { id: OpId, result: SR },

    CreateAsync { id: OpId, op: A },
    ResolveAsync { id: OpId, result: AR },
    CancelAsync { id: OpId },
    AbortAsync { id: OpId },
}
```

Semantics:

- `CreateSync`: emitted by the run when it needs an immediate answer.
- `ReturnSync`: supplied by the environment; must be the next inbound event for the matching sync id.
- `CreateAsync`: emitted by the run when it starts an async operation.
- `ResolveAsync`: supplied by the fulfiller to complete the async op normally.
- `CancelAsync`: emitted by the creator when it is no longer interested in the async op.
- `AbortAsync`: supplied by the fulfiller when it will never produce a result.

The wrapper must preserve outbound event ordering exactly.

## Bundle Abstraction

Keep the wrapper core generic by moving app-specific wiring into a consumed bundle.

```rust
pub trait SimBundle: Sized {
    type SyncOp;
    type AsyncOp;
    type SyncResult;
    type AsyncResult;
    type RunFuture: Future;

    fn build(
        self,
        driver: SimDriver<Self::SyncOp, Self::AsyncOp, Self::SyncResult, Self::AsyncResult>,
    ) -> Self::RunFuture;

    fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool;
    fn async_result_matches(op: &Self::AsyncOp, result: &Self::AsyncResult) -> bool;
}
```

Responsibilities:

- build the simulated HAL/backends using the provided driver.
- construct and return the concrete `run()` future.
- define how sync results match sync ops.
- define how async results match async ops.
- define the application's termination output type through `RunFuture::Output`.

This keeps the common wrapper independent of the exact application while allowing application crates to define their own op spaces and backend mappings.

## Core Internal State

The started wrapper should track:

- the pinned `run()` future returned by the bundle.
- the next operation id, starting at `0`.
- at most one pending sync op.
- zero or more pending async ops.
- a queue of newly emitted outbound events.
- a completion/termination state for the run.
- a waker/executor context for deterministic polling.

Suggested internal structures:

```rust
struct PendingSync<S> {
    id: OpId,
    op: S,
}

struct PendingAsync<A> {
    id: OpId,
    op: A,
}
```

The wrapper should validate inbound results against the stored pending op for that id.

## Execution Rules

### Start

- `start()` builds the driver and run future through the bundle.
- operation ids begin at `0` for each started run.
- it polls the run until one of these happens:
  - a sync op is emitted and remains unresolved,
  - all current progress stops on async waits,
  - the run terminates.
- it returns the emitted outbound events.

### Sync Calls

- only one sync op may be outstanding at a time.
- after a `CreateSync`, the wrapper is blocked until the next inbound event is the matching `ReturnSync`.
- no other inbound event is valid in that state.
- while blocked on a sync op, the wrapper makes no further internal progress.

### Async Calls

- multiple async ops may be pending simultaneously.
- `ResolveAsync` and `AbortAsync` must target a known pending async id.
- if the run drops a pending async future before completion, the wrapper emits `CancelAsync`.

### Termination

- once the run terminates, later inbound events are invalid.
- any outbound events emitted before termination must still be returned to the caller.

## Driver / Backend Boundary

The common core needs a driver object that simulated backends can use to talk to the wrapper runtime.

The driver should support:

- creating sync ops and blocking until the matching sync result is injected.
- creating async ops and producing futures that resolve later.
- emitting async cancellation when a created future is dropped.
- receiving async resolution or abort from the wrapper.

This driver is the bridge that lets app-specific bundles map trait method calls to the generic event protocol.

## Error Handling

Define clear protocol errors for invalid inbound events:

- unknown id
- wrong mode for id
- wrong result type for op
- duplicate completion
- unexpected event while a sync op is blocking
- event after termination

The wrapper should surface these as explicit errors internally; the public API can later decide whether to panic, return a `Result`, or store fatal state.

For v1, invalid inbound events should panic. That keeps the public API minimal (`start()` and `push()` only) while still making protocol violations obvious during simulator development.

## HTTP Callback Support

The core should not assume polling-style inbound services.

For callback-based hosted services such as the config HTTP server:

- the bundle's simulated backend stores the registered callback and server lifetime.
- inbound requests are represented as async operations created from outside the run.
- when the handler completes, the run emits the corresponding async resolution.
- server drop ends the lifetime and aborts any pending inbound requests.

This keeps the production callback abstraction intact while still fitting the generic wrapper protocol.

## Implementation Phases

1. Define `Event`, `OpId`, protocol error types, and `SimBundle`.
2. Implement `NewRunWrapper` / `RunWrapper` and the internal executor loop.
3. Implement pending sync/async bookkeeping and validation.
4. Implement the driver used by simulated backends.
5. Add unit tests for lifecycle, sync, async, and protocol validation.
6. Add a small app-side bundle example to prove the integration pattern.
7. Add callback-hosted service coverage, including HTTP-style request/response.

## First Consumer

The first consumer should be `info-panel-lib`, but the common wrapper should remain free of `info-panel-lib`-specific types. `info-panel-lib` should provide its own bundle, op enums, result enums, and simulated HAL adapters on top of the generic core.
