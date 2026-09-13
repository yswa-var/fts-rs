use std::time::Duration;

pub fn delay(attempt: u32) -> Duration {
    let seconds = 2u64.pow(attempt.min(5));

    Duration::from_secs(seconds.min(30))
}
