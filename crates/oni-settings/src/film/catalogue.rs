//! The closed list of film stocks a roll can name, each by a stable key: its brand, name, speed, type and canister.

use crate::modes::ConversionMode;

/// What a stock is: which pipeline reads it and which generic canister stands in for its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// One entry of the closed stock list, keyed by a stable `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilmStockKind {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon_name: &'static str,
    pub brand: &'static str,
    /// The speed the roll is exposed at, a push's included; none for a line or a generic stock.
    pub iso: Option<u16>,
    pub kind: FilmType,
    /// The picker lists it; a stock it no longer lists still reads, names and draws on a roll that has it.
    pub offered: bool,
    /// A whole line, the stock within it never named: the picker lists the line's stocks in its place.
    pub line: bool,
}

const fn stock(id: &'static str, display_name: &'static str, brand: &'static str, iso: Option<u16>, kind: FilmType) -> FilmStockKind {
    FilmStockKind { id, display_name, icon_name: id, brand, iso, kind, offered: true, line: false }
}

/// A stock of a line, drawn with the line's canister.
const fn of_line(id: &'static str, display_name: &'static str, brand: &'static str, iso: u16, kind: FilmType, line: &'static str) -> FilmStockKind {
    FilmStockKind { icon_name: line, ..stock(id, display_name, brand, Some(iso), kind) }
}

/// A stock the picker no longer lists (owner's call), kept so a roll already on it reads as it did.
const fn withdrawn(id: &'static str, display_name: &'static str, brand: &'static str, iso: Option<u16>, kind: FilmType) -> FilmStockKind {
    FilmStockKind { offered: false, ..stock(id, display_name, brand, iso, kind) }
}

/// A line split into its stocks: a roll set on the line keeps its name, canister and dyes, its stock unstated.
const fn line(id: &'static str, display_name: &'static str, brand: &'static str, kind: FilmType) -> FilmStockKind {
    FilmStockKind { line: true, ..withdrawn(id, display_name, brand, None, kind) }
}

/// A stock whose own canister is still to be drawn: its type's generic canister stands in until `id`.svg
/// arrives in the stocks' folder and this line reads `stock`.
const fn undrawn(id: &'static str, display_name: &'static str, brand: &'static str, iso: u16, kind: FilmType) -> FilmStockKind {
    FilmStockKind { icon_name: kind.generic(), ..stock(id, display_name, brand, Some(iso), kind) }
}

const CN: FilmType = FilmType::ColourNegative;
const CINE: FilmType = FilmType::CineNegative;
const SLIDE: FilmType = FilmType::ColourSlide;
const BW: FilmType = FilmType::BlackAndWhite;

pub const FILM_STOCKS: [FilmStockKind; 104] = [
    stock("generic-azure", "Generic Azure", "Generic", None, CN),
    stock("generic-black-and-white", "Generic Black & White", "Generic", None, BW),
    stock("generic-color-negative", "Generic Colour Negative", "Generic", None, CN),
    stock("generic-color-positive", "Generic Colour Positive", "Generic", None, SLIDE),
    stock("generic-redscale", "Generic Redscale", "Generic", None, CN),
    stock("agfaphoto-apx", "AgfaPhoto APX", "AgfaPhoto", None, BW),
    stock("cinestill-400d", "CineStill 400D", "CineStill", Some(400), CN),
    stock("cinestill-50d", "CineStill 50D", "CineStill", Some(50), CN),
    stock("cinestill-800t", "CineStill 800T", "CineStill", Some(800), CN),
    undrawn("foma-fomapan100", "Foma Fomapan 100", "Foma", 100, BW),
    stock("foma-fomapan200", "Foma Fomapan 200", "Foma", Some(200), BW),
    stock("foma-fomapan400", "Foma Fomapan 400", "Foma", Some(400), BW),
    withdrawn("foma-fomapan401", "Foma Fomapan 401", "Foma", Some(400), BW),
    undrawn("foma-fomapan-cine100", "Foma Fomapan Cine 100", "Foma", 100, BW),
    undrawn("foma-fomapan-cine400", "Foma Fomapan Cine 400", "Foma", 400, BW),
    undrawn("foma-ortho400", "Foma Ortho 400", "Foma", 400, BW),
    undrawn("foma-cine-ortho400", "Foma Cine Ortho 400", "Foma", 400, BW),
    undrawn("foma-retropan320", "Foma Retropan 320", "Foma", 320, BW),
    stock("fujifilm-acros", "Fujifilm Acros", "Fujifilm", Some(100), BW),
    undrawn("fujifilm-neopan400", "Fujifilm Neopan 400", "Fujifilm", 400, BW),
    undrawn("fujifilm-neopan1600", "Fujifilm Neopan 1600", "Fujifilm", 1600, BW),
    undrawn("fujifilm-neopan-ortho", "Fujifilm Neopan Ortho", "Fujifilm", 20, BW),
    of_line("fujifilm-c200", "Fujifilm C200", "Fujifilm", 200, CN, "fujifilm-fujicolor"),
    line("fujifilm-fujicolor", "Fujifilm Fujicolor", "Fujifilm", CN),
    line("fujifilm-fujicolor-pro", "Fujifilm Fujicolor Pro", "Fujifilm", CN),
    of_line("fujifilm-pro400h", "Fujifilm Pro 400H", "Fujifilm", 400, CN, "fujifilm-fujicolor-pro"),
    stock("fujifilm-provia100", "Fujifilm Provia 100", "Fujifilm", Some(100), SLIDE),
    line("fujifilm-superia", "Fujifilm Superia", "Fujifilm", CN),
    of_line("fujifilm-superia-xtra400", "Fujifilm Superia X-TRA 400", "Fujifilm", 400, CN, "fujifilm-superia"),
    stock("fujifilm-velvia100", "Fujifilm Velvia 100", "Fujifilm", Some(100), SLIDE),
    stock("fujifilm-velvia50", "Fujifilm Velvia 50", "Fujifilm", Some(50), SLIDE),
    of_line("fujifilm-160nc", "Fujifilm 160NC", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-160nl", "Fujifilm 160NL", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-160npc", "Fujifilm 160NPC", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-160ns", "Fujifilm 160NS", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-200", "Fujifilm 200", "Fujifilm", 200, CN, "fujifilm-fujicolor"),
    of_line("fujifilm-400", "Fujifilm 400", "Fujifilm", 400, CN, "fujifilm-fujicolor"),
    of_line("fujifilm-fujicolor100", "Fujifilm Fujicolor 100", "Fujifilm", 100, CN, "fujifilm-fujicolor"),
    undrawn("fujifilm-natura1600", "Fujifilm Natura 1600", "Fujifilm", 1600, CN),
    of_line("fujifilm-new-pro400", "Fujifilm New Pro 400", "Fujifilm", 400, CN, "fujifilm-fujicolor-pro"),
    undrawn("fujifilm-nexia400", "Fujifilm Nexia 400", "Fujifilm", 400, CN),
    undrawn("fujifilm-nexia-a200", "Fujifilm Nexia A200", "Fujifilm", 200, CN),
    undrawn("fujifilm-nexia-zoom-master800", "Fujifilm Nexia Zoom Master 800", "Fujifilm", 800, CN),
    of_line("fujifilm-pro160c", "Fujifilm Pro 160C", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro160nc", "Fujifilm Pro 160NC", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro160nh", "Fujifilm Pro 160NH", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro160ns", "Fujifilm Pro 160NS", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro160s", "Fujifilm Pro 160S", "Fujifilm", 160, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro800", "Fujifilm Pro 800", "Fujifilm", 800, CN, "fujifilm-fujicolor-pro"),
    of_line("fujifilm-pro800z", "Fujifilm Pro 800Z", "Fujifilm", 800, CN, "fujifilm-fujicolor-pro"),
    undrawn("fujifilm-reala-ace", "Fujifilm Reala Ace", "Fujifilm", 100, CN),
    of_line("fujifilm-superia100", "Fujifilm Superia 100", "Fujifilm", 100, CN, "fujifilm-superia"),
    of_line("fujifilm-superia200", "Fujifilm Superia 200", "Fujifilm", 200, CN, "fujifilm-superia"),
    of_line("fujifilm-superia1600", "Fujifilm Superia 1600", "Fujifilm", 1600, CN, "fujifilm-superia"),
    of_line("fujifilm-superia-premium400", "Fujifilm Superia Premium 400", "Fujifilm", 400, CN, "fujifilm-superia"),
    of_line("fujifilm-superia-reala", "Fujifilm Superia Reala", "Fujifilm", 100, CN, "fujifilm-superia"),
    of_line("fujifilm-superia-venus400", "Fujifilm Superia Venus 400", "Fujifilm", 400, CN, "fujifilm-superia"),
    of_line("fujifilm-superia-venus800", "Fujifilm Superia Venus 800", "Fujifilm", 800, CN, "fujifilm-superia"),
    of_line("fujifilm-superia-xtra800", "Fujifilm Superia X-TRA 800", "Fujifilm", 800, CN, "fujifilm-superia"),
    stock("harman-phoenix", "Harman Phoenix", "Harman", Some(200), CN),
    stock("harman-phoenixII", "Harman Phoenix II", "Harman", Some(200), CN),
    stock("harman-red", "Harman Red", "Harman", Some(125), CN),
    stock("ilford-delta100", "Ilford Delta 100", "Ilford", Some(100), BW),
    stock("ilford-delta3200", "Ilford Delta 3200", "Ilford", Some(3200), BW),
    stock("ilford-delta400", "Ilford Delta 400", "Ilford", Some(400), BW),
    stock("ilford-fp4", "Ilford FP4", "Ilford", Some(125), BW),
    stock("ilford-hp5", "Ilford HP5", "Ilford", Some(400), BW),
    stock("ilford-ortho", "Ilford Ortho", "Ilford", Some(80), BW),
    stock("ilford-panf", "Ilford Pan F", "Ilford", Some(50), BW),
    stock("ilford-sfx", "Ilford SFX", "Ilford", Some(200), BW),
    stock("ilford-xp2", "Ilford XP2", "Ilford", Some(400), BW),
    stock("kodak-400tx", "Kodak Tri-X 400", "Kodak", Some(400), BW),
    undrawn("kodak-320txp", "Kodak Tri-X 320", "Kodak", 320, BW),
    undrawn("kodak-tmax100", "Kodak T-Max 100", "Kodak", 100, BW),
    undrawn("kodak-tmax400", "Kodak T-Max 400", "Kodak", 400, BW),
    undrawn("kodak-tmax-p3200", "Kodak T-Max P3200", "Kodak", 3200, BW),
    undrawn("kodak-double-x", "Kodak Double-X", "Kodak", 250, BW),
    stock("kodak-colorplus", "Kodak ColorPlus", "Kodak", Some(200), CN),
    stock("kodak-ektachrome", "Kodak Ektachrome", "Kodak", Some(100), SLIDE),
    undrawn("kodak-ektar100", "Kodak Ektar 100", "Kodak", 100, CN),
    line("kodak-gold", "Kodak Gold", "Kodak", CN),
    of_line("kodak-gold200", "Kodak Gold 200", "Kodak", 200, CN, "kodak-gold"),
    undrawn("kodak-kodachrome64", "Kodak Kodachrome 64", "Kodak", 64, SLIDE),
    line("kodak-portra", "Kodak Portra", "Kodak", CN),
    of_line("kodak-portra160", "Kodak Portra 160", "Kodak", 160, CN, "kodak-portra"),
    of_line("kodak-portra400", "Kodak Portra 400", "Kodak", 400, CN, "kodak-portra"),
    of_line("kodak-portra800", "Kodak Portra 800", "Kodak", 800, CN, "kodak-portra"),
    of_line("kodak-portra800-push1", "Kodak Portra 800 (Push 1)", "Kodak", 1600, CN, "kodak-portra"),
    of_line("kodak-portra800-push2", "Kodak Portra 800 (Push 2)", "Kodak", 3200, CN, "kodak-portra"),
    of_line("kodak-portra160nc", "Kodak Portra 160NC", "Kodak", 160, CN, "kodak-portra"),
    of_line("kodak-portra160vc", "Kodak Portra 160VC", "Kodak", 160, CN, "kodak-portra"),
    of_line("kodak-portra400nc", "Kodak Portra 400NC", "Kodak", 400, CN, "kodak-portra"),
    of_line("kodak-portra400vc", "Kodak Portra 400VC", "Kodak", 400, CN, "kodak-portra"),
    line("kodak-ultramax", "Kodak UltraMax", "Kodak", CN),
    of_line("kodak-ultramax400", "Kodak UltraMax 400", "Kodak", 400, CN, "kodak-ultramax"),
    undrawn("kodak-verita200d", "Kodak Verita 200D", "Kodak", 200, CINE),
    undrawn("kodak-vision3-50d", "Kodak Vision3 50D", "Kodak", 50, CINE),
    undrawn("kodak-vision3-200t", "Kodak Vision3 200T", "Kodak", 200, CINE),
    undrawn("kodak-vision3-250d", "Kodak Vision3 250D", "Kodak", 250, CINE),
    undrawn("kodak-vision3-500t", "Kodak Vision3 500T", "Kodak", 500, CINE),
    stock("lomography-lomo-turquoise", "Lomography LomoChrome Turquoise", "Lomography", None, CN),
    stock("lucky-color200", "Lucky Color 200", "Lucky", Some(200), CN),
    stock("mira-color400", "Mira Color 400", "Mira", Some(400), CN),
    stock("mira-color800", "Mira Color 800", "Mira", Some(800), CN),
];

/// A stored stock key as this catalogue names it: a data-sheet twin's, which a build before format 60 may have
/// written, is its stock's.
pub fn stored_key<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let key = <Option<String> as serde::Deserialize>::deserialize(d)?;
    Ok(key.map(|id| match id.strip_suffix("-datasheet") {
        Some(stock) if film_stock(Some(stock)).is_some() => stock.to_string(),
        _ => id,
    }))
}

pub fn film_stock(id: Option<&str>) -> Option<&'static FilmStockKind> {
    let id = id?;
    FILM_STOCKS.iter().find(|k| k.id == id)
}

/// The stocks the picker lists, each with its place in `FILM_STOCKS`, which a pick names.
pub fn offered_stocks() -> impl Iterator<Item = (usize, &'static FilmStockKind)> {
    FILM_STOCKS.iter().enumerate().filter(|(_, k)| k.offered)
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
        let mut ids: Vec<_> = FILM_STOCKS.iter().map(|k| k.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), FILM_STOCKS.len());
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

    /// A stock draws its own canister, its line's, or its type's generic one; a generic stock and the canister
    /// standing in for it share its type; a precise stock states its speed.
    #[test]
    fn a_stock_draws_its_own_its_lines_or_its_types_canister_and_states_its_speed() {
        for kind in FILM_STOCKS {
            let drawn = film_stock(Some(kind.icon_name)).expect("its canister is a catalogue stock's");
            let own_line = drawn.line && drawn.brand == kind.brand && drawn.kind == kind.kind;
            assert!(drawn.id == kind.id || own_line || drawn.id == kind.kind.generic(), "{}", kind.id);
            if !kind.line && !kind.id.starts_with("generic") {
                assert!(kind.iso.is_some() || ["agfaphoto-apx", "lomography-lomo-turquoise"].contains(&kind.id), "{} states its speed", kind.id);
            }
        }
        for t in [FilmType::ColourNegative, FilmType::ColourSlide, FilmType::BlackAndWhite] {
            assert_eq!(film_stock(Some(t.generic())).map(|k| k.kind), Some(t));
        }
        assert_eq!(film_stock(Some("kodak-kodachrome64")).map(|k| k.icon_name), Some("generic-color-positive"));
        assert_eq!(film_stock(Some("kodak-portra800-push1")).map(|k| (k.icon_name, k.iso)), Some(("kodak-portra", Some(1600))));
        // Twin: a cine negative stands on the colour negative's canister, never a slide's.
        assert_ne!(FilmType::CineNegative.generic(), FilmType::ColourSlide.generic());
        assert_eq!(film_stock(Some("kodak-vision3-500t")).map(|k| k.icon_name), Some("generic-color-negative"));
    }
}
