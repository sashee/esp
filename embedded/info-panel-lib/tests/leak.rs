#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

mod common;

use common::*;
use info_panel_lib::{BootReason, HttpClient, MemoryFrameSource};
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

const WARMUP_ITERATIONS: usize = 10;
const LEAK_TEST_ITERATIONS: usize = 1000;
// 1 initial fetch + warmup iterations + 1 (snapshot "before") + leak_test iterations + 1 (panic)
const TOTAL_CALLS: usize = 1 + WARMUP_ITERATIONS + 1 + LEAK_TEST_ITERATIONS + 1;

/// Minimal HTTP client for leak testing. Unlike MockHttpClient, it does NOT
/// accumulate state between calls (no Vec of URLs, no Vec of responses).
/// This eliminates test-infrastructure noise from the heap measurement.
struct LeakTestHttpClient {
    call_count: usize,
    before: Arc<Mutex<Option<usize>>>,
    after: Arc<Mutex<Option<usize>>>,
    before_call: usize,
    after_call: usize,
}

impl LeakTestHttpClient {
    fn new(
        before: Arc<Mutex<Option<usize>>>,
        after: Arc<Mutex<Option<usize>>>,
        before_call: usize,
        after_call: usize,
    ) -> Self {
        Self {
            call_count: 0,
            before,
            after,
            before_call,
            after_call,
        }
    }
}

impl HttpClient for LeakTestHttpClient {
    async fn get(
        &mut self,
        _url: &str,
    ) -> anyhow::Result<Box<dyn tft_display::FrameSource<Error = anyhow::Error>>> {
        self.call_count += 1;
        let n = self.call_count;

        // Snapshot INSIDE get(), BEFORE the Vec<u8> is allocated.
        // Previous iteration's frame has already been dropped — clean state.
        if n == self.before_call {
            *self.before.lock().unwrap() = Some(dhat::HeapStats::get().curr_bytes);
        }
        if n == self.after_call {
            *self.after.lock().unwrap() = Some(dhat::HeapStats::get().curr_bytes);
        }
        if n >= TOTAL_CALLS {
            ok("leak iteration budget reached");
        }

        Ok(Box::new(MemoryFrameSource::new(vec![0u8; 128 * 160 * 2])))
    }
}

#[test]
fn test_refresh_cycle_no_memory_leak() {
    let _profiler = dhat::Profiler::builder().testing().build();

    let before: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
    let after: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));

    let before_call = 1 + WARMUP_ITERATIONS + 1;
    let after_call = before_call + LEAK_TEST_ITERATIONS;

    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(true);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // Silent clock: does not record sleep durations
    let clock = MockClock::from_ticks_silent(&[0, 250, 30_000_001, 60_000_001]);

    let http_client = LeakTestHttpClient::new(
        before.clone(),
        after.clone(),
        before_call,
        after_call,
    );

    let (display, _display_state) = tracked_display(global_counter);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(hal(
            wifi_backend,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            led,
            )))
    }));

    assert_ok_signal(result, "leak iteration budget reached");

    let before = before.lock().unwrap().unwrap();
    let after = after.lock().unwrap().unwrap();

    // Each iteration allocates ~40KB for the HTTP response Vec<u8>,
    // then drops it after write_frame. If memory is properly freed,
    // curr_bytes should be the same at both snapshots.
    // A leak would show curr_bytes growing by ~40KB * 1000 = ~40MB.
    //
    // We allow a small tolerance (1KB) for allocator bookkeeping that
    // may shift between snapshots, but growth must be flat, not linear.
    let drift = after as i64 - before as i64;
    assert!(
        drift.abs() < 1024,
        "memory leak detected: curr_bytes drifted by {} bytes over {} iterations \
         (before: {}, after: {}). A real leak would show ~{} bytes growth.",
        drift,
        LEAK_TEST_ITERATIONS,
        before,
        after,
        40_960 * LEAK_TEST_ITERATIONS,
    );
}
