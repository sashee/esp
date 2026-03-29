use std::time::Duration;

use super::*;
use crate::Event;

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestSyncOp {
    BootReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAsyncOp {
    Delay(Duration),
    WifiScan,
    HttpRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestSyncResult {
    BootReason(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAsyncResult {
    DelayDone,
    WifiScanDone,
    HttpResponse,
}

struct TestSpec;

impl NextEventsSpec<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> for TestSpec {
    type InboundAsyncKind = ();

    fn sync_result_matches(op: &TestSyncOp, result: &TestSyncResult) -> bool {
        matches!(
            (op, result),
            (TestSyncOp::BootReason, TestSyncResult::BootReason(_))
        )
    }

    fn async_result_matches(op: &TestAsyncOp, result: &TestAsyncResult) -> bool {
        matches!(
            (op, result),
            (TestAsyncOp::Delay(_), TestAsyncResult::DelayDone)
                | (TestAsyncOp::WifiScan, TestAsyncResult::WifiScanDone)
                | (TestAsyncOp::HttpRequest, TestAsyncResult::HttpResponse)
        )
    }

    fn async_timing(op: &TestAsyncOp) -> AsyncTiming {
        match op {
            TestAsyncOp::Delay(duration) => AsyncTiming::Delay(*duration),
            TestAsyncOp::WifiScan | TestAsyncOp::HttpRequest => AsyncTiming::Untimed,
        }
    }
}

fn start_step(
    outbound: Vec<Event<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>>,
) -> TraceStep<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> {
    TraceStep::start(outbound)
}

fn push_step(
    inbound: Event<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>,
    outbound: Vec<Event<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>>,
) -> TraceStep<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> {
    TraceStep::push(inbound, outbound)
}

#[test]
fn empty_trace_is_exact_zero() {
    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&[]),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn single_step_trace_is_exact_zero() {
    let trace = [start_step(vec![Event::CreateAsync {
        id: 1,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
    }])];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn return_sync_contributes_zero_time() {
    let trace = [
        start_step(vec![Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        }]),
        push_step(
            Event::ReturnSync {
                id: 1,
                result: TestSyncResult::BootReason(7),
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn resolved_timed_async_contributes_exact_duration() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(100))
    );
}

#[test]
fn multiple_resolved_timed_asyncs_add_exact_durations() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(20)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(30))
    );
}

#[test]
fn resolved_untimed_async_is_more_than_zero() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::ZERO)
    );
}

#[test]
fn timed_then_untimed_becomes_more_than_sum() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(100))
    );
}

#[test]
fn untimed_then_timed_becomes_more_than_sum() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(50))
    );
}

#[test]
fn multiple_untimed_resolves_stay_more_than_zero() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::ZERO)
    );
}

#[test]
fn same_step_inbound_create_and_outbound_resolve_async_is_exact_zero() {
    let trace = [
        start_step(vec![]),
        push_step(
            Event::CreateAsync {
                id: 7,
                op: TestAsyncOp::HttpRequest,
            },
            vec![Event::ResolveAsync {
                id: 7,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn later_outbound_resolve_of_inbound_created_async_is_exact_zero() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 7,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(
            Event::ReturnSync {
                id: 99,
                result: TestSyncResult::BootReason(1),
            },
            vec![Event::ResolveAsync {
                id: 7,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn inbound_async_resolution_only_reflects_outbound_timing_sources() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::ResolveAsync {
                id: 10,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(100))
    );
}

#[test]
fn inbound_async_resolution_after_untimed_outbound_async_is_more_than_zero() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![Event::ResolveAsync {
                id: 10,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::ZERO)
    );
}

#[test]
fn multiple_inbound_asyncs_do_not_add_time_by_themselves() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(
            Event::CreateAsync {
                id: 11,
                op: TestAsyncOp::HttpRequest,
            },
            vec![Event::ResolveAsync {
                id: 10,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
        push_step(
            Event::ReturnSync {
                id: 42,
                result: TestSyncResult::BootReason(2),
            },
            vec![Event::ResolveAsync {
                id: 11,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn inbound_and_outbound_async_mixed_only_counts_resolved_outbound_asyncs() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![
                Event::CreateAsync {
                    id: 1,
                    op: TestAsyncOp::Delay(Duration::from_millis(20)),
                },
                Event::CreateAsync {
                    id: 2,
                    op: TestAsyncOp::WifiScan,
                },
            ],
        ),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::ResolveAsync {
                id: 10,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(20))
    );
}

#[test]
fn multiple_timed_and_untimed_resolves_compose_correctly() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            },
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(20)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(30))
    );
}

#[test]
fn orphaned_resolve_async_is_ignored() {
    let trace = [
        start_step(vec![]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn orphaned_abort_async_is_ignored() {
    let trace = [
        start_step(vec![]),
        push_step(Event::AbortAsync { id: 1 }, vec![]),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn orphaned_cancel_async_is_ignored() {
    let trace = [
        start_step(vec![]),
        push_step(Event::CancelAsync { id: 1 }, vec![]),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn orphaned_return_sync_is_ignored() {
    let trace = [
        start_step(vec![]),
        push_step(
            Event::ReturnSync {
                id: 1,
                result: TestSyncResult::BootReason(7),
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn aborted_timed_async_contributes_zero() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        }]),
        push_step(Event::AbortAsync { id: 1 }, vec![]),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn canceled_timed_async_contributes_zero() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        }]),
        push_step(Event::CancelAsync { id: 1 }, vec![]),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn resolved_then_aborted_other_async_only_counts_resolve() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(40)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(Event::AbortAsync { id: 2 }, vec![]),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(40))
    );
}

#[test]
fn slice_excluding_create_ignores_later_resolve() {
    let full_trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(80)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&full_trace[1..]),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn slice_including_create_and_resolve_counts_time() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(80)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(80))
    );
}

#[test]
fn slice_ending_before_resolve_counts_zero() {
    let full_trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(80)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&full_trace[..1]),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn inbound_and_outbound_in_same_step_have_zero_separation() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(100))
    );
}

#[test]
fn all_events_in_same_step_are_treated_as_simultaneous() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(30)),
        }]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![
                Event::CreateSync {
                    id: 5,
                    op: TestSyncOp::BootReason,
                },
                Event::CreateAsync {
                    id: 2,
                    op: TestAsyncOp::WifiScan,
                },
            ],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(30))
    );
}

#[test]
fn same_step_inbound_create_and_outbound_resolve_does_not_count_time() {
    let trace = [
        start_step(vec![]),
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![Event::ResolveAsync {
                id: 10,
                result: TestAsyncResult::HttpResponse,
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn same_step_outbound_create_does_not_make_future_elapsed_nonzero_by_itself() {
    let trace = [
        start_step(vec![]),
        push_step(
            Event::ReturnSync {
                id: 1,
                result: TestSyncResult::BootReason(3),
            },
            vec![Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(25)),
            }],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::ZERO)
    );
}

#[test]
fn delay_then_wifi_scan_complete_is_more_than_delay() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(100))
    );
}

#[test]
fn wifi_scan_complete_then_equal_delay_is_more_than_zero_plus_delay() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::MoreThan(Duration::from_millis(100))
    );
}

#[test]
fn chain_of_resolves_accumulates_elapsed_lower_bound() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(60)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 1,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    assert_eq!(
        elapsed_time::<_, _, _, _, TestSpec>(&trace),
        ElapsedTime::Exact(Duration::from_millis(110))
    );
}
