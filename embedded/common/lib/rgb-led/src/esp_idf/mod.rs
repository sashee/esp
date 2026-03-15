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

use crate::RgbLedBackend;

const RMT_LED_RESOLUTION_HZ: Hertz = Hertz(10_000_000);
const T0H: Duration = Duration::from_nanos(350);
const T0L: Duration = Duration::from_nanos(800);
const T1H: Duration = Duration::from_nanos(700);
const T1L: Duration = Duration::from_nanos(600);
const TRESET: Duration = Duration::from_micros(281);

pub struct Ws2812RmtBackend<'a> {
    tx_channel: TxChannelDriver<'a>,
}

impl<'d> Ws2812RmtBackend<'d> {
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
}

impl RgbLedBackend for Ws2812RmtBackend<'_> {
    type Error = anyhow::Error;

    fn set_pixel_bytes(&mut self, bytes: [u8; 3]) -> Result<(), Self::Error> {
        let signal = ws2812_signal(bytes)?;

        self.tx_channel
            .send_and_wait(CopyEncoder::new()?, &signal, &TransmitConfig::default())?;

        Ok(())
    }
}

fn ws2812_signal(bytes: [u8; 3]) -> Result<Vec<Symbol>> {
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

    for byte in bytes {
        for bit in 0..8 {
            let symbol = if (byte & (0x80 >> bit)) != 0 {
                one
            } else {
                zero
            };
            signal.push(symbol);
        }
    }

    Ok(signal)
}
