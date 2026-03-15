use info_panel_lib::{fill_frame_with_color, rgb565, DeviceConfig, TFT_HEIGHT, TFT_WIDTH};
use std::collections::BTreeMap;

#[test]
fn test_rgb565_red() {
    assert_eq!(rgb565(255, 0, 0), 0xF800);
}

#[test]
fn test_rgb565_green() {
    assert_eq!(rgb565(0, 255, 0), 0x07E0);
}

#[test]
fn test_rgb565_blue() {
    assert_eq!(rgb565(0, 0, 255), 0x001F);
}

#[test]
fn test_rgb565_black() {
    assert_eq!(rgb565(0, 0, 0), 0x0000);
}

#[test]
fn test_rgb565_white() {
    assert_eq!(rgb565(255, 255, 255), 0xFFFF);
}

#[test]
fn test_fill_frame_size() {
    let frame = fill_frame_with_color(0x0000);
    assert_eq!(
        frame.len(),
        (TFT_WIDTH as usize) * (TFT_HEIGHT as usize) * 2
    );
}

#[test]
fn test_device_config_from_stored() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "128".to_string());

    let stored = StoredConfig::new(values);
    let config = DeviceConfig::from_stored(stored).unwrap();

    assert_eq!(config.ssid(), "test_ssid");
    assert_eq!(config.password(), "test_password");
    assert_eq!(config.url(), "http://example.com");
    assert!((config.led_brightness() - 0.502).abs() < 0.001); // 128/255 ≈ 0.502
}

#[test]
fn test_device_config_missing_led_brightness() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());

    let stored = StoredConfig::new(values);
    let result = DeviceConfig::from_stored(stored);

    assert!(result.is_err());
}

#[test]
fn test_device_config_invalid_led_brightness() {
    use config_portal::StoredConfig;

    let mut values = BTreeMap::new();
    values.insert("ssid".to_string(), "test_ssid".to_string());
    values.insert("pw".to_string(), "test_password".to_string());
    values.insert("url".to_string(), "http://example.com".to_string());
    values.insert("led_brightness".to_string(), "not_a_number".to_string());

    let stored = StoredConfig::new(values);
    let result = DeviceConfig::from_stored(stored);

    assert!(result.is_err());
}
