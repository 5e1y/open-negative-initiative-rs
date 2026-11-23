//! A photo a sidecar before format 39 kept named no stock of its own: it takes its film's, or the stock its roll names.

use super::chain::STOCK_DYES_VERSION;
use crate::PipelineSettings;
use crate::dyes::dye_set;
use std::path::Path;

/// The settings on their stock: a digital photo its film's, a scan the stock its sidecar's roll names.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= STOCK_DYES_VERSION {
        return settings;
    }
    let named = settings.film().map(|f| f.stock.dye_set()).or_else(|| settings.film.as_ref().and_then(|f| f.stock.as_deref()).and_then(dye_set));
    PipelineSettings { dye_set: named.unwrap_or(settings.dye_set), ..settings }
}

/// A scan of format `from` in `original`'s folder takes the stock its folder's roll marker names, which its
/// sidecar does not hold.
pub fn marked(from: i64, settings: PipelineSettings, original: &Path) -> PipelineSettings {
    if from >= STOCK_DYES_VERSION || settings.mode.is_digital() {
        return settings;
    }
    let marker = original.parent().and_then(crate::film::roll_folder::roll).and_then(|r| r.stock).and_then(|k| dye_set(&k));
    PipelineSettings { dye_set: marker.unwrap_or(settings.dye_set), ..settings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::film::{Film, roll_folder};
    use crate::{ConversionMode, DyeSet};

    #[test]
    fn a_photo_edited_before_its_stock_was_named_takes_its_rolls_or_its_films() {
        let older = STOCK_DYES_VERSION - 1;
        let at_rest = PipelineSettings::rest(ConversionMode::Negative, true);
        let mut rolled = at_rest.clone();
        rolled.film = Some(Film { stock: Some("kodak-gold".into()), ..Film::new("Roll") });
        assert_eq!(caught_up(older, rolled.clone()).dye_set, DyeSet::of("kodak-gold-200"), "its sidecar's roll names its stock");
        let digital = PipelineSettings::rest(ConversionMode::Digital, true);
        assert_eq!(caught_up(older, PipelineSettings { dye_set: DyeSet::of("generic-c41"), ..digital }).dye_set, DyeSet::of("kodak-portra-800"));
        // Twin: a file of this format keeps the stock it names.
        assert_eq!(caught_up(STOCK_DYES_VERSION, rolled).dye_set, at_rest.dye_set);
    }

    #[test]
    fn a_scan_in_a_marked_folder_takes_the_markers_stock() {
        let dir = std::env::temp_dir().join(format!("oni-measured-dyes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut film = Film::new("Roll");
        film.stock = Some("fujifilm-superia".into());
        roll_folder::set_roll(Some(&film), &dir).unwrap();
        let scan = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(marked(STOCK_DYES_VERSION - 1, scan.clone(), &dir.join("A.NEF")).dye_set, DyeSet::of("fujifilm-xtra-400"));
        // Twin: a file of this format keeps the set it names.
        assert_eq!(marked(STOCK_DYES_VERSION, scan.clone(), &dir.join("A.NEF")).dye_set, scan.dye_set);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
