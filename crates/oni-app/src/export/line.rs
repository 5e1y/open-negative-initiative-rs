//! Each frame's line: the scan's facts with its roll's camera and stock, the artist and copyright last
//! written, the Kit's default body and lens where nothing names one, and over them what a hand set or an
//! export kept of a default, which alone its sidecar holds.

use oni_export::{ExifMetadata, Row};
use oni_settings::ExifLine;
use oni_settings::film::{Film, film_stock, roll_folder};
use std::path::Path;

/// The body and the lens the Kit names by default: a scan's own name its digitiser, never the film's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Defaults {
    pub body: Option<String>,
    pub lens: Option<String>,
}

/// A frame's line as the card shows it, the facts it was derived from, and what derived at its opening.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub row: Row,
    facts: ExifMetadata,
    opened: ExifMetadata,
    /// The defaults shown at its opening, its roll and its sidecar naming no body, its sidecar no lens.
    pub default_body: Option<String>,
    default_lens: Option<String>,
}

/// The scan's own facts, its roll's camera body and lens over the scan's and its roll's stock as the film.
fn facts(path: &Path, roll: Option<&Film>) -> ExifMetadata {
    let mut scan = oni_export::scan_metadata(path);
    if let Some(body) = roll.and_then(|f| f.camera_body.as_deref()).filter(|b| !b.is_empty()) {
        scan.camera = body.to_owned();
    }
    if let Some(lens) = roll.and_then(|f| f.lens.as_deref()).filter(|l| !l.is_empty()) {
        scan.lens = lens.to_owned();
    }
    scan.film = film_stock(roll.and_then(|f| f.stock.as_deref())).map(|k| k.display_name.to_owned()).unwrap_or_default();
    scan
}

/// `facts`, the artist and copyright remembered filling those the scan leaves empty.
fn derived(facts: &ExifMetadata, artist: &str, copyright: &str) -> ExifMetadata {
    let fill = |scan: &String, remembered: &str| if scan.is_empty() { remembered } else { scan }.to_owned();
    ExifMetadata { artist: fill(&facts.artist, artist), copyright: fill(&facts.copyright, copyright), ..facts.clone() }
}

impl Line {
    /// What the frame keeps once written, against the artist and copyright just remembered: what a hand
    /// set, and a camera or lens where a default showed, as written, so a later default moves neither;
    /// the default body stays its roll's when `rolled`, the roll having taken it.
    pub fn typed(&self, artist: &str, copyright: &str, rolled: bool) -> ExifLine {
        let mut typed = self.row.metadata.beyond(&[&self.opened, &derived(&self.facts, artist, copyright)]);
        let m = &self.row.metadata;
        if self.default_lens.is_some() {
            typed.lens = Some(m.lens.clone());
        }
        if let Some(body) = &self.default_body {
            typed.camera = (!rolled || *body != m.camera).then(|| m.camera.clone());
        }
        typed
    }
}

/// `path`'s line: what its sidecar keeps laid over what is derived now, so a roll's stock or body set
/// since reaches every field no hand set; the defaults fill a camera and a lens nothing else names. Its
/// roll is the folder's marker, else the one its sidecar names, when the archive gives none.
pub fn opening_line(path: &Path, roll: Option<&Film>, artist: &str, copyright: &str, defaults: &Defaults) -> Line {
    let mosaic = oni_decode::decodes_via_mosaic(path);
    let loaded = oni_settings::sidecar::read(path, mosaic).ok();
    let kept = loaded.as_ref().map(|l| l.exif.clone()).unwrap_or_default();
    let own = || path.parent().and_then(roll_folder::roll).or_else(|| loaded.as_ref().and_then(|l| l.settings.film.clone()));
    let roll = roll.cloned().or_else(own);
    let facts = facts(path, roll.as_ref());
    let opened = derived(&facts, artist, copyright);
    let rolled = roll.as_ref().and_then(|f| f.camera_body.as_deref()).is_some_and(|b| !b.is_empty());
    let default_body = defaults.body.clone().filter(|_| !rolled && kept.camera.is_none());
    let lensed = roll.as_ref().and_then(|f| f.lens.as_deref()).is_some_and(|l| !l.is_empty());
    let default_lens = defaults.lens.clone().filter(|_| !lensed && kept.lens.is_none());
    let mut shown = opened.clone();
    if let Some(body) = &default_body {
        shown.camera = body.clone();
    }
    if let Some(lens) = &default_lens {
        shown.lens = lens.clone();
    }
    Line { row: Row { name: oni_export::stem(path), metadata: kept.over(&shown) }, facts, opened, default_body, default_lens }
}

/// What a hand set on `path`'s line kept in its sidecar, once its file is written.
pub fn keep_line(path: &Path, typed: &ExifLine) {
    let _ = oni_settings::sidecar::set_exif(typed.clone(), path, oni_decode::decodes_via_mosaic(path));
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::film::stocks;
    use std::path::PathBuf;

    fn frame(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("oni-export-line-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("frame.RW2");
        std::fs::write(&path, b"raw").unwrap();
        path
    }

    /// The scratch folder `frame` made, gone.
    fn cleared(frame: &Path) {
        let folder = frame.parent().unwrap();
        let _ = std::fs::remove_dir_all(folder);
    }

    fn roll(stock: usize, body: &str) -> Film {
        Film { stock: Some(stocks()[stock].id.into()), camera_body: Some(body.into()), ..Film::new("roll") }
    }

    #[test]
    fn a_kept_line_follows_its_roll() {
        let path = frame("follows");
        let bare = Film::new("roll");
        let line = opening_line(&path, Some(&bare), "", "", &Defaults::default());
        keep_line(&path, &line.typed("", "", false));
        assert!(!oni_settings::sidecar::url_for(&path).exists(), "a line as derived gives the frame no sidecar");

        let (b, c) = (roll(1, "Nikon FM2"), roll(2, "Leica M6"));
        let mut line = opening_line(&path, Some(&b), "", "", &Defaults::default());
        assert_eq!((line.row.metadata.film.as_str(), line.row.metadata.camera.as_str()), (stocks()[1].display_name, "Nikon FM2"));
        line.row.metadata.film = stocks()[3].display_name.into();
        keep_line(&path, &line.typed("", "", false));
        let later = opening_line(&path, Some(&c), "", "", &Defaults::default());
        assert_eq!(later.row.metadata.film, stocks()[3].display_name, "the film a hand picked stays");
        assert_eq!(later.row.metadata.camera, "Leica M6", "the camera follows the roll");

        // Twin: the whole line kept, the roll's body frozen in it, no longer follows the roll.
        let whole = opening_line(&path, Some(&b), "", "", &Defaults::default());
        keep_line(&path, &whole.row.metadata.beyond(&[]));
        assert_ne!(opening_line(&path, Some(&c), "", "", &Defaults::default()).row.metadata.camera, "Leica M6");
        cleared(&path);
    }

    fn defaults(body: &str, lens: &str) -> Defaults {
        Defaults { body: Some(body.into()), lens: Some(lens.into()) }
    }

    /// The camera and the lens `path`'s line shows under `d`.
    fn gear(path: &Path, roll: Option<&Film>, d: &Defaults) -> (String, String) {
        let m = opening_line(path, roll, "", "", d).row.metadata;
        (m.camera, m.lens)
    }

    fn written(path: &Path, roll: Option<&Film>, d: &Defaults) {
        keep_line(path, &opening_line(path, roll, "", "", d).typed("", "", false));
    }

    #[test]
    fn a_default_fills_what_nothing_names_and_a_frame_written_keeps_it() {
        let (fm2, m6) = (defaults("Nikon FM2", "Nikkor 50"), defaults("Leica M6", "Summicron 35"));
        let path = frame("defaults");
        assert_eq!(gear(&path, None, &fm2), ("Nikon FM2".into(), "Nikkor 50".into()));
        assert_eq!(gear(&path, Some(&roll(1, "Pentax MX")), &fm2).0, "Pentax MX", "the roll's body wins");
        // Written twice under the same defaults, it keeps them, and a later default moves nothing.
        written(&path, None, &fm2);
        written(&path, None, &fm2);
        assert_eq!(gear(&path, None, &m6), ("Nikon FM2".into(), "Nikkor 50".into()));
        let rolled = frame("defaults-rolled");
        written(&rolled, Some(&Film::new("roll")), &fm2);
        assert_eq!(gear(&rolled, Some(&Film::new("roll")), &m6).0, "Nikon FM2", "in a roll with no body too");
        // Twin: a frame never written follows the defaults as they change.
        let fresh = frame("defaults-fresh");
        assert_eq!(gear(&fresh, None, &fm2).1, "Nikkor 50");
        assert_eq!(gear(&fresh, None, &m6).1, "Summicron 35");
        for p in [&path, &rolled, &fresh] {
            cleared(p);
        }
    }

    /// A roll's lens names its frames' lens over the scan's and the Kit's default, follows the roll while no hand
    /// set it, and a lens a hand set stays.
    #[test]
    fn a_roll_s_lens_names_its_frames_and_follows_it() {
        let lensed = |lens: &str| Film { lens: Some(lens.into()), ..roll(1, "Nikon FM2") };
        let fm2 = defaults("Nikon FM2", "Nikkor 50");
        let path = frame("roll-lens");
        assert_eq!(gear(&path, Some(&lensed("Nikkor 35")), &fm2).1, "Nikkor 35", "the roll's lens over the default");
        written(&path, Some(&lensed("Nikkor 35")), &fm2);
        assert_eq!(gear(&path, Some(&lensed("Nikkor 85")), &fm2).1, "Nikkor 85", "a frame written follows its roll's lens");
        assert_eq!(gear(&path, Some(&roll(1, "Nikon FM2")), &fm2).1, "Nikkor 50", "twin: a roll naming none leaves the default");
        let mut line = opening_line(&path, Some(&lensed("Nikkor 35")), "", "", &fm2);
        line.row.metadata.lens = "Elmarit 28".into();
        keep_line(&path, &line.typed("", "", true));
        assert_eq!(gear(&path, Some(&lensed("Nikkor 85")), &fm2).1, "Elmarit 28", "the lens a hand set stays");
        cleared(&path);
    }

    /// The line an export keeps reaches the Kit's counts once the landed frame's row is read again.
    #[test]
    fn a_kept_line_reaches_the_kit_s_counts() {
        let path = frame("counted");
        let index = oni_library::index::LibraryIndex::open(Path::new(":memory:")).unwrap();
        let member = [oni_library::paths::canonical(&path)];
        crate::library::refresh(&index, &member);
        keep_line(&path, &opening_line(&path, None, "", "", &defaults("Nikon FM2", "Nikkor 50")).typed("", "", false));
        // Twin: until its row is read again, the index counts the lens nowhere.
        assert_eq!(index.gear_counts().unwrap().lens("Nikkor 50"), 0);
        crate::library::refresh(&index, &member);
        let counts = index.gear_counts().unwrap();
        assert_eq!((counts.body("nikon fm2"), counts.lens("Nikkor 50")), (1, 1));
        cleared(&path);
    }

    #[test]
    fn a_value_kept_is_never_taken_by_a_default() {
        let (fm2, m6) = (defaults("Nikon FM2", "Nikkor 50"), defaults("Leica M6", "Summicron 35"));
        let path = frame("kept-gear");
        keep_line(&path, &ExifLine { lens: Some("Elmarit 28".into()), ..ExifLine::default() });
        written(&path, None, &fm2);
        assert_eq!(gear(&path, None, &m6).1, "Elmarit 28", "a lens kept is never the default's");
        // A default cleared by hand stays cleared once written.
        let cleared_now = frame("kept-clear");
        let mut line = opening_line(&cleared_now, None, "", "", &fm2);
        line.row.metadata.lens.clear();
        keep_line(&cleared_now, &line.typed("", "", false));
        assert_eq!(gear(&cleared_now, None, &m6).1, "");
        // In a roll that took the body, the frame follows the roll's body set later.
        let rolled = frame("kept-rolled");
        keep_line(&rolled, &opening_line(&rolled, Some(&Film::new("roll")), "", "", &fm2).typed("", "", true));
        assert_eq!(gear(&rolled, Some(&roll(1, "Pentax MX")), &m6).0, "Pentax MX");
        // Twin: a whole block an older build wrote still yields to what derives once exported.
        let older = frame("kept-older");
        keep_line(&older, &ExifLine { camera: Some(String::new()), lens: Some(String::new()), ..ExifLine::default() });
        written(&older, None, &Defaults::default());
        assert_eq!(gear(&older, None, &m6), ("Leica M6".into(), "Summicron 35".into()));
        for p in [&path, &cleared_now, &rolled, &older] {
            cleared(p);
        }
    }

    /// A line 1.4.0 to 1.5.6 kept whole names the digitising camera; once its sidecar comes to format 23
    /// against the scan's facts, the folder's roll names the body and the Kit the lens, in the line and the counts.
    #[test]
    fn a_line_kept_whole_by_an_older_build_yields_to_the_roll_once_migrated() {
        use oni_settings::migrate::{self, Facts};
        let path = frame("scan-camera");
        let whole = concat!(
            r#"{"version":20,"settings":{},"exif":{"artist":"","camera":"NIKON CORPORATION NIKON Z f","copyright":"","#,
            r#""date":"2026:09:14 10:21:03","film":"","iso":"100","lens":"COLOR SKOPAR 28MM"}}"#
        );
        std::fs::write(oni_settings::sidecar::url_for(&path), whole).unwrap();
        roll_folder::set_roll(Some(&roll(2, "Leica M6")), path.parent().unwrap()).unwrap();
        let kit = defaults("Nikon FM2", "Nikkor 50");
        let index = oni_library::index::LibraryIndex::open(Path::new(":memory:")).unwrap();
        let member = [oni_library::paths::canonical(&path)];
        // Twin: before the step the scan's camera wins over the roll's body, in the line and in the counts.
        assert_eq!(gear(&path, None, &kit), ("NIKON CORPORATION NIKON Z f".into(), "COLOR SKOPAR 28MM".into()));
        crate::library::refresh(&index, &member);
        assert_eq!(index.gear_counts().unwrap().body("leica m6"), 0);
        let scan = ExifMetadata { camera: "NIKON CORPORATION NIKON Z f".into(), lens: "COLOR SKOPAR 28MM".into(), iso: "100".into(), date: "2026:09:14 10:21:03".into(), ..ExifMetadata::default() };
        let facts = Facts { exif: &|| Some(scan.clone()), ..Facts::bare(true) };
        assert!(matches!(migrate::file_with(&path, &facts), Ok(migrate::Outcome::Migrated { from: 20 })));
        assert_eq!(gear(&path, None, &kit), ("Leica M6".into(), "Nikkor 50".into()));
        assert_eq!(opening_line(&path, None, "", "", &kit).row.metadata.film, "", "the film kept empty stays a clear");
        crate::library::refresh(&index, &member);
        assert_eq!(index.gear_counts().unwrap().body("leica m6"), 1);
        cleared(&path);
    }

    #[test]
    fn a_roll_the_archive_misses_is_read_from_the_frame() {
        let path = frame("own-roll");
        let mut marked = Film::new("roll");
        marked.camera_body = Some("Pentax MX".into());
        roll_folder::set_roll(Some(&marked), path.parent().unwrap()).unwrap();
        assert_eq!(gear(&path, None, &defaults("Nikon FM2", "Nikkor 50")).0, "Pentax MX", "the folder's marker names the body");
        // Twin: without the marker the default fills it.
        roll_folder::set_roll(None, path.parent().unwrap()).unwrap();
        assert_eq!(gear(&path, None, &defaults("Nikon FM2", "Nikkor 50")).0, "Nikon FM2");
        cleared(&path);
    }

    #[test]
    fn the_first_lines_artist_is_not_frozen() {
        let path = frame("artist");
        let mut line = opening_line(&path, None, "", "", &Defaults::default());
        line.row.metadata.artist = "X".into();
        assert_eq!(line.typed("X", "", false).artist, None, "the artist remembered as this export writes is derived");
        // Twin: against the artist remembered before, the typed one is the hand's.
        assert_eq!(line.typed("", "", false).artist.as_deref(), Some("X"));
        cleared(&path);
    }

    #[test]
    fn a_line_left_alone_is_not_pinned_to_the_artist_remembered_before() {
        let (first, other) = (frame("first"), frame("other"));
        let mut line = opening_line(&first, None, "Alice", "", &Defaults::default());
        line.row.metadata.artist = "Bob".into();
        let left = opening_line(&other, None, "Alice", "", &Defaults::default());
        let typed = left.typed("Bob", "", false);
        keep_line(&other, &typed);
        assert!(typed.is_empty() && !oni_settings::sidecar::url_for(&other).exists(), "a line left alone gains no sidecar: {typed:?}");
        assert_eq!(opening_line(&other, None, "Bob", "", &Defaults::default()).row.metadata.artist, "Bob", "the artist remembered since reaches it");
        // Twin: told from what derives under the new artist alone, the old one is pinned.
        let pinned = left.row.metadata.beyond(&[&derived(&left.facts, "Bob", "")]);
        assert_eq!(pinned.artist.as_deref(), Some("Alice"));
        cleared(&first);
        cleared(&other);
    }

    #[test]
    fn a_field_cleared_by_hand_stays_cleared() {
        let path = frame("cleared");
        let whole = r#"{"version":20,"settings":{},"exif":{"artist":"","camera":"","copyright":"","date":"","film":"","iso":"","lens":""}}"#;
        std::fs::write(oni_settings::sidecar::url_for(&path), whole).unwrap();
        let line = opening_line(&path, None, "Alice", "", &Defaults::default());
        assert_eq!(line.row.metadata.artist, "", "an artist cleared in a block written whole stays cleared");
        keep_line(&path, &line.typed("Alice", "", false));
        assert_eq!(opening_line(&path, None, "Alice", "", &Defaults::default()).row.metadata.artist, "", "an export keeps the clear");

        let fresh = frame("cleared-fresh");
        let mut line = opening_line(&fresh, None, "Alice", "", &Defaults::default());
        line.row.metadata.artist.clear();
        keep_line(&fresh, &line.typed("Alice", "", false));
        assert_eq!(opening_line(&fresh, None, "Alice", "", &Defaults::default()).row.metadata.artist, "", "a clear typed now is kept");
        // Twin: a block without the artist's key derives it.
        keep_line(&fresh, &ExifLine { film: Some(String::new()), ..ExifLine::default() });
        assert_eq!(opening_line(&fresh, None, "Alice", "", &Defaults::default()).row.metadata.artist, "Alice");
        cleared(&path);
        cleared(&fresh);
    }
}
