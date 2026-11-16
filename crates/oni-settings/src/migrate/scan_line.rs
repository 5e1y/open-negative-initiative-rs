//! The EXIF line builds 1.4.0 to 1.5.6 kept whole, the digitising camera's facts in it: each field
//! holding the scan's own fact is released, so the roll's body and the Kit's gear derive again.

use super::chain::SCAN_LINE_RELEASED_VERSION;
use crate::sidecar::{ExifLine, ExifMetadata};

/// `line` without each camera, lens, ISO, date, artist and copyright holding the scan's own fact, equal
/// once trimmed. An empty value is a clear and stays, as does the film, which no scan names.
pub fn caught_up(version: i64, line: ExifLine, scan: &dyn Fn() -> Option<ExifMetadata>) -> ExifLine {
    let said = |f: &Option<String>| f.as_deref().is_some_and(|v| !v.trim().is_empty());
    let held = [&line.camera, &line.lens, &line.iso, &line.date, &line.artist, &line.copyright];
    if version >= SCAN_LINE_RELEASED_VERSION || !held.into_iter().any(said) {
        return line;
    }
    // Asked last, as asking reads the file; an original that cannot say leaves the line as it stands.
    let Some(scan) = scan() else { return line };
    let kept = |mine: Option<String>, theirs: &str| mine.filter(|m| m.trim().is_empty() || m.trim() != theirs.trim());
    ExifLine {
        camera: kept(line.camera, &scan.camera),
        lens: kept(line.lens, &scan.lens),
        film: line.film,
        iso: kept(line.iso, &scan.iso),
        date: kept(line.date, &scan.date),
        artist: kept(line.artist, &scan.artist),
        copyright: kept(line.copyright, &scan.copyright),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: i64 = SCAN_LINE_RELEASED_VERSION - 1;

    fn z_f() -> ExifMetadata {
        ExifMetadata {
            camera: "NIKON CORPORATION NIKON Z f".into(),
            lens: "NIKKOR Z MC 50mm f/2.8".into(),
            iso: "100".into(),
            date: "2026:09:14 10:21:03".into(),
            ..ExifMetadata::default()
        }
    }

    /// The whole line an export of 1.5.6 kept: the scan's facts, a lens typed over the scan's, the rest empty.
    fn whole() -> ExifLine {
        ExifLine {
            camera: Some("NIKON CORPORATION NIKON Z f ".into()),
            lens: Some("Summicron 35".into()),
            film: Some(String::new()),
            iso: Some("100".into()),
            date: Some("2026:09:14 10:21:03".into()),
            artist: Some(String::new()),
            copyright: Some(String::new()),
        }
    }

    #[test]
    fn the_scan_s_own_facts_are_released_and_the_hand_s_stay() {
        let out = caught_up(BEFORE, whole(), &|| Some(z_f()));
        let clears = ExifLine { film: Some(String::new()), artist: Some(String::new()), copyright: Some(String::new()), ..ExifLine::default() };
        assert_eq!(out, ExifLine { lens: Some("Summicron 35".into()), ..clears }, "camera, ISO and date released; a lens apart and the clears stay");
        // Twin: an ISO a hand typed apart from the scan's stays.
        let typed = ExifLine { iso: Some("400".into()), ..whole() };
        assert_eq!(caught_up(BEFORE, typed, &|| Some(z_f())).iso.as_deref(), Some("400"));
    }

    #[test]
    fn a_clear_stays_cleared_beside_a_scan_that_says_nothing() {
        let empty = ExifMetadata::default();
        let cleared = ExifLine { camera: Some("  ".into()), artist: Some(String::new()), ..whole() };
        let out = caught_up(BEFORE, cleared, &|| Some(empty.clone()));
        assert_eq!((out.camera.as_deref(), out.artist.as_deref(), out.film.as_deref()), (Some("  "), Some(""), Some("")), "every clear stays");
        assert_eq!(out.iso.as_deref(), Some("100"), "an empty scan fact releases nothing, as an unreadable EXIF");
        // Twin: a step releasing what equals the scan's, empty included, would drop the cleared artist.
        let loose = |m: Option<String>, t: &str| m.filter(|m| m.trim() != t.trim());
        assert_eq!(loose(Some(String::new()), &empty.artist), None);
    }

    #[test]
    fn a_line_of_clears_or_of_this_format_never_asks_the_scan() {
        let unasked = || -> Option<ExifMetadata> { panic!("the scan was asked") };
        let clears = ExifLine { film: Some(String::new()), artist: Some(String::new()), ..ExifLine::default() };
        assert_eq!(caught_up(BEFORE, clears.clone(), &unasked), clears);
        let film = ExifLine { film: Some("Kodak Portra 400".into()), ..ExifLine::default() };
        assert_eq!(caught_up(BEFORE, film.clone(), &unasked), film, "the film is never the scan's");
        assert_eq!(caught_up(SCAN_LINE_RELEASED_VERSION, whole(), &unasked), whole(), "a line of this format is the hand's");
        // Twin: one field holding a value before this format asks it.
        let asked = std::cell::Cell::new(false);
        let ask = || {
            asked.set(true);
            None
        };
        let _ = caught_up(BEFORE, ExifLine { iso: Some("100".into()), ..clears }, &ask);
        assert!(asked.get());
    }

    #[test]
    fn an_original_that_cannot_say_leaves_the_line_as_it_stands() {
        assert_eq!(caught_up(BEFORE, whole(), &|| None), whole());
        // Twin: the same line with the scan answering does move.
        assert_ne!(caught_up(BEFORE, whole(), &|| Some(z_f())), whole());
    }
}
