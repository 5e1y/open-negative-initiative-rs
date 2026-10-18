//! What the info sidebar reads off the library: a project's stocks counted by photo, the shot date
//! typed by hand and clamped on the calendar, and the roll or project a selection stands on.

use crate::film_group::FilmGroup;
use crate::index::Entry;
use crate::project::{self, Project, ProjectGroup};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use oni_settings::film::{FilmStockKind, film_stock};
use oni_settings::{Date, Id};
use std::collections::HashMap;

/// `InfoSidebar.stocks(among:)`: each stock of the photos' rolls with its count of photos, by
/// display name; a photo with no stock, or one the catalogue does not know, gives no row.
pub fn stocks(entries: &[Entry]) -> Vec<(&'static FilmStockKind, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for stock in entries.iter().filter_map(|e| e.film.as_ref()?.stock.as_deref()) {
        *counts.entry(stock).or_default() += 1;
    }
    let mut rows: Vec<_> = counts.into_iter().filter_map(|(id, n)| film_stock(Some(id)).map(|k| (k, n))).collect();
    rows.sort_by(|a, b| a.0.display_name.cmp(b.0.display_name));
    rows
}

/// `DateFields.made`: typed by hand, clamped rather than refused, so a day past the month's end
/// lands on its last day; a month outside 1 to 12 is refused. The local midnight of that day.
pub fn made(year: i32, month: u32, day: i32) -> Option<Date> {
    if !(1..=12).contains(&month) {
        return None;
    }
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1)? } else { NaiveDate::from_ymd_opt(year, month + 1, 1)? };
    let days = (next - first).num_days() as i32;
    let date = NaiveDate::from_ymd_opt(year, month, day.clamp(1, days) as u32)?;
    // A midnight a clock change skips gives the day's first hour that exists, as Calendar does.
    (0..24)
        .filter_map(|hour| date.and_hms_opt(hour, 0, 0))
        .find_map(|at| Local.from_local_datetime(&at).earliest())
        .map(|at| Date::from_unix_seconds(at.timestamp() as f64))
}

/// The local year, month and day the date fields show.
pub fn ymd(date: Date) -> (i32, u32, u32) {
    let unix = date.unix_seconds();
    let at = chrono::DateTime::from_timestamp(unix.floor() as i64, 0).unwrap_or_default().with_timezone(&Local);
    (at.year(), at.month(), at.day())
}

/// `resolvedRoll`: the roll a click on its row named, else the anchor photo's; a group with no
/// roll (a day, a folder) resolves to nothing.
pub fn resolved_roll<'a>(groups: &'a [FilmGroup], selected: Option<&crate::FilmKey>, anchor: Option<&str>) -> Option<&'a FilmGroup> {
    let group = match selected {
        Some(key) => groups.iter().find(|g| g.id == *key),
        None => groups.iter().find(|g| g.entries.iter().any(|e| Some(e.path.as_str()) == anchor)),
    }?;
    group.film.is_some().then_some(group)
}

/// `resolvedProjectGroup`: the project a click on its row named, else the anchor photo's.
pub fn resolved_project(projects: &[Project], entries: &[Entry], selected: Option<Id>, anchor: Option<&str>) -> Option<ProjectGroup> {
    let made = project::grouped(projects, entries);
    match selected {
        Some(id) => made.into_iter().find(|g| g.id == id),
        None => {
            let anchor = anchor?;
            made.into_iter().find(|g| g.entries.iter().any(|e| e.path == anchor))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::Film;

    fn shot(path: &str, stock: Option<&str>) -> Entry {
        let mut e = Entry::new(path, Date::since_reference_date(8e8), path);
        let mut film = Film::new(path);
        film.stock = stock.map(str::to_owned);
        e.film = Some(film);
        e
    }

    /// `InfoSidebar.selfCheck`, its stock half.
    #[test]
    fn a_projects_stocks_are_counted_by_photo_and_a_photo_with_none_gives_no_row() {
        let rows = stocks(&[shot("/a", Some("kodak-gold")), shot("/b", Some("kodak-gold")), shot("/c", None), shot("/d", Some("kodak-ektachrome"))]);
        let read: Vec<(&str, usize)> = rows.iter().map(|(k, n)| (k.id, *n)).collect();
        assert_eq!(read, [("kodak-ektachrome", 1), ("kodak-gold", 2)], "sorted by display name");
        assert!(stocks(&[shot("/x", Some("no-such-stock"))]).is_empty());
        // Twin: counted by roll instead of by photo, Gold would read once.
        assert_ne!(read[1].1, 1);
    }

    /// `InfoSidebar.selfCheck`, its date half: the clamp reads the real calendar.
    #[test]
    fn a_typed_date_clamps_to_its_months_last_day_and_an_impossible_month_is_refused() {
        assert_eq!(made(2026, 4, 15).map(ymd), Some((2026, 4, 15)), "a real date passes through untouched");
        assert_eq!(made(2026, 4, 31).map(ymd), Some((2026, 4, 30)), "April has 30 days, not into May");
        assert_eq!(made(2026, 13, 1), None);
        assert_eq!(made(2026, 0, 1), None);
        assert_eq!(made(2024, 2, 30).map(ymd), Some((2024, 2, 29)));
        assert_eq!(made(2026, 2, 30).map(ymd), Some((2026, 2, 28)));
        assert_eq!(made(2026, 12, 0).map(ymd), Some((2026, 12, 1)), "a day under one lands on the first");
        // Twin: a hardcoded 30-day month would keep 30 February.
        assert_ne!(made(2026, 2, 30).map(ymd), Some((2026, 2, 30)));
    }

    #[test]
    fn the_sidebar_stands_on_the_ringed_roll_else_the_anchors_and_never_on_a_day() {
        use crate::film_group::FilmKey;
        let roll = FilmGroup { id: FilmKey::Roll(Id::new()), film: Some(Film::new("Roll 1")), start: Date::since_reference_date(0.0), entries: vec![shot("/r/a", None)], heading: None };
        let day = FilmGroup { id: FilmKey::Day(0), film: None, start: Date::since_reference_date(0.0), entries: vec![shot("/d/a", None)], heading: None };
        let groups = [roll.clone(), day.clone()];
        assert_eq!(resolved_roll(&groups, None, Some("/r/a")).map(|g| g.id.clone()), Some(roll.id.clone()));
        assert!(resolved_roll(&groups, None, Some("/d/a")).is_none(), "a day is no roll");
        assert_eq!(resolved_roll(&groups, Some(&roll.id), Some("/d/a")).map(|g| g.id.clone()), Some(roll.id.clone()), "the ring wins");
        let trip = Project::new("Trip", Date::since_reference_date(0.0), vec!["/d/a".into()]);
        let entries = [shot("/d/a", None)];
        assert_eq!(resolved_project(std::slice::from_ref(&trip), &entries, None, Some("/d/a")).map(|g| g.id), Some(trip.id));
        // Twin: an anchor in no project resolves to none.
        assert!(resolved_project(std::slice::from_ref(&trip), &entries, None, Some("/r/a")).is_none());
    }
}
