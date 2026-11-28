//! The film stocks a roll can name, each by a stable key: its brand, name, speed, type and canister, read off its file.

use crate::dyes::DyeSet;
use crate::modes::ConversionMode;
use crate::virtual_film::VirtualStock;
use serde::{Deserialize, Serialize};

/// What a stock is: which pipeline reads it and which generic canister stands in for its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilmType {
    /// A still colour negative, developed in C-41 or as one.
    ColourNegative,
    /// A motion-picture colour negative, developed in ECN-2.
    CineNegative,
    /// A colour reversal film, a slide.
    ColourSlide,
    BlackAndWhite,
}

impl FilmType {
    /// The generic stock whose canister a stock of this type draws until its own is drawn.
    pub const fn generic(self) -> &'static str {
        match self {
            FilmType::ColourNegative | FilmType::CineNegative => "generic-color-negative",
            FilmType::ColourSlide => "generic-color-positive",
            FilmType::BlackAndWhite => "generic-black-and-white",
        }
    }
}

/// A stock a roll can name, as its file describes it (`crate::stocks`), keyed by a stable `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilmStockKind {
    pub id: &'static str,
    /// The file's name for it, as files and exports write it; a generic one is shown translated.
    pub display_name: &'static str,
    pub brand: &'static str,
    /// The speed the roll is exposed at, a push's included; none for a line or a generic stock.
    pub iso: Option<u16>,
    pub kind: FilmType,
    /// The picker lists it; a stock it no longer lists still reads, names and draws on a roll that has it.
    pub offered: bool,
    /// A whole line, the stock within it never named: the picker lists the line's stocks in its place.
    pub line: bool,
    /// A description rather than a trade name, translated where it is shown.
    pub generic: bool,
    /// Its canister's SVG; none draws its type's generic stock's.
    pub canister: Option<&'static str>,
    /// The measured dyes picking it writes on its roll's frames: none in black and white or on a slide.
    pub dyes: Option<DyeSet>,
    /// The virtual film a digital photo is shot on as this stock, where its data sheet is profiled.
    pub film: Option<VirtualStock>,
}

/// The stocks of the catalogue in use, in the menus' order, held whole while read.
pub struct Stocks(std::sync::Arc<crate::stocks::Catalogue>);

impl std::ops::Deref for Stocks {
    type Target = [FilmStockKind];

    fn deref(&self) -> &[FilmStockKind] {
        self.0.films()
    }
}

/// Every stock of the catalogue in use; a place in it names a stock until the catalogue changes.
pub fn stocks() -> Stocks {
    Stocks(crate::stocks::current())
}

/// A stored stock key as this catalogue names it: a data-sheet twin's, which a build before format 60 may have
/// written, is its stock's.
pub fn stored_key<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let key = <Option<String> as serde::Deserialize>::deserialize(d)?;
    Ok(key.map(|id| match id.strip_suffix("-datasheet") {
        Some(stock) if film_stock(Some(stock)).is_some() => stock.to_string(),
        _ => id,
    }))
}

pub fn film_stock(id: Option<&str>) -> Option<FilmStockKind> {
    crate::stocks::current().film(id?).copied()
}

/// The stocks the picker lists, each with its place in `stocks()`, which a pick names.
pub fn offered_stocks() -> Vec<(usize, FilmStockKind)> {
    stocks().iter().copied().enumerate().filter(|(_, k)| k.offered).collect()
}

/// A roll's generic stock while nobody picked one, read off the mode a hand already set; a digital
/// photo was shot on no film.
pub fn default_stock_id(mode: ConversionMode) -> Option<&'static str> {
    match mode {
        ConversionMode::Negative => Some("generic-color-negative"),
        ConversionMode::Positive => Some("generic-color-positive"),
        ConversionMode::Monochrome => Some("generic-black-and-white"),
        ConversionMode::Digital => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_holds_unique_keys() {
        let mut ids: Vec<_> = stocks().iter().map(|k| k.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), stocks().len());
        assert_eq!(stocks().len(), 105, "every stock of the shipped files, Portra 800's pushes and the neutral film included");
        assert_eq!(film_stock(Some("fujifilm-fujicolor")).map(|k| k.brand), Some("Fujifilm"));
        assert_eq!(film_stock(Some("fujifilm-fujicolour")), None);
    }

    #[test]
    fn a_data_sheet_twins_stored_key_reads_as_its_stocks() {
        #[derive(serde::Deserialize)]
        struct Stored {
            #[serde(default, deserialize_with = "stored_key")]
            stock: Option<String>,
        }
        let read = |json: &str| serde_json::from_str::<Stored>(json).unwrap().stock;
        assert_eq!(read(r#"{"stock":"kodak-portra400-datasheet"}"#).as_deref(), Some("kodak-portra400"));
        assert_eq!(read(r#"{"stock":"ilford-hp5"}"#).as_deref(), Some("ilford-hp5"));
        assert_eq!(read("{}"), None);
        // Twin: a key the catalogue holds under no stock stays as stored, never read as another.
        assert_eq!(read(r#"{"stock":"kodak-portra900-datasheet"}"#).as_deref(), Some("kodak-portra900-datasheet"));
    }

    /// A stock draws its own canister, its line's, or its type's generic stock's; a generic stock and the canister
    /// standing in for it share its type; a precise stock states its speed.
    #[test]
    fn a_stock_draws_its_own_its_lines_or_its_types_canister_and_states_its_speed() {
        let catalogue = crate::stocks::current();
        for kind in stocks().iter() {
            match kind.canister {
                Some(svg) => {
                    assert!(catalogue.canister(svg).is_some(), "{}: {svg} is shipped", kind.id);
                    let owner = film_stock(Some(svg.trim_end_matches(".svg"))).expect("a canister is named after its stock");
                    let own_line = owner.line && owner.brand == kind.brand && owner.kind == kind.kind;
                    assert!(owner.id == kind.id || own_line, "{}", kind.id);
                }
                None => assert!(film_stock(Some(kind.kind.generic())).and_then(|g| g.canister).is_some(), "{}", kind.id),
            }
            if !kind.line && !kind.generic {
                assert!(kind.iso.is_some() || ["agfaphoto-apx", "lomography-lomo-turquoise"].contains(&kind.id), "{} states its speed", kind.id);
            }
        }
        for t in [FilmType::ColourNegative, FilmType::ColourSlide, FilmType::BlackAndWhite] {
            assert_eq!(film_stock(Some(t.generic())).map(|k| k.kind), Some(t));
        }
        assert_eq!(film_stock(Some("generic-neutral")).map(|k| (k.canister, k.kind.generic())), Some((None, "generic-color-negative")));
        assert_eq!(film_stock(Some("kodak-portra800-push1")).map(|k| (k.canister, k.iso)), Some((Some("kodak-portra.svg"), Some(1600))));
        // Twin: a cine negative stands on the colour negative's canister, never a slide's.
        assert_ne!(FilmType::CineNegative.generic(), FilmType::ColourSlide.generic());
        assert_eq!(film_stock(Some("kodak-vision3-500t")).map(|k| (k.canister, k.kind.generic())), Some((Some("kodak-vision3-500t.svg"), "generic-color-negative")));
    }
}
