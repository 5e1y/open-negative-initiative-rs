//! `Frames`: the year's photos as the library and their sidecars hold them, counted every way the
//! year is told: by month, kind, roll, film, paper, camera, lens, medal and care taken.

use super::Moment;
use oni_settings::{ConversionMode, Medal};
use std::collections::HashMap;

/// One photo of the library, as its index row and its sidecar describe it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub path: String,
    pub fingerprint: String,
    /// When it was shot or scanned, else when it joined the library, as a Unix time.
    pub unix: f64,
    pub medal: Medal,
    pub mode: ConversionMode,
    /// Its roll's identity and name.
    pub roll: Option<(String, String)>,
    /// The catalogue's key of the film it stands on, a roll's or a digital photo's virtual one.
    pub stock: Option<String>,
    pub paper: Option<String>,
    pub body: Option<String>,
    pub lens: Option<String>,
    pub grain: f32,
    pub halation: f32,
    /// Dust strokes healed on it.
    pub dust: usize,
    pub cropped: bool,
    pub straightened: bool,
}

/// Names with their counts, the most counted first; a tie keeps the name's own order.
pub type Ranked = Vec<(String, usize)>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frames {
    pub count: usize,
    pub by_month: [usize; 12],
    pub negatives: usize,
    pub monochromes: usize,
    pub slides: usize,
    pub digitals: usize,
    pub rolls: usize,
    /// The roll holding the most frames, by name.
    pub longest_roll: Option<(String, usize)>,
    pub stocks: Ranked,
    pub papers: Ranked,
    pub bodies: Ranked,
    pub lenses: Ranked,
    /// Gold medals, the latest dated first.
    pub gold: Vec<String>,
    pub silver: usize,
    pub bronze: usize,
    /// Dust strokes healed over the year, and the frames they were healed on.
    pub dust: usize,
    pub dusted: usize,
    pub dustiest: Option<(String, usize)>,
    pub cropped: usize,
    pub straightened: usize,
    pub grained: usize,
    pub haloed: usize,
    pub first: Option<String>,
    pub last: Option<String>,
    /// The month and day most frames are dated on, and their count.
    pub busiest_day: Option<(u32, u32, usize)>,
    /// The year's frames by date, each path with its fingerprint: what the palette and the backdrops read.
    pub paths: Vec<(String, String)>,
}

fn ranked(counts: HashMap<String, usize>) -> Ranked {
    let mut rows: Ranked = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

impl Frames {
    /// The frames `moment` dates in `year`, in any order.
    pub fn build(frames: &[Frame], year: i32, moment: impl Fn(f64) -> Moment) -> Frames {
        let mut mine: Vec<(&Frame, Moment)> = frames.iter().map(|f| (f, moment(f.unix))).filter(|(_, m)| m.year == year).collect();
        mine.sort_by(|a, b| a.0.unix.total_cmp(&b.0.unix).then_with(|| a.0.path.cmp(&b.0.path)));
        let mut out = Frames { count: mine.len(), ..Default::default() };
        let (mut stocks, mut papers, mut bodies, mut lenses) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
        let mut rolls = HashMap::<&str, (&str, usize)>::new();
        let mut days = HashMap::<u32, (u32, u32, usize)>::new();
        for (frame, at) in &mine {
            out.by_month[(at.month.clamp(1, 12) - 1) as usize] += 1;
            days.entry(at.ordinal).or_insert((at.month, at.day, 0)).2 += 1;
            match frame.mode {
                ConversionMode::Negative => out.negatives += 1,
                ConversionMode::Monochrome => out.monochromes += 1,
                ConversionMode::Positive => out.slides += 1,
                ConversionMode::Digital => out.digitals += 1,
            }
            if let Some((id, name)) = &frame.roll {
                rolls.entry(id).or_insert((name, 0)).1 += 1;
            }
            for (table, value) in [(&mut stocks, &frame.stock), (&mut papers, &frame.paper), (&mut bodies, &frame.body), (&mut lenses, &frame.lens)] {
                if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                    *table.entry(value.to_string()).or_default() += 1;
                }
            }
            match frame.medal {
                Medal::Gold => out.gold.push(frame.path.clone()),
                Medal::Silver => out.silver += 1,
                Medal::Bronze => out.bronze += 1,
                Medal::None => {}
            }
            if frame.dust > 0 {
                out.dust += frame.dust;
                out.dusted += 1;
                if out.dustiest.as_ref().is_none_or(|(_, n)| frame.dust > *n) {
                    out.dustiest = Some((frame.path.clone(), frame.dust));
                }
            }
            out.cropped += usize::from(frame.cropped);
            out.straightened += usize::from(frame.straightened);
            out.grained += usize::from(frame.grain > 0.0);
            out.haloed += usize::from(frame.halation > 0.0);
        }
        out.gold.reverse();
        out.rolls = rolls.len();
        out.longest_roll = rolls.into_iter().min_by(|a, b| b.1.1.cmp(&a.1.1).then(a.0.cmp(b.0))).map(|(_, (name, n))| (name.to_string(), n));
        out.busiest_day = days.into_iter().min_by(|a, b| b.1.2.cmp(&a.1.2).then(a.0.cmp(&b.0))).map(|(_, d)| d);
        (out.stocks, out.papers, out.bodies, out.lenses) = (ranked(stocks), ranked(papers), ranked(bodies), ranked(lenses));
        out.first = mine.first().map(|(f, _)| f.path.clone());
        out.last = mine.last().map(|(f, _)| f.path.clone());
        out.paths = mine.iter().map(|(f, _)| (f.path.clone(), f.fingerprint.clone())).collect();
        out
    }

    /// The month holding the most frames (1 to 12) and their count, the earlier on a tie.
    pub fn busiest_month(&self) -> Option<(u32, usize)> {
        let (month, count) = self.by_month.iter().enumerate().fold((0, 0), |best, (m, n)| if *n > best.1 { (m, *n) } else { best });
        (count > 0).then_some((month as u32 + 1, count))
    }

    /// The share of the year's frames standing on the most used film.
    pub fn top_stock_share(&self) -> f64 {
        self.stocks.first().map_or(0.0, |(_, n)| *n as f64 / self.count.max(1) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(month: u32, day: u32) -> f64 {
        oni_settings::Date::parse_iso8601(&format!("2026-{month:02}-{day:02}T12:00:00Z")).unwrap().unix_seconds()
    }

    fn frame(path: &str, unix: f64) -> Frame {
        Frame { path: path.into(), fingerprint: format!("fp{path}"), unix, ..Default::default() }
    }

    fn year() -> Vec<Frame> {
        let roll = |n: &str| Some((format!("id-{n}"), n.to_string()));
        vec![
            Frame { roll: roll("Lisbon"), stock: Some("kodak-portra400".into()), medal: Medal::Gold, dust: 12, cropped: true, ..frame("/b", at(3, 2)) },
            Frame { roll: roll("Lisbon"), stock: Some("kodak-portra400".into()), body: Some("Nikon FM2".into()), ..frame("/a", at(3, 2)) },
            Frame { roll: roll("Winter"), stock: Some("ilford-hp5".into()), mode: ConversionMode::Monochrome, medal: Medal::Gold, dust: 3, ..frame("/c", at(11, 20)) },
            Frame { mode: ConversionMode::Digital, stock: Some("kodak-portra400".into()), grain: 0.4, halation: 0.2, paper: Some("oni-standard".into()), ..frame("/d", at(7, 1)) },
            Frame { mode: ConversionMode::Positive, medal: Medal::Silver, straightened: true, body: Some(" ".into()), ..frame("/e", at(3, 9)) },
            Frame { medal: Medal::Gold, ..frame("/old", at(3, 2) - 400.0 * 86_400.0) },
        ]
    }

    #[test]
    fn a_years_frames_are_counted_every_way_the_year_is_told() {
        let frames = Frames::build(&year(), 2026, Moment::utc);
        assert_eq!(frames.count, 5);
        assert_eq!((frames.negatives, frames.monochromes, frames.slides, frames.digitals), (2, 1, 1, 1));
        assert_eq!(frames.busiest_month(), Some((3, 3)));
        assert_eq!(frames.busiest_day, Some((3, 2, 2)));
        assert_eq!((frames.rolls, frames.longest_roll.clone()), (2, Some(("Lisbon".into(), 2))));
        assert_eq!(frames.stocks, [("kodak-portra400".to_string(), 3), ("ilford-hp5".to_string(), 1)]);
        assert!((frames.top_stock_share() - 0.6).abs() < 1e-9, "three of the year's five frames, one naming no film");
        assert_eq!(frames.bodies, [("Nikon FM2".to_string(), 1)], "a blank name names no camera");
        assert_eq!(frames.gold, ["/c", "/b"], "the latest gold first");
        assert_eq!((frames.silver, frames.bronze), (1, 0));
        assert_eq!((frames.dust, frames.dusted, frames.dustiest.clone()), (15, 2, Some(("/b".into(), 12))));
        assert_eq!((frames.cropped, frames.straightened, frames.grained, frames.haloed), (1, 1, 1, 1));
        assert_eq!((frames.first.as_deref(), frames.last.as_deref()), (Some("/a"), Some("/c")), "the same day reads by path");
        assert_eq!(frames.paths.len(), 5);
        // Twin: another year holds the old frame alone, its gold with it.
        let before = Frames::build(&year(), 2025, Moment::utc);
        assert_eq!((before.count, before.gold.as_slice()), (1, ["/old".to_string()].as_slice()));
        assert_ne!(before.busiest_month(), frames.busiest_month());
    }

    #[test]
    fn an_empty_year_names_nothing() {
        let frames = Frames::build(&year(), 2030, Moment::utc);
        assert_eq!(frames, Frames::default());
        assert_eq!((frames.busiest_month(), frames.top_stock_share()), (None, 0.0));
        // Twin: the year holding frames is not the empty one.
        assert_ne!(Frames::build(&year(), 2026, Moment::utc), Frames::default());
    }
}
