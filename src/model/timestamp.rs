use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NANOS_PER_SECOND: i64 = 1_000_000_000;

/// Nanoseconds since the Unix epoch, or [`Timestamp::UNKNOWN`] when the
/// filesystem did not report the time (no birth time on ext3, tmpfs, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const UNKNOWN: Self = Self(i64::MIN);

    pub fn from_parts(seconds: i64, nanoseconds: u32) -> Self {
        Self(
            seconds
                .saturating_mul(NANOS_PER_SECOND)
                .saturating_add(i64::from(nanoseconds)),
        )
    }

    pub fn nanoseconds(self) -> Option<i64> {
        (self != Self::UNKNOWN).then_some(self.0)
    }

    pub fn system_time(self) -> Option<SystemTime> {
        let nanos = self.nanoseconds()?;
        let offset = Duration::from_nanos(nanos.unsigned_abs());
        if nanos >= 0 {
            UNIX_EPOCH.checked_add(offset)
        } else {
            UNIX_EPOCH.checked_sub(offset)
        }
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_has_no_time_and_sorts_first() {
        assert_eq!(Timestamp::UNKNOWN.system_time(), None);
        assert!(Timestamp::UNKNOWN < Timestamp::from_parts(-1, 0));
    }

    #[test]
    fn parts_round_trip_through_system_time() {
        let stamp = Timestamp::from_parts(1_700_000_000, 5);
        let expected = UNIX_EPOCH + Duration::new(1_700_000_000, 5);
        assert_eq!(stamp.system_time(), Some(expected));
    }
}
