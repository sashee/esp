use anyhow::Result;
use core::time::Duration;
use esp_idf_hal::{
    gpio::OutputPin,
    rmt::{
        config::{MemoryAccess, TransmitConfig, TxChannelConfig},
        encoder::CopyEncoder,
        PinState, Symbol, TxChannelDriver,
    },
    units::Hertz,
};

pub use rgb::RGB8;

const RMT_LED_RESOLUTION_HZ: Hertz = Hertz(10_000_000);
const T0H: Duration = Duration::from_nanos(350);
const T0L: Duration = Duration::from_nanos(800);
const T1H: Duration = Duration::from_nanos(700);
const T1L: Duration = Duration::from_nanos(600);
const TRESET: Duration = Duration::from_micros(281);

pub struct WS2812RMT<'a> {
    tx_channel: TxChannelDriver<'a>,
}

impl<'d> WS2812RMT<'d> {
    // Rust ESP Board gpio2,  ESP32-C3-DevKitC-02 gpio8
    pub fn new(led: impl OutputPin + 'd) -> Result<Self> {
        let tx_channel = TxChannelDriver::new(
            led,
            &TxChannelConfig {
                resolution: RMT_LED_RESOLUTION_HZ,
                memory_access: MemoryAccess::Indirect {
                    memory_block_symbols: 64,
                },
                ..Default::default()
            },
        )?;

        Ok(Self { tx_channel })
    }

    pub fn set_pixel(&mut self, rgb: RGB8) -> Result<()> {
        let zero = Symbol::new_with(
            RMT_LED_RESOLUTION_HZ,
            PinState::High,
            T0H,
            PinState::Low,
            T0L,
        )?;
        let one = Symbol::new_with(
            RMT_LED_RESOLUTION_HZ,
            PinState::High,
            T1H,
            PinState::Low,
            T1L,
        )?;
        let reset =
            Symbol::new_half_split(RMT_LED_RESOLUTION_HZ, PinState::Low, PinState::Low, TRESET)?;

        let mut signal = Vec::with_capacity(25);
        signal.push(reset);

        for byte in [rgb.g, rgb.r, rgb.b] {
            for bit in 0..8 {
                let symbol = if (byte & (0x80 >> bit)) != 0 {
                    one
                } else {
                    zero
                };
                signal.push(symbol);
            }
        }

        self.tx_channel
            .send_and_wait(CopyEncoder::new()?, &signal, &TransmitConfig::default())?;

        Ok(())
    }
}
