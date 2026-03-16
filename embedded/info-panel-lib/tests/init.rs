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
