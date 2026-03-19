use std::cell::RefCell;

use anyhow::Result;

use crate::{Clock, FrameSource, TftBackend};

pub struct MockTftBackend {
    commands: Vec<Command>,
    writes: Vec<Vec<u8>>,
    pub should_error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetDcLow,
    SetDcHigh,
    SetRstLow,
    SetRstHigh,
    Write(Vec<u8>),
}

impl MockTftBackend {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            writes: Vec::new(),
            should_error: false,
        }
    }

    pub fn with_error(mut self) -> Self {
        self.should_error = true;
        self
    }

    pub fn take_commands(&mut self) -> Vec<Command> {
        core::mem::take(&mut self.commands)
    }

    pub fn take_writes(&mut self) -> Vec<Vec<u8>> {
        core::mem::take(&mut self.writes)
    }
}

impl Default for MockTftBackend {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MockFrameSource {
    data: Vec<u8>,
    chunk_sizes: Vec<usize>,
    offset: usize,
    chunk_index: usize,
}

impl MockFrameSource {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            chunk_sizes: Vec::new(),
            offset: 0,
            chunk_index: 0,
        }
    }

    pub fn with_chunk_sizes(mut self, chunk_sizes: Vec<usize>) -> Self {
        self.chunk_sizes = chunk_sizes;
        self
    }
}

impl FrameSource for MockFrameSource {
    type Error = anyhow::Error;

    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, Self::Error> {
        if self.offset >= self.data.len() {
            return Ok(0);
        }

        let remaining = self.data.len() - self.offset;
        let requested = self
            .chunk_sizes
            .get(self.chunk_index)
            .copied()
            .unwrap_or(buf.len())
            .min(buf.len())
            .min(remaining);

        buf[..requested].copy_from_slice(&self.data[self.offset..self.offset + requested]);
        self.offset += requested;
        self.chunk_index += 1;
        Ok(requested)
    }
}

impl TftBackend for MockTftBackend {
    type Error = anyhow::Error;

    fn set_dc_low(&mut self) -> Result<(), Self::Error> {
        if self.should_error {
            return Err(anyhow::anyhow!("dc low error"));
        }
        self.commands.push(Command::SetDcLow);
        Ok(())
    }

    fn set_dc_high(&mut self) -> Result<(), Self::Error> {
        if self.should_error {
            return Err(anyhow::anyhow!("dc high error"));
        }
        self.commands.push(Command::SetDcHigh);
        Ok(())
    }

    fn set_rst_low(&mut self) -> Result<(), Self::Error> {
        if self.should_error {
            return Err(anyhow::anyhow!("rst low error"));
        }
        self.commands.push(Command::SetRstLow);
        Ok(())
    }

    fn set_rst_high(&mut self) -> Result<(), Self::Error> {
        if self.should_error {
            return Err(anyhow::anyhow!("rst high error"));
        }
        self.commands.push(Command::SetRstHigh);
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        if self.should_error {
            return Err(anyhow::anyhow!("write error"));
        }
        self.commands.push(Command::Write(data.to_vec()));
        self.writes.push(data.to_vec());
        Ok(())
    }
}

thread_local! {
    static SLEEP_CALLS: RefCell<Vec<SleepCall>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone)]
pub struct SleepCall {
    pub millis: u64,
}

pub struct MockClock;

impl MockClock {
    pub fn new() -> Self {
        Self
    }

    pub fn take_sleep_calls() -> Vec<SleepCall> {
        SLEEP_CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MockClock {
    async fn sleep_ms(&mut self, millis: u64) {
        SLEEP_CALLS.with(|calls| {
            calls.borrow_mut().push(SleepCall { millis });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_WIDTH: u16 = 100;
    const TEST_HEIGHT: u16 = 100;

    fn frame_writes(writes: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        writes.into_iter().skip(7).collect()
    }

    #[test]
    fn test_tft_display_new() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);
        let _ = display;
    }

    #[test]
    fn test_write_frame_size_check() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let mut source = MockFrameSource::new(Vec::new());
        let result = display.write_frame(&mut source);
        assert!(result.is_err());

        let wrong_size = vec![0u8; 100];
        let mut source = MockFrameSource::new(wrong_size);
        let result = display.write_frame(&mut source);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_frame_accepts_correct_size() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let frame = vec![0u8; pixel_count];
        let mut source = MockFrameSource::new(frame);
        let result = display.write_frame(&mut source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_frame_fails_on_early_eof() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count - 1]);

        let result = display.write_frame(&mut source);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not match expected"), "unexpected error: {err}");
    }

    #[test]
    fn test_write_frame_accepts_arbitrary_chunk_boundaries() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let frame: Vec<u8> = (0..pixel_count).map(|i| (i % 251) as u8).collect();
        let mut source = MockFrameSource::new(frame.clone()).with_chunk_sizes(vec![1, 3, 7, 2, 511, 5]);

        display.write_frame(&mut source).unwrap();

        let writes = display.backend.take_writes();
        let payload: Vec<u8> = frame_writes(writes).into_iter().flatten().collect();
        assert_eq!(payload, frame);
    }

    #[test]
    fn test_write_frame_fails_on_extra_bytes() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut frame = vec![0u8; pixel_count];
        frame.extend_from_slice(&[1, 2, 3]);
        let mut source = MockFrameSource::new(frame);

        let result = display.write_frame(&mut source);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds expected"), "unexpected error: {err}");
    }

    #[test]
    fn test_write_frame_propagates_backend_errors() {
        let backend = MockTftBackend::new().with_error();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);

        let result = display.write_frame(&mut source);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_init_sequence() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        display.init().await.unwrap();

        let commands = display.backend.take_commands();
        
        assert!(commands.contains(&Command::SetRstHigh));
        assert!(commands.contains(&Command::SetRstLow));
        assert!(commands.contains(&Command::SetRstHigh));
        
        let writes: Vec<_> = commands.iter()
            .filter_map(|c| match c {
                Command::Write(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        
        assert!(writes.iter().any(|v| v == &[0x01]));
        assert!(writes.iter().any(|v| v == &[0x11]));
        assert!(writes.iter().any(|v| v == &[0x3A]));
        assert!(writes.iter().any(|v| v == &[0x05]));
        assert!(writes.iter().any(|v| v == &[0x36]));
        assert!(writes.iter().any(|v| v == &[0xC8]));
        assert!(writes.iter().any(|v| v == &[0x20]));
        assert!(writes.iter().any(|v| v == &[0x13]));
        assert!(writes.iter().any(|v| v == &[0x29]));
    }

    #[tokio::test]
    async fn test_init_reset_timing() {
        MockClock::take_sleep_calls();
        
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        display.init().await.unwrap();

        let sleep_calls = MockClock::take_sleep_calls();
        
        assert!(sleep_calls.len() >= 5);
        assert_eq!(sleep_calls[0].millis, 20);
        assert_eq!(sleep_calls[1].millis, 20);
        assert_eq!(sleep_calls[2].millis, 150);
    }

    #[test]
    fn test_write_frame_column_command() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        display.write_frame(&mut source).unwrap();

        let commands = display.backend.take_commands();
        let writes: Vec<_> = commands.iter()
            .filter_map(|c| match c {
                Command::Write(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        
        assert!(writes.iter().any(|v| v == &[0x2A]));
    }

    #[test]
    fn test_write_frame_row_command() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        display.write_frame(&mut source).unwrap();

        let commands = display.backend.take_commands();
        let writes: Vec<_> = commands.iter()
            .filter_map(|c| match c {
                Command::Write(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        
        assert!(writes.iter().any(|v| v == &[0x2B]));
    }

    #[test]
    fn test_write_frame_pixel_data() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let frame = vec![0xAA; pixel_count];
        let mut source = MockFrameSource::new(frame.clone()).with_chunk_sizes(vec![257, 513, 1024]);
        display.write_frame(&mut source).unwrap();

        let writes = display.backend.take_writes();
        
        assert!(writes.iter().any(|v| v == &[0x2C]));

        let pixel_data: Vec<u8> = frame_writes(writes).into_iter().flatten().collect();
        assert_eq!(pixel_data, frame);
    }

    #[test]
    fn test_write_cmd_data() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        display.write_frame(&mut source).unwrap();

        let commands = display.backend.take_commands();
        let writes: Vec<_> = commands.iter()
            .filter_map(|c| match c {
                Command::Write(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        
        assert!(writes.iter().any(|v| v == &[0x2A]));
        assert!(writes.iter().any(|v| v == &[0x2B]));
        assert!(writes.iter().any(|v| v == &[0x2C]));
    }

    #[test]
    fn test_write_data16_format() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, 240, 320);

        let pixel_count = 240 * 320 * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        display.write_frame(&mut source).unwrap();

        let writes = display.backend.take_writes();
        
        let col_start = writes.iter().find(|v| v.len() == 2 && v[0] == 0 && v[1] == 0);
        assert!(col_start.is_some());
        
        let col_end = writes.iter().find(|v| v.len() == 2 && v[0] == 0 && v[1] == 239);
        assert!(col_end.is_some());
        
        let row_start = writes.iter().find(|v| v.len() == 2 && v[0] == 0 && v[1] == 0);
        assert!(row_start.is_some());
        
        let row_end = writes.iter().find(|v| v.len() == 2 && v[0] == 1 && v[1] == 63);
        assert!(row_end.is_some());
    }

    #[tokio::test]
    async fn test_backend_error_propagation() {
        let backend = MockTftBackend::new().with_error();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        let result = display.init().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backend_error_during_write_frame() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, TEST_WIDTH, TEST_HEIGHT);

        display.backend.should_error = true;
        
        let pixel_count = (TEST_WIDTH as usize) * (TEST_HEIGHT as usize) * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        let result = display.write_frame(&mut source);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_width_height_edge_cases() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let _display = crate::TftDisplay::new(backend, clock, 240, 320);
    }

    #[test]
    fn test_specific_dimensions() {
        let backend = MockTftBackend::new();
        let clock = MockClock::new();
        let mut display = crate::TftDisplay::new(backend, clock, 240, 320);

        let pixel_count = 240 * 320 * 2;
        let mut source = MockFrameSource::new(vec![0u8; pixel_count]);
        let result = display.write_frame(&mut source);
        
        assert!(result.is_ok());
        
        let commands = display.backend.take_commands();
        let writes: Vec<_> = commands.iter()
            .filter_map(|c| match c {
                Command::Write(v) => Some(v.clone()),
                _ => None,
            })
            .collect();
        
        assert!(writes.iter().any(|v| v == &[0x2A]));
        assert!(writes.iter().any(|v| v == &[0x2B]));
        assert!(writes.iter().any(|v| v == &[0x2C]));
    }
}
