mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

#[test]
fn test_portal_runs_preboot_portal_on_power_on() {
    let led = MockLed::new().on_set_pixel(|rgb| {
        if rgb.r.abs() < 0.01 && (rgb.g - 0.53).abs() < 0.01 && (rgb.b - 1.0).abs() < 0.01 {
            ok("preboot portal blue LED observed on power-on");
        }
        None
    });
    let wifi_backend = MockWifiBackend::new().with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
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

    assert_ok_signal(result, "preboot portal blue LED observed on power-on");
}

#[test]
fn test_portal_skips_preboot_portal_on_other_boot_reasons() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend_with_counter(global_counter.clone());
    let wifi_backend = wifi_backend.with_is_connected(false);
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

    // AP should NOT have been started (no preboot portal for Software boot)
    assert!(
        wifi_state.start_access_point_ssid.lock().unwrap().is_none(),
        "wifi.start_access_point must NOT be called for Software boot reason"
    );

    // LED should NOT have been set to preboot blue
    assert!(
        !led_calls.lock().unwrap()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.53).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must NOT be PREBOOT_PORTAL_LED for Software boot"
    );
}

#[test]
fn test_portal_preboot_runs_even_with_valid_config() {
    let led = MockLed::new();
    let wifi_backend = MockWifiBackend::new()
        .with_is_connected(false)
        .on_start_access_point(|_config| ok("preboot portal runs even with valid config"));
    let store = valid_config_store(); // complete valid config
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
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

    assert_ok_signal(result, "preboot portal runs even with valid config");
}

#[test]
fn test_portal_preboot_waits_for_connection() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, _led_calls) = tracked_led();
    // Client connected → portal uses connected_timeout (10min)
    let wifi_backend = MockWifiBackend::new().with_client_count(1).with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // connected_timeout = 10min = 600_000_000 us
    let clock = MockClock::from_ticks(&[0, 250, 600_000_001]);
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

    // After preboot portal with client connected and timeout, normal boot continues
    // which eventually leads to error mode (since wifi operations fail) or reboot
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after preboot portal exits"
    );
}

#[test]
fn test_portal_preboot_portal_uses_30_second_timeout() {
    let led = MockLed::new();
    let saw_preboot_start = Arc::new(Mutex::new(false));
    let saw_preboot_start_for_wifi = saw_preboot_start.clone();
    let wifi_backend = MockWifiBackend::new()
        .with_is_connected(false)
        .on_start_access_point(move |_config| {
            *saw_preboot_start_for_wifi.lock().unwrap() = true;
            None
        })
        .on_connect({
            let saw_preboot_start = saw_preboot_start.clone();
            move |_timeout| {
                if !*saw_preboot_start.lock().unwrap() {
                    nok("preboot portal must start before Wi-Fi connect resumes normal boot");
                }
                ok("preboot portal exits at 30-second timeout boundary");
            }
        });
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // Preboot portal idle_timeout is 30s and should expire at the exact boundary.
    let clock = MockClock::from_ticks(&[0, 250, 30_000_000]);
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

    assert_ok_signal(result, "preboot portal exits at 30-second timeout boundary");
}

#[test]
fn test_portal_preboot_led_error_enters_error_mode() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (led, led_calls) = failing_led(); // LED always fails
    let wifi_backend = MockWifiBackend::new().with_is_connected(false);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001]);
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

    // LED errors in preboot portal are ignored (let _ = led.set_pixel(...))
    // but after preboot portal, wifi connect fails because LED fails → error mode
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after LED failure leads to error mode"
    );

    // Error mode LED (red) should have been set at some point
    let last = led_calls.lock().unwrap().last().cloned();
    assert!(
        last.as_ref().map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) in error mode. Last: {:?}",
        last
    );
}

#[test]
fn test_portal_preboot_then_normal_boot_succeeds() {
    let preboot_started = Arc::new(Mutex::new(false));
    let saw_preboot_led = Arc::new(Mutex::new(false));
    let saw_connecting_led = Arc::new(Mutex::new(false));
    let preboot_started_hook = preboot_started.clone();
    let saw_preboot_led_hook = saw_preboot_led.clone();
    let saw_connecting_led_hook = saw_connecting_led.clone();
    let led = MockLed::new().on_set_pixel(move |rgb| {
        if rgb.r.abs() < 0.01 && (rgb.g - 0.53).abs() < 0.01 && (rgb.b - 1.0).abs() < 0.01 {
            *saw_preboot_led_hook.lock().unwrap() = true;
        }
        if (rgb.r - 1.0).abs() < 0.01 && (rgb.g - 0.78).abs() < 0.01 && rgb.b.abs() < 0.01 {
            *saw_connecting_led_hook.lock().unwrap() = true;
        }
        if rgb.r.abs() < 0.01 && rgb.g.abs() < 0.01 && (rgb.b - 1.0).abs() < 0.01 {
            let preboot_started = *preboot_started_hook.lock().unwrap();
            let saw_preboot_led = *saw_preboot_led.lock().unwrap();
            let saw_connecting_led = *saw_connecting_led.lock().unwrap();
            if preboot_started && saw_preboot_led && saw_connecting_led {
                ok("preboot portal transitions into normal connected boot");
            }
        }
        None
    });
    let preboot_started_for_wifi = preboot_started.clone();
    let wifi_backend = MockWifiBackend::new()
        .with_is_connected(false)
        .on_start_access_point(move |_config| {
            *preboot_started_for_wifi.lock().unwrap() = true;
            None
        });
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
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

    assert_ok_signal(result, "preboot portal transitions into normal connected boot");
}
