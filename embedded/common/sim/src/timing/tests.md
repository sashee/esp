# Timing helper test plan

This document lists the planned tests for `timing::elapsed_time()`.

## Core behavior

- `empty_trace_is_exact_zero`
  - `[]` -> `Exact(0)`
- `single_step_trace_is_exact_zero`
  - one step with any events -> `Exact(0)`
- `return_sync_contributes_zero_time`
  - `CreateSync`, later `ReturnSync` -> `Exact(0)`

## Timed async

- `resolved_timed_async_contributes_exact_duration`
  - create `Delay(100)`, later resolve -> `Exact(100ms)`
- `multiple_resolved_timed_asyncs_add_exact_durations`
  - resolve `Delay(10)`, then resolve `Delay(20)` -> `Exact(30ms)`

## Untimed async

- `resolved_untimed_async_is_more_than_zero`
  - create untimed op, later resolve -> `MoreThan(0)`
- `timed_then_untimed_becomes_more_than_sum`
  - resolve `Delay(100)`, then resolve untimed -> `MoreThan(100ms)`
- `untimed_then_timed_becomes_more_than_sum`
  - resolve untimed, then resolve `Delay(50)` -> `MoreThan(50ms)`
- `multiple_untimed_resolves_stay_more_than_zero`
  - resolve two untimed ops -> `MoreThan(0)`

## Inbound async timing semantics

- `same_step_inbound_create_and_outbound_resolve_async_is_exact_zero`
  - inbound async create and outbound resolve in one step -> `Exact(0)`
- `later_outbound_resolve_of_inbound_created_async_is_exact_zero`
  - inbound async created in one step and resolved in a later step, with no resolved outbound async in between -> `Exact(0)`
- `inbound_async_resolution_only_reflects_outbound_timing_sources`
  - inbound async created, outbound `Delay(100)` created and later resolved, then inbound async resolved -> `Exact(100ms)`
- `inbound_async_resolution_after_untimed_outbound_async_is_more_than_zero`
  - inbound async created, outbound untimed async resolved, then inbound async resolved -> `MoreThan(0)`
- `multiple_inbound_asyncs_do_not_add_time_by_themselves`
  - several inbound async creates/resolves with no resolved outbound asyncs -> `Exact(0)`
- `inbound_and_outbound_async_mixed_only_counts_resolved_outbound_asyncs`
  - mixed inbound-created and outbound-created asyncs -> only resolved outbound-created asyncs contribute time

## Mixed composition

- `multiple_timed_and_untimed_resolves_compose_correctly`
  - resolve `Delay(10)`, untimed, `Delay(20)` -> `MoreThan(30ms)`

## Orphaned events at slice start

- `orphaned_resolve_async_is_ignored`
  - slice begins with resolve for async created before slice -> `Exact(0)`
- `orphaned_abort_async_is_ignored`
  - slice begins with abort for async created before slice -> `Exact(0)`
- `orphaned_cancel_async_is_ignored`
  - slice begins with cancel for async created before slice -> `Exact(0)`
- `orphaned_return_sync_is_ignored`
  - slice begins with return for sync created before slice -> `Exact(0)`

## Abort and cancel semantics

- `aborted_timed_async_contributes_zero`
  - create `Delay(100)`, later abort -> `Exact(0)`
- `canceled_timed_async_contributes_zero`
  - create `Delay(100)`, later cancel -> `Exact(0)`
- `resolved_then_aborted_other_async_only_counts_resolve`
  - one resolved timed async and one aborted async -> only resolved one counts

## Slice boundary semantics

- `slice_excluding_create_ignores_later_resolve`
  - full trace has create then resolve; sliced trace starts after create -> resolve ignored
- `slice_including_create_and_resolve_counts_time`
  - slice includes both create and resolve -> duration counted
- `slice_ending_before_resolve_counts_zero`
  - create inside slice, resolve outside slice -> `Exact(0)`

## Step semantics

- `inbound_and_outbound_in_same_step_have_zero_separation`
  - inbound resolve and outbound create in one step do not introduce extra separation
- `all_events_in_same_step_are_treated_as_simultaneous`
  - ordering within one step does not change result
- `same_step_inbound_create_and_outbound_resolve_does_not_count_time`
  - same-step inbound-created async resolved outbound contributes zero
- `same_step_outbound_create_does_not_make_future_elapsed_nonzero_by_itself`
  - outbound async created in one step but not yet resolved still contributes zero for that slice

## High-value realistic scenarios

- `delay_then_wifi_scan_complete_is_more_than_delay`
  - resolve `Delay(100)`, then resolve untimed -> `MoreThan(100ms)`
- `wifi_scan_complete_then_equal_delay_is_more_than_zero_plus_delay`
  - resolve untimed, then resolve `Delay(100)` -> `MoreThan(100ms)`
- `chain_of_resolves_accumulates_elapsed_lower_bound`
  - resolve `Delay(50)`, then resolve `Delay(60)` -> `Exact(110ms)`

## Notes

- `ReturnSync` is always zero-time
- only successful `ResolveAsync` contributes time
- only outbound-created asyncs can contribute time directly
- orphaned completion events caused by slicing are ignored, not counted
- there is no special behavior planned yet for zero-duration delays
