#[cfg(target_os = "espidf")]
pub mod esp_idf;

#[cfg(test)]
mod tests;

use anyhow::Result;

pub trait TftBackend {
    type Error;

    fn set_dc_low(&mut self) -> Result<(), Self::Error>;
    fn set_dc_high(&mut self) -> Result<(), Self::Error>;
    fn set_rst_low(&mut self) -> Result<(), Self::Error>;
    fn set_rst_high(&mut self) -> Result<(), Self::Error>;
    fn write(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

#[allow(async_fn_in_trait)]
pub trait Clock {
    async fn sleep_ms(&mut self, millis: u64);
}

pub struct TftDisplay<B, C> {
    backend: B,
    clock: C,
    width: u16,
    height: u16,
}

impl<B, C> TftDisplay<B, C>
where
    B: TftBackend,
    C: Clock,
{
    pub fn new(backend: B, clock: C, width: u16, height: u16) -> Self {
        Self {
            backend,
            clock,
            width,
            height,
        }
    }

    pub async fn init(&mut self) -> Result<(), B::Error> {
        self.backend.set_rst_high()?;
        self.clock.sleep_ms(20).await;
        self.backend.set_rst_low()?;
        self.clock.sleep_ms(20).await;
        self.backend.set_rst_high()?;
        self.clock.sleep_ms(150).await;

        self.write_cmd(0x01)?;
        self.clock.sleep_ms(150).await;

        self.write_cmd(0x11)?;
        self.clock.sleep_ms(250).await;

        self.write_cmd_data(0x3A, &[0x05])?;
        self.write_cmd_data(0x36, &[0xC8])?;
        self.write_cmd(0x20)?;

        self.write_cmd(0x13)?;
        self.clock.sleep_ms(10).await;
        self.write_cmd(0x29)?;
        self.clock.sleep_ms(100).await;

        Ok(())
    }

    pub fn write_frame(&mut self, data: &[u8]) -> Result<(), B::Error>
    where
        B::Error: From<anyhow::Error>,
    {
        let expected = (self.width as usize) * (self.height as usize) * 2;
        if data.len() != expected {
            return Err(B::Error::from(anyhow::anyhow!(
                "frame size {} does not match expected {}",
                data.len(),
                expected
            )));
        }

        self.write_cmd(0x2A)?;
        self.write_data16(0, self.width - 1)?;

        self.write_cmd(0x2B)?;
        self.write_data16(0, self.height - 1)?;

        self.write_cmd(0x2C)?;

        self.backend.set_dc_high()?;
        self.backend.write(data)?;

        Ok(())
    }

    fn write_cmd(&mut self, cmd: u8) -> Result<(), B::Error> {
        self.backend.set_dc_low()?;
        self.backend.write(&[cmd])
    }

    fn write_cmd_data(&mut self, cmd: u8, data: &[u8]) -> Result<(), B::Error> {
        self.write_cmd(cmd)?;
        self.backend.set_dc_high()?;
        self.backend.write(data)
    }

    fn write_data16(&mut self, start: u16, end: u16) -> Result<(), B::Error> {
        self.backend.set_dc_high()?;
        self.backend.write(&[(start >> 8) as u8, (start & 0xFF) as u8])?;
        self.backend.write(&[(end >> 8) as u8, (end & 0xFF) as u8])
    }
}
