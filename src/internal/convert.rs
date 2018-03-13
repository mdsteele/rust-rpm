use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::u32;

// ========================================================================= //

pub fn i32_to_system_time(time: i32) -> SystemTime {
    let seconds = ((time as i64) & 0xffffffff) as u64;
    UNIX_EPOCH + Duration::new(seconds, 0)
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
