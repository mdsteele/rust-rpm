use sha1::Sha1;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::u32;

// ========================================================================= //

pub fn u32_to_system_time(seconds: u32) -> SystemTime {
    UNIX_EPOCH + Duration::new(seconds as u64, 0)
}

pub fn system_time_to_u32(timestamp: SystemTime) -> u32 {
    match timestamp.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let seconds: u64 = duration.as_secs();
            if seconds >= u32::MAX as u64 {
                u32::MAX
            } else {
                seconds as u32
            }
        }
        Err(_) => 0,
    }
}

// ========================================================================= //

pub struct Sha1Writer {
    context: Sha1,
}

impl Sha1Writer {
    pub fn new() -> Sha1Writer { Sha1Writer { context: Sha1::new() } }

    pub fn digest(&self) -> String { self.context.hexdigest() }
}

impl Write for Sha1Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.context.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::{system_time_to_u32, u32_to_system_time};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn system_time_round_trip() {
        for &value in &[0, 54321, 1520908554, 0xffffffff] {
            assert_eq!(system_time_to_u32(u32_to_system_time(value)), value);
        }
    }

    #[test]
    fn system_time_to_u32_limits() {
        // Test that extreme timestamps get clamped to u32::{MIN,MAX}.
        let timestamp = UNIX_EPOCH - Duration::new(100_000_000, 0);
        assert_eq!(system_time_to_u32(timestamp), 0);
        let timestamp = UNIX_EPOCH + Duration::new(10_000_000_000, 0);
        assert_eq!(system_time_to_u32(timestamp), 0xffffffff);
    }
}

// ========================================================================= //
