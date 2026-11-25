//! Grain size a sidecar before format 47 kept as a share of its track, its cell 4^share pixels of a scan 6000
//! wide; moved once into a grain's width on the film, through the scan's own shape, so its render stays.

use super::Refusal;
use super::chain::GRAIN_ON_FILM_VERSION;
use crate::PipelineSettings;
use crate::blocks::Grain;
use crate::geometry::Size;
use crate::tolerant;
use serde_json::Value;

/// The former law: the finest cell a share of the scan's width, Size's top two stops coarser, its rest halfway.
const FORMER_FINEST: f64 = 1.0 / 6000.0;
const FORMER_STOPS: f64 = 2.0;
const FORMER_REST: f64 = 0.5;
/// A grain's mean width along a line, in cells of the former texture's lattice.
const FORMER_CLUMP_CELLS: f64 = 2.64;

/// The width on the film a former `share` laid on a scan whose width is `aspect` of its long side.
pub fn microns(share: f64, aspect: f64) -> f32 {
    (FORMER_FINEST * libm::exp2(FORMER_STOPS * share) * aspect * Grain::FRAME_LONG * FORMER_CLUMP_CELLS) as f32
}

/// The settings with Grain size on the film; `stored` is the document's own settings object as written. A grain
/// off laid nothing and takes this rest; one laid asks the scan's shape, last, as asking reads the file.
pub fn caught_up(version: i64, mut settings: PipelineSettings, stored: Option<&Value>, scan: &dyn Fn(&PipelineSettings) -> Result<Size, Refusal>) -> Result<PipelineSettings, Refusal> {
    if version >= GRAIN_ON_FILM_VERSION {
        return Ok(settings);
    }
    if settings.grain.amount == 0.0 {
        settings.grain.size = Grain::SIZE_REST;
        return Ok(settings);
    }
    let share = tolerant::ns_number(stored.and_then(|s| s.get("grain")).and_then(|g| g.get("size"))).filter(|v| v.is_finite()).unwrap_or(FORMER_REST);
    let (w, h) = scan(&settings)?;
    if !(w > 0.0 && h > 0.0) {
        return Err(Refusal::ScanUnknown);
    }
    settings.grain.size = microns(share, w / w.max(h));
    Ok(settings)
}

/// `caught_up` for a document of no file, a preset or a paste: its photo read as wider than tall.
pub fn unfiled(version: i64, settings: PipelineSettings, stored: Option<&Value>) -> PipelineSettings {
    let fallback = settings.clone();
    caught_up(version, settings, stored, &|_| Ok((1.0, 1.0))).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConversionMode;
    use serde_json::json;

    fn grained(amount: f32) -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
        s.grain.amount = amount;
        s
    }

    #[test]
    fn a_former_share_becomes_the_width_it_laid_on_the_scan_its_shape_gives() {
        let older = GRAIN_ON_FILM_VERSION - 1;
        let stored = json!({ "grain": { "amount": 0.4, "size": 0.4 } });
        let landscape = caught_up(older, grained(0.4), Some(&stored), &|_| Ok((6048.0, 4032.0))).unwrap();
        let portrait = caught_up(older, grained(0.4), Some(&stored), &|_| Ok((4032.0, 6048.0))).unwrap();
        // The former cell, 4^0.4 pixels of a scan 6000 wide, laid back as this format's cell on the long side.
        let former = |w: f64| w * 4f64.powf(0.4) / 6000.0;
        let cell = |s: &PipelineSettings| Grain::pixels(s.grain.size, 6048.0) / FORMER_CLUMP_CELLS;
        assert!((cell(&landscape) / former(6048.0) - 1.0).abs() < 1e-6, "{}", landscape.grain.size);
        assert!((cell(&portrait) / former(4032.0) - 1.0).abs() < 1e-6, "{}", portrait.grain.size);
        // Twin: the portrait read as wide as it is long lays a cell half as coarse again.
        assert!((cell(&landscape) / former(4032.0) - 1.0).abs() > 0.4);
        // A document of this format keeps its value, which a second pass would move.
        assert_eq!(caught_up(GRAIN_ON_FILM_VERSION, landscape.clone(), Some(&stored), &|_| Err(Refusal::ScanUnknown)).unwrap(), landscape);
    }

    #[test]
    fn a_grain_off_takes_this_rest_without_asking_the_file() {
        let older = GRAIN_ON_FILM_VERSION - 1;
        for size in [json!(0.5), json!(0.8)] {
            let off = caught_up(older, grained(0.0), Some(&json!({ "grain": { "amount": 0, "size": size } })), &|_| Err(Refusal::ScanUnknown)).unwrap();
            assert_eq!(off.grain.size, Grain::SIZE_REST);
        }
        // An absent size was on the former rest.
        let absent = caught_up(older, grained(0.3), Some(&json!({ "grain": { "amount": 0.3 } })), &|_| Ok((6000.0, 4000.0))).unwrap();
        assert!((absent.grain.size - microns(FORMER_REST, 1.0)).abs() < 1e-4);
        // Twin: a grain laid asks the file, and its absence holds the step.
        assert_eq!(caught_up(older, grained(0.3), Some(&json!({ "grain": { "amount": 0.3, "size": 0.5 } })), &|_| Err(Refusal::ScanUnknown)), Err(Refusal::ScanUnknown));
    }

    #[test]
    fn a_document_of_no_file_reads_its_photo_wider_than_tall() {
        let older = GRAIN_ON_FILM_VERSION - 1;
        let s = unfiled(older, grained(0.5), Some(&json!({ "grain": { "amount": 0.5, "size": 0.0 } })));
        assert!((s.grain.size - microns(0.0, 1.0)).abs() < 1e-6);
        assert!((microns(0.0, 1.0) - 6.0 * FORMER_CLUMP_CELLS as f32).abs() < 1e-4, "the former finest is 6 µm of a 36 mm frame");
    }

    #[test]
    fn a_grain_held_for_its_file_reads_in_this_format_s_units() {
        let mut document = serde_json::to_value(crate::Document::of(grained(0.4))).unwrap();
        (document["version"], document["settings"]["grain"]["size"]) = (json!(GRAIN_ON_FILM_VERSION - 1), json!(0.5));
        let (read, held) = super::super::reading(&serde_json::to_vec(&document).unwrap(), &super::super::Facts::bare(true), false).unwrap();
        assert_eq!(held, Some(Refusal::ScanUnknown), "the step waits for the file");
        assert!((read.document.settings.grain.size - microns(0.5, 1.0)).abs() < 1e-4, "{}", read.document.settings.grain.size);
        // Twin: the former share read as micrometres is refused by the same judge.
        assert!((0.5 - microns(0.5, 1.0)).abs() >= 1e-4);
    }
}
