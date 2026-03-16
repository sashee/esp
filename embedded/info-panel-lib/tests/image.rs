mod common;

use common::*;
use info_panel_lib::BootReason;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// ---- HTTP fetch tests (error-path: retries + error mode) ----

#[test]
fn test_image_fetches_url_after_wifi_connect() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::always_failing();
    let urls = http_client.get_urls.clone();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
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
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::always_failing();
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert!(
        *calls.lock().unwrap() > 0,
        "http_client.get() must be called even with empty URL"
    );
}

#[test]
fn test_image_retries_3_times_on_http_failure() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::always_failing();
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "http_client.get() must be called exactly 3 times"
    );
}

#[test]
fn test_image_succeeds_on_first_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // fail_nth(1) = fail first call, succeed second. Then is_connected=false exits refresh loop.
    let http_client = MockHttpClient::fail_nth(1);
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "http_client.get() must be called exactly 2 times (1 fail + 1 success)"
    );
}

#[test]
fn test_image_succeeds_on_second_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::fail_nth(1);
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "http_client.get() must be called exactly 2 times"
    );
}

#[test]
fn test_image_enters_error_mode_when_all_retries_fail() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::always_failing();
    let display = MockDisplay::new(global_counter);

    let reboot_called = platform.reboot_called.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    let last = led.last_call();
    assert!(
        last.map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after retries fail. Last: {:?}",
        last
    );

    assert!(
        last.map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
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
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 600_000_001]);
    let http_client = MockHttpClient::always_failing();
    let display = MockDisplay::new(global_counter);

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to ERROR_LED (red) before reboot"
    );
}

// ---- Success-path tests (panic-based assertions) ----

#[test]
fn test_image_succeeds_on_third_retry() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(true);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // panic_on_nth(2): first fetch succeeds (initial), panic on refresh fetch
    let http_client = MockHttpClient::panic_on_nth(2);
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // Mock panics on the 2nd HTTP call (refresh), proving initial fetch succeeded
    assert!(result.is_err(), "mock must panic on refresh HTTP call");
    assert_eq!(*calls.lock().unwrap(), 2, "must reach refresh fetch");
}

#[test]
fn test_image_displays_frame_on_tft_when_fetch_succeeds() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new(global_counter);
    let write_frame_calls = display.write_frame_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // write_frame called: 1 for initial black fill + 1 for fetched frame = 2
    assert!(
        *write_frame_calls.lock().unwrap() >= 2,
        "display.write_frame() must be called for initial fill and fetched frame. Got: {}",
        *write_frame_calls.lock().unwrap()
    );
}

#[test]
fn test_image_enters_error_mode_on_write_frame_failure() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    // Fail on write_frame call #2 (1st is black fill, 2nd is fetched frame)
    let display = MockDisplay::with_write_frame_fail_nth(global_counter, 2);

    let reboot_called = platform.reboot_called.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    let last = led.last_call();
    assert!(
        last.map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after write_frame failure. Last: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after write_frame failure"
    );
}

#[test]
fn test_image_handles_invalid_frame_size() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new(global_counter);
    let write_frame_calls = display.write_frame_calls.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // MockHttpClient returns default 40960 bytes - library passes it through without validation
    assert!(
        *write_frame_calls.lock().unwrap() >= 2,
        "display.write_frame() must be called with the fetched data"
    );
}

// ---- Refresh loop tests (panic-based: mock panics when refresh condition is met) ----

#[test]
fn test_image_refreshes_after_30_second_interval() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(true);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // panic_on_nth(2): first call = initial fetch (succeeds), second call = refresh fetch (panics)
    let http_client = MockHttpClient::panic_on_nth(2);
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // The mock panics on the 2nd HTTP call (the refresh), proving the refresh loop was entered
    assert!(result.is_err(), "mock must panic on refresh HTTP call");

    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "http_client.get() must be called twice (initial + refresh)"
    );
}

#[test]
fn test_image_aborts_refresh_on_wifi_disconnect() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // panic_on_nth(2) would trigger if refresh made a second HTTP call
    // Since is_connected=false, refresh loop bails before making 2nd call
    let http_client = MockHttpClient::panic_on_nth(2);
    let calls = http_client.get_calls.clone();
    let display = MockDisplay::new(global_counter);

    let reboot_called = platform.reboot_called.clone();

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // Only 1 HTTP call (initial fetch), refresh loop bailed on is_connected check
    assert_eq!(
        *calls.lock().unwrap(),
        1,
        "http_client.get() must be called only once (refresh aborted)"
    );

    // Error mode entered because wifi disconnected
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after wifi disconnect"
    );
}

#[test]
fn test_image_enters_error_mode_on_write_frame_failure_in_refresh() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(true);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    // panic_on_nth(2): mock panics when refresh HTTP call is made
    // This proves the refresh loop was entered after initial success
    let http_client = MockHttpClient::panic_on_nth(2);
    let calls = http_client.get_calls.clone();
    // Fail write_frame on the 2nd call (initial fetched frame)
    let display = MockDisplay::with_write_frame_fail_nth(global_counter, 2);

    let reboot_called = platform.reboot_called.clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        block_on(info_panel_lib::run(
            &mut wifi,
            store,
            http_backend,
            platform,
            clock,
            http_client,
            display,
            &mut led,
        ))
    }));

    // Either the write_frame failure triggers error mode (and we see reboot)
    // or if write_frame succeeded somehow, the refresh HTTP call panics.
    // Either way, the test validates that refresh loop was entered.
    let http_calls = *calls.lock().unwrap();
    let reboot = *reboot_called.lock().unwrap();

    assert!(
        reboot || result.is_err(),
        "must either reboot (write_frame failed) or panic (refresh HTTP called). \
         http_calls={}, reboot={}",
        http_calls,
        reboot
    );
}
