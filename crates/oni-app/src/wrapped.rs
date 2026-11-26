//! The year on film told scene by scene: what each frame of the strip says, read off a `Year`. A scene
//! with nothing to tell is not laid, so a short year is a short story and an empty one an invitation.

use oni_i18n::{named, tr, trn};
use oni_library::wrapped::Year;
use oni_library::wrapped::persona::ENOUGH;

pub mod backdrop;
pub mod cover;
pub mod fit;
pub mod gather;
pub mod sample;
pub mod share;
mod staging;
mod words;

use staging::{Pool, counted_scene, upper, worded};
pub use words::{MONTHS, figure, film_name, persona_words, row_name};
use words::{counted, day_name, kinds, named_rows, palette_words, paper_name, percent, trait_name};

/// `ONI_WRAPPED_ONLY`: the year on film alone, shown at launch over an app that opens no folder, shows no
/// launch sheet and runs no sweep, and quits as it closes: a year looked at with nothing written in the library.
pub const ALONE: &str = "ONI_WRAPPED_ONLY";

pub fn alone() -> bool {
    std::env::var_os(ALONE).is_some()
}

/// The colours a year's palette gathers its photos around.
pub const SWATCHES: usize = 5;

/// The gold medals a frame shows, three rows of two; the lines its summary holds; the habits a look's note tells.
pub const GOLD_SHOWN: usize = 6;
pub const SUMMARY_LINES: usize = 4;
pub const HABITS: usize = 3;

/// How a scene is laid on its frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// The year, giant, over its best photo.
    Title,
    /// One photo filling the frame and the scene's giant figure or word over it.
    #[default]
    Hero,
    /// One photo whole, laid as a print: a scene whose subject is that photo.
    Print,
    /// A wall of photos under the figure.
    Mosaic,
    /// The year's colours, each a slice of the photo nearest it.
    Palette,
    /// The year on one frame: a photo, the character and its lines.
    Summary,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub kind: Kind,
    /// A small line over the figure, saying what it counts.
    pub eyebrow: String,
    /// The scene's giant words, in capitals, broken as their size breaks them; when `count` is given they
    /// are its number, counted up as the scene shows.
    pub figure: String,
    pub count: Option<usize>,
    /// What follows the counted number: an hour's ` H`.
    pub unit: String,
    /// The figure's size, in the frame's units.
    pub size: f32,
    /// The bold words under the figure, in capitals.
    pub headline: String,
    /// The line with a wink, in the text face.
    pub note: String,
    /// The photos it shows, by path: at least one wherever the year holds one.
    pub photos: Vec<String>,
    pub lines: Vec<Line>,
    pub swatches: Vec<([u8; 3], f32)>,
}

pub fn scenes(year: &Year) -> Vec<Scene> {
    let (f, d) = (&year.frames, &year.darkroom);
    let mut pool = Pool::of(year);
    let mut out = vec![Scene {
        kind: Kind::Title,
        headline: upper(&format!("{}\n{}", tr("Your Year"), tr("On Film"))),
        note: tr("Read off this computer alone. Nothing was sent anywhere.").into(),
        photos: pool.deal(1),
        ..worded("", &year.year.to_string(), "", String::new(), None)
    }];
    if year.is_empty() {
        out.push(worded(tr("A fresh roll"), tr("Frame 0"), tr("Your year on film is still unexposed."), tr("Everything you convert from now on will be told here next December.").into(), None));
        return out;
    }
    if f.count > 0 {
        out.push(counted_scene(tr("This year"), f.count, "", &trn("frame brought to light", "frames brought to light", f.count as u64), kinds(year), pool.claim(f.first.as_ref())));
    } else if d.photos > 0 {
        out.push(counted_scene(tr("This year"), d.photos, "", &trn("photo opened in the darkroom", "photos opened in the darkroom", d.photos as u64), String::new(), pool.take()));
    }
    if let Some((month, count)) = f.busiest_month() {
        let day = f.busiest_day.filter(|(_, _, n)| *n > 1).map(|(m, d, n)| {
            let alone = tr!("{day} alone holds {count}", day = day_name(m, d), count = figure(n));
            if n >= SLEEPLESS { format!("{alone}. {}", tr("Did you sleep?")) } else { alone }
        });
        let frames = counted(trn("{count} frame", "{count} frames", count as u64), count);
        out.push(worded(tr("Your busiest month"), tr(MONTHS[(month - 1) as usize]), &frames, day.unwrap_or_default(), pool.claim(f.picks.month.as_ref())));
    }
    if let Some((key, count)) = f.stocks.first() {
        let share = tr!("{count} frames, {share} % of your year", count = figure(*count), share = percent(*count as f64 / f.count.max(1) as f64));
        let others: Vec<String> = f.stocks.iter().skip(1).take(2).map(|(k, _)| film_name(k)).collect();
        let note = if others.is_empty() { String::new() } else { tr!("Ahead of {others}.", others = others.join(", ")) };
        out.push(worded(tr("Your film of the year"), &film_name(key), &share, note, pool.claim(f.picks.stock.as_ref())));
    }
    if f.rolls > 0 {
        let longest = f.longest_roll.as_ref().filter(|_| f.rolls > 1).map(|(name, n)| tr!("The longest, {name}, holds {count} frames", name = named(name), count = figure(*n)));
        out.push(counted_scene(tr("Through the gate"), f.rolls, "", &trn("roll", "rolls", f.rolls as u64), longest.unwrap_or_default(), pool.claim(f.picks.roll.as_ref())));
    }
    if let Some((body, count)) = f.bodies.first().filter(|(_, n)| *n >= ENOUGH) {
        let together = counted(trn("{count} frame together", "{count} frames together", *count as u64), *count);
        let lens = f.lenses.first().map(|(lens, _)| tr!("Mostly through the {lens}.", lens = lens)).unwrap_or_default();
        out.push(worded(tr("Your partner in crime"), body, &together, lens, pool.claim(f.picks.body.as_ref())));
    }
    if !f.gold.is_empty() {
        let others = match (f.silver, f.bronze) {
            (0, 0) if f.gold.len() >= GENEROUS => tr("You're generous. We like that.").to_string(),
            (0, 0) => tr("You're picky. Good.").to_string(),
            (s, 0) => tr!("and {silver} silver", silver = figure(s)),
            (0, b) => tr!("and {bronze} bronze", bronze = figure(b)),
            (s, b) => tr!("and {silver} silver, {bronze} bronze", silver = figure(s), bronze = figure(b)),
        };
        let earned = trn("photo earned its gold", "photos earned their gold", f.gold.len() as u64);
        let photos = pool.fresh(&f.gold, GOLD_SHOWN);
        out.push(Scene { kind: Kind::Mosaic, photos, ..counted_scene(tr("The only thing I see is gold"), f.gold.len(), "", &earned, others, None) });
    }
    if let Some((path, edits)) = &d.most_edited {
        let gestures = trn("gesture on a single photo", "gestures on a single photo", *edits as u64);
        out.push(Scene { kind: Kind::Print, ..counted_scene(tr("Your most edited photo"), *edits, "", &gestures, tr("Overcooked? Slow-cooked.").into(), Some(path.clone())) });
    }
    let hands = named_rows(&d.favourite_rows);
    if let Some((name, count)) = hands.first() {
        let moved = counted(trn("moved {count} time", "moved {count} times", *count as u64), *count);
        let then = match (hands.get(1), hands.get(2)) {
            (Some((second, _)), Some((third, _))) => tr!("Then {second} and {third}.", second = second, third = third),
            (Some((second, _)), None) => tr!("Then {second}.", second = second),
            _ => String::new(),
        };
        out.push(worded(tr("Your hands knew the way"), name, &moved, then, pool.take()));
    }
    if f.dust > 0 {
        let most = f.dustiest.as_ref().map_or(0, |(_, n)| *n);
        let on = tr!("on {frames}; the dustiest took {most}", frames = counted(trn("{count} frame", "{count} frames", f.dusted as u64), f.dusted), most = figure(most));
        let note = if most >= DUSTY { format!("{on}. {}", tr("Clean that scanner.")) } else { on };
        let photo = f.dustiest.as_ref().map(|(p, _)| p);
        out.push(counted_scene(tr("Frames rescued"), f.dust, "", &trn("speck of dust healed", "specks of dust healed", f.dust as u64), note, pool.claim(photo)));
    }
    out.extend(look(year, &mut pool));
    if year.palette.sampled >= 6 && !year.colours.is_empty() {
        let read = counted(trn("read off {count} of your photos", "read off {count} of your photos", year.palette.sampled as u64), year.palette.sampled);
        out.push(Scene {
            kind: Kind::Palette,
            photos: year.colours.clone(),
            swatches: year.palette.swatches.iter().map(|s| (s.rgb, s.share)).collect(),
            ..worded(tr("The colours of your year"), palette_words(year), "", read, None)
        });
    }
    if d.hours >= 1.0 {
        let mut notes = vec![counted(trn("{count} day at the desk", "{count} days at the desk", d.days as u64), d.days)];
        if d.streak > 1 {
            notes.push(tr!("{count} in a row", count = figure(d.streak)));
        }
        if let Some(hour) = d.favourite_hour() {
            notes.push(tr!("mostly around {hour}:00", hour = hour));
        }
        let wink = if d.night_share() >= NIGHT { format!(". {}", tr("We don't judge.")) } else { String::new() };
        let unit = upper(&tr("{count} h").replace("{count}", ""));
        out.push(counted_scene(tr("Time in the darkroom"), d.hours.round() as usize, &unit, tr("spent with your negatives"), notes.join("  \u{b7}  ") + &wink, pool.take()));
    }
    if d.exported > 0 {
        out.push(counted_scene(tr("Out into the world"), d.exported, "", &trn("export", "exports", d.exported as u64), tr("Now post them.").into(), pool.take()));
    }
    let (name, line, reason) = persona_words(year);
    let touch = year.persona.also.first().map(|t| tr!("With a touch of {other}.", other = trait_name(*t)));
    let because = [reason, touch.unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(". ");
    out.push(Scene { headline: line, ..worded(tr("Your film personality"), &name, "", because, pool.take()) });
    out.push(summary(year, &name, &mut pool));
    out.push(worded("", tr("See you next year"), tr("on film, obviously."), String::new(), pool.claim(f.last.as_ref())));
    out
}

/// A day past this many frames, a gold wall past this many medals, a frame past this many specks and a
/// year this far into the night each earn their wink.
const SLEEPLESS: usize = 100;
const GENEROUS: usize = 50;
const DUSTY: usize = 50;
const NIGHT: f64 = 0.25;

/// The year's look: its preset, else its grain, else its paper, where one covers enough photos to be a
/// habit; the other habits are its note.
fn look(year: &Year, pool: &mut Pool) -> Option<Scene> {
    let (f, d) = (&year.frames, &year.darkroom);
    let photos = |n: usize| counted(trn("{count} photo", "{count} photos", n as u64), n);
    let paper = f.papers.first().filter(|(_, n)| *n >= ENOUGH);
    let (word, headline, told) = if let Some((name, n)) = &d.favourite_preset {
        (name.clone(), tr!("laid on {photos}", photos = photos(*n)), "preset")
    } else if f.grained >= ENOUGH {
        (tr("Grain").to_string(), tr!("on {photos}", photos = photos(f.grained)), "grain")
    } else if let Some((key, n)) = paper {
        (paper_name(key), tr!("printed on {photos}", photos = photos(*n)), "paper")
    } else {
        return None;
    };
    let habits = [
        (told != "grain" && f.grained > 0).then(|| tr!("grain on {photos}", photos = photos(f.grained))),
        (f.haloed > 0).then(|| tr!("halation on {photos}", photos = photos(f.haloed))),
        (f.cropped > 0).then(|| tr!("{photos} cropped", photos = photos(f.cropped))),
        (f.straightened > 0).then(|| tr!("{photos} straightened", photos = photos(f.straightened))),
        (d.balanced > 0).then(|| tr!("Balance pressed {count} times", count = figure(d.balanced))),
    ];
    let note = habits.into_iter().flatten().take(HABITS).collect::<Vec<_>>().join("  \u{b7}  ");
    Some(worded(tr("Your signature look"), &word, &headline, note, pool.take()))
}

fn summary(year: &Year, persona: &str, pool: &mut Pool) -> Scene {
    let (f, d) = (&year.frames, &year.darkroom);
    let line = |label: &str, value: String| Line { label: upper(label), value: upper(&value) };
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
    if d.hours >= 1.0 {
        lines.push(line(tr("In the darkroom"), tr!("{count} h", count = figure(d.hours.round() as usize))));
    }
    if f.dust > 0 {
        lines.push(line(tr("Dust healed"), figure(f.dust)));
    }
    lines.truncate(SUMMARY_LINES);
    Scene { kind: Kind::Summary, lines, photos: pool.deal(1), ..worded(tr("My year on film"), &year.year.to_string(), persona, String::new(), None) }
}

#[cfg(test)]
mod tests;
