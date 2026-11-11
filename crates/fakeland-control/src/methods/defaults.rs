//! Default values of method parameters.

use fakeland_types::Dur;

/// The default timeout of window waits.
pub fn wait_timeout() -> Dur {
    Dur::from_secs(10)
}

/// The default quiet time of a settled wait.
pub fn quiet() -> Dur {
    Dur::from_millis(250)
}

/// The default timeout of a clipboard read.
pub fn clipboard_timeout() -> Dur {
    Dur::from_secs(2)
}

/// The default duration of a drag.
pub fn drag_duration() -> Dur {
    Dur::from_millis(300)
}

/// The default lifetime of a trigger set.
pub fn trigger_lifetime() -> Dur {
    Dur::from_secs(3600)
}

/// The default span of frame statistics.
pub fn stats_span() -> Dur {
    Dur::from_secs(5)
}

/// Returns 1.
pub fn one() -> u32 {
    1
}

/// Returns true.
pub fn yes() -> bool {
    true
}

/// The default queue length of a subscription.
pub fn subscription_capacity() -> usize {
    4096
}

/// The default count of log lines.
pub fn log_lines() -> usize {
    200
}

/// The default MIME type of clipboard text.
pub fn text_mime() -> String {
    "text/plain;charset=utf-8".to_owned()
}
