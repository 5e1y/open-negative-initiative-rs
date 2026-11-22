//! A roll, its film stock catalogue (`catalogue`), and the marker a folder made a roll carries beside its photos.

use crate::date::{self, Date};
use crate::id::Id;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

mod catalogue;
pub use catalogue::*;

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
