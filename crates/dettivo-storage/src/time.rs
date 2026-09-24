//! ISO 8601 UTC timestamps as the store keeps them (`2026-02-13T16:00:00Z`):
//! they sort as text, the contract carries them as they are, and the
//! retention cutoffs are plain string comparisons.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time, seconds precision.
pub fn now_iso() -> String {
    iso_from_unix(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    )
}

/// A timestamp formatted in the host timezone, including its historical DST
/// offset. Stored timestamps themselves remain UTC.
#[allow(unsafe_code)] // The libc conversion is isolated here; callers stay safe.
pub(crate) fn local_title_stamp(text: &str) -> Option<String> {
    let timestamp: libc::time_t = unix_from_iso(text)?;
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid; localtime_r writes a complete tm on
    // success, with caller-owned storage so concurrent title creation is safe.
    if unsafe { libc::localtime_r(&timestamp, local.as_mut_ptr()) }.is_null() {
        return None;
    }
    // SAFETY: the successful call above initialized local.
    let local = unsafe { local.assume_init() };
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        local.tm_year + 1900,
        local.tm_mon + 1,
        local.tm_mday,
        local.tm_hour,
        local.tm_min
    ))
}

/// Seconds since the epoch, for ids and cutoffs.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a Unix timestamp.
pub fn iso_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// The Unix timestamp of an ISO 8601 UTC string (`Z` suffix, optional
/// fractional seconds), or of a bare date (`YYYY-MM-DD`, midnight).
pub fn unix_from_iso(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, time) = match text.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    // A day past the month's end (2026-02-31) would silently roll over.
    if civil_from_days(days) != (y, m, d) {
        return None;
    }
    let mut secs = days * 86_400;
    if let Some(time) = time {
        let time = time.strip_suffix('Z').unwrap_or(time);
        let time = time.split_once('.').map(|(t, _)| t).unwrap_or(time);
        let mut parts = time.split(':');
        let h: i64 = parts.next()?.parse().ok()?;
        let mi: i64 = parts.next()?.parse().ok()?;
        let s: i64 = parts.next().unwrap_or("0").parse().ok()?;
        if parts.next().is_some() || h > 23 || mi > 59 || s > 60 {
            return None;
        }
        secs += h * 3600 + mi * 60 + s;
    }
    Some(secs)
}

/// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// A civil date to days since 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trips_and_dates_are_civil() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        let t = unix_from_iso("2026-02-13T16:00:00Z").unwrap();
        assert_eq!(iso_from_unix(t), "2026-02-13T16:00:00Z");
        assert_eq!(
            unix_from_iso("2026-02-13"),
            unix_from_iso("2026-02-13T00:00:00Z")
        );
        assert_eq!(unix_from_iso("2026-02-13T16:00:00.250Z"), Some(t));
        assert_eq!(unix_from_iso("2026-13-01"), None);
        assert_eq!(unix_from_iso("nope"), None);
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    }

    #[test]
    fn dates_that_do_not_exist_are_rejected() {
        assert!(unix_from_iso("2026-02-31").is_none());
        assert!(unix_from_iso("2025-02-29").is_none());
        assert!(unix_from_iso("2024-02-29").is_some());
        assert!(unix_from_iso("2026-04-31T00:00:00Z").is_none());
    }
}
