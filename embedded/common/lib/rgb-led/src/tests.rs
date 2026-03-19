use crate::{pixel_bytes, ColorOrder, Rgb, RgbLed, RgbLedBackend};

#[derive(Debug, Eq, PartialEq)]
struct MockError(&'static str);

impl core::fmt::Display for MockError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

struct MockBackend {
    last_bytes: Option<[u8; 3]>,
    fail_with: Option<&'static str>,
    color_order: ColorOrder,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            last_bytes: None,
            fail_with: None,
            color_order: ColorOrder::RGB,
        }
    }

    fn with_color_order(mut self, color_order: ColorOrder) -> Self {
        self.color_order = color_order;
        self
    }

    fn failing(message: &'static str) -> Self {
        Self {
            last_bytes: None,
            fail_with: Some(message),
            color_order: ColorOrder::RGB,
        }
    }
}

impl RgbLedBackend for MockBackend {
    type Error = MockError;

    fn color_order(&self) -> ColorOrder {
        self.color_order
    }

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> Result<(), Self::Error> {
        if let Some(message) = self.fail_with {
            return Err(MockError(message));
        }
        self.last_bytes = Some(bytes);
        Ok(())
    }
}

#[test]
fn brightness_zero_turns_led_off() {
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(1.0, 0.25, 0.75), 0.0),
        [0, 0, 0]
    );
}

#[test]
fn brightness_is_clamped() {
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(0.5, 0.25, 2.0), 2.0),
        [128, 64, 255]
    );
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(-1.0, 0.25, 0.5), -1.0),
        [0, 0, 0]
    );
}

#[test]
fn color_order_reorders_scaled_bytes() {
    let rgb = Rgb::new(1.0, 0.5, 0.25);

    assert_eq!(pixel_bytes(ColorOrder::RGB, rgb, 0.5), [128, 64, 32]);
    assert_eq!(pixel_bytes(ColorOrder::RBG, rgb, 0.5), [128, 32, 64]);
    assert_eq!(pixel_bytes(ColorOrder::GRB, rgb, 0.5), [64, 128, 32]);
    assert_eq!(pixel_bytes(ColorOrder::GBR, rgb, 0.5), [64, 32, 128]);
    assert_eq!(pixel_bytes(ColorOrder::BRG, rgb, 0.5), [32, 128, 64]);
    assert_eq!(pixel_bytes(ColorOrder::BGR, rgb, 0.5), [32, 64, 128]);
}

#[test]
fn rounding_matches_nearest_byte_boundaries() {
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(0.5 / 255.0, 0.0, 0.0), 1.0),
        [1, 0, 0]
    );
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(127.4 / 255.0, 0.0, 0.0), 1.0),
        [127, 0, 0]
    );
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(127.5 / 255.0, 0.0, 0.0), 1.0),
        [128, 0, 0]
    );
}

#[test]
fn channel_and_brightness_clamp_independently() {
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(0.8, 0.4, 0.2), 2.0),
        [204, 102, 51]
    );
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(-0.5, 0.5, 1.5), 0.5),
        [0, 64, 128]
    );
}

#[test]
fn white_levels_scale_as_expected() {
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(1.0, 1.0, 1.0), 1.0),
        [255, 255, 255]
    );
    assert_eq!(
        pixel_bytes(ColorOrder::RGB, Rgb::new(1.0, 1.0, 1.0), 0.5),
        [128, 128, 128]
    );
}

#[test]
fn rgb_led_passes_calculated_bytes_to_backend() {
    let mut led = RgbLed::new(MockBackend::new().with_color_order(ColorOrder::GBR));

    led.set_pixel(Rgb::new(1.0, 0.25, 0.75), 0.5).unwrap();

    assert_eq!(led.backend.last_bytes, Some([32, 96, 128]));
}

#[test]
fn rgb_led_propagates_backend_errors() {
    let mut led = RgbLed::new(MockBackend::failing("backend failed"));

    let err = led.set_pixel(Rgb::new(1.0, 0.25, 0.75), 0.5).unwrap_err();

    assert_eq!(err.to_string(), "backend failed");
}

#[test]
fn rgb_led_uses_backend_color_order() {
    let mut led = RgbLed::new(MockBackend::new().with_color_order(ColorOrder::BRG));

    led.set_pixel(Rgb::new(1.0, 0.5, 0.25), 0.5).unwrap();

    assert_eq!(led.backend.last_bytes, Some([32, 128, 64]));
}
