mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// Portal needs 3 clock ticks: started_at, first elapsed, timeout elapsed
fn portal_timeout_clock() -> MockClock {
    MockClock::from_ticks(&[0, 250, 60_000_001])
}

fn preboot_timeout_clock() -> MockClock {
    MockClock::from_ticks(&[0, 250, 30_000_001])
}

#[test]
fn test_portal_starts_ap_when_nvs_empty() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = portal_timeout_clock();
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
        "wifi.start_access_point must be called when NVS is empty"
    );
    assert!(
        state
            .start_access_point_ssid
            .as_deref()
            .unwrap()
            .starts_with("InfoPanel-"),
        "AP SSID must start with 'InfoPanel-'. Got: {:?}",
        state.start_access_point_ssid
    );
}

#[test]
fn test_portal_ap_start_failure_still_reboots() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    // start_access_point will fail... but the portal uses `let _ = enter_config_mode()`
    // so errors are ignored. The portal always reboots.
    let wifi_backend = MockWifiBackend::default();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = portal_timeout_clock();
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

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must always be called even if AP start has issues"
    );
}

#[test]
fn test_portal_sets_green_led_when_required_portal_runs() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = portal_timeout_clock();
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

    // REQUIRED_PORTAL_LED = green (0.0, 1.0, 0.0)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 1.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to REQUIRED_PORTAL_LED (green). Got: {:?}",
        led.calls()
    );
}

#[test]
fn test_portal_uses_correct_idle_timeout() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    // elapsed = 60_000_000 exactly (idle_timeout boundary) → should exit
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
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

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called when elapsed >= idle_timeout (60s)"
    );
}

#[test]
fn test_portal_restarts_after_idle_timeout() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = portal_timeout_clock();
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

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after idle timeout"
    );
}

#[test]
fn test_portal_continues_after_client_connection_timeout() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    // With client_count=1, portal detects client immediately and uses connected_timeout (10min)
    let wifi_backend = MockWifiBackend::with_client_count(1);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    // Need tick >= connected_timeout (10min = 600_000_000 us) for portal to exit
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

    assert!(
        *reboot_called.lock().unwrap(),
        "platform.reboot() must be called after connected timeout"
    );
}
