//! The year on film told scene by scene: what each frame of the strip says, read off a `Year`. A scene
//! with nothing to tell is not laid, so a short year is a short story and an empty one an invitation.

use oni_i18n::{named, tr, trn};
use oni_library::wrapped::Year;

pub mod gather;
pub mod sample;
pub mod share;
mod words;

pub use words::{MONTHS, figure, film_name, persona_words, row_name};
use words::{counted, day_name, kinds, named_rows, palette_words, paper_name, percent, trait_name};

/// The colours a year's palette gathers its photos around.
pub const SWATCHES: usize = 5;

/// The gold medals a frame shows, two rows of three prints; and the lines its summary holds.
pub const GOLD_SHOWN: usize = 6;
pub const SUMMARY_LINES: usize = 5;

/// How a scene is laid on its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    Title,
    /// A number and its words, over one photo when it names one.
    #[default]
    Figure,
    /// A figure over a row of bars, one lit: the months, the hours.
    Bars,
    /// A film and its canister.
    Film,
    /// A figure over named lines, each with its share.
    Lines,
    /// A grid of photos.
    Photos,
    Palette,
    Persona,
    Summary,
    Farewell,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    pub label: String,
    pub value: String,
    /// The line's weight against the heaviest, 0 to 1; 0 draws no bar.
    pub share: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub kind: Kind,
    pub eyebrow: String,
    /// The scene's large words; when `count` is given they are its number, counted up as the scene shows.
    pub figure: String,
    pub count: Option<usize>,
    /// What follows the counted number: an hour's ` h`.
    pub unit: String,
    pub caption: String,
    pub note: String,
    /// The photos it shows, by path.
    pub photos: Vec<String>,
    pub lines: Vec<Line>,
    pub bars: Vec<f32>,
    pub lit: Option<usize>,
    /// What is written under the bars, spread evenly.
    pub marks: Vec<String>,
    pub swatches: Vec<([u8; 3], f32)>,
    /// The catalogue's key of the film whose canister it shows.
    pub stock: Option<String>,
}

fn normalised(values: &[usize]) -> Vec<f32> {
    let top = values.iter().copied().max().unwrap_or(0).max(1) as f32;
    values.iter().map(|v| *v as f32 / top).collect()
}

fn ranked_lines(rows: &[(String, usize)], name: impl Fn(&str) -> String) -> Vec<Line> {
    let top = rows.first().map_or(1, |(_, n)| *n).max(1) as f32;
    rows.iter().map(|(key, n)| Line { label: name(key), value: figure(*n), share: *n as f32 / top }).collect()
}

/// The photos a year is proudest of: its gold medals, then its first and last frames.
fn best(year: &Year, count: usize) -> Vec<String> {
    let mut out: Vec<String> = year.frames.gold.clone();
    for path in [&year.frames.last, &year.frames.first].into_iter().flatten().chain(year.frames.paths.iter().rev().map(|(p, _)| p)) {
        if !out.contains(path) {
            out.push(path.clone());
        }
    }
    out.truncate(count);
    out
}

pub fn scenes(year: &Year) -> Vec<Scene> {
    let (f, d) = (&year.frames, &year.darkroom);
    let mut out = vec![Scene {
        kind: Kind::Title,
        figure: year.year.to_string(),
        note: tr("Read off this computer alone. Nothing was sent anywhere.").into(),
        photos: best(year, 3),
        ..Default::default()
    }];
    if year.is_empty() {
        out.push(Scene {
            eyebrow: tr("A fresh roll").into(),
            figure: tr("Frame 0").into(),
            caption: tr("Your year on film is still unexposed.").into(),
            note: tr("Everything you convert from now on will be told here next December.").into(),
            ..Default::default()
        });
        out.push(farewell());
        return out;
    }
    if f.count > 0 {
        out.push(Scene {
            eyebrow: tr("This year").into(),
            count: Some(f.count),
            caption: trn("frame brought to light", "frames brought to light", f.count as u64),
            note: kinds(year),
            photos: f.first.iter().cloned().collect(),
            ..Default::default()
        });
    } else if d.photos > 0 {
        out.push(Scene {
            eyebrow: tr("This year").into(),
            count: Some(d.photos),
            caption: trn("photo opened in the darkroom", "photos opened in the darkroom", d.photos as u64),
            ..Default::default()
        });
    }
    if let Some((month, count)) = f.busiest_month() {
        let day = f.busiest_day.filter(|(_, _, n)| *n > 1).map(|(m, d, n)| tr!("{day} alone holds {count}", day = day_name(m, d), count = figure(n)));
        out.push(Scene {
            kind: Kind::Bars,
            eyebrow: tr("Your busiest month").into(),
            figure: tr(MONTHS[(month - 1) as usize]).into(),
            caption: counted(trn("{count} frame", "{count} frames", count as u64), count),
            note: day.unwrap_or_default(),
            bars: normalised(&f.by_month),
            lit: Some((month - 1) as usize),
            marks: MONTHS.iter().map(|m| tr(m).chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()).collect(),
            ..Default::default()
        });
    }
    if let Some((key, count)) = f.stocks.first() {
        out.push(Scene {
            kind: Kind::Film,
            eyebrow: tr("Your film of the year").into(),
            figure: film_name(key),
            caption: tr!("{count} frames, {share} % of your year", count = figure(*count), share = percent(*count as f64 / f.count.max(1) as f64)),
            note: if f.stocks.len() > 1 { counted(trn("{count} film this year", "{count} films this year", f.stocks.len() as u64), f.stocks.len()) } else { String::new() },
            lines: ranked_lines(&f.stocks[1..f.stocks.len().min(4)], film_name),
            stock: Some(key.clone()),
            ..Default::default()
        });
    }
    if f.rolls > 0 {
        let longest = f.longest_roll.as_ref().filter(|_| f.rolls > 1).map(|(name, n)| tr!("The longest, {name}, holds {count} frames", name = named(name), count = figure(*n)));
        out.push(Scene {
            eyebrow: tr("Through the gate").into(),
            count: Some(f.rolls),
            caption: trn("roll", "rolls", f.rolls as u64),
            note: longest.unwrap_or_default(),
            photos: f.last.iter().cloned().collect(),
            ..Default::default()
        });
    }
    if let Some((body, count)) = f.bodies.first() {
        out.push(Scene {
            kind: Kind::Lines,
            eyebrow: tr("Your faithful companion").into(),
            figure: body.clone(),
            caption: counted(trn("{count} frame together", "{count} frames together", *count as u64), *count),
            lines: ranked_lines(&f.lenses[..f.lenses.len().min(3)], str::to_string),
            ..Default::default()
        });
    }
    if !f.gold.is_empty() {
        let others = match (f.silver, f.bronze) {
            (0, 0) => String::new(),
            (s, 0) => tr!("and {silver} silver", silver = figure(s)),
            (0, b) => tr!("and {bronze} bronze", bronze = figure(b)),
            (s, b) => tr!("and {silver} silver, {bronze} bronze", silver = figure(s), bronze = figure(b)),
        };
        out.push(Scene {
            kind: Kind::Photos,
            eyebrow: tr("The only thing I see is gold").into(),
            count: Some(f.gold.len()),
            caption: trn("photo earned its gold", "photos earned their gold", f.gold.len() as u64),
            note: others,
            photos: f.gold.iter().take(GOLD_SHOWN).cloned().collect(),
            ..Default::default()
        });
    }
    if let Some((path, edits)) = &d.most_edited {
        out.push(Scene {
            eyebrow: tr("Your most edited photo").into(),
            count: Some(*edits),
            caption: trn("gesture on a single photo", "gestures on a single photo", *edits as u64),
            note: tr("Overcooked?").into(),
            photos: vec![path.clone()],
            ..Default::default()
        });
    }
    let hands = named_rows(&d.favourite_rows);
    if let Some((_, top)) = hands.first() {
        let top = (*top).max(1) as f32;
        out.push(Scene {
            kind: Kind::Lines,
            eyebrow: tr("Your hands knew the way").into(),
            count: Some(d.gestures),
            caption: trn("gesture this year", "gestures this year", d.gestures as u64),
            lines: hands.iter().take(3).map(|(name, n)| Line { label: name.clone(), value: figure(*n), share: *n as f32 / top }).collect(),
            ..Default::default()
        });
    }
    if f.dust > 0 {
        let most = f.dustiest.as_ref().map_or(0, |(_, n)| *n);
        out.push(Scene {
            eyebrow: tr("Frames rescued").into(),
            count: Some(f.dust),
            caption: trn("speck of dust healed", "specks of dust healed", f.dust as u64),
            note: tr!("on {frames}; the dustiest took {most}", frames = counted(trn("{count} frame", "{count} frames", f.dusted as u64), f.dusted), most = figure(most)),
            photos: f.dustiest.iter().map(|(p, _)| p.clone()).collect(),
            ..Default::default()
        });
    }
    let look = look_lines(year);
    if !look.is_empty() {
        out.push(Scene {
            kind: Kind::Lines,
            eyebrow: tr("Your signature look").into(),
            figure: d.favourite_preset.as_ref().map(|(name, _)| name.clone()).or_else(|| f.papers.first().map(|(key, _)| paper_name(key))).unwrap_or_default(),
            lines: look,
            ..Default::default()
        });
    }
    if year.palette.sampled >= 6 {
        out.push(Scene {
            kind: Kind::Palette,
            eyebrow: tr("The colours of your year").into(),
            figure: palette_words(year).into(),
            caption: counted(trn("read off {count} of your photos", "read off {count} of your photos", year.palette.sampled as u64), year.palette.sampled),
            swatches: year.palette.swatches.iter().map(|s| (s.rgb, s.share)).collect(),
            ..Default::default()
        });
    }
    if d.hours >= 1.0 {
        let hours = d.hours.round() as usize;
        let mut notes = vec![counted(trn("{count} day at the desk", "{count} days at the desk", d.days as u64), d.days)];
        if d.streak > 1 {
            notes.push(tr!("{count} in a row", count = figure(d.streak)));
        }
        if let Some(hour) = d.favourite_hour() {
            notes.push(tr!("mostly around {hour}:00", hour = hour));
        }
        out.push(Scene {
            kind: Kind::Bars,
            eyebrow: tr("Time in the darkroom").into(),
            count: Some(hours),
            unit: tr("{count} h").replace("{count}", ""),
            caption: tr("spent with your negatives").into(),
            note: notes.join("  \u{b7}  "),
            bars: normalised(&d.by_hour),
            lit: d.favourite_hour().map(|h| h as usize),
            marks: ["0", "6", "12", "18", "24"].map(String::from).to_vec(),
            ..Default::default()
        });
    }
    if d.exported > 0 {
        out.push(Scene {
            eyebrow: tr("Out into the world").into(),
            count: Some(d.exported),
            caption: trn("export", "exports", d.exported as u64),
            photos: best(year, 1),
            ..Default::default()
        });
    }
    let (name, line, reason) = persona_words(year);
    out.push(Scene {
        kind: Kind::Persona,
        eyebrow: tr("Your film personality").into(),
        figure: name.clone(),
        caption: line,
        note: reason,
        lines: year.persona.also.iter().map(|t| Line { label: trait_name(*t).into(), ..Default::default() }).collect(),
        ..Default::default()
    });
    out.push(summary(year, name));
    out.push(farewell());
    out
}

fn look_lines(year: &Year) -> Vec<Line> {
    let (f, d) = (&year.frames, &year.darkroom);
    let line = |label: &str, value: String| Line { label: label.into(), value, share: 0.0 };
    let photos = |n: usize| counted(trn("{count} photo", "{count} photos", n as u64), n);
    let mut out = Vec::new();
    if let Some((name, count)) = &d.favourite_preset {
        out.push(line(tr("Preset"), tr!("{name}, laid {count} times", name = named(name), count = figure(*count))));
    }
    if let Some((key, count)) = f.papers.first() {
        out.push(line(tr("Paper"), paper_name(key)));
        out.push(line(tr("Printed on paper"), photos(*count)));
    }
    for (label, count) in [(tr("Grain"), f.grained), (tr("Halation"), f.haloed), (tr("Cropped"), f.cropped), (tr("Straightened"), f.straightened)] {
        if count > 0 {
            out.push(line(label, photos(count)));
        }
    }
    if d.balanced > 0 {
        out.push(line(tr("Balance"), tr!("pressed {count} times", count = figure(d.balanced))));
    }
    if d.pasted > 0 {
        out.push(line(tr("Settings pasted"), photos(d.pasted)));
    }
    out
}

fn summary(year: &Year, persona: String) -> Scene {
    let (f, d) = (&year.frames, &year.darkroom);
    let line = |label: &str, value: String| Line { label: label.into(), value, share: 0.0 };
    let mut lines = Vec::new();
    if f.count > 0 {
        lines.push(line(tr("Frames"), figure(f.count)));
    }
    if f.rolls > 0 {
        lines.push(line(tr("Rolls"), figure(f.rolls)));
    }
    if let Some((key, _)) = f.stocks.first() {
        lines.push(line(tr("Film"), film_name(key)));
    }
    if let Some((body, _)) = f.bodies.first() {
        lines.push(line(tr("Camera"), body.clone()));
    }
    if d.hours >= 1.0 {
        lines.push(line(tr("In the darkroom"), tr!("{count} h", count = figure(d.hours.round() as usize))));
    }
    if f.dust > 0 {
        lines.push(line(tr("Dust healed"), figure(f.dust)));
    }
    lines.truncate(SUMMARY_LINES);
    Scene {
        kind: Kind::Summary,
        eyebrow: tr!("My {year} on film", year = year.year),
        figure: persona,
        lines,
        photos: best(year, 4),
        swatches: year.palette.swatches.iter().map(|s| (s.rgb, s.share)).collect(),
        ..Default::default()
    }
}

fn farewell() -> Scene {
    Scene { kind: Kind::Farewell, eyebrow: tr("See you next year").into(), caption: tr("on film.").into(), ..Default::default() }
}

#[cfg(test)]
mod tests;
