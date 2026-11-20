//! Date and time for file names and log lines: local time on Windows, UTC elsewhere.

/// A moment, to the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Stamp {
    /// Now, in local time on Windows (UTC elsewhere).
    pub fn now() -> Stamp {
        crate::platform::local_time().unwrap_or_else(|| {
            let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            Stamp::from_unix(seconds)
        })
    }

    /// The UTC moment `seconds` after 1970-01-01.
    pub fn from_unix(seconds: u64) -> Stamp {
        let days = (seconds / 86_400) as i64;
        let rest = seconds % 86_400;
        let (year, month, day) = civil_from_days(days);
        Stamp {
            year,
            month,
            day,
            hour: (rest / 3600) as u32,
            minute: (rest % 3600 / 60) as u32,
            second: (rest % 60) as u32,
        }
    }

    /// « 2026-11-20 14:35:12 ».
    pub fn display(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// « 2026-11-20_14-35-12 », for file names.
    pub fn file_part(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Year, month, day of a day count since 1970-01-01 (proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (u32, u32, u32) {
    // Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as u32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_times_become_calendar_dates() {
        assert_eq!(Stamp::from_unix(0).display(), "1970-01-01 00:00:00");
        // 2026-11-20 14:35:12 UTC
        assert_eq!(Stamp::from_unix(1_795_185_312).display(), "2026-11-20 14:35:12");
        // 2024-02-29 (leap day) 23:59:59 UTC
        assert_eq!(Stamp::from_unix(1_709_251_199).display(), "2024-02-29 23:59:59");
        assert_eq!(Stamp::from_unix(1_709_251_200).display(), "2024-03-01 00:00:00");
    }

    #[test]
    fn file_part_has_no_colon_or_space() {
        let part = Stamp::from_unix(1_795_185_312).file_part();
        assert_eq!(part, "2026-11-20_14-35-12");
        assert!(!part.contains([':', ' ']));
    }
}
