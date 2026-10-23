//! `WrappedView.cards`: what the year's summary says, card by card, in Swift's wording.

use oni_i18n::{tr, trn};
use oni_library::wrapped::WrappedYear;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub eyebrow: String,
    pub figure: Option<String>,
    pub caption: String,
    pub photos: Vec<String>,
}

/// `NumberFormatter.decimal`: thousands grouped by commas.
pub fn figure(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A row's rank in `Parameters.all`, named as the preset editor's box that carries it.
pub fn row_name(rank: usize) -> String {
    let Some(row) = oni_core::parameters::all().get(rank) else { return tr("Settings").into() };
    oni_core::preset_parameters::entries().find(|e| e.rows.contains(&row.id)).map_or_else(|| row.id.to_string(), |e| tr(e.label).to_string())
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

pub fn cards(year: &WrappedYear) -> Vec<Card> {
    let mut out = Vec::new();
    let mut add = |eyebrow: &str, figure: Option<String>, caption: String, photos: Vec<String>| {
        out.push(Card { eyebrow: eyebrow.into(), figure, caption, photos });
    };
    if year.photos > 0 {
        let caption = trn("negative brought back to life", "negatives brought back to life", year.photos as u64);
        add(tr("This year"), Some(figure(year.photos)), caption, Vec::new());
    }
    if let Some((path, edits)) = &year.most_edited {
        add(
            tr("Your most edited photo"),
            Some(tr!("{count} gestures", count = figure(*edits))),
            tr!("{name} — overcooked?", name = file_name(path)),
            vec![path.clone()],
        );
    }
    if !year.favourite_rows.is_empty() {
        let names: Vec<String> = year.favourite_rows.iter().map(|(row, _)| row_name(*row)).collect();
        add(tr("Your hands knew the way"), None, names.join("  \u{b7}  "), Vec::new());
    }
    if !year.gold.is_empty() {
        let caption = trn("photo earned its gold", "photos earned their gold", year.gold.len() as u64);
        add(tr("The only thing I see is gold"), Some(figure(year.gold.len())), caption, year.gold.iter().take(9).cloned().collect());
    }
    if let Some((_, count)) = year.favourite_balance {
        add(tr("Your go-to balance"), Some(tr("Balance").into()), tr!("pressed {count} times", count = figure(count)), Vec::new());
    }
    if let Some((name, count)) = &year.favourite_preset {
        add(tr("Your signature look"), Some(name.clone()), tr!("laid on {count} photos", count = figure(*count)), Vec::new());
    }
    if year.exported > 0 || year.rolls > 0 {
        let exports = trn("export", "exports", year.exported as u64);
        let rolls = trn("roll made", "rolls made", year.rolls as u64);
        let caption = tr!("{exports}, and {count} {rolls}", exports = exports, count = figure(year.rolls), rolls = rolls);
        add(tr("Out into the world"), Some(figure(year.exported)), caption, Vec::new());
    }
    if year.hours >= 1.0 {
        add(tr("Time in the darkroom"), Some(format!("{} h", figure(year.hours.round() as usize))), tr("spent with your negatives").into(), Vec::new());
    }
    add(tr("See you next year"), None, tr("on film.").into(), Vec::new());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_year_deals_every_card_in_swifts_words() {
        let photos: Vec<String> = (0..12).map(|i| format!("/roll/F{i:02}.NEF")).collect();
        let deck = cards(&WrappedYear::sample(2026, &photos));
        let eyebrows: Vec<&str> = deck.iter().map(|c| c.eyebrow.as_str()).collect();
        assert_eq!(
            eyebrows,
            [
                "This year",
                "Your most edited photo",
                "Your hands knew the way",
                "The only thing I see is gold",
                "Your go-to balance",
                "Your signature look",
                "Out into the world",
                "Time in the darkroom",
                "See you next year"
            ]
        );
        assert_eq!(deck[0].figure.as_deref(), Some("5,000"));
        assert_eq!(deck[1].caption, "F00.NEF \u{2014} overcooked?");
        assert_eq!(deck[1].figure.as_deref(), Some("412 gestures"));
        assert_eq!(deck[3].photos.len(), 9);
        assert_eq!(deck[6].caption, "exports, and 43 rolls made");
        assert_eq!(deck[7].figure.as_deref(), Some("162 h"));
        assert_eq!(deck[2].caption, format!("{}  \u{b7}  {}  \u{b7}  {}", row_name(0), row_name(1), row_name(2)));
    }

    #[test]
    fn an_empty_year_says_only_goodbye_and_singulars_stay_singular() {
        let empty = cards(&WrappedYear { year: 2026, ..Default::default() });
        assert_eq!(empty.len(), 1);
        assert_eq!((empty[0].eyebrow.as_str(), empty[0].caption.as_str()), ("See you next year", "on film."));
        let one = cards(&WrappedYear { year: 2026, photos: 1, exported: 1, rolls: 1, gold: vec!["/a".into()], hours: 0.9, ..Default::default() });
        assert_eq!(one[0].caption, "negative brought back to life");
        assert_eq!(one[1].caption, "photo earned its gold");
        assert_eq!(one[2].caption, "export, and 1 roll made");
        // Twin: under an hour the darkroom card is not dealt; at an hour it is.
        assert!(!one.iter().any(|c| c.eyebrow == "Time in the darkroom"));
        let hour = cards(&WrappedYear { year: 2026, hours: 1.0, ..Default::default() });
        assert!(hour.iter().any(|c| c.eyebrow == "Time in the darkroom"));
    }

    #[test]
    fn figures_group_their_thousands_and_rows_are_named_by_their_box() {
        assert_eq!([figure(0), figure(999), figure(1_284), figure(1_234_567)], ["0", "999", "1,284", "1,234,567"]);
        assert_eq!(row_name(usize::MAX), "Settings");
        let lighting = oni_core::parameters::all().iter().position(|r| r.id == "levelsGlobal").unwrap();
        assert_eq!(row_name(lighting), "Lighting");
        // Twin: the row's own id is not what is shown.
        assert_ne!(row_name(lighting), "levelsGlobal");
    }
}
