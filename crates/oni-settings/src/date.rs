//! `Date`, as the sidecar writes it (`.iso8601`, whole seconds in UTC) and as a plain
//! `JSONEncoder` writes it (seconds since 2001-01-01, the roll marker's encoding).

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Seconds between 1970-01-01 and Foundation's reference date, 2001-01-01, both UTC.
pub const REFERENCE_DATE_OFFSET: f64 = 978_307_200.0;
const REFERENCE_DATE_OFFSET_SECONDS: i64 = 978_307_200;

/// An instant held as Foundation holds it, seconds since 2001, so the roll marker's `Double`
/// reads back bit for bit: a detour through 1970 loses the last bit on 43 % of instants.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Date(f64);

impl Date {
    pub fn now() -> Self {
        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Date::from_unix_seconds(elapsed)
    }

    pub fn since_reference_date(seconds: f64) -> Self {
        Date(seconds)
    }

    pub fn from_unix_seconds(seconds: f64) -> Self {
        Date(seconds - REFERENCE_DATE_OFFSET)
    }

    pub fn time_interval_since_reference_date(self) -> f64 {
        self.0
    }

    pub fn unix_seconds(self) -> f64 {
        self.0 + REFERENCE_DATE_OFFSET
    }

    /// `ISO8601DateFormatter` with its default options: `2023-11-14T22:13:20Z`.
    pub fn iso8601(self) -> String {
        let total = self.0.floor() as i64 + REFERENCE_DATE_OFFSET_SECONDS;
        let days = total.div_euclid(86_400);
        let secs = total.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        )
    }

    /// The internet date-time `ISO8601DateFormatter` reads: no fractional seconds, `Z` or `±HH:MM`.
    pub fn parse_iso8601(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
            return None;
        }
        let num = |range: std::ops::Range<usize>| -> Option<i64> {
            let part = text.get(range)?;
            part.bytes().all(|c| c.is_ascii_digit()).then(|| part.parse().ok())?
        };
        let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
        let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        let offset = match &text[19..] {
            "Z" => 0,
            zone if zone.len() == 6 && (zone.starts_with('+') || zone.starts_with('-')) && &zone[3..4] == ":" => {
                let h = num(20..22)?;
                let m = num(23..25)?;
                let sign = if zone.starts_with('-') { -1 } else { 1 };
                sign * (h * 3600 + m * 60)
            }
            _ => return None,
        };
        let days = days_from_civil(year, month, day);
        let unix = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
        Some(Date((unix - REFERENCE_DATE_OFFSET_SECONDS) as f64))
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso8601())
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Date::parse_iso8601(&text)
            .ok_or_else(|| de::Error::custom(format!("Expected date string to be ISO8601-formatted: {text}")))
    }
}

/// The plain `JSONEncoder` form, seconds since the reference date.
pub mod reference_seconds {
    use super::Date;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &Option<Date>, s: S) -> Result<S::Ok, S::Error> {
        match date {
            Some(d) => s.serialize_f64(d.0),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Date>, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.map(Date))
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_instant_spells_and_reads_back() {
        let stamped = Date::from_unix_seconds(1_700_000_000.0);
        assert_eq!(stamped.iso8601(), "2023-11-14T22:13:20Z");
        assert_eq!(Date::parse_iso8601("2023-11-14T22:13:20Z"), Some(stamped));
        assert_eq!(Date::parse_iso8601("2023-11-15T00:13:20+02:00"), Some(stamped));
        assert_eq!(Date::since_reference_date(0.0).iso8601(), "2001-01-01T00:00:00Z");
        // Twin: a fractional second is refused, as the formatter's default options refuse it.
        assert_eq!(Date::parse_iso8601("2023-11-14T22:13:20.5Z"), None);
        assert_ne!(Date::from_unix_seconds(1_700_000_001.0).iso8601(), stamped.iso8601());
    }
}
