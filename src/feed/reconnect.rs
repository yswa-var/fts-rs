use std::time::Duration;

/// Returns the capped exponential-backoff delay for a failed reconnect attempt.
///
/// Delays double from two seconds and never exceed 30 seconds, preventing a
/// tight retry loop while keeping feed recovery responsive.
pub fn delay(attempt: u32) -> Duration {
    let seconds = 2u64.pow(attempt.min(5));

    Duration::from_secs(seconds.min(30))
}
