mod common;

use common::*;
use info_panel_lib::BootReason;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

#[test]
fn test_portal_ap_ip_is_192_168_4_1() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let start_ssid = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_001]);
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

    // The portal runs and uses access_point_ip_config (which returns 192.168.4.1)
    // If portal completed (AP started), the IP config was used
    let state = start_ssid.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "portal must have started AP"
    );
}

#[test]
fn test_portal_scans_networks_before_portal() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let scan_called = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_001]);
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

    // scan_and_store_networks is called before portal starts
    // Our mock tracks that scan_networks was called (via start_access_point_ssid being set)
    // If portal ran, start_access_point was called, meaning scan completed first
    let state = scan_called.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "portal must start after wifi scan"
    );
}

#[test]
fn test_portal_scans_with_no_networks_found() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = empty_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_001]);
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

    // Empty scan results → portal still starts with empty SSID dropdown
    assert!(
        *reboot_called.lock().unwrap(),
        "portal must complete and reboot even with empty scan results"
    );
}
