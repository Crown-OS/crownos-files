use std::fmt;

const BYTES_PER_KILOBYTE: u64 = 1000;
const STEP: f64 = BYTES_PER_KILOBYTE as f64;
const UNITS: [&str; 6] = ["KB", "MB", "GB", "TB", "PB", "EB"];
const ROUNDS_UP_TO_NEXT_UNIT: f64 = 999.5;
const ONE_DECIMAL_BELOW: f64 = 9.95;

/// A byte count in SI units the way the status bar shows it: `"412 GB"`,
/// `"1.2 MB"`, `"1 TB"`. Formatting never allocates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HumanSize(pub u64);

impl fmt::Display for HumanSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            1 => return formatter.write_str("1 byte"),
            bytes if bytes < BYTES_PER_KILOBYTE => return write!(formatter, "{bytes} bytes"),
            _ => {}
        }
        let mut value = self.0 as f64 / STEP;
        let mut unit = 0;
        while value >= ROUNDS_UP_TO_NEXT_UNIT && unit + 1 < UNITS.len() {
            value /= STEP;
            unit += 1;
        }
        let unit = UNITS[unit];
        let tenths = (value * 10.0).round() as u64;
        if value < ONE_DECIMAL_BELOW && !tenths.is_multiple_of(10) {
            write!(formatter, "{}.{} {unit}", tenths / 10, tenths % 10)
        } else {
            write!(formatter, "{value:.0} {unit}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human(bytes: u64) -> String {
        HumanSize(bytes).to_string()
    }

    #[test]
    fn formats_like_the_status_bar() {
        assert_eq!(human(0), "0 bytes");
        assert_eq!(human(1), "1 byte");
        assert_eq!(human(999), "999 bytes");
        assert_eq!(human(1_000), "1 KB");
        assert_eq!(human(1_234), "1.2 KB");
        assert_eq!(human(12_345), "12 KB");
        assert_eq!(human(999_700), "1 MB");
        assert_eq!(human(412_000_000_000), "412 GB");
        assert_eq!(human(1_000_000_000_000), "1 TB");
        assert_eq!(human(u64::MAX), "18 EB");
    }
}
