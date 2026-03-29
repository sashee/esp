# Simulator Wrapper Test Plan

## Core Lifecycle

- `start_emits_initial_events`
  - `NewRunWrapper::new(bundle).start()` returns the first outbound events in exact poll order.
- `start_creates_independent_runs`
  - two separately constructed wrappers have isolated state, isolated pending ops, and op ids starting at `0` in each run.
- `push_preserves_event_order`
  - one inbound event that causes multiple outbound events returns them in the exact order they were emitted.

## Sync Semantics

- `sync_call_blocks_until_matching_return`
  - after `CreateSync`, execution does not advance until the matching `ReturnSync` is pushed.
- `sync_requires_next_event_to_be_its_return`
  - while blocked on a sync op, any other inbound event is rejected.
- `sync_return_rejects_unknown_id`
  - a sync return for an unknown id is rejected.
- `sync_return_rejects_wrong_result_variant`
  - a sync return whose result does not match the stored sync op is rejected.
- `sync_only_one_outstanding_at_a_time`
  - the wrapper never exposes more than one unresolved sync op.

## Async Semantics

- `async_call_can_be_resolved_later`
  - `CreateAsync` followed by a later `ResolveAsync` resumes the run and emits subsequent events.
- `async_multiple_pending_ops_supported`
  - multiple async ops may be pending at the same time and can resolve in any order.
- `async_resolve_rejects_unknown_id`
  - an async resolve for an unknown id is rejected.
- `async_resolve_rejects_wrong_result_variant`
  - an async resolve whose result does not match the stored async op is rejected.
- `async_abort_completes_pending_op`
  - `AbortAsync` completes a pending async op as unfulfillable and unblocks the awaiting code.
- `async_cancel_from_run_is_emitted`
  - if the run drops a pending async future, the wrapper emits `CancelAsync` immediately.
- `async_cannot_resolve_after_cancel`
  - once canceled, an async op cannot later be resolved.
- `async_cannot_resolve_after_abort`
  - once aborted, an async op cannot later be resolved.

## Protocol Validation

- `event_kind_must_match_pending_mode`
  - sync ids cannot be answered with async events and async ids cannot be answered with sync events.
- `sync_block_rejects_async_resolve`
  - while blocked on a sync op, `ResolveAsync` is rejected.
- `sync_block_rejects_async_abort`
  - while blocked on a sync op, `AbortAsync` is rejected.
- `sync_block_rejects_async_cancel`
  - while blocked on a sync op, `CancelAsync` is rejected.
- `inbound_create_sync_is_rejected`
  - inbound `CreateSync` is rejected because sync calls are only emitted by the run.
- `duplicate_completion_is_rejected`
  - a second resolve, abort, or cancel for the same id is rejected.
- `duplicate_async_resolve_is_rejected`
  - a second `ResolveAsync` for the same async id is rejected.
- `duplicate_inbound_async_cancel_is_rejected`
  - a second `CancelAsync` for the same inbound async id is rejected.
- `unknown_inbound_async_cancel_is_rejected`
  - canceling an unknown inbound async id is rejected.
- `unknown_inbound_async_abort_is_rejected`
  - aborting an unknown inbound async id is rejected.
- `duplicate_inbound_async_id_is_rejected`
  - creating an inbound async op with an already used id is rejected.
- `duplicate_inbound_async_id_conflicts_with_outbound_id`
  - creating an inbound async op with an id already used by an outbound async op is rejected.
- `finished_run_rejects_further_events`
  - after the run terminates, further inbound events are rejected or ignored consistently.

## Termination

- `run_reboot_terminates_wrapper`
  - a simulated reboot ends the run in the expected way.
- `fatal_error_terminates_wrapper`
  - fatal run failure ends the wrapper consistently and does not lose already emitted events.
- `termination_preserves_prior_outputs`
  - outbound events emitted before termination are still returned in order.
- `drop_wrapper_while_blocked_on_sync`
  - dropping the wrapper while a sync op is outstanding shuts down cleanly.
- `drop_wrapper_with_pending_async_ops`
  - dropping the wrapper while async ops are pending shuts down cleanly.
- `bundle_build_panic_terminates_wrapper`
  - if bundle construction panics before emitting any event, the wrapper terminates consistently.

## HTTP Callback Bridge

- `http_server_start_registers_handler_lifetime`
  - starting the config HTTP backend creates a live server state with stored endpoints and handler.
- `http_request_inbound_async_flow`
  - an inbound HTTP request is modeled as an async op and later resolved with a response from the handler.
- `http_request_response_matches_request_id`
  - each HTTP response is matched to the correct inbound request id.
- `http_server_drop_ends_server_lifetime`
  - once the server handle drops, no new inbound requests are accepted.
- `http_pending_requests_abort_on_server_drop`
  - pending inbound HTTP requests are aborted if the server disappears before responding.
- `http_multiple_requests_preserve_ids`
  - multiple inbound HTTP requests can be processed sequentially without mixing ids.
- `http_request_cancel_before_response`
  - canceling an inbound HTTP request before the handler responds prevents later resolution.
- `http_non_http_request_is_aborted`
  - a non-HTTP inbound async op sent to the HTTP service loop is aborted.

## Bundle Integration

- `bundle_builds_run_future_once`
  - the bundle is consumed by `start()` and used exactly once to build the run future and simulated HAL.
- `bundle_sync_matcher_used_for_validation`
  - sync result validation is delegated to the bundle.
- `bundle_async_matcher_used_for_validation`
  - async result validation is delegated to the bundle.

## Generic Smoke Tests

- `boot_reason_sync_flow`
  - a representative run issues the expected initial sync platform call.
- `wifi_scan_happy_path`
  - a representative async operation create/resolve sequence resumes the run correctly.
- `config_portal_concurrent_waits`
  - a representative concurrent-waits path can expose multiple simultaneous async waits and resolve one while canceling the others.
- `display_init_sequence_visible`
  - a representative mixed sync/async initialization path produces the expected ordered calls.

## Suggested Implementation Order

1. Core lifecycle and sync blocking.
2. Async pending, resolve, cancel, and abort.
3. Protocol validation failures.
4. HTTP callback bridge behavior.
5. Generic smoke tests.
