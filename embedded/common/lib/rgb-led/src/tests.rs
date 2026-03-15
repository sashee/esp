use crate::{pixel_bytes, ColorOrder, Rgb, RgbLed, RgbLedBackend};
use core::convert::Infallible;

struct MockBackend {
    last_bytes: Option<[u8; 3]>,
}

impl MockBackend {
    fn new() -> Self {
        Self { last_bytes: None }
    }
}

impl RgbLedBackend for MockBackend {
    type Error = Infallible;

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> Result<(), Self::Error> {
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
    assert_eq!(pixel_bytes(ColorOrder::GRB, rgb, 0.5), [64, 128, 32]);
    assert_eq!(pixel_bytes(ColorOrder::BGR, rgb, 0.5), [32, 64, 128]);
}

#[test]
fn rgb_led_passes_calculated_bytes_to_backend() {
    let mut led = RgbLed::new(MockBackend::new(), ColorOrder::GBR);

    led.set_pixel(Rgb::new(1.0, 0.25, 0.75), 0.5).unwrap();

    assert_eq!(led.backend.last_bytes, Some([32, 96, 128]));
}
