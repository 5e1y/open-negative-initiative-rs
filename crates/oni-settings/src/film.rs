//! A roll, its film stock catalogue, and the marker a folder made a roll carries beside its photos.

use crate::blocks::SeparationFamily;
use crate::date::{self, Date};
use crate::id::Id;
use crate::json;
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
}

impl Film {
    pub fn new(name: &str) -> Self {
        Film { id: Id::new(), name: name.to_owned(), manual_order: None, stock: None, shot_date: None, camera_body: None }
    }
}

/// One entry of the closed stock list, keyed by a stable `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilmStockKind {
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon_name: &'static str,
    pub brand: &'static str,
}

const fn stock(id: &'static str, display_name: &'static str, brand: &'static str) -> FilmStockKind {
    FilmStockKind { id, display_name, icon_name: id, brand }
}

pub const FILM_STOCKS: [FilmStockKind; 41] = [
    stock("generic-azure", "Generic Azure", "Generic"),
    stock("generic-black-and-white", "Generic Black & White", "Generic"),
    stock("generic-color-negative", "Generic Color Negative", "Generic"),
    stock("generic-color-positive", "Generic Color Positive", "Generic"),
    stock("generic-redscale", "Generic Redscale", "Generic"),
    stock("agfaphoto-apx", "AgfaPhoto APX", "AgfaPhoto"),
    stock("cinestill-400d", "CineStill 400D", "CineStill"),
    stock("cinestill-50d", "CineStill 50D", "CineStill"),
    stock("cinestill-800t", "CineStill 800T", "CineStill"),
    stock("foma-fomapan200", "Foma Fomapan 200", "Foma"),
    stock("foma-fomapan400", "Foma Fomapan 400", "Foma"),
    stock("foma-fomapan401", "Foma Fomapan 401", "Foma"),
    stock("fujifilm-acros", "Fujifilm Acros", "Fujifilm"),
    stock("fujifilm-fujicolor", "Fujifilm Fujicolor", "Fujifilm"),
    stock("fujifilm-fujicolor-pro", "Fujifilm Fujicolor Pro", "Fujifilm"),
    stock("fujifilm-provia100", "Fujifilm Provia 100", "Fujifilm"),
    stock("fujifilm-superia", "Fujifilm Superia", "Fujifilm"),
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
    stock("kodak-gold", "Kodak Gold", "Kodak"),
    stock("kodak-portra", "Kodak Portra", "Kodak"),
    stock("kodak-ultramax", "Kodak UltraMax", "Kodak"),
    stock("lomography-lomo-turquoise", "Lomography LomoChrome Turquoise", "Lomography"),
    stock("lucky-color200", "Lucky Color 200", "Lucky"),
    stock("mira-color400", "Mira Color 400", "Mira"),
    stock("mira-color800", "Mira Color 800", "Mira"),
];

pub fn film_stock(id: Option<&str>) -> Option<&'static FilmStockKind> {
    let id = id?;
    FILM_STOCKS.iter().find(|k| k.id == id)
}

/// Each stock's dye set, which picking it writes on the roll's frames; black and white has none.
/// As long as the catalogue, so a stock added there cannot go without its family.
pub const STOCK_DYES: [(&str, Option<SeparationFamily>); FILM_STOCKS.len()] = {
    use SeparationFamily::*;
    [
        ("generic-azure", Some(GenericC41)),
        ("generic-black-and-white", None),
        ("generic-color-negative", Some(GenericC41)),
        ("generic-color-positive", Some(E6)),
        ("generic-redscale", Some(GenericC41)),
        ("agfaphoto-apx", None),
        ("cinestill-400d", Some(Ecn2)),
        ("cinestill-50d", Some(Ecn2)),
        ("cinestill-800t", Some(Ecn2)),
        ("foma-fomapan200", None),
        ("foma-fomapan400", None),
        ("foma-fomapan401", None),
        ("fujifilm-acros", None),
        ("fujifilm-fujicolor", Some(FujiC41)),
        ("fujifilm-fujicolor-pro", Some(FujiC41)),
        ("fujifilm-provia100", Some(E6)),
        ("fujifilm-superia", Some(FujiC41)),
        ("fujifilm-velvia100", Some(E6)),
        ("fujifilm-velvia50", Some(E6)),
        ("harman-phoenix", Some(GenericC41)),
        ("harman-phoenixII", Some(GenericC41)),
        ("harman-red", Some(GenericC41)),
        ("ilford-delta100", None),
        ("ilford-delta3200", None),
        ("ilford-delta400", None),
        ("ilford-fp4", None),
        ("ilford-hp5", None),
        ("ilford-ortho", None),
        ("ilford-panf", None),
        ("ilford-sfx", None),
        ("ilford-xp2", None),
        ("kodak-400tx", None),
        ("kodak-colorplus", Some(KodakC41)),
        ("kodak-ektachrome", Some(E6)),
        ("kodak-gold", Some(KodakC41)),
        ("kodak-portra", Some(KodakC41)),
        ("kodak-ultramax", Some(KodakC41)),
        ("lomography-lomo-turquoise", Some(GenericC41)),
        ("lucky-color200", Some(GenericC41)),
        ("mira-color400", Some(GenericC41)),
        ("mira-color800", Some(GenericC41)),
    ]
};

/// The dye family `stock_id` writes on its roll's frames: none for black and white or an unknown key.
pub fn dye_family(stock_id: &str) -> Option<SeparationFamily> {
    STOCK_DYES.iter().find(|(id, _)| *id == stock_id).and_then(|(_, family)| *family)
}

/// A roll's generic stock while nobody picked one, read off the mode a hand already set.
pub fn default_stock_id(mode: ConversionMode) -> &'static str {
    match mode {
        ConversionMode::Negative => "generic-color-negative",
        ConversionMode::Positive => "generic-color-positive",
        ConversionMode::Monochrome => "generic-black-and-white",
    }
}

/// The roll marker as a plain `JSONEncoder` writes it: compact, its date in reference seconds.
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
        };
        json::to_node(&wire).map(|node| json::compact(&node)).unwrap_or_default()
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
    fn every_stock_names_its_dyes() {
        for kind in &FILM_STOCKS {
            assert!(STOCK_DYES.iter().any(|(id, _)| *id == kind.id), "{} has no dye family", kind.id);
        }
        assert_eq!(dye_family("kodak-portra"), Some(SeparationFamily::KodakC41));
        assert_eq!(dye_family("fujifilm-velvia50"), Some(SeparationFamily::E6));
        assert_eq!(dye_family("cinestill-800t"), Some(SeparationFamily::Ecn2));
        assert_eq!(dye_family("ilford-hp5"), None, "black and white unmixes nothing");
        // Twin: a key the catalogue lacks names no family.
        assert_eq!(dye_family("kodak-portra-900"), None);
        assert!(!STOCK_DYES.iter().any(|(id, _)| *id == "kodak-portra-900"));
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
