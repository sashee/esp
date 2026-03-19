mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

#[test]
fn test_portal_ap_ip_is_192_168_4_1() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // Verify the AP was started
    assert!(
        wifi_state.start_access_point_ssid.lock().unwrap().is_some(),
        "portal must have started AP"
    );

    // Verify the IP config returned by the mock is 192.168.4.1
    assert_eq!(
        wifi_state.access_point_ip.lock().unwrap().as_deref(),
        Some("192.168.4.1"),
        "AP IP must be 192.168.4.1. Got: {:?}",
        *wifi_state.access_point_ip.lock().unwrap()
    );
}

#[test]
fn test_portal_scans_networks_before_portal() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend();
    let scan_order = wifi_state.scan_order.clone();
    let start_ap_order = wifi_state.start_ap_order.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
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

    // scan_networks must be called before start_access_point
    let scan = scan_order.lock().unwrap();
    let start_ap = start_ap_order.lock().unwrap();

    assert!(scan.is_some(), "wifi.scan_networks() must be called");
    assert!(start_ap.is_some(), "wifi.start_access_point() must be called");
    assert!(
        scan.unwrap() < start_ap.unwrap(),
        "scan_networks (order {:?}) must be called BEFORE start_access_point (order {:?})",
        *scan,
        *start_ap
    );
}

#[test]
fn test_portal_scans_with_duplicate_ssid() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let wifi_backend = MockWifiBackend::new().with_scan_networks_result(vec![
        wifi::FoundNetwork::new("HomeNetwork", None, Some(-50)),
        wifi::FoundNetwork::new("HomeNetwork", None, Some(-60)),
        wifi::FoundNetwork::new("OtherNet", None, Some(-70)),
    ]);
    let mut wifi = wifi::Wifi::new(wifi_backend);
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

    // Portal should complete normally even with duplicate SSIDs
    assert!(
        *reboot_called.lock().unwrap(),
        "portal must complete and reboot even with duplicate SSIDs in scan results"
    );
}

#[test]
fn test_portal_ap_stop_is_called_on_exit() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let (mut led, _led_calls) = tracked_led();
    let (wifi_backend, wifi_state) = tracked_wifi_backend();
    let mut wifi = wifi::Wifi::new(wifi_backend);
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
        "platform.reboot() must be called"
    );

    // After portal completes, the AP should have been stopped
    assert!(
        !wifi_state.started.load(std::sync::atomic::Ordering::SeqCst),
        "wifi AP should be stopped after portal exits"
    );
}
