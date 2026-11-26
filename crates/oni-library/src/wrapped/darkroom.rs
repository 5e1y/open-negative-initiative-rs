//! `Darkroom`: a year of gestures read off the journal, what the hands did and when.

use super::Moment;
use crate::journal::{Kind, Record};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Two gestures further apart than this are two sittings, the gap between them not counted.
pub const SESSION_BREAK: u32 = 600;

/// `Medal::Gold`'s rank, the detail a gold medal's record carries.
pub const GOLD: u16 = 3;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Darkroom {
    /// Distinct photos opened in the editor.
    pub photos: usize,
    /// Settled edits, one a row a gesture moved.
    pub gestures: usize,
    pub most_edited: Option<(String, usize)>,
    /// Every parameter row moved, by its rank in `Parameters.all`, with its count, the most moved first.
    pub favourite_rows: Vec<(usize, usize)>,
    pub favourite_preset: Option<(String, usize)>,
    pub balanced: usize,
    pub pasted: usize,
    pub exported: usize,
    pub rolls: usize,
    /// Frames measured by Prepare the Roll.
    pub measured: usize,
    pub hours: f64,
    /// The longest sitting, in hours.
    pub longest_sitting: f64,
    /// Every gesture by the hour of the day it was made at.
    pub by_hour: [usize; 24],
    pub days: usize,
    /// The longest run of days in a row with a gesture.
    pub streak: usize,
    /// The month and day most gestures were made on, and their count.
    pub busiest_day: Option<(u32, u32, usize)>,
}

/// The key that reached `order` first wins a tie, so the same journal always names the same photo.
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

/// The longest run of consecutive days among `days`, days of one year.
fn longest_run(days: &BTreeSet<u32>) -> usize {
    let (mut best, mut run, mut last) = (0, 0, None::<u32>);
    for day in days {
        run = if last.is_some_and(|l| l + 1 == *day) { run + 1 } else { 1 };
        best = best.max(run);
        last = Some(*day);
    }
    best
}

impl Darkroom {
    /// `hidden` names the photos in the ONI Trash: their records count for nothing, as if never made.
    pub fn build(records: &[Record], names: &[String], year: i32, hidden: &HashSet<String>, moment: impl Fn(u32) -> Moment) -> Darkroom {
        let mut out = Darkroom::default();
        let name = |index: u32| names.get(index as usize).cloned();
        let mut mine: Vec<(&Record, Moment)> = records
            .iter()
            .filter(|r| !name(r.subject).is_some_and(|n| hidden.contains(&n)))
            .map(|r| (r, moment(r.time)))
            .filter(|(_, m)| m.year == year)
            .collect();
        mine.sort_by_key(|(r, _)| r.time);
        let mut opened = HashSet::new();
        let (mut edits, mut edit_order) = (HashMap::<u32, usize>::new(), Vec::new());
        let mut rows = HashMap::<usize, usize>::new();
        let (mut presets, mut preset_order) = (HashMap::<u32, usize>::new(), Vec::new());
        let (mut days, mut per_day) = (BTreeSet::new(), HashMap::<u32, (u32, u32, usize)>::new());
        let (mut last, mut sitting) = (None::<u32>, 0.0);
        for (record, at) in mine {
            if let Some(previous) = last
                && record.time - previous <= SESSION_BREAK
            {
                let gap = f64::from(record.time - previous) / 3600.0;
                out.hours += gap;
                sitting += gap;
            } else {
                sitting = 0.0;
            }
            out.longest_sitting = out.longest_sitting.max(sitting);
            last = Some(record.time);
            out.by_hour[at.hour as usize % 24] += 1;
            days.insert(at.ordinal);
            per_day.entry(at.ordinal).or_insert((at.month, at.day, 0)).2 += 1;
            match record.kind {
                Kind::Opened => {
                    opened.insert(record.subject);
                }
                Kind::Edited => {
                    out.gestures += 1;
                    if !edits.contains_key(&record.subject) {
                        edit_order.push(record.subject);
                    }
                    *edits.entry(record.subject).or_default() += 1;
                    *rows.entry(usize::from(record.detail)).or_default() += 1;
                }
                Kind::Exported => out.exported += 1,
                Kind::Preset => {
                    if !presets.contains_key(&record.value) {
                        preset_order.push(record.value);
                    }
                    *presets.entry(record.value).or_default() += 1;
                }
                Kind::Balanced => out.balanced += 1,
                Kind::Rolled => out.rolls += 1,
                Kind::Pasted => out.pasted += record.value as usize,
                Kind::Measured => out.measured += record.value as usize,
                Kind::Medal | Kind::Exposed => {}
            }
        }
        out.photos = opened.len();
        out.days = days.len();
        out.streak = longest_run(&days);
        out.busiest_day = per_day.into_iter().min_by(|a, b| b.1.2.cmp(&a.1.2).then(a.0.cmp(&b.0))).map(|(_, d)| d);
        out.most_edited = most(&edits, &edit_order).and_then(|(subject, n)| name(subject).map(|p| (p, n)));
        let mut ranked: Vec<(usize, usize)> = rows.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.favourite_rows = ranked;
        out.favourite_preset = most(&presets, &preset_order).and_then(|(index, n)| name(index).map(|p| (p, n)));
        out
    }

    /// The share of the year's gestures made between 22:00 and 04:59.
    pub fn night_share(&self) -> f64 {
        let all: usize = self.by_hour.iter().sum();
        let night: usize = self.by_hour.iter().enumerate().filter(|(h, _)| *h >= 22 || *h < 5).map(|(_, n)| n).sum();
        if all == 0 { 0.0 } else { night as f64 / all as f64 }
    }

    /// The share made between 05:00 and 08:59.
    pub fn morning_share(&self) -> f64 {
        let all: usize = self.by_hour.iter().sum();
        let morning: usize = self.by_hour[5..9].iter().sum();
        if all == 0 { 0.0 } else { morning as f64 / all as f64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{NOTHING, epoch};

    fn record(kind: Kind, detail: u16, time: u32, subject: u32, value: u32) -> Record {
        Record { kind, detail, time, subject, value }
    }

    fn utc(time: u32) -> Moment {
        Moment::utc(Moment::journal_unix(time))
    }

    fn june() -> u32 {
        (oni_settings::Date::parse_iso8601("2026-06-01T00:00:00Z").unwrap().unix_seconds() - epoch().unix_seconds()) as u32
    }

    #[test]
    fn a_years_gestures_read_back_and_a_year_later_count_for_nothing() {
        let june = june();
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
            record(Kind::Pasted, 0, june + 410, NOTHING, 7),
        ];
        let year = Darkroom::build(&records, &names, 2026, &HashSet::new(), utc);
        assert_eq!((year.photos, year.gestures), (2, 3));
        assert_eq!(year.most_edited, Some(("/a".into(), 2)));
        assert_eq!(year.favourite_rows.first(), Some(&(4, 2)));
        assert_eq!(year.favourite_preset, Some(("Portra warm".into(), 1)));
        assert_eq!((year.exported, year.pasted), (1, 7));
        assert!((year.hours - 410.0 / 3600.0).abs() < 1e-9);
        assert_eq!(year.by_hour[0], records.len(), "midnight UTC, every gesture");
        assert_eq!((year.days, year.streak, year.busiest_day), (1, 1, Some((6, 1, records.len()))));
        // Twin: the same gestures a year later count for nothing.
        let later: Vec<Record> = records.iter().map(|r| Record { time: r.time + 366 * 86_400, ..*r }).collect();
        let next = Darkroom::build(&later, &names, 2026, &HashSet::new(), utc);
        assert_eq!((next.photos, next.most_edited.clone(), next.days), (0, None, 0));
    }

    #[test]
    fn a_trashed_photo_counts_for_nothing_and_a_long_pause_is_not_time_spent() {
        let names: Vec<String> = ["/a", "/b"].map(String::from).to_vec();
        let t = 200_000_000;
        let records = [
            record(Kind::Opened, 0, t, 0, 0),
            record(Kind::Opened, 0, t + 600, 1, 0),
            record(Kind::Edited, 1, t + 1_201, 0, 0),
            record(Kind::Edited, 1, t + 1_300, 1, 0),
            record(Kind::Edited, 1, t + 1_400, 1, 0),
            record(Kind::Balanced, 0, t + 1_500, NOTHING, 0),
            record(Kind::Balanced, 2, t + 1_510, NOTHING, 0),
        ];
        let year_of = |time: u32| Moment { year: 2026, hour: (time / 3600 % 24), ordinal: 1, month: 1, day: 1 };
        let all = Darkroom::build(&records, &names, 2026, &HashSet::new(), year_of);
        assert_eq!((all.photos, all.balanced), (2, 2));
        // 600 s counts, 601 s is a new sitting; then 99 + 100 + 100 + 10.
        assert!((all.hours - (600.0 + 99.0 + 100.0 + 100.0 + 10.0) / 3600.0).abs() < 1e-9);
        assert!((all.longest_sitting - 600.0 / 3600.0).abs() < 1e-9, "the first sitting, longer than the second's 309 s");
        let hidden = HashSet::from(["/a".to_string()]);
        let kept = Darkroom::build(&records, &names, 2026, &hidden, year_of);
        assert_eq!((kept.photos, kept.gestures), (1, 2));
        // Twin: with nothing hidden the same journal still counts the trashed photo.
        assert_ne!(kept, all);
    }

    #[test]
    fn days_in_a_row_make_a_streak_and_the_night_is_told_from_the_morning() {
        let day = 86_400;
        let start = june();
        let at = |d: u32, hour: u32| record(Kind::Edited, 0, start + d * day + hour * 3600, 0, 0);
        let records = [at(0, 23), at(1, 23), at(2, 1), at(2, 23), at(5, 6), at(6, 6), at(30, 12)];
        let names = vec!["/a".to_string()];
        let year = Darkroom::build(&records, &names, 2026, &HashSet::new(), utc);
        assert_eq!((year.days, year.streak), (6, 3), "June 1 to 3 in a row, 6 and 7, then July 1");
        assert_eq!(year.busiest_day, Some((6, 3, 2)));
        assert!((year.night_share() - 4.0 / 7.0).abs() < 1e-9);
        assert!((year.morning_share() - 2.0 / 7.0).abs() < 1e-9);
        // Twin: a day skipped breaks the run.
        assert_eq!(longest_run(&BTreeSet::from([1, 2, 4, 5, 6])), 3);
        assert_ne!(longest_run(&BTreeSet::from([1, 3, 5])), 3);
    }
}
