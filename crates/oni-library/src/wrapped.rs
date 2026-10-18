//! `WrappedYear`: a year read off the journal, the numbers the year's summary shows.

use crate::journal::{Kind, Record, epoch};
use std::collections::{HashMap, HashSet};

/// Two gestures further apart than this are two sittings, the gap between them not counted.
pub const SESSION_BREAK: u32 = 600;

/// `Medal.allCases.firstIndex(of: .gold)`, the detail a gold medal's record carries.
pub const GOLD: u16 = 3;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WrappedYear {
    pub year: i32,
    pub photos: usize,
    pub most_edited: Option<(String, usize)>,
    pub favourite_rows: Vec<(usize, usize)>,
    pub gold: Vec<String>,
    pub favourite_balance: Option<(usize, usize)>,
    pub favourite_preset: Option<(String, usize)>,
    pub exported: usize,
    pub rolls: usize,
    pub hours: f64,
}

/// The calendar year a record's time falls in, on this machine's clock.
pub fn local_year(time: u32) -> i32 {
    use chrono::Datelike;
    let unix = epoch().unix_seconds() as i64 + i64::from(time);
    chrono::DateTime::from_timestamp(unix, 0).map_or(0, |d| d.with_timezone(&chrono::Local).year())
}

/// This machine's year and month now, which `offerWrapped` reads.
pub fn today() -> (i32, u32) {
    use chrono::Datelike;
    let now = chrono::Local::now();
    (now.year(), now.month())
}

/// The same year read in UTC, as the self-check reads it.
pub fn utc_year(time: u32) -> i32 {
    use chrono::Datelike;
    let unix = epoch().unix_seconds() as i64 + i64::from(time);
    chrono::DateTime::from_timestamp(unix, 0).map_or(0, |d| d.year())
}

/// The key that reached `top` first wins a tie, so the same journal always names the same photo.
fn most<K: Copy + Eq + std::hash::Hash>(counts: &HashMap<K, usize>, order: &[K]) -> Option<(K, usize)> {
    let mut best: Option<(K, usize)> = None;
    for key in order {
        let count = counts[key];
        if best.is_none_or(|(_, b)| count > b) {
            best = Some((*key, count));
        }
    }
    best
}

impl WrappedYear {
    /// `hidden` names the photos in the ONI Trash: their records count for nothing, as if never made.
    pub fn build(records: &[Record], names: &[String], year: i32, hidden: &HashSet<String>, year_of: impl Fn(u32) -> i32) -> WrappedYear {
        let mut out = WrappedYear { year, ..Default::default() };
        let name = |index: u32| names.get(index as usize).cloned();
        let mut mine: Vec<&Record> = records
            .iter()
            .filter(|r| year_of(r.time) == year && !name(r.subject).is_some_and(|n| hidden.contains(&n)))
            .collect();
        mine.sort_by_key(|r| r.time);
        let mut opened = HashSet::new();
        let (mut edits, mut edit_order) = (HashMap::<u32, usize>::new(), Vec::new());
        let mut rows = HashMap::<usize, usize>::new();
        let (mut medals, mut gold_order) = (HashMap::<u32, u16>::new(), Vec::<u32>::new());
        let mut balances = HashMap::<u16, usize>::new();
        let (mut presets, mut preset_order) = (HashMap::<u32, usize>::new(), Vec::new());
        let mut last: Option<u32> = None;
        for record in mine {
            if let Some(previous) = last
                && record.time - previous <= SESSION_BREAK
            {
                out.hours += f64::from(record.time - previous) / 3600.0;
            }
            last = Some(record.time);
            match record.kind {
                Kind::Opened => {
                    opened.insert(record.subject);
                }
                Kind::Edited => {
                    if !edits.contains_key(&record.subject) {
                        edit_order.push(record.subject);
                    }
                    *edits.entry(record.subject).or_default() += 1;
                    *rows.entry(usize::from(record.detail)).or_default() += 1;
                }
                Kind::Exported => out.exported += 1,
                Kind::Medal => {
                    medals.insert(record.subject, record.detail);
                    if record.detail == GOLD {
                        gold_order.retain(|s| *s != record.subject);
                        gold_order.push(record.subject);
                    }
                }
                Kind::Preset => {
                    if !presets.contains_key(&record.value) {
                        preset_order.push(record.value);
                    }
                    *presets.entry(record.value).or_default() += 1;
                }
                Kind::Balanced => *balances.entry(record.detail).or_default() += 1,
                Kind::Rolled => out.rolls += 1,
                Kind::Exposed | Kind::Pasted | Kind::Measured => {}
            }
        }
        out.photos = opened.len();
        out.most_edited = most(&edits, &edit_order).and_then(|(subject, n)| name(subject).map(|p| (p, n)));
        let mut ranked: Vec<(usize, usize)> = rows.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.favourite_rows = ranked.into_iter().take(3).collect();
        out.gold = gold_order.into_iter().filter(|s| medals.get(s) == Some(&GOLD)).filter_map(name).collect();
        // One key stands, so every press counts toward it, a retired key's included.
        if !balances.is_empty() {
            out.favourite_balance = Some((0, balances.values().sum()));
        }
        out.favourite_preset = most(&presets, &preset_order).and_then(|(index, n)| name(index).map(|p| (p, n)));
        out
    }

    /// `ONI_WRAPPED_SAMPLE`: a full year, so the summary can be looked at before one is lived.
    pub fn sample(year: i32, photos: &[String]) -> WrappedYear {
        WrappedYear {
            year,
            photos: photos.len().max(5_000),
            most_edited: photos.first().map(|p| (p.clone(), 412)),
            favourite_rows: vec![(0, 180), (1, 120), (2, 60)],
            gold: photos.iter().skip(1).take(9).cloned().collect(),
            favourite_balance: Some((0, 318)),
            favourite_preset: Some(("Portra warm".into(), 212)),
            exported: 1_284,
            rolls: 43,
            hours: 162.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::NOTHING;

    fn record(kind: Kind, detail: u16, time: u32, subject: u32, value: u32) -> Record {
        Record { kind, detail, time, subject, value }
    }

    /// `WrappedYear.selfCheck`: June 2026 in UTC, read back as its cards' numbers.
    #[test]
    fn a_years_gestures_read_back_as_its_cards_and_a_year_later_count_for_nothing() {
        let june = (oni_settings::Date::parse_iso8601("2026-06-01T00:00:00Z").unwrap().unix_seconds() - epoch().unix_seconds()) as u32;
        let names: Vec<String> = ["/a", "/b", "Portra warm"].map(String::from).to_vec();
        let records = [
            record(Kind::Opened, 0, june, 0, 0),
            record(Kind::Opened, 0, june + 60, 1, 0),
            record(Kind::Edited, 4, june + 90, 0, 0),
            record(Kind::Edited, 4, june + 120, 0, 0),
            record(Kind::Edited, 9, june + 150, 1, 0),
            record(Kind::Medal, GOLD, june + 200, 1, 0),
            record(Kind::Preset, 0, june + 300, 0, 2),
            record(Kind::Exported, 0, june + 400, 0, 0),
        ];
        let year = WrappedYear::build(&records, &names, 2026, &HashSet::new(), utc_year);
        assert_eq!(year.photos, 2);
        assert_eq!(year.most_edited, Some(("/a".into(), 2)));
        assert_eq!(year.favourite_rows.first(), Some(&(4, 2)));
        assert_eq!(year.gold, ["/b"]);
        assert_eq!(year.favourite_preset, Some(("Portra warm".into(), 1)));
        assert_eq!(year.exported, 1);
        assert!((year.hours - 400.0 / 3600.0).abs() < 1e-9);
        // Twin: the same gestures a year later count for nothing.
        let later: Vec<Record> = records.iter().map(|r| Record { time: r.time + 366 * 86_400, ..*r }).collect();
        let next = WrappedYear::build(&later, &names, 2026, &HashSet::new(), utc_year);
        assert_eq!((next.photos, next.most_edited.clone()), (0, None));
    }

    #[test]
    fn a_trashed_photo_counts_for_nothing_and_a_long_pause_is_not_time_spent() {
        let names: Vec<String> = ["/a", "/b"].map(String::from).to_vec();
        let t = 200_000_000;
        let records = [
            record(Kind::Opened, 0, t, 0, 0),
            record(Kind::Opened, 0, t + 600, 1, 0),
            record(Kind::Medal, GOLD, t + 1_201, 0, 0),
            record(Kind::Medal, GOLD, t + 1_300, 1, 0),
            record(Kind::Medal, 1, t + 1_400, 1, 0),
            record(Kind::Balanced, 0, t + 1_500, NOTHING, 0),
            record(Kind::Balanced, 2, t + 1_510, NOTHING, 0),
        ];
        let year_of = |_| 2026;
        let all = WrappedYear::build(&records, &names, 2026, &HashSet::new(), year_of);
        assert_eq!(all.photos, 2);
        assert_eq!(all.gold, ["/a"], "a gold taken back by bronze leaves the gold list");
        assert_eq!(all.favourite_balance, Some((0, 2)), "every key's press counts toward the one that stands");
        // 600 s counts, 601 s is a new sitting; then 99 + 100 + 100 + 10.
        assert!((all.hours - (600.0 + 99.0 + 100.0 + 100.0 + 10.0) / 3600.0).abs() < 1e-9);
        let hidden = HashSet::from(["/a".to_string()]);
        let kept = WrappedYear::build(&records, &names, 2026, &hidden, year_of);
        assert_eq!(kept.photos, 1);
        assert!(kept.gold.is_empty());
        // Twin: with nothing hidden the same journal still counts the trashed photo.
        assert_ne!(kept, all);
    }
}
