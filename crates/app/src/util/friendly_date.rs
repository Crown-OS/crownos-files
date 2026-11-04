use std::time::{SystemTime, UNIX_EPOCH};

use crate::{model::timestamp::Timestamp, util::local_time::LocalDateTime};

const SECONDS_PER_DAY: i64 = 86_400;
const NANOS_PER_SECOND: i64 = 1_000_000_000;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The moment dates are written relative to: "Today, 09:14", "Yesterday, 18:20", "Oct 30, 2026".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Today {
    unix_seconds: i64,
}

impl Today {
    pub fn now() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        Self::at(i64::try_from(elapsed).unwrap_or(i64::MAX))
    }

    pub const fn at(unix_seconds: i64) -> Self {
        Self { unix_seconds }
    }

    /// `stamp` as the details panel and list view show it; `None` when unknown.
    pub fn describe(self, stamp: Timestamp) -> Option<String> {
        let seconds = stamp.nanoseconds()?.div_euclid(NANOS_PER_SECOND);
        let when = LocalDateTime::from_unix(seconds)?;
        let today = LocalDateTime::from_unix(self.unix_seconds)?;
        let yesterday = LocalDateTime::from_unix(self.unix_seconds - SECONDS_PER_DAY)?;
        let clock = || format!("{:02}:{:02}", when.hour, when.minute);
        Some(if when.same_day(&today) {
            format!("Today, {}", clock())
        } else if when.same_day(&yesterday) {
            format!("Yesterday, {}", clock())
        } else {
            let month = MONTHS[usize::from(when.month.clamp(1, 12)) - 1];
            format!("{month} {}, {}", when.day, when.year)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOV_4_2026_NOON_UTC: i64 = 1_793_793_600;

    fn stamp(seconds: i64) -> Timestamp {
        Timestamp::from_parts(seconds, 0)
    }

    #[test]
    fn recent_days_are_named_and_older_ones_dated() {
        let today = Today::at(NOV_4_2026_NOON_UTC);
        let described = |offset| today.describe(stamp(NOV_4_2026_NOON_UTC + offset));
        assert!(described(0).is_some_and(|label| label.starts_with("Today, ")));
        assert!(described(-SECONDS_PER_DAY).is_some_and(|label| label.starts_with("Yesterday, ")));
        assert_eq!(
            described(-7 * SECONDS_PER_DAY).as_deref(),
            Some("Oct 28, 2026")
        );
        assert_eq!(today.describe(Timestamp::UNKNOWN), None);
    }
}
