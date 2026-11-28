//! A slide before format 70 rendered on ONI's print or through an internegative on a paper; it is projected now, every
//! edit kept as stored, a paper it named included, which it no longer prints on. Its sidecar named no film: its roll's
//! marker names the reversal film it is read back to.

use super::chain::SLIDE_PROJECTOR_VERSION;
use crate::PipelineSettings;
use crate::dyes::slide_dyes;
use std::path::Path;

/// A scanned slide of format `from` in `original`'s folder takes the profiled reversal film its folder's roll marker
/// names, else the one its sidecar's own roll names; any other stock leaves its dyes' key as it is.
pub fn marked(from: i64, settings: PipelineSettings, original: &Path) -> PipelineSettings {
    if from >= SLIDE_PROJECTOR_VERSION || !settings.mode.is_slide() {
        return settings;
    }
    let marker = original.parent().and_then(crate::film::roll_folder::roll).and_then(|r| r.stock);
    let named = marker.or_else(|| settings.film.as_ref().and_then(|f| f.stock.clone())).and_then(|k| slide_dyes(&k));
    match named {
        Some(dye_set) => PipelineSettings { dye_set, ..settings },
        None => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::VERSION;
    use crate::film::{Film, roll_folder};
    use crate::{ConversionMode, DyeSet, PaperStock, VirtualStock};

    fn stored(s: &PipelineSettings, version: i64) -> Vec<u8> {
        let stored = super::super::reshape::as_stored(&crate::Document::of(s.clone()), version);
        serde_json::to_vec(&stored).unwrap()
    }

    fn read(bytes: &[u8]) -> PipelineSettings {
        super::super::document(bytes).unwrap().document.settings
    }

    /// A slide as a build of format 69 kept it: on a paper, Lighting and the colour tools moved.
    fn edited_slide() -> PipelineSettings {
        let mut s = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..PipelineSettings::rest(ConversionMode::Positive, true) };
        (s.print.exposure, s.print.contrast, s.print.shadows, s.density, s.gains.stops) = (0.4, 0.1, -0.05, 0.2, [0.1, 0.0, -0.2]);
        s.print.set_filters([0.1, 0.0, -0.1]);
        s
    }

    #[test]
    fn a_slide_before_comes_onto_the_projector_with_every_edit_kept() {
        let older = SLIDE_PROJECTOR_VERSION - 1;
        let slide = edited_slide();
        let read_back = read(&stored(&slide, older));
        assert_eq!(read_back, PipelineSettings { print_paper: None, ..slide.clone() }, "nothing moves but the paper, a default then");
        assert!(read_back.projector().is_some() && read_back.paper().is_none() && !read_back.print.projected_blacks);
        // A digital photo on a reversal film prints on none; on a negative film, on its own.
        let digital = PipelineSettings::rest(ConversionMode::Digital, true);
        let on_slide = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..digital.on_stock(VirtualStock::of("fujifilm-velvia-50")) };
        let read_slide = read(&stored(&on_slide, older));
        assert!(read_slide.print_paper.is_none() && read_slide.paper().is_none());
        assert_eq!(read(&stored(&digital, older)).paper(), digital.print_paper);
        // Round trip: written at this format, it reads back whole, Projected blacks included.
        let mut now = read_back.clone();
        now.print.projected_blacks = true;
        let bytes = stored(&now, VERSION);
        assert_eq!(read(&bytes), now);
        assert!(String::from_utf8_lossy(&bytes).contains("\"projectedBlacks\":true"));
        // Twin: off, the switch is not written, so a document at rest reads as before.
        assert!(!String::from_utf8_lossy(&stored(&read_back, VERSION)).contains("projectedBlacks"));
    }

    #[test]
    fn a_slide_before_reads_the_reversal_film_its_roll_names() {
        let dir = std::env::temp_dir().join(format!("oni-slide-projector-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let roll = Film { stock: Some("fujifilm-velvia100".into()), ..Film::new("Roll") };
        std::fs::write(roll_folder::marker(&dir), roll_folder::encode(&roll)).unwrap();
        let slide = edited_slide();
        let older = SLIDE_PROJECTOR_VERSION - 1;
        let marked = marked(older, slide.clone(), &dir.join("A.RW2"));
        assert_eq!(marked.projector(), Some(VirtualStock::of("fujifilm-velvia-100")));
        assert_eq!(PipelineSettings { dye_set: slide.dye_set, ..marked.clone() }, slide, "its dyes' key alone moves");
        // A roll naming a stock no sheet profiles leaves the key a negative's pick wrote: it projects on the default film.
        let portra = PipelineSettings { dye_set: DyeSet::of("kodak-portra-400"), ..slide.clone() };
        let kodachrome = Film { stock: Some("kodak-kodachrome64".into()), ..roll.clone() };
        std::fs::write(roll_folder::marker(&dir), roll_folder::encode(&kodachrome)).unwrap();
        let kept = super::marked(older, portra.clone(), &dir.join("A.RW2"));
        assert!(kept == portra && kept.projector() == Some(VirtualStock::SLIDE));
        // Twin: a slide of this format keeps the film it names, and a negative is never marked so.
        std::fs::write(roll_folder::marker(&dir), roll_folder::encode(&roll)).unwrap();
        assert_eq!(super::marked(VERSION, slide.clone(), &dir.join("A.RW2")), slide);
        let negative = PipelineSettings { mode: ConversionMode::Negative, ..slide.clone() };
        assert_eq!(super::marked(older, negative.clone(), &dir.join("A.RW2")), negative);
        // With no marker, the roll its own sidecar names.
        std::fs::remove_file(roll_folder::marker(&dir)).unwrap();
        let own = PipelineSettings { film: Some(roll.clone()), ..slide.clone() };
        assert_eq!(super::marked(older, own.clone(), &dir.join("A.RW2")).projector(), Some(VirtualStock::of("fujifilm-velvia-100")));
        assert_eq!(super::marked(older, slide.clone(), &dir.join("A.RW2")), slide);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slide_brought_on_disk_keeps_its_edits_and_takes_its_rolls_film() {
        let dir = std::env::temp_dir().join(format!("oni-slide-projector-disk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let roll = Film { stock: Some("fujifilm-velvia100".into()), ..Film::new("Roll") };
        std::fs::write(roll_folder::marker(&dir), roll_folder::encode(&roll)).unwrap();
        let original = dir.join("A.tif");
        std::fs::write(&original, b"").unwrap();
        let slide = PipelineSettings { decoded_from_mosaic: false, ..edited_slide() };
        std::fs::write(crate::sidecar::url_for(&original), stored(&slide, SLIDE_PROJECTOR_VERSION - 1)).unwrap();
        let outcome = super::super::file(&original, Some(false));
        assert_eq!(outcome, Ok(super::super::Outcome::Migrated { from: SLIDE_PROJECTOR_VERSION - 1 }));
        let written = std::fs::read(crate::sidecar::url_for(&original)).unwrap();
        assert_eq!(super::super::standing(&written), super::super::Standing::Current);
        let read_back = crate::sidecar::read(&original, false).unwrap().settings;
        assert_eq!(read_back, PipelineSettings { dye_set: DyeSet::of("fujifilm-velvia-100"), print_paper: None, ..slide.clone() });
        // Twin: brought again, nothing moves.
        assert_eq!(super::super::file(&original, Some(false)), Ok(super::super::Outcome::Current));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
