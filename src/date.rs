//! Calendar dates as whole days, proleptic Gregorian, no time zone.

/// Days since 1970-01-01 of an ISO date `YYYY-MM-DD` (years 0000 to 9999).
/// `None` for anything else, including days that do not exist
/// (`2026-02-29`).
pub fn parse_iso_date(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<i64> {
        let mut v = 0i64;
        for &c in &b[r] {
            if !c.is_ascii_digit() {
                return None;
            }
            v = v * 10 + i64::from(c - b'0');
        }
        Some(v)
    };
    let y = digits(0..4)?;
    let m = digits(5..7)?;
    let d = digits(8..10)?;
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Whole days from `from` to `to` (negative when `to` is earlier).
pub fn day_offset(from: &str, to: &str) -> Option<i64> {
    Some(parse_iso_date(to)? - parse_iso_date(from)?)
}

/// The calendar date `(year, month, day)` of a count of days since
/// 1970-01-01, the inverse of [`parse_iso_date`].
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The date `years` calendar years after `days` (days since 1970-01-01):
/// the same month and day, and 28 February for 29 February in a year that
/// has none, where a period in years ends under the Tax Code (art. 6.1).
pub fn add_years(days: i64, years: i64) -> i64 {
    let (y, m, d) = civil_from_days(days);
    let y = y + years;
    let d = if m == 2 && d == 29 && !is_leap(y) {
        28
    } else {
        d
    };
    days_from_civil(y, m, d)
}

/// Whole calendar years from `from` to `to` (days since 1970-01-01): the
/// most anniversaries of `from` on or before `to`, zero when `to` is
/// earlier.
pub fn full_years(from: i64, to: i64) -> i64 {
    let mut n = civil_from_days(to).0 - civil_from_days(from).0;
    if n > 0 && add_years(from, n) > to {
        n -= 1;
    }
    n.max(0)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant's days_from_civil.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_days() {
        assert_eq!(parse_iso_date("1970-01-01"), Some(0));
        assert_eq!(parse_iso_date("2000-03-01"), Some(11_017));
        assert_eq!(parse_iso_date("0000-01-01"), Some(-719_528));
        assert_eq!(day_offset("2026-09-04", "2027-09-04"), Some(365));
        assert_eq!(day_offset("2027-09-04", "2028-09-04"), Some(366));
    }

    #[test]
    fn anniversaries() {
        let day = |s: &str| parse_iso_date(s).unwrap();
        assert_eq!(add_years(day("2026-09-04"), 3), day("2029-09-04"));
        assert_eq!(add_years(day("2024-02-29"), 1), day("2025-02-28"));
        assert_eq!(add_years(day("2024-02-29"), 4), day("2028-02-29"));
        assert_eq!(full_years(day("2026-09-04"), day("2029-09-03")), 2);
        assert_eq!(full_years(day("2026-09-04"), day("2029-09-04")), 3);
        assert_eq!(full_years(day("2026-09-04"), day("2026-09-03")), 0);
        assert_eq!(full_years(day("2024-02-29"), day("2025-02-28")), 1);
    }

    #[test]
    fn civil_dates_round_trip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-719_528), (0, 1, 1));
        for s in ["2024-02-29", "2026-09-04", "2028-12-31", "1969-12-31"] {
            let (y, m, d) = civil_from_days(parse_iso_date(s).unwrap());
            assert_eq!(format!("{y:04}-{m:02}-{d:02}"), s);
        }
    }

    #[test]
    fn rejects_malformed_dates() {
        for s in [
            "",
            "2026-9-04",
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026/09/04",
            "2026-09-04T00:00:00Z",
            "+026-09-04",
        ] {
            assert_eq!(parse_iso_date(s), None, "{s}");
        }
        assert!(parse_iso_date("2024-02-29").is_some());
    }
}
