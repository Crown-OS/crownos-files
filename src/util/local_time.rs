use std::{
    fmt,
    mem::MaybeUninit,
    time::{SystemTime, UNIX_EPOCH},
};

const TM_YEAR_BASE: i32 = 1900;

/// A wall-clock time in the local time zone, as `.trashinfo` files store it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl LocalDateTime {
    pub fn now() -> Option<Self> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
        let seconds = libc::time_t::try_from(elapsed.as_secs()).ok()?;
        let mut parts = MaybeUninit::<libc::tm>::uninit();
        // SAFETY: both pointers are valid for the call, and `parts` is
        // initialised whenever `localtime_r` returns non-null.
        let parts = unsafe {
            if libc::localtime_r(&seconds, parts.as_mut_ptr()).is_null() {
                return None;
            }
            parts.assume_init()
        };
        Some(Self {
            year: parts.tm_year + TM_YEAR_BASE,
            month: u8::try_from(parts.tm_mon + 1).ok()?,
            day: u8::try_from(parts.tm_mday).ok()?,
            hour: u8::try_from(parts.tm_hour).ok()?,
            minute: u8::try_from(parts.tm_min).ok()?,
            second: u8::try_from(parts.tm_sec).ok()?,
        })
    }

    /// Parses `YYYY-MM-DDThh:mm:ss`.
    pub fn parse(text: &str) -> Option<Self> {
        let (date, time) = text.trim().split_once('T')?;
        let mut date = date.splitn(3, '-');
        let mut time = time.splitn(3, ':');
        Some(Self {
            year: date.next()?.parse().ok()?,
            month: date.next()?.parse().ok()?,
            day: date.next()?.parse().ok()?,
            hour: time.next()?.parse().ok()?,
            minute: time.next()?.parse().ok()?,
            second: time.next()?.parse().ok()?,
        })
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_the_trash_spec_layout() {
        let stamp = LocalDateTime::parse("2004-08-31T22:32:08").unwrap();
        assert_eq!(stamp.to_string(), "2004-08-31T22:32:08");
        assert_eq!((stamp.month, stamp.second), (8, 8));
        assert_eq!(LocalDateTime::parse("2004-08-31 22:32:08"), None);
    }

    #[test]
    fn now_round_trips() {
        let now = LocalDateTime::now().unwrap();
        assert!((1..=12).contains(&now.month));
        assert_eq!(LocalDateTime::parse(&now.to_string()), Some(now));
    }
}
