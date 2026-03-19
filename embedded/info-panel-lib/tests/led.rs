mod common;

use common::*;
use info_panel_lib::BootReason;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

#[test]
fn test_led_uses_default_brightness_for_portal() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let wifi_backend = MockWifiBackend::default();
    let store = empty_config_store(); // no led_brightness → triggers required portal
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_001]);
    let http_client = MockHttpClient::new();
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

    // REQUIRED_PORTAL_LED with PORTAL_LED_BRIGHTNESS (0.06)
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 0.0).abs() < 0.01
                && (c.g - 1.0).abs() < 0.01
                && (c.b - 0.0).abs() < 0.01
                && (c.brightness - 0.06).abs() < 0.001
        }),
        "portal LED must use brightness 0.06. Got: {:?}",
        *led_calls.lock().unwrap()
    );
}

#[test]
fn test_led_uses_config_brightness_for_connecting() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "200".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // CONNECTING_LED (orange: 1.0, 0.78, 0.0) with brightness 200/255 ≈ 0.784
    let expected_brightness = 200.0 / 255.0;
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 1.0).abs() < 0.01
                && (c.g - 0.78).abs() < 0.01
                && (c.b - 0.0).abs() < 0.01
                && (c.brightness - expected_brightness).abs() < 0.01
        }),
        "connecting LED must use brightness {:.3}. Got: {:?}",
        expected_brightness,
        *led_calls.lock().unwrap()
    );
}

#[test]
fn test_led_uses_config_brightness_for_connected() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // CONNECTED_LED (blue: 0.0, 0.0, 1.0) with brightness 128/255 ≈ 0.502
    let expected_brightness = 128.0 / 255.0;
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 0.0).abs() < 0.01
                && (c.g - 0.0).abs() < 0.01
                && (c.b - 1.0).abs() < 0.01
                && (c.brightness - expected_brightness).abs() < 0.01
        }),
        "connected LED must use brightness {:.3}. Got: {:?}",
        expected_brightness,
        *led_calls.lock().unwrap()
    );
}

#[test]
fn test_led_off_when_brightness_is_zero() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "0".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // All non-error LEDs should have brightness 0.0
    let binding = led_calls.lock().unwrap();
    let non_error_calls: Vec<_> = binding
        .iter()
        .filter(|c| !((c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01))
        .collect();

    assert!(
        non_error_calls.iter().all(|c| c.brightness.abs() < 0.001),
        "non-error LEDs must have brightness 0.0. Got: {:?}",
        non_error_calls
    );
}

#[test]
fn test_led_max_brightness_when_brightness_is_255() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "255".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // CONNECTING_LED (orange) with brightness 255/255 = 1.0
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 1.0).abs() < 0.01
                && (c.g - 0.78).abs() < 0.01
                && (c.b - 0.0).abs() < 0.01
                && (c.brightness - 1.0).abs() < 0.001
        }),
        "connecting LED must use brightness 1.0. Got: {:?}",
        *led_calls.lock().unwrap()
    );
}
