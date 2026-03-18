mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

#[test]
fn test_portal_runs_preboot_portal_on_power_on() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::default();
    wifi_backend.set_is_connected(false);
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // Preboot portal: 30s idle timeout. Need enough ticks for portal + main flow.
    // After preboot portal, normal boot continues (connect wifi, fetch, etc.)
    // But wifi backend already set started=true from portal, so connect fails → error mode
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001]);
    let http_client = MockHttpClient::new();
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

    let state = start_ssid.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "wifi.start_access_point must be called for preboot portal on PowerOn"
    );

    // PREBOOT_PORTAL_LED = blue (0.0, 0.53, 1.0)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.53).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must be set to PREBOOT_PORTAL_LED (blue). Got: {:?}",
        led.calls()
    );
}

#[test]
fn test_portal_skips_preboot_portal_on_other_boot_reasons() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
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

    // AP should NOT have been started (no preboot portal for Software boot)
    let state = start_ssid.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_none(),
        "wifi.start_access_point must NOT be called for Software boot reason"
    );

    // LED should NOT have been set to preboot blue
    assert!(
        !led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.53).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must NOT be PREBOOT_PORTAL_LED for Software boot"
    );
}

#[test]
fn test_portal_preboot_runs_even_with_valid_config() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::default();
    wifi_backend.set_is_connected(false);
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store(); // complete valid config
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001]);
    let http_client = MockHttpClient::new();
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

    // Preboot portal runs even with valid config
    let state = start_ssid.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "preboot portal must run on PowerOn even with valid config"
    );
}

#[test]
fn test_portal_preboot_waits_for_connection() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    // Client connected → portal uses connected_timeout (10min)
    let mut wifi_backend = MockWifiBackend::with_client_count(1);
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // connected_timeout = 10min = 600_000_000 us
    let clock = MockClock::from_ticks(&[0, 250, 600_000_001]);
    let http_client = MockHttpClient::new();
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

    // After preboot portal with client connected and timeout, normal boot continues
    // which eventually leads to error mode (since wifi operations fail) or reboot
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after preboot portal exits"
    );
}

#[test]
fn test_portal_preboot_portal_uses_30_second_timeout() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::default();
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // Preboot portal: idle_timeout = 30s. Use exactly 30s boundary + some margin
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001]);
    let sleep_durations = clock.sleep_durations.clone();
    let http_client = MockHttpClient::new();
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

    // Portal polls every 250ms (from config-portal)
    let sleeps = sleep_durations.lock().unwrap();
    assert!(
        sleeps
            .iter()
            .any(|d| *d == embassy_time::Duration::from_millis(250)),
        "preboot portal must sleep 250ms between polls. Got: {:?}",
        *sleeps
    );
}

#[test]
fn test_portal_preboot_led_error_enters_error_mode() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::failing(); // LED always fails
    let mut wifi_backend = MockWifiBackend::default();
    wifi_backend.set_is_connected(false);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001]);
    let http_client = MockHttpClient::new();
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

    // LED errors in preboot portal are ignored (let _ = led.set_pixel(...))
    // but after preboot portal, wifi connect fails because LED fails → error mode
    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after LED failure leads to error mode"
    );

    // Error mode LED (red) should have been set at some point
    let last = led.last_call();
    assert!(
        last.map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) in error mode. Last: {:?}",
        last
    );
}

#[test]
fn test_portal_preboot_then_normal_boot_succeeds() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    wifi_backend.set_is_connected(false);
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::PowerOn);
    // Need ticks for: preboot portal (30s idle → exits), then wifi connect, then fetch, then refresh disconnect
    let clock = MockClock::from_ticks(&[0, 250, 30_000_001, 60_000_001]);
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

    // Preboot portal should have been started
    let state = start_ssid.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "preboot portal must have started AP on PowerOn"
    );
    drop(state);

    // Preboot LED (blue 0.0, 0.53, 1.0) should have been set
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.53).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must be set to PREBOOT_PORTAL_LED (blue) during preboot. Got: {:?}",
        led.calls()
    );

    // After preboot portal, wifi connect should have been attempted (orange LED)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.78).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to CONNECTING_LED (orange) after preboot portal. Got: {:?}",
        led.calls()
    );

    // Connected LED (blue) should have been set after successful wifi connect
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must be set to CONNECTED_LED (blue) after wifi connect. Got: {:?}",
        led.calls()
    );
}
