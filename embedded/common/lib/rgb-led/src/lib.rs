#[cfg(target_os = "espidf")]
pub mod esp_idf;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorOrder {
    RGB,
    RBG,
    GRB,
    GBR,
    BRG,
    BGR,
}

impl ColorOrder {
    const fn bytes(self, rgb: [u8; 3]) -> [u8; 3] {
        match self {
            Self::RGB => rgb,
            Self::RBG => [rgb[0], rgb[2], rgb[1]],
            Self::GRB => [rgb[1], rgb[0], rgb[2]],
            Self::GBR => [rgb[1], rgb[2], rgb[0]],
            Self::BRG => [rgb[2], rgb[0], rgb[1]],
            Self::BGR => [rgb[2], rgb[1], rgb[0]],
        }
    }
}

pub trait RgbLedBackend {
    type Error;

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> Result<(), Self::Error>;
}

pub struct RgbLed<B> {
    backend: B,
    color_order: ColorOrder,
}

impl<B> RgbLed<B>
where
    B: RgbLedBackend,
{
    pub const fn new(backend: B, color_order: ColorOrder) -> Self {
        Self {
            backend,
            color_order,
        }
    }

    pub fn set_pixel(&mut self, rgb: Rgb, brightness: f32) -> Result<(), B::Error> {
        self.backend
            .set_pixel_bytes(pixel_bytes(self.color_order, rgb, brightness))
    }
}

pub fn pixel_bytes(color_order: ColorOrder, rgb: Rgb, brightness: f32) -> [u8; 3] {
    color_order.bytes([
        scale_channel(rgb.r, brightness),
        scale_channel(rgb.g, brightness),
        scale_channel(rgb.b, brightness),
    ])
}

fn scale_channel(channel: f32, brightness: f32) -> u8 {
    let channel = channel.clamp(0.0, 1.0);
    let brightness = brightness.clamp(0.0, 1.0);
    (channel * brightness * 255.0).round() as u8
}
