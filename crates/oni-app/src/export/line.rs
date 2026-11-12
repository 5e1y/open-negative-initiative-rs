//! Each frame's line: the scan's facts with its roll's camera and stock, the artist and copyright last
//! written, and over them what a hand set, which alone its sidecar keeps.

use oni_export::{ExifMetadata, Row};
use oni_settings::film::{Film, film_stock};
use std::path::Path;

/// A frame's line as the card shows it, and the facts it was derived from at its opening.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub row: Row,
    pub facts: ExifMetadata,
}

/// The scan's own facts, its roll's camera body over the scan's camera and its roll's stock as the film.
fn facts(path: &Path, roll: Option<&Film>) -> ExifMetadata {
    let mut scan = oni_export::scan_metadata(path);
    if let Some(body) = roll.and_then(|f| f.camera_body.as_deref()).filter(|b| !b.is_empty()) {
        scan.camera = body.to_owned();
    }
    scan.film = film_stock(roll.and_then(|f| f.stock.as_deref())).map(|k| k.display_name.to_owned()).unwrap_or_default();
    scan
}

impl Line {
    /// The facts, the artist and copyright last written filling those the scan leaves empty.
    fn derived(&self, artist: &str, copyright: &str) -> ExifMetadata {
        let remembered = ExifMetadata { artist: artist.to_owned(), copyright: copyright.to_owned(), ..ExifMetadata::default() };
        self.facts.over(&remembered)
    }

    /// What a hand set on the line: every field its derived value would fill again is left out.
    pub fn typed(&self, artist: &str, copyright: &str) -> ExifMetadata {
        self.row.metadata.beyond(&self.derived(artist, copyright))
    }
}

/// `path`'s line: what its sidecar keeps laid over what is derived now, so a roll's stock or body set
/// since reaches every field no hand set.
pub fn opening_line(path: &Path, roll: Option<&Film>, artist: &str, copyright: &str) -> Line {
    let mosaic = oni_decode::decodes_via_mosaic(path);
    let kept = oni_settings::sidecar::read(path, mosaic, false).ok().and_then(|l| l.exif).unwrap_or_default();
    let mut line = Line { row: Row { name: oni_export::stem(path), metadata: ExifMetadata::default() }, facts: facts(path, roll) };
    line.row.metadata = kept.over(&line.derived(artist, copyright));
    line
}

/// What a hand set on `path`'s line kept in its sidecar, once its file is written.
pub fn keep_line(path: &Path, typed: &ExifMetadata) {
    let _ = oni_settings::sidecar::set_exif(typed.clone(), path, oni_decode::decodes_via_mosaic(path));
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::film::FILM_STOCKS;
    use std::path::PathBuf;

    fn frame(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("oni-export-line-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("frame.RW2");
        std::fs::write(&path, b"raw").unwrap();
        path
    }

    fn roll(stock: usize, body: &str) -> Film {
        Film { stock: Some(FILM_STOCKS[stock].id.into()), camera_body: Some(body.into()), ..Film::new("roll") }
    }

    #[test]
    fn a_kept_line_follows_its_roll() {
        let path = frame("follows");
        let bare = Film::new("roll");
        let line = opening_line(&path, Some(&bare), "", "");
        keep_line(&path, &line.typed("", ""));
        assert!(!oni_settings::sidecar::url_for(&path).exists(), "a line as derived gives the frame no sidecar");

        let (b, c) = (roll(1, "Nikon FM2"), roll(2, "Leica M6"));
        let mut line = opening_line(&path, Some(&b), "", "");
        assert_eq!((line.row.metadata.film.as_str(), line.row.metadata.camera.as_str()), (FILM_STOCKS[1].display_name, "Nikon FM2"));
        line.row.metadata.film = FILM_STOCKS[3].display_name.into();
        keep_line(&path, &line.typed("", ""));
        let later = opening_line(&path, Some(&c), "", "");
        assert_eq!(later.row.metadata.film, FILM_STOCKS[3].display_name, "the film a hand picked stays");
        assert_eq!(later.row.metadata.camera, "Leica M6", "the camera follows the roll");

        // Twin: the whole line kept, the roll's body frozen in it, no longer follows the roll.
        let whole = opening_line(&path, Some(&b), "", "");
        keep_line(&path, &whole.row.metadata);
        assert_ne!(opening_line(&path, Some(&c), "", "").row.metadata.camera, "Leica M6");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_first_lines_artist_is_not_frozen() {
        let path = frame("artist");
        let mut line = opening_line(&path, None, "", "");
        line.row.metadata.artist = "X".into();
        assert_eq!(line.typed("X", "").artist, "", "the artist remembered as this export writes is derived");
        // Twin: against the artist remembered before, the typed one is the hand's.
        assert_eq!(line.typed("", "").artist, "X");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
