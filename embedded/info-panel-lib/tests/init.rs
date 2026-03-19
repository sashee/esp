mod common;

use common::*;
use info_panel_lib::BootReason;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

#[test]
fn test_init_clears_tft_on_startup() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, _http_state) = always_failing_http_client();
    let (display, display_state) = tracked_display(global_counter);

    let init_called = display_state.init_called.clone();
    let init_order = display_state.init_order.clone();
    let initial_clear_order = display_state.initial_clear_order.clone();
    let initial_clear_calls = display_state.fill_solid_calls.clone();
    let connect_order = wifi_state.connect_order.clone();
    let start_ap_order = wifi_state.start_ap_order.clone();

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

    assert!(*init_called.lock().unwrap(), "display.init() must be called");

    let init = init_order.lock().unwrap();
    let clear = initial_clear_order.lock().unwrap();
    let clear_calls = initial_clear_calls.lock().unwrap();
    let connect = connect_order.lock().unwrap();
    let start_ap = start_ap_order.lock().unwrap();

    assert!(init.is_some(), "display.init() must be called");
    assert_eq!(*clear_calls, 1, "initial clear must happen exactly once");
    assert!(clear.is_some(), "initial clear must be called");
    assert!(connect.is_some(), "wifi.connect() must be called");
    assert!(
        init.unwrap() < clear.unwrap(),
        "display.init() (order {:?}) must be called BEFORE initial clear (order {:?})",
        init,
        clear,
    );
    assert!(
        clear.unwrap() < connect.unwrap(),
        "initial clear (order {:?}) must be called BEFORE wifi.connect() (order {:?})",
        clear,
        connect
    );
    if let Some(start_ap) = *start_ap {
        assert!(
            clear.unwrap() < start_ap,
            "initial clear (order {:?}) must be called BEFORE portal AP start (order {:?})",
            clear,
            start_ap
        );
    }
}

#[test]
fn test_init_clears_tft_before_required_portal() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    let start_ap_order = Arc::new(Mutex::new(None));
    let start_ap_order_hook = start_ap_order.clone();
    let counter_for_start_ap = global_counter.clone();
    let wifi_backend = MockWifiBackend::new().on_start_access_point(move |_config| {
        *start_ap_order_hook.lock().unwrap() =
            Some(counter_for_start_ap.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        ok("required portal started");
    });
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let (display, display_state) = tracked_display(global_counter);

    let init_order = display_state.init_order.clone();
    let initial_clear_order = display_state.initial_clear_order.clone();
    let initial_clear_calls = display_state.fill_solid_calls.clone();

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
    assert_ok_signal(result, "required portal started");

    let init = init_order.lock().unwrap();
    let clear = initial_clear_order.lock().unwrap();
    let clear_calls = initial_clear_calls.lock().unwrap();
    let start_ap = start_ap_order.lock().unwrap();

    assert!(init.is_some(), "display.init() must be called");
    assert_eq!(*clear_calls, 1, "initial clear must happen exactly once");
    assert!(clear.is_some(), "initial clear must be called");
    assert!(start_ap.is_some(), "required portal must start AP");
    assert!(
        init.unwrap() < clear.unwrap(),
        "display.init() (order {:?}) must be called BEFORE initial clear (order {:?})",
        init,
        clear,
    );
    assert!(
        clear.unwrap() < start_ap.unwrap(),
        "initial clear (order {:?}) must be called BEFORE portal AP start (order {:?})",
        clear,
        start_ap,
    );
}

#[test]
fn test_init_enters_error_mode_when_display_init_fails() {
    let (led, led_calls) = tracked_led();
    let wifi_backend = MockWifiBackend::default();
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let display = FailingDisplay::new();

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

    // LED should have been set to red (ERROR_LED = 1.0, 0.0, 0.0)
    assert_eq!(
        led_calls.lock().unwrap().last().map(|c| (c.r, c.g, c.b)),
        Some((1.0, 0.0, 0.0)),
        "LED must be set to ERROR_LED (red) after display init failure"
    );

    // Verify error mode LED brightness is 0.06
    assert_eq!(
        led_calls.lock().unwrap().last().map(|c| c.brightness),
        Some(0.06),
        "ERROR_LED brightness must be 0.06"
    );

    // Verify reboot was called
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after entering error mode"
    );
}

#[test]
fn test_init_connects_wifi_when_nvs_has_complete_config() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
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

    // WiFi configure_client should have been called with stored ssid and password
    assert_eq!(
        wifi_state.configured_ssid.lock().unwrap().as_deref(),
        Some("test_ssid"),
        "wifi.configure_client must be called with stored ssid"
    );
    assert_eq!(
        wifi_state.configured_password.lock().unwrap().as_deref(),
        Some("test_pw"),
        "wifi.configure_client must be called with stored password"
    );

    // LED should have been set to orange (CONNECTING_LED = 1.0, 0.78, 0.0)
    assert!(
        led_calls.lock().unwrap()
            .iter()
            .any(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.78).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to CONNECTING_LED (orange) during connection. Got: {:?}",
        *led_calls.lock().unwrap()
    );

    // LED should have been set to blue (CONNECTED_LED = 0.0, 0.0, 1.0)
    assert!(
        led_calls.lock().unwrap()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must be set to CONNECTED_LED (blue) after connection. Got: {:?}",
        *led_calls.lock().unwrap()
    );
}

#[test]
fn test_init_goes_to_required_portal_when_led_brightness_missing() {
    let saw_green = Arc::new(Mutex::new(false));
    let saw_green_hook = saw_green.clone();
    let led = MockLed::new().on_set_pixel(move |rgb| {
        if rgb.r.abs() < 0.01 && (rgb.g - 1.0).abs() < 0.01 && rgb.b.abs() < 0.01 {
            *saw_green_hook.lock().unwrap() = true;
        }
        None
    });
    let saw_green_for_wifi = saw_green.clone();
    let wifi_backend = MockWifiBackend::new().on_start_access_point(move |config| {
        if !*saw_green_for_wifi.lock().unwrap() {
            nok("required portal AP should start after green LED is set");
        }
        if !config.ssid.starts_with("InfoPanel-") {
            nok("required portal AP SSID should start with InfoPanel-");
        }
        ok("missing brightness enters required portal");
    });

    // Store with ssid, pw, url but NO led_brightness
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
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

    assert_ok_signal(result, "missing brightness enters required portal");
}

#[test]
fn test_init_goes_to_required_portal_when_led_brightness_invalid() {
    let led = MockLed::new();
    let wifi_backend = MockWifiBackend::new().on_start_access_point(|_config| {
        ok("invalid brightness enters required portal");
    });

    // Store with all fields but led_brightness is not a valid u8
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "not_a_number".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
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

    assert_ok_signal(result, "invalid brightness enters required portal");
}

#[test]
fn test_init_goes_to_required_portal_when_config_corrupted() {
    let led = MockLed::new();
    let wifi_backend = MockWifiBackend::new().on_start_access_point(|_config| {
        ok("corrupted config enters required portal");
    });

    // Store with only ssid and pw (missing url and led_brightness)
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
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

    assert_ok_signal(result, "corrupted config enters required portal");
}

#[test]
fn test_init_enters_error_mode_when_led_set_fails_during_connect() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = failing_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
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

    // Last LED call should be red (error mode)
    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after LED failure. Last call: {:?}",
        last
    );

    // Verify error mode LED brightness is 0.06
    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    // Verify reboot was called
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after entering error mode"
    );
}

#[test]
fn test_init_sets_blue_led_when_wifi_connected() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
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

    // LED should have been set to blue (CONNECTED_LED = 0.0, 0.0, 1.0)
    // with brightness from config (128/255 ≈ 0.502)
    let expected_brightness = 128.0 / 255.0;
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 0.0).abs() < 0.01
                && (c.g - 0.0).abs() < 0.01
                && (c.b - 1.0).abs() < 0.01
                && (c.brightness - expected_brightness).abs() < 0.01
        }),
        "LED must be set to CONNECTED_LED (blue, brightness={:.3}). Got: {:?}",
        expected_brightness,
        *led_calls.lock().unwrap()
    );
}

#[test]
fn test_init_sets_orange_led_during_wifi_connection() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let (http_client, _http_state) = always_failing_http_client();
    let (display, _display_state) = tracked_display(global_counter);
    let connect_order = wifi_state.connect_order.clone();

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

    // CONNECTING_LED (orange: 1.0, 0.78, 0.0) must be set with config brightness (128/255)
    let expected_brightness = 128.0 / 255.0;
    assert!(
        led_calls.lock().unwrap().iter().any(|c| {
            (c.r - 1.0).abs() < 0.01
                && (c.g - 0.78).abs() < 0.01
                && (c.b - 0.0).abs() < 0.01
                && (c.brightness - expected_brightness).abs() < 0.01
        }),
        "LED must be set to CONNECTING_LED (orange) with brightness {:.3} during connection. Got: {:?}",
        expected_brightness,
        *led_calls.lock().unwrap()
    );

    // Verify wifi.connect() was actually called
    assert!(
        connect_order.lock().unwrap().is_some(),
        "wifi.connect() must have been called"
    );
}

#[test]
fn test_init_enters_error_mode_when_wifi_connect_fails() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.on_connect(|_timeout| Some(Err(anyhow::anyhow!("mock connect error"))));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
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

    // Error mode LED (red) with brightness 0.06
    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after wifi connect failure. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after wifi connect failure"
    );
}

#[test]
fn test_init_enters_error_mode_when_wifi_configure_fails() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, _wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.on_configure_client(|_credentials, _channel, _auth| {
        Some(Err(anyhow::anyhow!("mock configure_client error")))
    });
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
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

    // Error mode LED (red) with brightness 0.06
    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after wifi configure failure. Last: {:?}",
        last
    );

    assert!(
        last.as_ref().map(|c| (c.brightness - 0.06).abs() < 0.001).unwrap_or(false),
        "ERROR_LED brightness must be 0.06. Got: {:?}",
        last
    );

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after wifi configure failure"
    );
}
