use anyhow::Result;
use embassy_time::Timer;
use esp_idf_hal::{
    gpio::{Output, PinDriver},
    spi::{SpiDeviceDriver, SpiDriver},
};

use crate::{Clock, TftBackend};

pub struct SpiTftBackend<'d> {
    spi: SpiDeviceDriver<'d, SpiDriver<'d>>,
    dc: PinDriver<'d, Output>,
    rst: PinDriver<'d, Output>,
}

impl<'d> SpiTftBackend<'d> {
    pub fn new(
        spi: SpiDeviceDriver<'d, SpiDriver<'d>>,
        dc: PinDriver<'d, Output>,
        rst: PinDriver<'d, Output>,
    ) -> Self {
        Self { spi, dc, rst }
    }
}

impl TftBackend for SpiTftBackend<'_> {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> Result<(), Self::Error> {
        Ok(self.dc.set_low()?)
    }

    fn set_dc_high(&mut self) -> Result<(), Self::Error> {
        Ok(self.dc.set_high()?)
    }

    fn set_rst_low(&mut self) -> Result<(), Self::Error> {
        Ok(self.rst.set_low()?)
    }

    fn set_rst_high(&mut self) -> Result<(), Self::Error> {
        Ok(self.rst.set_high()?)
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        Ok(self.spi.write(data)?)
    }
}

impl Clock for EspDelay {
    async fn sleep_ms(&mut self, millis: u64) {
        Timer::after_millis(millis).await;
    }
}

pub struct EspDelay;
