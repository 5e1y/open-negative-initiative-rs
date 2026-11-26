//! The year's words: its counts as the language writes them, the names the screen gives a film, a paper,
//! a row and a day, and each character's name, line and reason.

use oni_i18n::{Language, marked, tr, trn};
use oni_library::wrapped::{Trait, Year};

pub const MONTHS: [&str; 12] = [
    marked("January"),
    marked("February"),
    marked("March"),
    marked("April"),
    marked("May"),
    marked("June"),
    marked("July"),
    marked("August"),
    marked("September"),
    marked("October"),
    marked("November"),
    marked("December"),
];

/// A count with its thousands grouped as the language writes them.
pub fn figure(value: usize) -> String {
    let separator = match oni_i18n::current() {
        Language::English => ",",
        Language::French => "\u{202f}",
        Language::Spanish => ".",
    };
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(c);
    }
    out
}

/// A plural's words with their count grouped at `{count}`.
pub(super) fn counted(words: String, n: usize) -> String {
    words.replace("{count}", &figure(n))
}

pub(super) fn percent(share: f64) -> String {
    format!("{}", (share * 100.0).round() as i64)
}

/// A row's rank in `Parameters.all`, named as the preset editor's box that carries it; the two rows
/// of the white balance are named as it.
pub fn row_name(rank: usize) -> String {
    let Some(row) = oni_core::parameters::all().get(rank) else { return tr("Settings").into() };
    if oni_core::parameters::WHITE_BALANCE_ROWS.contains(&row.id) {
        return tr("White balance").into();
    }
    oni_core::preset_parameters::entries().find(|e| e.rows.contains(&row.id)).map_or_else(|| row.id.to_string(), |e| tr(e.label).to_string())
}

/// A film as the screen names it: its catalogue's name, or its key when no file defines it.
pub fn film_name(key: &str) -> String {
    oni_settings::film::film_stock(Some(key)).map_or_else(|| key.to_string(), |k| crate::stocks::shown_name(&k).to_string())
}

pub(super) fn paper_name(key: &str) -> String {
    oni_settings::PaperStock::all().into_iter().find(|p| p.key() == key).map_or_else(|| key.to_string(), |p| p.name().to_string())
}

pub(super) fn day_name(month: u32, day: u32) -> String {
    tr!("{month} {day}", month = tr(MONTHS[(month.clamp(1, 12) - 1) as usize]), day = day)
}

/// A persona's name, the line that draws it, and the measure behind it.
pub fn persona_words(year: &Year) -> (String, String, String) {
    let measure = year.persona.measure;
    let share = percent(measure);
    let count = figure(measure.round().max(0.0) as usize);
    let (name, line, reason) = match year.persona.kind {
        Trait::NightDeveloper => (tr("The Night Developer"), tr("You work when the house sleeps."), tr!("{share} % of your gestures between 10 pm and 5 am", share = share)),
        Trait::SilverSoul => (tr("The Silver Soul"), tr("Colour is a detail. Light is the subject."), tr!("{share} % of your year in black and white", share = share)),
        Trait::Projectionist => (tr("The Projectionist"), tr("A slide, a lamp, a white wall: nothing to invert, nowhere to hide."), tr!("{share} % of your year on slides", share = share)),
        Trait::TimeTraveller => (tr("The Time Traveller"), tr("You shoot today and develop yesterday."), tr!("{share} % of your year shot digitally, developed on film", share = share)),
        Trait::Loyalist => {
            let film = year.frames.stocks.first().map(|(key, _)| film_name(key)).unwrap_or_default();
            (tr("The Loyalist"), tr("One film, and you know it by heart."), tr!("{share} % of your frames on {film}", share = share, film = film))
        }
        Trait::Explorer => (tr("The Explorer"), tr("Every roll is a new country."), tr!("{count} films tried this year", count = count)),
        Trait::DustHunter => (tr("The Dust Hunter"), tr("No speck escapes you."), tr!("{count} specks healed, one by one", count = count)),
        Trait::Curator => (tr("The Curator"), tr("You look twice, and choose once."), tr!("{count} medals given", count = count)),
        Trait::Tinkerer => (tr("The Printer"), tr("A photo is never quite finished: one more test strip."), tr!("{count} gestures a photo, on average", count = count)),
        Trait::Purist => (tr("The Purist"), tr("The negative already said it all."), tr!("{count} gestures a photo, on average", count = count)),
        Trait::Marathoner => (tr("The Regular"), tr("The darkroom light is always on."), tr!("{count} days in a row at the desk", count = count)),
        Trait::FilmPhotographer => (tr("The Film Photographer"), tr("No label fits: you shoot, you scan, you keep going."), String::new()),
    };
    (name.to_string(), line.to_string(), reason)
}

pub(super) fn trait_name(kind: Trait) -> &'static str {
    match kind {
        Trait::NightDeveloper => tr("The Night Developer"),
        Trait::SilverSoul => tr("The Silver Soul"),
        Trait::Projectionist => tr("The Projectionist"),
        Trait::TimeTraveller => tr("The Time Traveller"),
        Trait::Loyalist => tr("The Loyalist"),
        Trait::Explorer => tr("The Explorer"),
        Trait::DustHunter => tr("The Dust Hunter"),
        Trait::Curator => tr("The Curator"),
        Trait::Tinkerer => tr("The Printer"),
        Trait::Purist => tr("The Purist"),
        Trait::Marathoner => tr("The Regular"),
        Trait::FilmPhotographer => tr("The Film Photographer"),
    }
}

/// The kinds of frames a year holds, those it holds none of left out.
pub(super) fn kinds(year: &Year) -> String {
    let f = &year.frames;
    let parts = [
        (f.negatives, trn("{count} colour negative", "{count} colour negatives", f.negatives as u64)),
        (f.monochromes, trn("{count} in black and white", "{count} in black and white", f.monochromes as u64)),
        (f.slides, trn("{count} slide", "{count} slides", f.slides as u64)),
        (f.digitals, trn("{count} digital photo on film", "{count} digital photos on film", f.digitals as u64)),
    ];
    parts.into_iter().filter(|(n, _)| *n > 0).map(|(n, words)| counted(words, n)).collect::<Vec<_>>().join("  \u{b7}  ")
}

pub(super) fn palette_words(year: &Year) -> &'static str {
    let palette = &year.palette;
    if palette.is_monochrome() {
        tr("A year in black and white")
    } else if palette.warmth > 0.03 {
        tr("A warm year")
    } else if palette.warmth < 0.0 {
        tr("A cool year")
    } else {
        tr("A year in balance")
    }
}

/// The rows moved, by the name the screen gives them: rows sharing a name counted as one, the most moved first.
pub(super) fn named_rows(rows: &[(usize, usize)]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for (row, n) in rows {
        let name = row_name(*row);
        match out.iter_mut().find(|(known, _)| *known == name) {
            Some(known) => known.1 += n,
            None => out.push((name, *n)),
        }
    }
    out.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    out
}
