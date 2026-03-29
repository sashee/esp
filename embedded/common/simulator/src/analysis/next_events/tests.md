# Next Events Test Plan

## Goal

Verify that `possible_next_events()` reconstructs replay state correctly, returns only valid next actions, and attaches warnings only to valid-but-suspicious actions.

For malformed replay history, verify that `possible_next_events()` fails rather than attempting best-effort recovery.

Termination is out of scope for this function. The input is only a replayable event series.

## Core Replay

- empty trace returns no protocol-driven next events.
- multi-step traces reconstruct pending state correctly across turns.

## Replay Failure Cases

- `ReturnSync` with no pending sync causes failure.
- wrong `ReturnSync` id causes failure.
- non-`ReturnSync` inbound while sync-blocked causes failure.
- resolving an unknown outbound async id causes failure.
- aborting an unknown outbound async id causes failure.
- canceling an unknown inbound async id causes failure.
- resolving an inbound async id with an outbound-only event causes failure.
- canceling an outbound async id with an inbound-only event causes failure.
- duplicate completion of an outbound async id causes failure.
- duplicate cancel of an inbound async id causes failure.
- duplicate outbound async creation id causes failure.
- duplicate inbound async creation id causes failure.
- inbound/outbound async id collision causes failure.
- wrong sync result type for the pending sync op causes failure.
- wrong async result type for the pending async op causes failure.

## Sync Behavior

- a pending outbound sync op returns only the matching `ReturnSync` possibility.
- while blocked on sync, no `ResolveAsync` or `AbortAsync` options are returned.
- while blocked on sync, no inbound async creation kinds are returned.
- after the matching `ReturnSync`, later async possibilities appear if the replayed outbound events create them.

## Outbound Async Behavior

- one pending outbound async op returns both `ResolveAsync` and `AbortAsync`.
- multiple pending outbound async ops all appear in the possible-next list.
- a resolved outbound async op no longer appears.
- an aborted outbound async op no longer appears.
- a canceled outbound async op no longer appears.
- completed or removed async ids are not reintroduced by later replay steps.

## Inbound Async Behavior

- a queued inbound async op returns `CancelInboundAsync`.
- an active inbound async op returns `CancelInboundAsync`.
- a canceled inbound async op no longer appears.
- a resolved inbound async op no longer appears.
- an aborted inbound async op no longer appears.
- multiple pending inbound async ops all expose cancel options.

## Inbound Async Kind Exposure

- inbound async kinds from `possible_inbound_async()` are included when replay is not sync-blocked and not terminated.
- inbound async kinds from `possible_inbound_async()` are included when replay is not sync-blocked.
- no inbound async kinds are returned when the spec reports none.
- inbound async kinds are suppressed while blocked on sync.
- multiple inbound async kinds can be returned together.

## Timing Warnings

- untimed async ops never receive delay warnings.
- a single pending delay can be resolved without warning.
- resolving a later-created longer delay warns if an earlier shorter delay is still pending.
- resolving a later-created equal delay warns if an earlier equal delay is still pending.
- resolving a later-created shorter delay does not warn.
- once the earlier delay resolves, the later delay warning disappears.
- once the earlier delay aborts, the later delay warning disappears.
- once the earlier delay is canceled, the later delay warning disappears.
- non-delay pending async ops do not affect delay warnings.
- warnings are attached only to `ResolveAsync`, not to `AbortAsync` or `CreateInboundAsync`.

## Mixed Scenarios

- sync followed by async replay produces the expected transition from `ReturnSync` to async next events.
- outbound async options and inbound async creation kinds can appear together when both are valid.
- outbound async options and inbound async cancel options can appear together when both are valid.
- multiple delays mixed with untimed async ops produce warnings only for the constrained delays.
- returned `ResolveAsync` and `AbortAsync` possibilities preserve the original pending op values.

## App-Specific Hook Coverage

- a spec can expose an HTTP-request inbound kind after replaying server-start events.
- that HTTP-request kind disappears after replaying server-stop or equivalent shutdown events.
- spec logic can inspect prior trace steps rather than only the current pending async set.

## Spec / Implementation Alignment

- replay errors cover wrong-domain async events and unsupported event directions.
- termination is intentionally not modeled in `possible_next_events()`.

## Notes

- Invalid replay history is asserted as a failure from `possible_next_events()`.
- Hard-invalid next actions are verified by absence from the possible-next list.
- Timing issues are verified as warnings on otherwise valid actions.
- Inbound-created async validation is only kind-level, not concrete-argument-level.
