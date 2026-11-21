//! A roll, its film stock catalogue, and the marker a folder made a roll carries beside its photos.

use crate::date::{self, Date};
use crate::id::Id;
use crate::modes::ConversionMode;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// A roll: the frames a hand put together, identified by `id` so a rename never scatters them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Film {
    pub id: Id,
    pub name: String,
    /// The roll's own place among its root siblings, `None` until dragged.
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
    pub manual_order: Option<f64>,
    /// The catalogue's key, never the display name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stock: Option<String>,
    #[serde(rename = "shotDate", skip_serializing_if = "Option::is_none", default)]
    pub shot_date: Option<Date>,
    #[serde(rename = "cameraBody", skip_serializing_if = "Option::is_none", default)]
    pub camera_body: Option<String>,
    /// The lens the whole roll was shot with, as its body: hand-entered, never read off a scan.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lens: Option<String>,
}

impl Film {
    pub fn new(name: &str) -> Self {
        Film { id: Id::new(), name: name.to_owned(), manual_order: None, stock: None, shot_date: None, camera_body: None, lens: None }
    }
}

/// One entry of the closed stock list, keyed by a stable `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilmStockKind {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon_name: &'static str,
    pub brand: &'static str,
    /// The picker lists it; a stock it no longer lists still reads, names and draws on a roll that has it.
    pub offered: bool,
    /// A whole line, the stock within it never named: the picker lists the line's stocks in its place.
    pub line: bool,
}

const fn stock(id: &'static str, display_name: &'static str, brand: &'static str) -> FilmStockKind {
    FilmStockKind { id, display_name, icon_name: id, brand, offered: true, line: false }
}

/// A stock of a line, drawn with the line's canister.
const fn of_line(id: &'static str, display_name: &'static str, brand: &'static str, line: &'static str) -> FilmStockKind {
    FilmStockKind { icon_name: line, ..stock(id, display_name, brand) }
}

/// A stock the picker no longer lists (owner's call), kept so a roll already on it reads as it did.
const fn withdrawn(id: &'static str, display_name: &'static str, brand: &'static str) -> FilmStockKind {
    FilmStockKind { offered: false, ..stock(id, display_name, brand) }
}

/// A line split into its stocks: a roll set on the line keeps its name, canister and dyes, its stock unstated.
const fn line(id: &'static str, display_name: &'static str, brand: &'static str) -> FilmStockKind {
    FilmStockKind { line: true, ..withdrawn(id, display_name, brand) }
}

/// A stock whose own canister is still to be drawn: the generic colour negative's stands in.
const fn undrawn(id: &'static str, display_name: &'static str, brand: &'static str) -> FilmStockKind {
    of_line(id, display_name, brand, "generic-color-negative")
}

pub const FILM_STOCKS: [FilmStockKind; 55] = [
    stock("generic-azure", "Generic Azure", "Generic"),
    stock("generic-black-and-white", "Generic Black & White", "Generic"),
    stock("generic-color-negative", "Generic Colour Negative", "Generic"),
    stock("generic-color-positive", "Generic Colour Positive", "Generic"),
    stock("generic-redscale", "Generic Redscale", "Generic"),
    stock("agfaphoto-apx", "AgfaPhoto APX", "AgfaPhoto"),
    stock("cinestill-400d", "CineStill 400D", "CineStill"),
    stock("cinestill-50d", "CineStill 50D", "CineStill"),
    stock("cinestill-800t", "CineStill 800T", "CineStill"),
    stock("foma-fomapan200", "Foma Fomapan 200", "Foma"),
    stock("foma-fomapan400", "Foma Fomapan 400", "Foma"),
    withdrawn("foma-fomapan401", "Foma Fomapan 401", "Foma"),
    stock("fujifilm-acros", "Fujifilm Acros", "Fujifilm"),
    of_line("fujifilm-c200", "Fujifilm C200", "Fujifilm", "fujifilm-fujicolor"),
    line("fujifilm-fujicolor", "Fujifilm Fujicolor", "Fujifilm"),
    line("fujifilm-fujicolor-pro", "Fujifilm Fujicolor Pro", "Fujifilm"),
    of_line("fujifilm-pro400h", "Fujifilm Pro 400H", "Fujifilm", "fujifilm-fujicolor-pro"),
    stock("fujifilm-provia100", "Fujifilm Provia 100", "Fujifilm"),
    line("fujifilm-superia", "Fujifilm Superia", "Fujifilm"),
    of_line("fujifilm-superia-xtra400", "Fujifilm Superia X-TRA 400", "Fujifilm", "fujifilm-superia"),
    stock("fujifilm-velvia100", "Fujifilm Velvia 100", "Fujifilm"),
    stock("fujifilm-velvia50", "Fujifilm Velvia 50", "Fujifilm"),
    stock("harman-phoenix", "Harman Phoenix", "Harman"),
    stock("harman-phoenixII", "Harman Phoenix II", "Harman"),
    stock("harman-red", "Harman Red", "Harman"),
    stock("ilford-delta100", "Ilford Delta 100", "Ilford"),
    stock("ilford-delta3200", "Ilford Delta 3200", "Ilford"),
    stock("ilford-delta400", "Ilford Delta 400", "Ilford"),
    stock("ilford-fp4", "Ilford FP4", "Ilford"),
    stock("ilford-hp5", "Ilford HP5", "Ilford"),
    stock("ilford-ortho", "Ilford Ortho", "Ilford"),
    stock("ilford-panf", "Ilford Pan F", "Ilford"),
    stock("ilford-sfx", "Ilford SFX", "Ilford"),
    stock("ilford-xp2", "Ilford XP2", "Ilford"),
    stock("kodak-400tx", "Kodak Tri-X 400", "Kodak"),
    stock("kodak-colorplus", "Kodak ColorPlus", "Kodak"),
    stock("kodak-ektachrome", "Kodak Ektachrome", "Kodak"),
    undrawn("kodak-ektar100", "Kodak Ektar 100", "Kodak"),
    line("kodak-gold", "Kodak Gold", "Kodak"),
    of_line("kodak-gold200", "Kodak Gold 200", "Kodak", "kodak-gold"),
    line("kodak-portra", "Kodak Portra", "Kodak"),
    of_line("kodak-portra160", "Kodak Portra 160", "Kodak", "kodak-portra"),
    of_line("kodak-portra400", "Kodak Portra 400", "Kodak", "kodak-portra"),
    of_line("kodak-portra800", "Kodak Portra 800", "Kodak", "kodak-portra"),
    line("kodak-ultramax", "Kodak UltraMax", "Kodak"),
    of_line("kodak-ultramax400", "Kodak UltraMax 400", "Kodak", "kodak-ultramax"),
    undrawn("kodak-verita200d", "Kodak Verita 200D", "Kodak"),
    undrawn("kodak-vision3-50d", "Kodak Vision3 50D", "Kodak"),
    undrawn("kodak-vision3-200t", "Kodak Vision3 200T", "Kodak"),
    undrawn("kodak-vision3-250d", "Kodak Vision3 250D", "Kodak"),
    undrawn("kodak-vision3-500t", "Kodak Vision3 500T", "Kodak"),
    stock("lomography-lomo-turquoise", "Lomography LomoChrome Turquoise", "Lomography"),
    stock("lucky-color200", "Lucky Color 200", "Lucky"),
    stock("mira-color400", "Mira Color 400", "Mira"),
    stock("mira-color800", "Mira Color 800", "Mira"),
];

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

/// The roll marker, compact, its date in reference seconds: builds that rewrite it with no version
/// guard read these keys, so they never move.
#[derive(Serialize, Deserialize)]
struct MarkerWire {
    id: Id,
    name: String,
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
    manual_order: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    stock: Option<String>,
    #[serde(rename = "shotDate", with = "date::reference_seconds", skip_serializing_if = "Option::is_none", default)]
    shot_date: Option<Date>,
    #[serde(rename = "cameraBody", skip_serializing_if = "Option::is_none", default)]
    camera_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    lens: Option<String>,
}

/// A folder made a roll carries its roll in this hidden file, so the roll follows the folder.
pub mod roll_folder {
    use super::*;

    pub const ROLL_MARKER: &str = ".oni-roll.json";

    pub fn marker(folder: &Path) -> PathBuf {
        folder.join(ROLL_MARKER)
    }

    pub fn encode(film: &Film) -> String {
        let wire = MarkerWire {
            id: film.id,
            name: film.name.clone(),
            manual_order: film.manual_order,
            stock: film.stock.clone(),
            shot_date: film.shot_date,
            camera_body: film.camera_body.clone(),
            lens: film.lens.clone(),
        };
        serde_json::to_string(&wire).unwrap_or_default()
    }

    pub fn decode(bytes: &[u8]) -> Option<Film> {
        let wire: MarkerWire = serde_json::from_slice(bytes).ok()?;
        Some(Film {
            id: wire.id,
            name: wire.name,
            manual_order: wire.manual_order,
            stock: wire.stock,
            shot_date: wire.shot_date,
            camera_body: wire.camera_body,
            lens: wire.lens,
        })
    }

    /// The folder's roll; an absent or unreadable marker is no roll.
    pub fn roll(folder: &Path) -> Option<Film> {
        std::fs::read(marker(folder)).ok().and_then(|bytes| decode(&bytes))
    }

    /// Marks the folder a roll, or clears the mark on `None`.
    pub fn set_roll(film: Option<&Film>, folder: &Path) -> io::Result<()> {
        let marker = marker(folder);
        match film {
            None => match std::fs::remove_file(&marker) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            },
            Some(film) => crate::sidecar::write_atomically(&marker, encode(film).as_bytes()),
        }
    }

    /// The folder made and marked a roll; nothing is left behind when the disk refuses.
    pub fn make(name: &str, parent: &Path, film: &Film) -> io::Result<PathBuf> {
        let folder = parent.join(name);
        std::fs::create_dir(&folder)?;
        if let Err(e) = set_roll(Some(film), &folder) {
            let _ = std::fs::remove_dir_all(&folder);
            return Err(e);
        }
        Ok(folder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A roll's lens rides in its marker and its sidecar copy alike, and a roll naming none writes no key, so a
    /// marker from before reads as it was.
    #[test]
    fn a_roll_s_lens_reads_back_and_none_writes_no_key() {
        let mut film = Film::new("Roll");
        film.lens = Some("Nikkor 50mm f/1.8".into());
        assert_eq!(roll_folder::decode(roll_folder::encode(&film).as_bytes()), Some(film.clone()));
        assert_eq!(serde_json::from_str::<Film>(&serde_json::to_string(&film).unwrap()).unwrap(), film);
        let bare = Film::new("Roll");
        assert!(!roll_folder::encode(&bare).contains("lens") && !serde_json::to_string(&bare).unwrap().contains("lens"));
        let before = r#"{"id":"6A2D4F1C-9F2B-4C1E-9B11-7F0A0E3D2C11","name":"Roll","cameraBody":"Nikon FM2"}"#;
        let read = roll_folder::decode(before.as_bytes()).expect("a marker from before reads");
        assert_eq!((read.camera_body.as_deref(), read.lens), (Some("Nikon FM2"), None));
    }

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
    fn foma_401_is_no_longer_offered_yet_a_roll_on_it_still_reads() {
        let offered: Vec<&str> = offered_stocks().map(|(_, k)| k.id).collect();
        assert!(!offered.contains(&"foma-fomapan401"), "the picker no longer lists Foma 401");
        let lines = FILM_STOCKS.iter().filter(|k| k.line).count();
        assert_eq!(offered.len(), FILM_STOCKS.len() - 1 - lines, "every other stock but the lines is still offered");
        assert!(offered.contains(&"foma-fomapan400") && offered.contains(&"foma-fomapan200"), "twin: the other Fomapans stay");
        for (i, kind) in offered_stocks() {
            assert_eq!(FILM_STOCKS[i].id, kind.id, "a pick names the stock's place in the catalogue");
        }
        let kept = film_stock(Some("foma-fomapan401")).expect("a roll on Foma 401 still finds its stock");
        assert_eq!((kept.display_name, kept.icon_name, kept.brand), ("Foma Fomapan 401", "foma-fomapan401", "Foma"));
        let mut film = Film::new("Roll 401");
        film.stock = Some("foma-fomapan401".into());
        let back = roll_folder::decode(roll_folder::encode(&film).as_bytes()).unwrap();
        assert_eq!(back.stock.as_deref(), Some("foma-fomapan401"), "its marker reads and writes it unchanged");
        // Twin: a stock still offered is listed, so the filter is the withdrawn flag's alone.
        assert!(film_stock(Some("foma-fomapan400")).is_some_and(|k| k.offered));
        assert!(!kept.offered);
    }

    /// A line's stocks draw with its canister under their own names, and a roll set on the line still reads,
    /// names and draws it, its key unchanged in its marker.
    #[test]
    fn a_lines_stocks_keep_its_canister_and_a_roll_on_the_line_reads_as_it_did() {
        for (line, stocks) in [
            ("kodak-portra", &["kodak-portra160", "kodak-portra400", "kodak-portra800"][..]),
            ("kodak-gold", &["kodak-gold200"]),
            ("kodak-ultramax", &["kodak-ultramax400"]),
            ("fujifilm-fujicolor", &["fujifilm-c200"]),
            ("fujifilm-fujicolor-pro", &["fujifilm-pro400h"]),
            ("fujifilm-superia", &["fujifilm-superia-xtra400"]),
        ] {
            let kept = film_stock(Some(line)).expect("a line still reads");
            assert!(kept.line && !kept.offered && kept.icon_name == line, "{line}");
            for id in stocks {
                let kind = film_stock(Some(id)).expect("a line's stock is in the catalogue");
                assert!(kind.offered && !kind.line, "{id}");
                assert_eq!((kind.icon_name, kind.brand), (line, kept.brand), "{id} draws its line's canister");
                assert_ne!(kind.display_name, kept.display_name, "{id} is named as the precise stock");
            }
            let mut film = Film::new("Roll");
            film.stock = Some(line.into());
            let back = roll_folder::decode(roll_folder::encode(&film).as_bytes()).unwrap();
            assert_eq!(back.stock.as_deref(), Some(line), "its marker keeps the line's key");
        }
        // Twin: a precise stock is no line, and Foma 401, withdrawn, is no line either.
        assert!(!film_stock(Some("kodak-portra400")).unwrap().line && !film_stock(Some("foma-fomapan401")).unwrap().line);
    }

    #[test]
    fn a_marker_round_trips_with_its_date_in_reference_seconds() {
        let mut film = Film::new("Roll #8");
        film.stock = Some("kodak-gold".into());
        film.shot_date = Some(Date::since_reference_date(700_000_000.0));
        let text = roll_folder::encode(&film);
        assert!(text.contains("\"shotDate\":700000000"), "{text}");
        assert_eq!(roll_folder::decode(text.as_bytes()), Some(film.clone()));
        // Twin: another stock reads another stock.
        let mut other = film.clone();
        other.stock = Some("kodak-portra".into());
        assert_ne!(roll_folder::decode(roll_folder::encode(&other).as_bytes()), Some(film));
    }

    #[test]
    fn a_shot_date_reads_back_to_the_bit() {
        let mut lost_through_1970 = 0;
        for i in 0..1000u32 {
            let seconds = 700_000_000.0 + f64::from(i) * 104_729.123_456_789;
            let mut film = Film::new("Roll");
            film.shot_date = Some(Date::since_reference_date(seconds));
            let back = roll_folder::decode(roll_folder::encode(&film).as_bytes()).unwrap();
            let read = back.shot_date.unwrap().time_interval_since_reference_date();
            assert_eq!(read.to_bits(), seconds.to_bits(), "{seconds}");
            let detour = (seconds + date::REFERENCE_DATE_OFFSET) - date::REFERENCE_DATE_OFFSET;
            lost_through_1970 += usize::from(detour.to_bits() != seconds.to_bits());
        }
        // Twin: holding the instant from 1970 drops the last bit on a share of these dates.
        assert!(lost_through_1970 > 100, "{lost_through_1970}");
    }
}
