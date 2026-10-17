//! What a gallery draws a heading for, and the library cut into rolls. Membership is read off each
//! frame, never a table: an index is a disposable cache, and a roll living only there dies with it.

use crate::index::Entry;
use crate::paths;
use chrono::{DateTime, Local, TimeZone};
use oni_settings::{Date, Film, Id};
use std::collections::HashMap;
use std::path::MAIN_SEPARATOR;

/// The bucket a frame falls in: its roll, its capture day (local midnight, whole seconds since
/// 1970), or the folder it lies in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilmKey {
    Roll(Id),
    Day(i64),
    Folder(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilmGroup {
    pub id: FilmKey,
    pub film: Option<Film>,
    /// Earliest capture in the group: its place in the feed absent a hand-dragged root order.
    pub start: Date,
    pub entries: Vec<Entry>,
    pub heading: Option<String>,
}

impl FilmGroup {
    pub fn is_explicit(&self) -> bool {
        self.film.is_some()
    }

    fn named(&self) -> Option<&str> {
        self.film.as_ref().map(|f| f.name.as_str()).filter(|n| !n.trim_matches(is_blank).is_empty())
    }

    /// A named roll heads with its name, a nameless one with its heading, else its day.
    pub fn display_title(&self) -> String {
        if let Some(name) = self.named() {
            return name.to_owned();
        }
        if let Some(heading) = &self.heading {
            return heading.clone();
        }
        day_label(start_of_day(self.start))
    }

    pub fn trail(&self) -> Option<String> {
        let heading = self.heading.as_ref()?;
        heading.rfind(MAIN_SEPARATOR).map(|cut| heading[..cut].to_owned())
    }

    pub fn heading_title(&self) -> String {
        if let Some(name) = self.named() {
            return name.to_owned();
        }
        if let Some(heading) = &self.heading {
            return paths::name(heading);
        }
        self.display_title()
    }

    /// The hand-set name alone, `None` on a day, so a rename field opens empty there.
    pub fn explicit_name(&self) -> Option<&str> {
        self.film.as_ref().map(|f| f.name.as_str())
    }
}

/// `CharacterSet.whitespaces`: tabs and space separators, never line breaks.
fn is_blank(c: char) -> bool {
    c == '\t'
        || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

fn local(date: Date) -> DateTime<Local> {
    let unix = date.unix_seconds();
    let seconds = unix.floor();
    let nanos = ((unix - seconds) * 1e9) as u32;
    DateTime::from_timestamp(seconds as i64, nanos).unwrap_or_default().with_timezone(&Local)
}

/// `Calendar.current.startOfDay(for:)`: the local midnight opening the day `date` falls in.
pub fn start_of_day(date: Date) -> Date {
    // A day whose midnight a clock change skips starts at its first hour that exists, as Calendar does.
    let day = local(date).date_naive();
    (0..24)
        .filter_map(|hour| day.and_hms_opt(hour, 0, 0))
        .find_map(|at| Local.from_local_datetime(&at).earliest())
        .map(|at| Date::from_unix_seconds(at.timestamp() as f64))
        .unwrap_or_else(|| Date::from_unix_seconds(date.unix_seconds().floor()))
}

/// The one place a day is spelled, `Saturday, October 17, 2026`.
pub fn day_label(day: Date) -> String {
    local(day).format("%A, %B %-d, %Y").to_string()
}

pub fn key(entry: &Entry) -> FilmKey {
    match &entry.film {
        Some(film) => FilmKey::Roll(film.id),
        None => FilmKey::Day(day_start(entry).unix_seconds() as i64),
    }
}

/// Capture day, so a roll stays one group across however many sessions it is imported in.
pub fn day_start(entry: &Entry) -> Date {
    start_of_day(capture(entry))
}

pub fn capture(entry: &Entry) -> Date {
    entry.captured_at.unwrap_or(entry.imported_at)
}

/// A frame's position among its siblings: hand-placed if ever dragged, its capture otherwise.
pub fn order(entry: &Entry) -> f64 {
    entry.manual_order.unwrap_or_else(|| capture(entry).time_interval_since_reference_date())
}

/// A roll's place among its root siblings: hand-placed if dragged, its earliest capture otherwise.
pub fn root_order(group: &FilmGroup) -> f64 {
    group.film.as_ref().and_then(|f| f.manual_order).unwrap_or_else(|| group.start.time_interval_since_reference_date())
}

/// The floor a drop's new orders spread above `ahead`, against a neighbour tying or reversing it.
pub fn behind(ahead: f64, prior_order: Option<f64>) -> f64 {
    prior_order.filter(|&p| p < ahead).unwrap_or(ahead - 86_400.0)
}

/// Spreads the run tied with `entries[index]` across the real gap below it, so any position in a
/// roll stamped with one capture time becomes insertable.
pub fn detied(entries: &[Entry], index: usize) -> Vec<(String, f64)> {
    let ahead = order(&entries[index]);
    let mut start = index;
    while start > 0 && order(&entries[start - 1]) >= ahead {
        start -= 1;
    }
    if start >= index {
        return Vec::new();
    }
    let floor = if start > 0 { order(&entries[start - 1]) } else { ahead - 86_400.0 };
    let span = ahead - floor;
    let count = (index - start + 1) as f64;
    (start..=index)
        .enumerate()
        .map(|(offset, i)| (entries[i].path.clone(), floor + span * (offset + 1) as f64 / (count + 1.0)))
        .collect()
}

pub const UNTITLED_STEM: &str = "Roll ";

/// The lowest free number, so forming one roll after another never proposes a name on screen.
pub fn untitled(taken: &[Film]) -> String {
    let numbers: std::collections::HashSet<i64> =
        taken.iter().filter_map(|f| f.name.strip_prefix(UNTITLED_STEM).and_then(|n| n.parse().ok())).collect();
    let mut number = 1;
    while numbers.contains(&number) {
        number += 1;
    }
    format!("{UNTITLED_STEM}{number}")
}

/// The library cut into rolls, each by its own order and the rolls by root order.
pub fn grouped(entries: &[Entry]) -> Vec<FilmGroup> {
    let mut buckets: HashMap<FilmKey, Vec<Entry>> = HashMap::new();
    let mut films: HashMap<FilmKey, Film> = HashMap::new();
    for entry in entries {
        let k = key(entry);
        buckets.entry(k.clone()).or_default().push(entry.clone());
        if let Some(film) = &entry.film {
            films.insert(k, film.clone());
        }
    }
    let mut groups: Vec<FilmGroup> = buckets
        .into_iter()
        .map(|(k, mut members)| {
            members.sort_by(|a, b| order(a).total_cmp(&order(b)));
            // The group's place stays on capture: reordering photos must not walk the roll.
            let start = members
                .iter()
                .map(capture)
                .min_by(|a, b| crate::seconds(*a).total_cmp(&crate::seconds(*b)))
                .unwrap_or(crate::distant_past());
            FilmGroup { film: films.get(&k).cloned(), id: k, start, entries: members, heading: None }
        })
        .collect();
    groups.sort_by(|a, b| {
        root_order(a).total_cmp(&root_order(b)).then_with(|| {
            let first = |g: &FilmGroup| g.entries.first().map(|e| e.path.clone()).unwrap_or_default();
            first(a).cmp(&first(b))
        })
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_bucket_by_id_and_days_by_local_midnight() {
        let base = Date::since_reference_date(800_000_000.0);
        let roll = Film::new("Trip");
        let mut a = Entry::new("/r/a", base, "a");
        a.film = Some(roll.clone());
        let mut b = Entry::new("/r/b", crate::later(base, 60.0), "b");
        b.film = Some(roll.clone());
        let c = Entry::new("/r/c", crate::later(base, 30.0), "c");
        let groups = grouped(&[b, c, a]);
        assert_eq!(groups.len(), 2);
        let trip = groups.iter().find(|g| g.film.is_some()).unwrap();
        assert_eq!(trip.entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["/r/a", "/r/b"]);
        assert_eq!(trip.display_title(), "Trip");
        let day = groups.iter().find(|g| g.film.is_none()).unwrap();
        assert_eq!(day.id, FilmKey::Day(start_of_day(crate::later(base, 30.0)).unix_seconds() as i64));
        // Twin: the bare instant is not its day's start unless it falls on local midnight.
        let midnight = start_of_day(base);
        assert!(midnight <= base && crate::seconds(base) - crate::seconds(midnight) < 86_400.0 + 3_600.0);
        assert_eq!(start_of_day(midnight), midnight);
    }

    /// Sao Paulo skipped from 00:00 to 01:00 on 2018-11-04; run in a child holding that zone,
    /// since the local zone is read once per process.
    #[test]
    fn a_day_whose_midnight_is_skipped_starts_at_its_first_hour() {
        let noon = Date::from_unix_seconds(1_541_340_000.0);
        if std::env::var("TZ").as_deref() != Ok("America/Sao_Paulo") {
            if !std::path::Path::new("/usr/share/zoneinfo/America/Sao_Paulo").exists() {
                eprintln!("no zone data for America/Sao_Paulo: not exercised");
                return;
            }
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "film_group::tests::a_day_whose_midnight_is_skipped_starts_at_its_first_hour"])
                .env("TZ", "America/Sao_Paulo")
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&child.stdout);
            assert!(child.status.success() && text.contains("1 passed"), "{text}");
            return;
        }
        assert_eq!(start_of_day(noon).unix_seconds(), 1_541_300_400.0, "01:00 local, 03:00 UTC");
        // Twin: twelve hours back from noon lands at 23:00 the day before, since the day is 23 hours long.
        let naive = Date::from_unix_seconds(noon.unix_seconds() - 12.0 * 3600.0);
        assert_ne!(local(naive).date_naive(), local(noon).date_naive());
    }

    #[test]
    fn a_fresh_roll_takes_the_lowest_free_number() {
        let taken = [Film::new("Roll 1"), Film::new("Roll 3")];
        assert_eq!(untitled(&taken), "Roll 2");
        assert_eq!(untitled(&[]), "Roll 1");
        // Twin: the count plus one would collide with Roll 3.
        assert_ne!(untitled(&taken), format!("Roll {}", taken.len() + 1));
    }

    #[test]
    fn a_tied_run_spreads_below_its_last_member() {
        let at = Date::since_reference_date(1000.0);
        let entries: Vec<Entry> = (0..3)
            .map(|i| {
                let mut e = Entry::new(&format!("/r/{i}"), at, "x");
                e.captured_at = Some(at);
                e
            })
            .collect();
        let spread = detied(&entries, 2);
        let orders: Vec<f64> = spread.iter().map(|s| s.1).collect();
        assert_eq!(spread.len(), 3);
        assert!(orders.windows(2).all(|w| w[0] < w[1]) && orders[2] < 1000.0);
        // Twin: a frame with no tie below it needs nothing spread.
        assert!(detied(&entries, 0).is_empty());
    }
}
