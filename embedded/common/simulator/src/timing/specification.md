# Timing analysis specification

## Goal

Add a reusable timing-analysis helper for `simulator` that can answer:

- what abstract elapsed time is implied by a trace slice

This helper is intended to support multiple use cases:

- local behavioral checks in test-only simulators, such as: `set_rst_low` happened after `set_rst_high`; was the elapsed time exactly `20ms`?
- future improvements to `next_events` timing warnings

The helper is not intended to reconstruct wall-clock time. It only reasons from the trace and the timing metadata available through `NextEventsSpec::async_timing(...)`.

## Proposed API

The first version operates on a trace slice and returns the elapsed abstract time between the first and last step in that slice.

```rust
pub enum ElapsedTime {
    Exact(Duration),
    MoreThan(Duration),
}

pub fn elapsed_time<S, A, SR, AR, Spec>(
    trace: &[TraceStep<S, A, SR, AR>],
) -> ElapsedTime
where
    S: Clone,
    A: Clone,
    Spec: NextEventsSpec<S, A, SR, AR>;
```

Meaning:

- `Exact(d)` means the elapsed time implied by the trace is exactly `d`
- `MoreThan(d)` means the elapsed time implied by the trace is strictly greater than `d`

## Trace-slice semantics

The function treats the provided slice as the complete analysis window.

- the elapsed time is measured between the first and the last step in the slice
- all events inside a single `TraceStep` are simultaneous
- inbound and outbound events in the same step have zero separation
- callers can choose a narrower interval by passing a subslice of the original trace

Examples:

- `elapsed_time(&[]) == Exact(0)`
- `elapsed_time(&trace[i..=i]) == Exact(0)`
- `elapsed_time(&trace[i..=j])` means the time between step `i` and step `j`

## What contributes time

Only successful resolution of outbound-created async operations contributes elapsed time.

- outbound `CreateAsync` later matched by inbound `ResolveAsync`
  - timed async op contributes its exact duration
  - untimed async op contributes a strictly positive amount of time
- inbound-created async operations do not contribute time directly, even if they resolve in a later step
- `ReturnSync` contributes `0`
- `AbortAsync` contributes `0`
- `CancelAsync` contributes `0`

The helper uses `NextEventsSpec::async_timing(...)` to determine whether an outbound async op is timed or untimed.

Rationale:

- outbound asyncs represent work initiated by `run()` whose completion can prove elapsed time
- inbound asyncs represent work initiated by the harness and are observation points, not timing sources
- if an inbound async resolves later, any elapsed time should be attributed only to resolved outbound async work that happened in between

## Slice boundary rule

The trace slice is self-contained.

If a completion event appears inside the slice, but its corresponding start event is outside the slice, the completion is ignored for elapsed-time purposes.

This applies to:

- `ResolveAsync` without a matching `CreateAsync` inside the slice
- `AbortAsync` without a matching `CreateAsync` inside the slice
- `CancelAsync` without a matching `CreateAsync` inside the slice
- `ReturnSync` without a matching `CreateSync` inside the slice

Rationale:

- the helper should not attribute time to an operation whose start is outside the analysis window
- callers can widen the slice if they want to include that operation
- this avoids rewriting `TraceStep` or creating a normalized trace representation that no longer matches replay semantics

## Same-step semantics

All events within one step are treated as happening at the same instant.

This means:

- an inbound `ResolveAsync` and the outbound events produced in response are simultaneous
- a create and resolve that both appear in the same step are both considered part of that same instant
- there is no additional time contribution from event ordering inside one step

In the current trace format, the important same-step case is:

- inbound `CreateAsync` and matching outbound `ResolveAsync` within the same step contribute `0`, because that is just `run()` responding immediately within the same instant

Outbound-created asyncs contribute only when their completion appears in a later step as an inbound `ResolveAsync`.

## Accumulation rules

Elapsed values compose as follows:

- `Exact(a) + Exact(b) = Exact(a + b)`
- `Exact(a) + MoreThan(b) = MoreThan(a + b)`
- `MoreThan(a) + Exact(b) = MoreThan(a + b)`
- `MoreThan(a) + MoreThan(b) = MoreThan(a + b)`

Consequences:

- multiple timed resolves add exactly
- any resolved untimed async makes the total inexact and strictly greater than the summed exact durations

## Expected use patterns

### Local timing assertions

Example pattern:

- scan the trace for the last step containing `set_rst_high`
- when a later step contains `set_rst_low`, call `elapsed_time(&trace[high_step..=low_step])`
- compare the result with the expected timing requirement

### Future `next_events` warnings

The warning logic can later use this helper by slicing from an earlier async creation step through the current end of trace, then comparing the returned `ElapsedTime` against that pending async's required timing.

## Contract summary

The helper's behavior is defined by these rules:

- it works on a slice of `TraceStep`s
- it returns the elapsed abstract time between the first and last step in that slice
- all events inside one step are simultaneous
- only resolved outbound-created asyncs can contribute time directly
- resolved inbound-created asyncs do not contribute time directly
- `ReturnSync`, `AbortAsync`, and `CancelAsync` are zero-time
- completion events whose matching start is outside the slice are ignored
- timed contributions remain exact unless any contributing resolved outbound async is untimed
- any contributing untimed outbound async makes the result `MoreThan(...)`

## Non-goals for the first version

- no wall-clock timestamps
- no exact count of untimed positive increments
- no separate normalized trace type
- no attempt to infer elapsed time from abort or cancel events
- no direct timing contribution from inbound-created asyncs
- no special semantics yet for zero-duration delays
