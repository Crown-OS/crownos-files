use std::time::Duration;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;

/// A video's length as its badge shows it: "0:42", "12:05", "1:02:03".
pub fn duration_label(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (hours, minutes, seconds) = (seconds / HOUR, seconds % HOUR / MINUTE, seconds % MINUTE);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_clips_show_minutes_and_long_ones_hours() {
        assert_eq!(duration_label(Duration::from_millis(42_900)), "0:42");
        assert_eq!(duration_label(Duration::from_secs(725)), "12:05");
        assert_eq!(duration_label(Duration::from_secs(3_723)), "1:02:03");
    }
}
