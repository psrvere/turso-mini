use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct Instant {
    pub secs: i64, // seconds since Unix epoch
    pub micros: u32, // micro secondds for sub second precision
}

impl Instant {
    pub fn to_system_time(self) -> SystemTime {
        if self.secs > 0 {
            UNIX_EPOCH + Duration::new(self.secs as u64, self.micros*1000 )
        } else {
            let postive_secs = (-self.secs) as u64;
            if self.micros > 0 {
                let nanos_to_subtract = (1_000_000 - self.micros) * 1000;
                UNIX_EPOCH - Duration::new(postive_secs - 1, nanos_to_subtract)
            } else {
                UNIX_EPOCH - Duration::new(postive_secs, 0)
            }
        }
    }
}

pub trait Clock {
    fn now(&self) -> Instant;
}