mod common;

use common::*;
use info_panel_lib::{BootReason, Led, Platform};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// A MockDisplay that returns an error on init
struct FailingDisplay;

impl info_panel_lib::DisplayWrite for FailingDisplay {
    async fn init(&mut self) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("display init failed"))
    }
    fn write_frame(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn test_init_clears_tft_on_startup() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::always_failing();
    let display = MockDisplay::new(global_counter);

    let init_called = display.init_called.clone();
    let init_order = display.init_order.clone();
    let connect_order = wifi.backend().connect_order.clone();

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

    assert!(*init_called.lock().unwrap(), "display.init() must be called");

    let init = init_order.lock().unwrap();
    let connect = connect_order.lock().unwrap();

    assert!(init.is_some(), "display.init() must be called");
    assert!(connect.is_some(), "wifi.connect() must be called");
    assert!(
        init.unwrap() < connect.unwrap(),
        "display.init() (order {:?}) must be called BEFORE wifi.connect() (order {:?})",
        init,
        connect
    );
}

#[test]
fn test_init_enters_error_mode_when_display_init_fails() {
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::default());
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
    let http_client = MockHttpClient::new();
    let display = FailingDisplay;

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

    // LED should have been set to red (ERROR_LED = 1.0, 0.0, 0.0)
    assert_eq!(
        led.last_call().map(|c| (c.r, c.g, c.b)),
        Some((1.0, 0.0, 0.0)),
        "LED must be set to ERROR_LED (red) after display init failure"
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
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::with_counter(global_counter.clone());
    let wifi_state = wifi_backend.state.clone();
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

    // WiFi configure_client should have been called with stored ssid and password
    let state = wifi_state.lock().unwrap();
    assert_eq!(
        state.configured_ssid.as_deref(),
        Some("test_ssid"),
        "wifi.configure_client must be called with stored ssid"
    );
    assert_eq!(
        state.configured_password.as_deref(),
        Some("test_pw"),
        "wifi.configure_client must be called with stored password"
    );
    drop(state);

    // LED should have been set to orange (CONNECTING_LED = 1.0, 0.78, 0.0)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.78).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to CONNECTING_LED (orange) during connection. Got: {:?}",
        led.calls()
    );

    // LED should have been set to blue (CONNECTED_LED = 0.0, 0.0, 1.0)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 1.0).abs() < 0.01),
        "LED must be set to CONNECTED_LED (blue) after connection. Got: {:?}",
        led.calls()
    );
}

#[test]
fn test_init_goes_to_required_portal_when_led_brightness_missing() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let wifi_state = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);

    // Store with ssid, pw, url but NO led_brightness
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
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

    // WiFi AP should have been started (SSID is captured, even if portal later stopped it)
    let state = wifi_state.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "wifi.start_access_point must be called when led_brightness is missing"
    );
    assert!(
        state.start_access_point_ssid.as_deref().unwrap().starts_with("InfoPanel-"),
        "AP SSID must start with 'InfoPanel-'. Got: {:?}",
        state.start_access_point_ssid
    );

    // LED should have been set to green (REQUIRED_PORTAL_LED = 0.0, 1.0, 0.0)
    assert!(
        led.calls()
            .iter()
            .any(|c| (c.r - 0.0).abs() < 0.01 && (c.g - 1.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01),
        "LED must be set to REQUIRED_PORTAL_LED (green). Got: {:?}",
        led.calls()
    );
}

#[test]
fn test_init_goes_to_required_portal_when_led_brightness_invalid() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let wifi_state = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);

    // Store with all fields but led_brightness is not a valid u8
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "not_a_number".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
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

    // WiFi AP should have been started
    let state = wifi_state.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "wifi.start_access_point must be called when led_brightness is invalid"
    );
}

#[test]
fn test_init_goes_to_required_portal_when_config_corrupted() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::new();
    let wifi_backend = MockWifiBackend::default();
    let wifi_state = wifi_backend.state.clone();
    let mut wifi = wifi::Wifi::new(wifi_backend);

    // Store with only ssid and pw (missing url and led_brightness)
    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_pw".to_string());
    let store = config_store_with_values(values);

    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250, 60_000_000]);
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

    // WiFi AP should have been started (missing fields = ConfigState::Missing)
    let state = wifi_state.lock().unwrap();
    assert!(
        state.start_access_point_ssid.is_some(),
        "wifi.start_access_point must be called when config is corrupted"
    );
}

#[test]
fn test_init_enters_error_mode_when_led_set_fails_during_connect() {
    let global_counter = Arc::new(AtomicU32::new(1));
    let mut led = MockLed::failing();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
    let store = valid_config_store();
    let http_backend = MockHttpBackend;
    let platform = MockPlatform::new([0x12, 0x34, 0x56, 0x78, 0xAA, 0xBB], BootReason::Software);
    let clock = MockClock::from_ticks(&[0, 250]);
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

    // Last LED call should be red (error mode)
    let last = led.last_call();
    assert!(
        last.map(|c| (c.r - 1.0).abs() < 0.01 && (c.g - 0.0).abs() < 0.01 && (c.b - 0.0).abs() < 0.01)
            .unwrap_or(false),
        "LED must be set to ERROR_LED (red) after LED failure. Last call: {:?}",
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
    let mut led = MockLed::new();
    let mut wifi = wifi::Wifi::new(MockWifiBackend::with_counter(global_counter.clone()));
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

    // LED should have been set to blue (CONNECTED_LED = 0.0, 0.0, 1.0)
    // with brightness from config (128/255 ≈ 0.502)
    let expected_brightness = 128.0 / 255.0;
    assert!(
        led.calls().iter().any(|c| {
            (c.r - 0.0).abs() < 0.01
                && (c.g - 0.0).abs() < 0.01
                && (c.b - 1.0).abs() < 0.01
                && (c.brightness - expected_brightness).abs() < 0.01
        }),
        "LED must be set to CONNECTED_LED (blue, brightness={:.3}). Got: {:?}",
        expected_brightness,
        led.calls()
    );
}
