use byte_unit::{Byte, UnitType};
use std::time::Duration;

pub fn format_size(bytes: u64) -> String {
    let adjusted = Byte::from_u64(bytes).get_appropriate_unit(UnitType::Binary);
    format!("{adjusted:.2}")
}

pub fn format_cook_time(duration: Duration) -> String {
    if duration.as_secs_f64() >= 1.0 {
        return format!("{:.2}s", duration.as_secs_f64());
    }

    if duration.as_millis() >= 1 {
        return format!("{:.2}ms", duration.as_secs_f64() * 1000.0);
    }

    if duration.as_micros() >= 1 {
        return format!("{:.2}us", duration.as_secs_f64() * 1_000_000.0);
    }

    format!("{}ns", duration.as_nanos())
}
