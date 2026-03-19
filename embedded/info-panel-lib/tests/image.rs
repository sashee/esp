mod common;

use common::*;
use info_panel_lib::BootReason;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

// ---- HTTP fetch tests (error-path: retries + error mode) ----

#[test]
fn test_image_fetches_url_after_wifi_connect() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, http_state) = always_failing_http_client();
    let urls = http_state.get_urls.clone();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    let fetched_urls = urls.lock().unwrap();
    assert!(
        fetched_urls
            .iter()
            .any(|u| u == "http://example.com"),
        "http_client.get() must be called with the stored URL. Got: {:?}",
        *fetched_urls
    );
}

#[test]
fn test_image_fetches_empty_url_when_url_empty() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, http_state) = always_failing_http_client();
    let calls = http_state.get_calls.clone();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert!(
        *calls.lock().unwrap() > 0,
        "http_client.get() must be called even with empty URL"
    );
}

#[test]
fn test_image_retries_3_times_on_http_failure() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let (clock, sleep_durations) = sequenced_clock(&[0, 250]);
    let (http_client, http_state) = always_failing_http_client();
    let calls = http_state.get_calls.clone();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "http_client.get() must be called exactly 3 times"
    );

    // Retry backoff: 1 second sleep after each failed attempt (3 sleeps for 3 attempts)
    let sleeps = sleep_durations.lock().unwrap();
    let retry_sleeps: Vec<_> = sleeps
        .iter()
        .filter(|d| **d == embassy_time::Duration::from_secs(1))
        .collect();
    assert_eq!(
        retry_sleeps.len(),
        3,
        "must sleep 1s after each failed attempt (3 sleeps for 3 attempts). All sleeps: {:?}",
        *sleeps
    );
}

#[test]
fn test_image_succeeds_on_first_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // fail_up_to(1) = fail first call, succeed second. Then is_connected=false exits refresh loop.
    let (http_client, http_state) = fail_up_to_http_client(1);
    let calls = http_state.get_calls.clone();
    let (display, display_state) = tracked_display(global_counter);
    let write_frame_calls = display_state.write_frame_calls.clone();
    let initial_clear_calls = display_state.initial_clear_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "http_client.get() must be called exactly 2 times (1 fail + 1 success)"
    );

    assert!(
        *initial_clear_calls.lock().unwrap() == 1 && *write_frame_calls.lock().unwrap() >= 1,
        "initial clear must happen once and display.write_frame() must render fetched frame. Got fill={}, write={}",
        *initial_clear_calls.lock().unwrap(),
        *write_frame_calls.lock().unwrap()
    );
}

#[test]
fn test_image_succeeds_on_second_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // fail_up_to(2) = fail first and second calls, succeed on third. Then is_connected=false exits.
    let (http_client, http_state) = fail_up_to_http_client(2);
    let calls = http_state.get_calls.clone();
    let (display, display_state) = tracked_display(global_counter);
    let write_frame_calls = display_state.write_frame_calls.clone();
    let initial_clear_calls = display_state.initial_clear_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "http_client.get() must be called exactly 3 times (2 fails + 1 success)"
    );

    assert!(
        *initial_clear_calls.lock().unwrap() == 1 && *write_frame_calls.lock().unwrap() >= 1,
        "initial clear must happen once and display.write_frame() must render fetched frame. Got fill={}, write={}",
        *initial_clear_calls.lock().unwrap(),
        *write_frame_calls.lock().unwrap()
    );
}

#[test]
fn test_image_enters_error_mode_when_all_retries_fail() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, _http_state) = always_failing_http_client();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after retries fail. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after entering error mode"
    );
}

#[test]
fn test_image_error_mode_waits_before_restart() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let (clock, sleep_durations) = sequenced_clock(&[0, 250, 600_000_001]);
    let (http_client, _http_state) = always_failing_http_client();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert!(
        led_calls.lock().unwrap()
            .iter()
            .any(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to ERROR_LED (red) before reboot"
    );

    // Verify error mode sleep duration: 10 minutes = 600 seconds
    let sleeps = sleep_durations.lock().unwrap();
    assert!(
        sleeps
            .iter()
            .any(|d| *d == embassy_time::Duration::from_secs(600)),
        "error mode must sleep for 10 minutes (600s). Got: {:?}",
        *sleeps
    );
}

// ---- Success-path tests (panic-based assertions) ----

#[test]
fn test_image_succeeds_on_third_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // fail_up_to(2) = fail attempts 1 and 2, succeed on attempt 3. is_connected=false exits.
    let (http_client, http_state) = fail_up_to_http_client(2);
    let calls = http_state.get_calls.clone();
    let (display, display_state) = tracked_display(global_counter);
    let write_frame_calls = display_state.write_frame_calls.clone();
    let initial_clear_calls = display_state.initial_clear_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "http_client.get() must be called exactly 3 times (2 fails + 1 success on 3rd attempt)"
    );

    assert!(
        *initial_clear_calls.lock().unwrap() == 1 && *write_frame_calls.lock().unwrap() >= 1,
        "initial clear must happen once and display.write_frame() must render fetched frame. Got fill={}, write={}",
        *initial_clear_calls.lock().unwrap(),
        *write_frame_calls.lock().unwrap()
    );
}

#[test]
fn test_image_displays_frame_on_tft_when_fetch_succeeds() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let (display, display_state) = tracked_display(global_counter);
    let write_frame_calls = display_state.write_frame_calls.clone();
    let initial_clear_calls = display_state.initial_clear_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert!(
        *initial_clear_calls.lock().unwrap() == 1 && *write_frame_calls.lock().unwrap() >= 1,
        "initial clear must happen once and display.write_frame() must render fetched frame. Got fill={}, write={}",
        *initial_clear_calls.lock().unwrap(),
        *write_frame_calls.lock().unwrap()
    );
}

#[test]
fn test_image_enters_error_mode_on_write_frame_failure() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let (display, _display_state) = display_with_write_frame_fail_nth(global_counter, 1);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after write_frame failure. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after write_frame failure"
    );
}

#[test]
fn test_image_enters_error_mode_on_initial_fill_failure() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, http_state) = tracked_http_client();
    let get_calls = http_state.get_calls.clone();
    let (display, _display_state) = display_with_initial_clear_fail_nth(global_counter, 1);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after initial clear failure. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after initial clear failure"
    );

    // Image fetch should never have been reached since initial clear failed
    assert_eq!(
        *get_calls.lock().unwrap(),
        0,
        "http_client.get() must NOT be called when initial clear fails"
    );
}

#[test]
fn test_image_handles_invalid_frame_size() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // Return only 100 bytes instead of expected 128*160*2 = 40960
    let (http_client, http_state) = tracked_http_client_with_response(vec![0u8; 100]);
    let get_calls = http_state.get_calls.clone();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    assert_eq!(
        *get_calls.lock().unwrap(),
        1,
        "http_client.get() must be attempted exactly once before invalid frame size aborts rendering"
    );

    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && c.g.abs() < 0.01 && c.b.abs() < 0.01)
            .unwrap_or(false),
        "invalid frame size must enter error mode and set ERROR_LED. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06 after invalid frame size. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after invalid frame size"
    );
}

// ---- Refresh loop tests (panic-based: mock panics when refresh condition is met) ----

#[test]
fn test_image_refreshes_after_30_second_interval() {
    let led = MockLed::new();
    let wifi_backend = MockWifiBackend::new().with_is_connected(true);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let saw_refresh_sleep = Arc::new(Mutex::new(false));
    let saw_refresh_sleep_hook = saw_refresh_sleep.clone();
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0)).on_sleep(move |duration| {
        if duration == embassy_time::Duration::from_secs(30) {
            *saw_refresh_sleep_hook.lock().unwrap() = true;
        }
        None
    });
    let get_calls = Arc::new(Mutex::new(0));
    let get_calls_hook = get_calls.clone();
    let sleep_check = saw_refresh_sleep.clone();
    let http_client = MockHttpClient::new().on_get(move |_url| {
        let mut calls = get_calls_hook.lock().unwrap();
        *calls += 1;
        if *calls == 2 {
            if !*sleep_check.lock().unwrap() {
                nok("refresh fetch happened before 30-second sleep");
            }
            ok("refresh fetch observed after 30-second interval");
        }
        None
    });
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "refresh fetch observed after 30-second interval");
}

#[test]
fn test_image_aborts_refresh_on_wifi_disconnect() {
    let led_calls = Arc::new(Mutex::new(Vec::new()));
    let led_calls_hook = led_calls.clone();
    let led = MockLed::new().on_set_pixel(move |call| {
        led_calls_hook.lock().unwrap().push(call);
        None
    });
    let wifi_backend = MockWifiBackend::new().with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let get_calls = Arc::new(Mutex::new(0));
    let get_calls_hook = get_calls.clone();
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new().on_get(move |_url| {
        *get_calls_hook.lock().unwrap() += 1;
        None
    });
    let reboot_calls = get_calls.clone();
    let reboot_leds = led_calls.clone();
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software)
        .on_reboot(move || {
            let calls = *reboot_calls.lock().unwrap();
            if calls != 1 {
                nok("wifi disconnect should stop before a second HTTP fetch");
            }
            let last = reboot_leds.lock().unwrap().last().cloned();
            if !last
                .as_ref()
                .map(|c| {
                    (c.r - 1.0).abs() < 0.01
                        && c.g.abs() < 0.01
                        && c.b.abs() < 0.01
                        && (c.brightness - 0.06).abs() < 0.001
                })
                .unwrap_or(false)
            {
                nok("wifi disconnect should enter error-mode LED before reboot");
            }
            ok("wifi disconnect aborts refresh before second HTTP fetch");
        });
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "wifi disconnect aborts refresh before second HTTP fetch");
}

#[test]
fn test_image_multiple_refresh_cycles() {
    let led = MockLed::new();
    let wifi_backend = MockWifiBackend::new().with_is_connected(true);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let refresh_sleeps = Arc::new(Mutex::new(0usize));
    let refresh_sleeps_hook = refresh_sleeps.clone();
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0)).on_sleep(move |duration| {
        if duration == embassy_time::Duration::from_secs(30) {
            *refresh_sleeps_hook.lock().unwrap() += 1;
        }
        None
    });
    let get_calls = Arc::new(Mutex::new(0usize));
    let get_calls_hook = get_calls.clone();
    let refresh_sleeps_check = refresh_sleeps.clone();
    let http_client = MockHttpClient::new().on_get(move |_url| {
        let mut calls = get_calls_hook.lock().unwrap();
        *calls += 1;
        if *calls == 3 {
            if *refresh_sleeps_check.lock().unwrap() < 2 {
                nok("second refresh arrived before two 30-second sleeps");
            }
            ok("two refresh cycles observed");
        }
        None
    });
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "two refresh cycles observed");
}

#[test]
fn test_image_enters_error_mode_on_write_frame_failure_in_refresh() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(true);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, http_state) = tracked_http_client();
    let calls = http_state.get_calls.clone();
    // Fail write_frame on the 2nd rendered image frame: first successful fetch renders,
    // second fetch happens in the refresh loop and its render fails.
    let (display, _display_state) = display_with_write_frame_fail_nth(global_counter, 2);

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

    let http_calls = *calls.lock().unwrap();
    assert!(
        result.is_err(),
        "refresh-path write_frame failure should terminate via platform.reboot()"
    );
    assert_eq!(
        http_calls,
        2,
        "refresh-path write_frame failure must happen on the second HTTP fetch"
    );

    let reboot = *reboot_called.lock().unwrap();
    assert!(
        reboot,
        "refresh-path write_frame failure must enter error mode and reboot. \
         http_calls={}, reboot={}",
        http_calls,
        reboot
    );
    assert!(
        panic_message(result.err().unwrap()).contains("mock reboot"),
        "refresh-path write_frame failure should end with the reboot sentinel panic"
    );

    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after write_frame failure in refresh. Last: {:?}",
        last
    );
}

#[test]
fn test_image_fails_when_url_invalid() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "not_a_valid_url".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, http_state) = always_failing_http_client();
    let urls = http_state.get_urls.clone();
    let (display, _display_state) = tracked_display(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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

    // HTTP request was attempted with the invalid URL
    let fetched_urls = urls.lock().unwrap();
    assert!(
        fetched_urls.iter().any(|u| u == "not_a_valid_url"),
        "http_client.get() must be called with the invalid URL. Got: {:?}",
        *fetched_urls
    );

    // 3 retries were attempted
    assert_eq!(
        fetched_urls.len(),
        3,
        "http_client.get() must be called exactly 3 times (all retries fail)"
    );

    // Error mode LED: red with brightness 0.06
    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red). Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after entering error mode"
    );
}
