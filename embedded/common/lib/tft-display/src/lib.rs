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

pub trait FrameSource {
    type Error;

    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, Self::Error>;
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

    pub fn write_frame<S>(&mut self, source: &mut S) -> Result<(), B::Error>
    where
        B::Error: From<anyhow::Error>,
        S: FrameSource + ?Sized,
        B::Error: From<S::Error>,
    {
        let expected = (self.width as usize) * (self.height as usize) * 2;

        self.write_cmd(0x2A)?;
        self.write_data16(0, self.width - 1)?;

        self.write_cmd(0x2B)?;
        self.write_data16(0, self.height - 1)?;

        self.write_cmd(0x2C)?;

        self.backend.set_dc_high()?;

        let mut total = 0usize;
        let mut buf = [0u8; 512];

        while total < expected {
            let remaining = expected - total;
            let to_read = remaining.min(buf.len());
            let read = source.read(&mut buf[..to_read]).map_err(B::Error::from)?;
            if read == 0 {
                return Err(B::Error::from(anyhow::anyhow!(
                    "frame size {} does not match expected {}",
                    total,
                    expected
                )));
            }
            self.backend.write(&buf[..read])?;
            total += read;
        }

        let mut extra = [0u8; 1];
        let read = source.read(&mut extra).map_err(B::Error::from)?;
        if read != 0 {
            return Err(B::Error::from(anyhow::anyhow!(
                "frame size exceeds expected {}",
                expected
            )));
        }

        Ok(())
    }

    pub fn fill_solid(&mut self, color: u16) -> Result<(), B::Error>
    where
        B::Error: From<anyhow::Error>,
    {
        self.write_cmd(0x2A)?;
        self.write_data16(0, self.width - 1)?;

        self.write_cmd(0x2B)?;
        self.write_data16(0, self.height - 1)?;

        self.write_cmd(0x2C)?;

        self.backend.set_dc_high()?;

        let mut buf = [0u8; 512];
        let hi = (color >> 8) as u8;
        let lo = (color & 0xFF) as u8;
        for chunk in buf.chunks_exact_mut(2) {
            chunk[0] = hi;
            chunk[1] = lo;
        }

        let total = (self.width as usize) * (self.height as usize) * 2;
        let mut written = 0usize;
        while written < total {
            let remaining = total - written;
            let to_write = remaining.min(buf.len());
            self.backend.write(&buf[..to_write])?;
            written += to_write;
        }

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
