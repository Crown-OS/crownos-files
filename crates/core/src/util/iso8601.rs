//! UTC timestamps as XBEL stores them: `YYYY-MM-DDThh:mm:ss[.ffffff]Z`.

const SECONDS_PER_DAY: i64 = 86_400;
const DAYS_PER_ERA: i64 = 146_097;
const DAYS_FROM_CIVIL_EPOCH: i64 = 719_468;

pub fn format_utc(seconds: i64, microseconds: u32) -> String {
    let (days, of_day) = (
        seconds.div_euclid(SECONDS_PER_DAY),
        seconds.rem_euclid(SECONDS_PER_DAY),
    );
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{microseconds:06}Z",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// Seconds since the Unix epoch; fractions are dropped and a missing `Z`
/// is read as UTC too.
pub fn parse_utc(text: &str) -> Option<i64> {
    let (date, time) = text.trim().split_once('T')?;
    let mut date = date.splitn(3, '-');
    let year: i64 = date.next()?.parse().ok()?;
    let month: i64 = date.next()?.parse().ok()?;
    let day: i64 = date.next()?.parse().ok()?;
    let time = time.trim_end_matches('Z');
    let time = time.split_once('.').map_or(time, |(whole, _)| whole);
    let mut time = time.splitn(3, ':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: i64 = time.next()?.parse().ok()?;
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second <= 60;
    valid.then(|| {
        days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second
    })
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_ERA + day_of_era - DAYS_FROM_CIVIL_EPOCH
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + DAYS_FROM_CIVIL_EPOCH;
    let era = shifted.div_euclid(DAYS_PER_ERA);
    let day_of_era = shifted.rem_euclid(DAYS_PER_ERA);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants_format_and_parse_back() {
        assert_eq!(format_utc(0, 0), "1970-01-01T00:00:00.000000Z");
        assert_eq!(format_utc(1_709_210_096, 42), "2024-02-29T12:34:56.000042Z");
        assert_eq!(
            parse_utc("2024-02-29T12:34:56.000042Z"),
            Some(1_709_210_096)
        );
        assert_eq!(parse_utc("2024-02-29T12:34:56Z"), Some(1_709_210_096));
        assert_eq!(parse_utc("1969-12-31T23:59:59Z"), Some(-1));
    }

    #[test]
    fn garbage_does_not_parse() {
        assert_eq!(parse_utc("yesterday"), None);
        assert_eq!(parse_utc("2024-13-01T00:00:00Z"), None);
    }
}
