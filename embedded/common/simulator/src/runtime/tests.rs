use super::*;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
enum TestSyncOp {
    BootReason,
    Echo,
    Reboot,
}

#[derive(Clone, Debug, PartialEq)]
enum TestAsyncOp {
    ScanNetworks,
    Sleep(&'static str),
    CancelMe,
    AbortMe,
    HttpRequest(String),
    Never,
}

#[derive(Debug, PartialEq)]
enum TestSyncResult {
    BootReason(u32),
    Echo(&'static str),
    Reboot,
}

#[derive(Debug, PartialEq)]
enum TestAsyncResult {
    ScanNetworks(Result<Vec<&'static str>, &'static str>),
    SleepDone(&'static str),
    HttpResponse(String),
    Never,
}

struct TestBundle<F> {
    build: F,
}

impl<F, Fut> SimBundle for TestBundle<F>
where
    F: FnOnce(SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>) -> Fut
        + Send
        + 'static,
    Fut: Future + Send + 'static,
{
    type SyncOp = TestSyncOp;
    type AsyncOp = TestAsyncOp;
    type SyncResult = TestSyncResult;
    type AsyncResult = TestAsyncResult;
    type RunFuture = Fut;

    fn build(
        self,
        driver: SimDriver<Self::SyncOp, Self::AsyncOp, Self::SyncResult, Self::AsyncResult>,
    ) -> Self::RunFuture {
        (self.build)(driver)
    }

    fn sync_result_matches(op: &Self::SyncOp, result: &Self::SyncResult) -> bool {
        matches!(
            (op, result),
            (TestSyncOp::BootReason, TestSyncResult::BootReason(_))
                | (TestSyncOp::Echo, TestSyncResult::Echo(_))
                | (TestSyncOp::Reboot, TestSyncResult::Reboot)
        )
    }

    fn async_result_matches(op: &Self::AsyncOp, result: &Self::AsyncResult) -> bool {
        matches!(
            (op, result),
            (TestAsyncOp::ScanNetworks, TestAsyncResult::ScanNetworks(_))
                | (TestAsyncOp::Sleep(_), TestAsyncResult::SleepDone(_))
                | (TestAsyncOp::HttpRequest(_), TestAsyncResult::HttpResponse(_))
                | (TestAsyncOp::Never, TestAsyncResult::Never)
        )
    }
}

fn bundle<F, Fut>(build: F) -> TestBundle<F>
where
    F: FnOnce(SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>) -> Fut
        + Send
        + 'static,
    Fut: Future + Send + 'static,
{
    TestBundle { build }
}

fn assert_panic_contains<F: FnOnce() -> R, R>(f: F, expected: &str) {
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .err()
        .expect("expected panic");
    let msg = if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else {
        "<non-string panic>".to_string()
    };
    assert!(msg.contains(expected), "expected `{msg}` to contain `{expected}`");
}

async fn forever() {
    let () = std::future::pending::<()>().await;
    unreachable!()
}

#[test]
fn start_emits_initial_events() {
    let (wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();
    drop(wrapper);

    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::BootReason }]);
}

#[test]
fn start_creates_independent_runs() {
    let (wrapper1, events1) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();
    let (wrapper2, events2) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_eq!(events1, vec![Event::CreateSync { id: 0, op: TestSyncOp::BootReason }]);
    assert_eq!(events2, vec![Event::CreateSync { id: 0, op: TestSyncOp::BootReason }]);

    let mut wrapper1 = wrapper1;
    let mut wrapper2 = wrapper2;
    let out1 = wrapper1.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::BootReason(11),
    });
    assert!(out1.is_empty());

    let out2 = wrapper2.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::BootReason(22),
    });
    assert!(out2.is_empty());
}

#[test]
fn push_preserves_event_order() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        match driver.create_async(TestAsyncOp::ScanNetworks).await {
            AsyncCompletion::Resolved(TestAsyncResult::ScanNetworks(_)) => {}
            _ => panic!("unexpected completion"),
        }
        let _a = driver.create_async(TestAsyncOp::Sleep("one"));
        let _b = driver.create_async(TestAsyncOp::Sleep("two"));
        forever().await;
    }))
    .start();
    assert_eq!(events, vec![Event::CreateAsync { id: 0, op: TestAsyncOp::ScanNetworks }]);

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::ScanNetworks(Ok(vec!["a"])),
    });
    assert_eq!(
        out,
        vec![
            Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("one") },
            Event::CreateAsync { id: 2, op: TestAsyncOp::Sleep("two") },
        ]
    );
}

#[test]
fn sync_call_blocks_until_matching_return() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        let _pending = driver.create_async(TestAsyncOp::Sleep("after-sync"));
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::BootReason(1),
    });

    assert_eq!(out, vec![Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("after-sync") }]);
}

#[test]
fn sync_requires_next_event_to_be_its_return() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 99,
                op: TestAsyncOp::Never,
            });
        },
        "next event must be its ReturnSync",
    );
}

#[test]
fn sync_block_rejects_async_resolve() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::Never,
            });
        },
        "next event must be its ReturnSync",
    );
}

#[test]
fn sync_block_rejects_async_abort() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::AbortAsync { id: 0 });
        },
        "next event must be its ReturnSync",
    );
}

#[test]
fn sync_block_rejects_async_cancel() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CancelAsync { id: 0 });
        },
        "next event must be its ReturnSync",
    );
}

#[test]
fn sync_return_rejects_unknown_id() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();
    assert_eq!(events.len(), 1);

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ReturnSync {
                id: 5,
                result: TestSyncResult::BootReason(1),
            });
        },
        "sync response id mismatch",
    );
}

#[test]
fn sync_return_rejects_wrong_result_variant() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ReturnSync {
                id: 0,
                result: TestSyncResult::Echo("nope"),
            });
        },
        "sync result does not match op",
    );
}

#[test]
fn sync_only_one_outstanding_at_a_time() {
    let (wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        let _ = driver.create_sync(TestSyncOp::Echo);
        forever().await;
    }))
    .start();
    drop(wrapper);
    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::BootReason }]);
}

#[test]
fn async_call_can_be_resolved_later() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_async(TestAsyncOp::ScanNetworks).await;
        let _ = driver.create_sync(TestSyncOp::Echo);
        forever().await;
    }))
    .start();
    assert_eq!(events, vec![Event::CreateAsync { id: 0, op: TestAsyncOp::ScanNetworks }]);

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::ScanNetworks(Ok(vec!["a", "b"])),
    });

    assert_eq!(out, vec![Event::CreateSync { id: 1, op: TestSyncOp::Echo }]);
}

#[test]
fn async_multiple_pending_ops_supported() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let late = driver.create_async(TestAsyncOp::Sleep("late"));
        let early = driver.create_async(TestAsyncOp::Sleep("early"));
        let _ = early.await;
        let _ = late.await;
        let _ = driver.create_sync(TestSyncOp::Echo);
        forever().await;
    }))
    .start();
    assert_eq!(
        events,
        vec![
            Event::CreateAsync { id: 0, op: TestAsyncOp::Sleep("late") },
            Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("early") },
        ]
    );

    let out = wrapper.push(Event::ResolveAsync {
        id: 1,
        result: TestAsyncResult::SleepDone("early"),
    });
    assert!(out.is_empty());

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::SleepDone("late"),
    });
    assert_eq!(out, vec![Event::CreateSync { id: 2, op: TestSyncOp::Echo }]);
}

#[test]
fn async_resolve_rejects_unknown_id() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 77,
                result: TestAsyncResult::ScanNetworks(Ok(vec![])),
            });
        },
        "unknown async id 77",
    );
}

#[test]
fn async_resolve_rejects_wrong_result_variant() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::SleepDone("wrong"),
            });
        },
        "async result does not match op",
    );
}

#[test]
fn async_abort_completes_pending_op() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        match driver.create_async(TestAsyncOp::AbortMe).await {
            AsyncCompletion::Aborted => {}
            _ => panic!("expected abort"),
        }
        let _ = driver.create_sync(TestSyncOp::Echo);
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::AbortAsync { id: 0 });
    assert_eq!(out, vec![Event::CreateSync { id: 1, op: TestSyncOp::Echo }]);
}

#[test]
fn async_cancel_from_run_is_emitted() {
    let (wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let fut = driver.create_async(TestAsyncOp::CancelMe);
        drop(fut);
        forever().await;
    }))
    .start();
    drop(wrapper);

    assert_eq!(
        events,
        vec![
            Event::CreateAsync { id: 0, op: TestAsyncOp::CancelMe },
            Event::CancelAsync { id: 0 },
        ]
    );
}

#[test]
fn async_cannot_resolve_after_cancel() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let fut = driver.create_async(TestAsyncOp::CancelMe);
        drop(fut);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::SleepDone("x"),
            });
        },
        "unknown async id 0",
    );
}

#[test]
fn async_cannot_resolve_after_abort() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::AbortMe);
        forever().await;
    }))
    .start();
    let out = wrapper.push(Event::AbortAsync { id: 0 });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::SleepDone("x"),
            });
        },
        "async id 0 is already completed",
    );
}

#[test]
fn event_kind_must_match_pending_mode() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ReturnSync {
                id: 0,
                result: TestSyncResult::BootReason(1),
            });
        },
        "unknown sync id 0",
    );

    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::Never,
            });
        },
        "next event must be its ReturnSync",
    );
}

#[test]
fn inbound_create_sync_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateSync {
                id: 99,
                op: TestSyncOp::Echo,
            });
        },
        "inbound CreateSync is not supported",
    );
}

#[test]
fn duplicate_completion_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::ScanNetworks(Ok(vec![])),
    });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::AbortAsync { id: 0 });
        },
        "already completed",
    );
}

#[test]
fn duplicate_async_resolve_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::ScanNetworks(Ok(vec![])),
    });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::ScanNetworks(Ok(vec![])),
            });
        },
        "already completed",
    );
}

#[test]
fn duplicate_inbound_async_cancel_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|_driver| async move {
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 7,
        op: TestAsyncOp::HttpRequest("/x".to_string()),
    });
    assert!(out.is_empty());

    let out = wrapper.push(Event::CancelAsync { id: 7 });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CancelAsync { id: 7 });
        },
        "already canceled",
    );
}

#[test]
fn unknown_inbound_async_cancel_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|_driver| async move {
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CancelAsync { id: 88 });
        },
        "unknown inbound async id 88",
    );
}

#[test]
fn unknown_inbound_async_abort_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|_driver| async move {
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::HttpRequest("/x".to_string()),
            });
            let _ = wrapper.push(Event::AbortAsync { id: 1 });
        },
        "unknown async id 1",
    );
}

#[test]
fn duplicate_inbound_async_id_is_rejected() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|_driver| async move {
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 7,
        op: TestAsyncOp::HttpRequest("/one".to_string()),
    });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 7,
                op: TestAsyncOp::HttpRequest("/two".to_string()),
            });
        },
        "duplicate async id 7",
    );
}

#[test]
fn duplicate_inbound_async_id_conflicts_with_outbound_id() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 0,
                op: TestAsyncOp::HttpRequest("/two".to_string()),
            });
        },
        "duplicate async id 0",
    );
}

#[test]
fn finished_run_rejects_further_events() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|_driver| async move {})).start();
    assert!(events.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Never,
            });
        },
        "cannot push after termination",
    );
}

#[test]
fn run_reboot_terminates_wrapper() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::Reboot);
    }))
    .start();

    let out = wrapper.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::Reboot,
    });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 2,
                op: TestAsyncOp::Never,
            });
        },
        "cannot push after termination",
    );
}

#[test]
fn fatal_error_terminates_wrapper() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::Echo);
        panic!("fatal");
    }))
    .start();

    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::Echo }]);

    let out = wrapper.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::Echo("ok"),
    });
    assert!(out.is_empty());

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Never,
            });
        },
        "cannot push after termination",
    );
}

#[test]
fn drop_wrapper_while_blocked_on_sync() {
    let (wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();

    drop(wrapper);
}

#[test]
fn drop_wrapper_with_pending_async_ops() {
    let (wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::ScanNetworks);
        forever().await;
    }))
    .start();

    drop(wrapper);
}

#[test]
fn bundle_build_panic_terminates_wrapper() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|_driver| {
        panic!("build panic");
        #[allow(unreachable_code)]
        async move {}
    }))
    .start();

    assert!(events.is_empty());
    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::CreateAsync {
                id: 1,
                op: TestAsyncOp::Never,
            });
        },
        "cannot push after termination",
    );
}

#[test]
fn termination_preserves_prior_outputs() {
    let (_wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::Echo);
        panic!("fatal");
    }))
    .start();

    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::Echo }]);
}

#[test]
fn http_server_start_registers_handler_lifetime() {
    let active = Arc::new(AtomicBool::new(false));
    let handled = Arc::new(AtomicBool::new(false));
    let seen = active.clone();
    let handled_seen = handled.clone();
    let (wrapper, _events) = NewRunWrapper::new(bundle(move |driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start(move |request| {
            let handled = handled_seen.clone();
            async move {
                handled.store(true, Ordering::SeqCst);
                format!("ok:{request}")
            }
        });
        seen.store(true, Ordering::SeqCst);
        forever().await;
    }))
    .start();

    assert!(active.load(Ordering::SeqCst));
    let mut wrapper = wrapper;
    let out = wrapper.push(Event::CreateAsync {
        id: 400,
        op: TestAsyncOp::HttpRequest("/alive".to_string()),
    });
    assert_eq!(out, vec![Event::ResolveAsync {
        id: 400,
        result: TestAsyncResult::HttpResponse("ok:/alive".to_string()),
    }]);
    assert!(handled.load(Ordering::SeqCst));
}

#[test]
fn http_request_inbound_async_flow() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start(|request| async move { format!("resp:{request}") });
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 77,
        op: TestAsyncOp::HttpRequest("/".to_string()),
    });
    assert_eq!(
        out,
        vec![Event::ResolveAsync {
            id: 77,
            result: TestAsyncResult::HttpResponse("resp:/".to_string()),
        }]
    );
}

#[test]
fn http_request_response_matches_request_id() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start(|request| async move { format!("resp:{request}") });
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 123,
        op: TestAsyncOp::HttpRequest("/save".to_string()),
    });
    assert_eq!(
        out[0],
        Event::ResolveAsync {
            id: 123,
            result: TestAsyncResult::HttpResponse("resp:/save".to_string()),
        }
    );
}

#[test]
fn http_server_drop_ends_server_lifetime() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let handle = server.start(|request| async move { format!("resp:{request}") });
        drop(handle);
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 50,
        op: TestAsyncOp::HttpRequest("/dead".to_string()),
    });
    assert_eq!(out, vec![Event::AbortAsync { id: 50 }]);
}

#[test]
fn http_pending_requests_abort_on_server_drop() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let handle = server.start_with_wait(|request, driver| async move {
            let _ = driver.create_async(TestAsyncOp::Never).await;
            format!("resp:{request}")
        });
        let drop_driver = driver.clone();
        driver.spawn(async move {
            let _ = drop_driver.create_async(TestAsyncOp::Sleep("drop-server")).await;
            drop(handle);
        });
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 91,
        op: TestAsyncOp::HttpRequest("/wait".to_string()),
    });
    assert_eq!(
        out,
        vec![
            Event::CreateAsync { id: 0, op: TestAsyncOp::Never },
            Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("drop-server") },
        ]
    );

    let out = wrapper.push(Event::ResolveAsync {
        id: 1,
        result: TestAsyncResult::SleepDone("drop-server"),
    });
    assert_eq!(out, vec![Event::AbortAsync { id: 91 }]);
}

#[test]
fn http_multiple_requests_preserve_ids() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start(|request| async move { format!("resp:{request}") });
        forever().await;
    }))
    .start();

    let out1 = wrapper.push(Event::CreateAsync {
        id: 101,
        op: TestAsyncOp::HttpRequest("/one".to_string()),
    });
    let out2 = wrapper.push(Event::CreateAsync {
        id: 102,
        op: TestAsyncOp::HttpRequest("/two".to_string()),
    });

    assert_eq!(out1, vec![Event::ResolveAsync {
        id: 101,
        result: TestAsyncResult::HttpResponse("resp:/one".to_string()),
    }]);
    assert_eq!(out2, vec![Event::ResolveAsync {
        id: 102,
        result: TestAsyncResult::HttpResponse("resp:/two".to_string()),
    }]);
}

#[test]
fn http_request_cancel_before_response() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start_with_wait(|request, driver| async move {
            let _ = driver.create_async(TestAsyncOp::Never).await;
            format!("resp:{request}")
        });
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 201,
        op: TestAsyncOp::HttpRequest("/cancel".to_string()),
    });
    assert_eq!(out, vec![Event::CreateAsync {
        id: 0,
        op: TestAsyncOp::Never,
    }]);

    let out = wrapper.push(Event::CancelAsync { id: 201 });
    assert!(out.is_empty());

    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::Never,
    });
    assert!(out.is_empty());
}

#[test]
fn http_non_http_request_is_aborted() {
    let (mut wrapper, _events) = NewRunWrapper::new(bundle(|driver| async move {
        let server = MockHttpServer::new(driver.clone());
        let _handle = server.start(|request| async move { format!("resp:{request}") });
        forever().await;
    }))
    .start();

    let out = wrapper.push(Event::CreateAsync {
        id: 301,
        op: TestAsyncOp::Never,
    });
    assert_eq!(out, vec![Event::AbortAsync { id: 301 }]);
}

#[test]
fn bundle_builds_run_future_once() {
    let called = Arc::new(Mutex::new(0usize));
    let called_clone = called.clone();
    let (wrapper, _events) = NewRunWrapper::new(bundle(move |_driver| {
        let called = called_clone.clone();
        async move {
            *called.lock().unwrap() += 1;
        }
    }))
    .start();
    drop(wrapper);
    assert_eq!(*called.lock().unwrap(), 1);
}

#[test]
fn bundle_sync_matcher_used_for_validation() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::Echo);
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ReturnSync {
                id: 0,
                result: TestSyncResult::BootReason(1),
            });
        },
        "sync result does not match op",
    );
}

#[test]
fn bundle_async_matcher_used_for_validation() {
    let (mut wrapper, _) = NewRunWrapper::new(bundle(|driver| async move {
        let _pending = driver.create_async(TestAsyncOp::Sleep("x"));
        forever().await;
    }))
    .start();

    assert_panic_contains(
        move || {
            let _ = wrapper.push(Event::ResolveAsync {
                id: 0,
                result: TestAsyncResult::Never,
            });
        },
        "async result does not match op",
    );
}

struct MockHttpServer {
    driver: SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>,
}

struct MockServerHandle {
    active: Arc<AtomicBool>,
    in_flight: Arc<Mutex<Vec<OpId>>>,
    driver: SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>,
}

impl Drop for MockServerHandle {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        let ids = self.in_flight.lock().unwrap().clone();
        for id in ids {
            self.driver.abort_inbound_async(id);
        }
    }
}

impl MockHttpServer {
    fn new(driver: SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>) -> Self {
        Self { driver }
    }

    fn start<H, Fut>(&self, handler: H) -> MockServerHandle
    where
        H: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        self.start_with_wait(move |request, _driver| handler(request))
    }

    fn start_with_wait<H, Fut>(&self, handler: H) -> MockServerHandle
    where
        H: Fn(String, SimDriver<TestSyncOp, TestAsyncOp, TestSyncResult, TestAsyncResult>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = String> + Send + 'static,
    {
        let active = Arc::new(AtomicBool::new(true));
        let in_flight = Arc::new(Mutex::new(Vec::new()));
        let active_loop = active.clone();
        let in_flight_loop = in_flight.clone();
        let driver = self.driver.clone();
        let loop_driver = self.driver.clone();
        self.driver.spawn(async move {
            loop {
                let inbound = loop_driver.next_inbound_async().await;
                match inbound.op {
                    TestAsyncOp::HttpRequest(path) => {
                        if !active_loop.load(Ordering::SeqCst) {
                            loop_driver.abort_inbound_async(inbound.id);
                            continue;
                        }
                        in_flight_loop.lock().unwrap().push(inbound.id);
                        let response = handler(path, loop_driver.clone()).await;
                        in_flight_loop.lock().unwrap().retain(|id| *id != inbound.id);
                        if active_loop.load(Ordering::SeqCst) {
                            loop_driver.resolve_inbound_async(
                                inbound.id,
                                TestAsyncResult::HttpResponse(response),
                            );
                        } else {
                            loop_driver.abort_inbound_async(inbound.id);
                        }
                    }
                    _ => {
                        loop_driver.abort_inbound_async(inbound.id);
                    }
                }
            }
        });
        MockServerHandle {
            active,
            in_flight,
            driver,
        }
    }
}

#[test]
fn boot_reason_sync_flow() {
    let (wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::BootReason);
        forever().await;
    }))
    .start();
    drop(wrapper);
    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::BootReason }]);
}

#[test]
fn wifi_scan_happy_path() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_async(TestAsyncOp::ScanNetworks).await;
        let _pending = driver.create_async(TestAsyncOp::Sleep("after-scan"));
        forever().await;
    }))
    .start();
    assert_eq!(events, vec![Event::CreateAsync { id: 0, op: TestAsyncOp::ScanNetworks }]);
    let out = wrapper.push(Event::ResolveAsync {
        id: 0,
        result: TestAsyncResult::ScanNetworks(Ok(vec!["wifi"])),
    });
    assert_eq!(out, vec![Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("after-scan") }]);
}

#[test]
fn config_portal_concurrent_waits() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let client = driver.create_async(TestAsyncOp::Sleep("client"));
        let stopped = driver.create_async(TestAsyncOp::Sleep("stopped"));
        let tick = driver.create_async(TestAsyncOp::Sleep("tick"));
        let _ = tick.await;
        drop(client);
        drop(stopped);
        forever().await;
    }))
    .start();
    assert_eq!(
        events,
        vec![
            Event::CreateAsync { id: 0, op: TestAsyncOp::Sleep("client") },
            Event::CreateAsync { id: 1, op: TestAsyncOp::Sleep("stopped") },
            Event::CreateAsync { id: 2, op: TestAsyncOp::Sleep("tick") },
        ]
    );

    let out = wrapper.push(Event::ResolveAsync {
        id: 2,
        result: TestAsyncResult::SleepDone("tick"),
    });
    assert_eq!(out, vec![Event::CancelAsync { id: 0 }, Event::CancelAsync { id: 1 }]);
}

#[test]
fn display_init_sequence_visible() {
    let (mut wrapper, events) = NewRunWrapper::new(bundle(|driver| async move {
        let _ = driver.create_sync(TestSyncOp::Echo);
        let _ = driver.create_sync(TestSyncOp::BootReason);
        let _pending = driver.create_async(TestAsyncOp::Sleep("20ms"));
        forever().await;
    }))
    .start();
    assert_eq!(events, vec![Event::CreateSync { id: 0, op: TestSyncOp::Echo }]);
    let out = wrapper.push(Event::ReturnSync {
        id: 0,
        result: TestSyncResult::Echo("ok"),
    });
    assert_eq!(out, vec![Event::CreateSync { id: 1, op: TestSyncOp::BootReason }]);

    let out = wrapper.push(Event::ReturnSync {
        id: 1,
        result: TestSyncResult::BootReason(42),
    });
    assert_eq!(out, vec![Event::CreateAsync { id: 2, op: TestAsyncOp::Sleep("20ms") }]);
}
