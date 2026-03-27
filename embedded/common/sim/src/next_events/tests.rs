use super::*;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestSyncOp {
    BootReason,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAsyncOp {
    WifiScan,
    Delay(Duration),
    HttpServerStart,
    HttpServerStop,
    HttpRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestSyncResult {
    BootReason(u32),
    Wrong,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum TestAsyncResult {
    WifiScanDone,
    DelayDone,
    Unit,
    HttpResponse,
    Wrong,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InboundKind {
    HttpRequest,
    Tick,
}

struct BaseSpec;

impl NextEventsSpec<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> for BaseSpec {
    type InboundAsyncKind = InboundKind;

    fn sync_result_matches(op: &TestSyncOp, result: &TestSyncResult) -> bool {
        matches!(
            (op, result),
            (TestSyncOp::BootReason, TestSyncResult::BootReason(_))
        )
    }

    fn async_result_matches(op: &TestAsyncOp, result: &TestAsyncResult) -> bool {
        matches!(
            (op, result),
            (TestAsyncOp::WifiScan, TestAsyncResult::WifiScanDone)
                | (TestAsyncOp::Delay(_), TestAsyncResult::DelayDone)
                | (TestAsyncOp::HttpServerStart, TestAsyncResult::Unit)
                | (TestAsyncOp::HttpServerStop, TestAsyncResult::Unit)
                | (TestAsyncOp::HttpRequest, TestAsyncResult::HttpResponse)
        )
    }

    fn async_timing(op: &TestAsyncOp) -> AsyncTiming {
        match op {
            TestAsyncOp::Delay(duration) => AsyncTiming::Delay(*duration),
            _ => AsyncTiming::Untimed,
        }
    }
}

struct HttpSpec;

impl NextEventsSpec<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> for HttpSpec {
    type InboundAsyncKind = InboundKind;

    fn sync_result_matches(op: &TestSyncOp, result: &TestSyncResult) -> bool {
        BaseSpec::sync_result_matches(op, result)
    }

    fn async_result_matches(op: &TestAsyncOp, result: &TestAsyncResult) -> bool {
        BaseSpec::async_result_matches(op, result)
    }

    fn async_timing(op: &TestAsyncOp) -> AsyncTiming {
        BaseSpec::async_timing(op)
    }

    fn possible_inbound_async(
        trace: &[TraceStep<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>],
    ) -> Vec<Self::InboundAsyncKind> {
        let mut running = false;
        for step in trace {
            for event in &step.outbound {
                match event {
                    Event::CreateAsync {
                        op: TestAsyncOp::HttpServerStart,
                        ..
                    } => running = true,
                    Event::CreateAsync {
                        op: TestAsyncOp::HttpServerStop,
                        ..
                    } => running = false,
                    _ => {}
                }
            }
        }

        if running {
            vec![
                InboundKind::HttpRequest,
                InboundKind::HttpRequest,
                InboundKind::Tick,
            ]
        } else {
            Vec::new()
        }
    }
}

struct NoInboundSpec;

impl NextEventsSpec<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult> for NoInboundSpec {
    type InboundAsyncKind = InboundKind;

    fn sync_result_matches(op: &TestSyncOp, result: &TestSyncResult) -> bool {
        BaseSpec::sync_result_matches(op, result)
    }

    fn async_result_matches(op: &TestAsyncOp, result: &TestAsyncResult) -> bool {
        BaseSpec::async_result_matches(op, result)
    }

    fn async_timing(op: &TestAsyncOp) -> AsyncTiming {
        BaseSpec::async_timing(op)
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

fn resolve_event(
    events: &[PossibleEvent<TestSyncOp, TestAsyncOp, InboundKind>],
    id: OpId,
) -> &PossibleEvent<TestSyncOp, TestAsyncOp, InboundKind> {
    events
        .iter()
        .find(|event| matches!(event, PossibleEvent::ResolveAsync { id: event_id, .. } if *event_id == id))
        .unwrap()
}

#[test]
fn empty_trace_returns_no_events() {
    let events = possible_next_events::<_, _, _, _, BaseSpec>(&[]).unwrap();
    assert!(events.is_empty());
}

#[test]
fn pending_sync_returns_only_matching_return_sync() {
    let trace = [start_step(vec![Event::CreateSync {
        id: 0,
        op: TestSyncOp::BootReason,
    }])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![PossibleEvent::ReturnSync {
            id: 0,
            op: TestSyncOp::BootReason,
        }]
    );
}

#[test]
fn pending_sync_suppresses_async_and_inbound_kinds() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 0,
            op: TestAsyncOp::HttpServerStart,
        },
        Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        },
    ])];

    let events = possible_next_events::<_, _, _, _, HttpSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![PossibleEvent::ReturnSync {
            id: 1,
            op: TestSyncOp::BootReason,
        }]
    );
}

#[test]
fn pending_outbound_async_returns_resolve_and_abort() {
    let trace = [start_step(vec![Event::CreateAsync {
        id: 7,
        op: TestAsyncOp::WifiScan,
    }])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 7,
                op: TestAsyncOp::WifiScan,
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 7,
                op: TestAsyncOp::WifiScan,
            },
        ]
    );
}

#[test]
fn multiple_pending_outbound_asyncs_all_appear() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 7,
            op: TestAsyncOp::WifiScan,
        },
        Event::CreateAsync {
            id: 8,
            op: TestAsyncOp::Delay(Duration::from_millis(5)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(events.len(), 4);
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 7,
        op: TestAsyncOp::WifiScan,
        warnings: vec![],
    }));
    assert!(events.contains(&PossibleEvent::AbortAsync {
        id: 7,
        op: TestAsyncOp::WifiScan,
    }));
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 8,
        op: TestAsyncOp::Delay(Duration::from_millis(5)),
        warnings: vec![],
    }));
    assert!(events.contains(&PossibleEvent::AbortAsync {
        id: 8,
        op: TestAsyncOp::Delay(Duration::from_millis(5)),
    }));
}

#[test]
fn pending_inbound_async_returns_cancel() {
    let trace = [push_step(
        Event::CreateAsync {
            id: 55,
            op: TestAsyncOp::HttpRequest,
        },
        vec![],
    )];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![PossibleEvent::CancelInboundAsync {
            id: 55,
            op: TestAsyncOp::HttpRequest,
        }]
    );
}

#[test]
fn multiple_pending_inbound_asyncs_all_expose_cancel() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 55,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(
            Event::CreateAsync {
                id: 56,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.contains(&PossibleEvent::CancelInboundAsync {
        id: 55,
        op: TestAsyncOp::HttpRequest,
    }));
    assert!(events.contains(&PossibleEvent::CancelInboundAsync {
        id: 56,
        op: TestAsyncOp::HttpRequest,
    }));
}

#[test]
fn inbound_kinds_are_exposed_and_deduplicated() {
    let trace = [start_step(vec![Event::CreateAsync {
        id: 1,
        op: TestAsyncOp::HttpServerStart,
    }])];

    let events = possible_next_events::<_, _, _, _, HttpSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 1,
                op: TestAsyncOp::HttpServerStart,
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 1,
                op: TestAsyncOp::HttpServerStart,
            },
            PossibleEvent::CreateInboundAsync {
                kind: InboundKind::HttpRequest,
            },
            PossibleEvent::CreateInboundAsync {
                kind: InboundKind::Tick,
            },
        ]
    );
}

#[test]
fn no_inbound_kinds_returned_when_spec_reports_none() {
    let trace = [start_step(vec![Event::CreateAsync {
        id: 1,
        op: TestAsyncOp::WifiScan,
    }])];

    let events = possible_next_events::<_, _, _, _, NoInboundSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 1,
                op: TestAsyncOp::WifiScan,
            },
        ]
    );
}

#[test]
fn inbound_kinds_disappear_after_server_stop() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::HttpServerStart,
        }]),
        start_step(vec![Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::HttpServerStop,
        }]),
    ];

    let events = possible_next_events::<_, _, _, _, HttpSpec>(&trace).unwrap();
    assert!(!events.contains(&PossibleEvent::CreateInboundAsync {
        kind: InboundKind::HttpRequest,
    }));
    assert!(!events.contains(&PossibleEvent::CreateInboundAsync {
        kind: InboundKind::Tick,
    }));
}

#[test]
fn single_delay_has_no_warning() {
    let trace = [start_step(vec![Event::CreateAsync {
        id: 1,
        op: TestAsyncOp::Delay(Duration::from_millis(10)),
    }])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
        ]
    );
}

#[test]
fn untimed_async_has_no_warning() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::WifiScan,
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::WifiScan,
        warnings: vec![],
    }));
}

#[test]
fn later_equal_delay_has_no_warning() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::Delay(Duration::from_millis(10)),
        warnings: vec![],
    }));
}

#[test]
fn later_longer_delay_still_warns_while_earlier_shorter_pending() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
            PossibleEvent::ResolveAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
                warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
                    pending_id: 1,
                    pending_duration: Duration::from_millis(10),
                })],
            },
            PossibleEvent::AbortAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]
    );
}

#[test]
fn later_shorter_delay_has_no_warning() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events[2],
        PossibleEvent::ResolveAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
            warnings: vec![],
        }
    );
}

#[test]
fn chain_accumulation_warns_when_earlier_long_delay_must_have_finished() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(60)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 3,
        op: TestAsyncOp::Delay(Duration::from_millis(60)),
        warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
            pending_id: 1,
            pending_duration: Duration::from_millis(100),
        })],
    }));
}

#[test]
fn repeated_chain_accumulation_warns_once_elapsed_exceeds_earlier_delay() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(200)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 4,
                op: TestAsyncOp::Delay(Duration::from_millis(101)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 4,
        op: TestAsyncOp::Delay(Duration::from_millis(101)),
        warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
            pending_id: 1,
            pending_duration: Duration::from_millis(200),
        })],
    }));
}

#[test]
fn accumulated_exact_tie_has_no_warning() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(40)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(60)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        resolve_event(&events, 3),
        &PossibleEvent::ResolveAsync {
            id: 3,
            op: TestAsyncOp::Delay(Duration::from_millis(60)),
            warnings: vec![],
        }
    );
}

#[test]
fn created_after_elapsed_time_with_equal_deadline_has_no_warning() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(90)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        resolve_event(&events, 3),
        &PossibleEvent::ResolveAsync {
            id: 3,
            op: TestAsyncOp::Delay(Duration::from_millis(90)),
            warnings: vec![],
        }
    );
}

#[test]
fn created_after_elapsed_time_with_strictly_later_deadline_warns() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 2,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(91)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 3,
        op: TestAsyncOp::Delay(Duration::from_millis(91)),
        warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
            pending_id: 1,
            pending_duration: Duration::from_millis(100),
        })],
    }));
}

#[test]
fn overlapping_older_pending_delay_warns_when_strictly_earlier() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(90)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 4,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 4,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
        warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
            pending_id: 1,
            pending_duration: Duration::from_millis(90),
        })],
    }));
}

#[test]
fn overlapping_equal_deadline_has_no_warning() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
        ]),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::DelayDone,
            },
            vec![Event::CreateAsync {
                id: 4,
                op: TestAsyncOp::Delay(Duration::from_millis(50)),
            }],
        ),
        push_step(
            Event::ResolveAsync {
                id: 4,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        resolve_event(&events, 2),
        &PossibleEvent::ResolveAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
            warnings: vec![],
        }
    );
}

#[test]
fn untimed_completion_makes_later_equal_delay_strictly_later() {
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
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 3,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
        warnings: vec![Warning::Timing(TimingWarning::EarlierDelayStillPending {
            pending_id: 1,
            pending_duration: Duration::from_millis(100),
        })],
    }));
}

#[test]
fn untimed_completion_does_not_force_shorter_later_delay() {
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
                id: 2,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::Delay(Duration::from_millis(99)),
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        resolve_event(&events, 3),
        &PossibleEvent::ResolveAsync {
            id: 3,
            op: TestAsyncOp::Delay(Duration::from_millis(99)),
            warnings: vec![],
        }
    );
}

#[test]
fn multiple_pending_earlier_delays_warn_on_later_resolution() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(30)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(60)),
        },
        Event::CreateAsync {
            id: 3,
            op: TestAsyncOp::Delay(Duration::from_millis(90)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 3,
        op: TestAsyncOp::Delay(Duration::from_millis(90)),
        warnings: vec![
            Warning::Timing(TimingWarning::EarlierDelayStillPending {
                pending_id: 1,
                pending_duration: Duration::from_millis(30),
            }),
            Warning::Timing(TimingWarning::EarlierDelayStillPending {
                pending_id: 2,
                pending_duration: Duration::from_millis(60),
            }),
        ],
    }));
}

#[test]
fn warning_disappears_after_earlier_delay_resolves() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
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
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]
    );
}

#[test]
fn warning_disappears_after_earlier_delay_aborts() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]),
        push_step(Event::AbortAsync { id: 1 }, vec![]),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
        warnings: vec![],
    }));
}

#[test]
fn warning_disappears_after_earlier_delay_is_canceled() {
    let trace = [
        start_step(vec![
            Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Delay(Duration::from_millis(10)),
            },
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Delay(Duration::from_millis(100)),
            },
        ]),
        start_step(vec![Event::CancelAsync { id: 1 }]),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
        warnings: vec![],
    }));
}

#[test]
fn warnings_only_apply_to_resolve_async() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(10)),
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::Delay(Duration::from_millis(100)),
        },
    ])];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::AbortAsync {
        id: 2,
        op: TestAsyncOp::Delay(Duration::from_millis(100)),
    }));
}

#[test]
fn sync_followed_by_async_transitions_to_async_options() {
    let trace = [
        start_step(vec![Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        }]),
        push_step(
            Event::ReturnSync {
                id: 1,
                result: TestSyncResult::BootReason(9),
            },
            vec![Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            }],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert_eq!(
        events,
        vec![
            PossibleEvent::ResolveAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
                warnings: vec![],
            },
            PossibleEvent::AbortAsync {
                id: 2,
                op: TestAsyncOp::WifiScan,
            },
        ]
    );
}

#[test]
fn outbound_async_and_inbound_kinds_can_appear_together() {
    let trace = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::HttpServerStart,
        },
        Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::WifiScan,
        },
    ])];

    let events = possible_next_events::<_, _, _, _, HttpSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 2,
        op: TestAsyncOp::WifiScan,
        warnings: vec![],
    }));
    assert!(events.contains(&PossibleEvent::CreateInboundAsync {
        kind: InboundKind::HttpRequest,
    }));
}

#[test]
fn outbound_async_and_inbound_cancel_can_appear_together() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(
            Event::CreateAsync {
                id: 9,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
    ];

    let events = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap();
    assert!(events.contains(&PossibleEvent::ResolveAsync {
        id: 1,
        op: TestAsyncOp::WifiScan,
        warnings: vec![],
    }));
    assert!(events.contains(&PossibleEvent::CancelInboundAsync {
        id: 9,
        op: TestAsyncOp::HttpRequest,
    }));
}

#[test]
fn return_sync_without_pending_sync_fails() {
    let trace = [push_step(
        Event::ReturnSync {
            id: 1,
            result: TestSyncResult::BootReason(7),
        },
        vec![],
    )];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::UnknownSyncId(1));
}

#[test]
fn wrong_return_sync_id_fails() {
    let trace = [
        start_step(vec![Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        }]),
        push_step(
            Event::ReturnSync {
                id: 2,
                result: TestSyncResult::BootReason(7),
            },
            vec![],
        ),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(
        err,
        ReplayError::SyncIdMismatch {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn non_return_sync_while_sync_blocked_fails() {
    let trace = [
        start_step(vec![Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        }]),
        push_step(
            Event::ResolveAsync {
                id: 99,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::UnexpectedEventWhileSyncBlocked);
}

#[test]
fn wrong_async_domain_fails() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 9,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 9,
                result: TestAsyncResult::HttpResponse,
            },
            vec![],
        ),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::InboundAsyncWrongEventKind(9));
}

#[test]
fn canceling_outbound_async_with_inbound_only_event_fails() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 9,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(Event::CancelAsync { id: 9 }, vec![]),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::OutboundAsyncWrongEventKind(9));
}

#[test]
fn resolving_unknown_outbound_async_fails() {
    let trace = [push_step(
        Event::ResolveAsync {
            id: 9,
            result: TestAsyncResult::WifiScanDone,
        },
        vec![],
    )];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::UnknownOutboundAsyncId(9));
}

#[test]
fn aborting_unknown_outbound_async_fails() {
    let trace = [push_step(Event::AbortAsync { id: 9 }, vec![])];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::UnknownOutboundAsyncId(9));
}

#[test]
fn canceling_unknown_inbound_async_fails() {
    let trace = [push_step(Event::CancelAsync { id: 9 }, vec![])];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::UnknownInboundAsyncId(9));
}

#[test]
fn duplicate_completion_of_outbound_async_fails() {
    let trace = [
        start_step(vec![Event::CreateAsync {
            id: 3,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::WifiScanDone,
            },
            vec![],
        ),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::AsyncAlreadyCompleted(3));
}

#[test]
fn duplicate_cancel_of_inbound_async_fails() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(Event::CancelAsync { id: 3 }, vec![]),
        push_step(Event::CancelAsync { id: 3 }, vec![]),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::AsyncAlreadyCompleted(3));
}

#[test]
fn duplicate_inbound_async_creation_id_fails() {
    let trace = [
        push_step(
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(
            Event::CreateAsync {
                id: 3,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
    ];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::DuplicateAsyncId(3));
}

#[test]
fn resolved_aborted_and_canceled_asyncs_no_longer_appear() {
    let resolved = [
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
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&resolved)
        .unwrap()
        .is_empty());

    let aborted = [
        start_step(vec![Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(Event::AbortAsync { id: 2 }, vec![]),
    ];
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&aborted)
        .unwrap()
        .is_empty());

    let canceled = [
        start_step(vec![Event::CreateAsync {
            id: 3,
            op: TestAsyncOp::WifiScan,
        }]),
        start_step(vec![Event::CancelAsync { id: 3 }]),
    ];
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&canceled)
        .unwrap()
        .is_empty());
}

#[test]
fn resolved_aborted_and_canceled_inbound_asyncs_no_longer_appear() {
    let canceled = [
        push_step(
            Event::CreateAsync {
                id: 10,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
        push_step(Event::CancelAsync { id: 10 }, vec![]),
    ];
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&canceled)
        .unwrap()
        .is_empty());

    let resolved = [push_step(
        Event::CreateAsync {
            id: 11,
            op: TestAsyncOp::HttpRequest,
        },
        vec![Event::ResolveAsync {
            id: 11,
            result: TestAsyncResult::HttpResponse,
        }],
    )];
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&resolved)
        .unwrap()
        .is_empty());

    let aborted = [push_step(
        Event::CreateAsync {
            id: 12,
            op: TestAsyncOp::HttpRequest,
        },
        vec![Event::AbortAsync { id: 12 }],
    )];
    assert!(possible_next_events::<_, _, _, _, BaseSpec>(&aborted)
        .unwrap()
        .is_empty());
}

#[test]
fn duplicate_async_id_and_collisions_fail() {
    let duplicate = [start_step(vec![
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::WifiScan,
        },
        Event::CreateAsync {
            id: 1,
            op: TestAsyncOp::Delay(Duration::from_millis(1)),
        },
    ])];
    let err = possible_next_events::<_, _, _, _, BaseSpec>(&duplicate).unwrap_err();
    assert_eq!(err, ReplayError::DuplicateAsyncId(1));

    let collision = [
        start_step(vec![Event::CreateAsync {
            id: 2,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(
            Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::HttpRequest,
            },
            vec![],
        ),
    ];
    let err = possible_next_events::<_, _, _, _, BaseSpec>(&collision).unwrap_err();
    assert_eq!(err, ReplayError::AsyncIdCollision(2));
}

#[test]
fn wrong_result_types_fail() {
    let wrong_async = [
        start_step(vec![Event::CreateAsync {
            id: 3,
            op: TestAsyncOp::WifiScan,
        }]),
        push_step(
            Event::ResolveAsync {
                id: 3,
                result: TestAsyncResult::DelayDone,
            },
            vec![],
        ),
    ];
    let err = possible_next_events::<_, _, _, _, BaseSpec>(&wrong_async).unwrap_err();
    assert_eq!(err, ReplayError::WrongAsyncResultType(3));

    let wrong_sync = [
        start_step(vec![Event::CreateSync {
            id: 4,
            op: TestSyncOp::BootReason,
        }]),
        push_step(
            Event::ReturnSync {
                id: 4,
                result: TestSyncResult::Wrong,
            },
            vec![],
        ),
    ];
    let err = possible_next_events::<_, _, _, _, BaseSpec>(&wrong_sync).unwrap_err();
    assert_eq!(err, ReplayError::WrongSyncResultType(4));
}

#[test]
fn outbound_return_sync_is_rejected() {
    let trace = [start_step(vec![Event::ReturnSync {
        id: 1,
        result: TestSyncResult::BootReason(7),
    }])];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::OutboundReturnSyncUnsupported);
}

#[test]
fn inbound_create_sync_is_rejected() {
    let trace = [push_step(
        Event::CreateSync {
            id: 1,
            op: TestSyncOp::BootReason,
        },
        vec![],
    )];

    let err = possible_next_events::<_, _, _, _, BaseSpec>(&trace).unwrap_err();
    assert_eq!(err, ReplayError::InboundCreateSyncUnsupported);
}
