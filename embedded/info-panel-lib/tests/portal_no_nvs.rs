mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// Portal needs 3 clock ticks: started_at, first elapsed, timeout elapsed
fn portal_timeout_clock() -> MockClock {
    MockClock::from_ticks(&[0, 250, 60_000_001])
}

#[test]
fn test_portal_starts_ap_when_nvs_empty() {
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::new().on_start_access_point(|config| {
        if !config.ssid.starts_with("InfoPanel-") {
            nok("required portal AP SSID should start with InfoPanel-");
        }
        ok("required portal started access point for missing config");
    });
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "required portal started access point for missing config");
}

#[test]
fn test_portal_always_reboots_after_portal_exits() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let wifi_backend = MockWifiBackend::default();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = portal_timeout_clock();
    let http_client = MockHttpClient::new();
    let (display, _display_state) = tracked_display(global_counter);

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
        "platform.reboot() must always be called after portal exits"
    );
}

#[test]
fn test_portal_sets_green_led_when_required_portal_runs() {
    let mut led = MockLed::new().on_set_pixel(|rgb, _brightness| {
        if rgb.r.abs() < 0.01 && (rgb.g - 1.0).abs() < 0.01 && rgb.b.abs() < 0.01 {
            ok("required portal green LED observed");
        }
        None
    });
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "required portal green LED observed");
}

#[test]
fn test_portal_uses_correct_idle_timeout() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    // elapsed = 60_000_000 exactly (idle_timeout boundary) → should exit
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
    let http_client = MockHttpClient::new();
    let (display, _display_state) = tracked_display(global_counter);

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
    let (mut led, _led_calls) = tracked_led();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_001]);
    let http_client = MockHttpClient::new();
    let (display, _display_state) = tracked_display(global_counter);

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
    let (mut led, _led_calls) = tracked_led();
    // With client_count=1, portal detects client immediately and uses connected_timeout (10min)
    let wifi_backend = MockWifiBackend::new().with_client_count(1);
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let (platform, reboot_called) =
        tracked_platform([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    // Need tick >= connected_timeout (10min = 600_000_000 us) for portal to exit
    let clock = MockClock::from_ticks(&[0, 250, 600_000_001]);
    let http_client = MockHttpClient::new();
    let (display, _display_state) = tracked_display(global_counter);

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

#[test]
fn test_portal_scan_failure_does_not_prevent_portal() {
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::new()
        .on_scan_networks(|| Some(Err(anyhow::anyhow!("mock scan_networks error"))))
        .on_start_access_point(|_config| ok("portal starts AP even when scan fails"));
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::new(embassy_time::Instant::from_ticks(0));
    let http_client = MockHttpClient::new();
    let display = MockDisplay::new();

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

    assert_ok_signal(result, "portal starts AP even when scan fails");
}
