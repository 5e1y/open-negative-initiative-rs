//! The info sidebar's roll card, beyond Swift: a folder of photos is a roll, its stock writes the
//! dye family it names on every frame, and an unmeasured roll says so.

use super::*;
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::dye_family;
use oni_settings::PipelineSettings;

impl State {
    /// The photos a folder's marker gathers: every frame lying in it, passes and trashed photos aside.
    pub(in crate::app) fn folder_frames(&self, folder: &str) -> Vec<String> {
        self.rows
            .iter()
            .filter(|e| e.pass_of.is_none() && e.trashed_at.is_none() && oni_library::paths::same(&oni_library::paths::parent(&e.path), folder))
            .map(|e| e.path.clone())
            .collect()
    }
}

/// The family a stock names written on a frame; a black-and-white stock leaves it alone.
pub(super) fn dyed(settings: &mut PipelineSettings, family: Option<SeparationFamily>) {
    if let Some(family) = family {
        settings.separation_family = Some(family);
    }
}

/// The name the Roll tab's Dyes tabs give a family.
fn family_name(family: SeparationFamily) -> &'static str {
    match family {
        SeparationFamily::KodakC41 => tr("Kodak"),
        SeparationFamily::FujiC41 => tr("Fuji"),
        SeparationFamily::GenericC41 => tr("Generic"),
        SeparationFamily::Ecn2 => tr("ECN-2"),
        SeparationFamily::E6 => tr("E-6"),
    }
}

/// The quiet note under a roll's stock naming the dyes it writes, empty for none.
pub(super) fn dyes_note(stock: Option<&str>) -> String {
    stock.and_then(dye_family).map(|f| tr!("Dyes: {family}", family = family_name(f))).unwrap_or_default()
}

impl Ctl {
    /// `setFilmStock`, and beyond Swift the dye family it names on every frame of the roll: one
    /// gallery step with the marker, a family picked by hand overwritten, the dose kept.
    pub(in crate::app) fn set_roll_stock(self: &Rc<Self>, id: &str) {
        self.update_roll(tr("Set Film Stock"), |f| f.stock = Some(id.to_string()));
    }

    /// The stock of a roll New Roll just made writes its dyes on the frames moved in.
    pub(in crate::app) fn dyes_follow(self: &Rc<Self>, folder: &str, stock: &str) {
        let Some(family) = dye_family(stock) else { return };
        let members = self.state.borrow().folder_frames(folder);
        let label = tr("Set Film Stock");
        self.gesture_as(label, &members, |store| store.lay_roll(&members, None, |s| dyed(s, Some(family)), label));
    }

    /// A roll marker a gesture, an undo or a redo wrote: the folder's roll in the gallery and the navigator.
    pub(in crate::app) fn marker_landed(self: &Rc<Self>, folder: &str, film: Option<Film>) {
        let made = film.is_some();
        self.roll_written(folder, film);
        self.state.borrow_mut().nav.roll_edited(folder, made);
        self.lay_archive();
        self.publish_navigator();
    }

    /// Whether `roll` has a measured frame, read off this thread once per roll; `None` while it reads.
    /// A slide is never measured, so a roll holding one asks for no measure.
    pub(super) fn roll_measured(&self, key: String, frames: &[String]) -> Option<bool> {
        let mosaic = {
            let mut s = self.state.borrow_mut();
            if let Some((held, measured)) = &s.org.measured
                && *held == key
            {
                return *measured;
            }
            s.org.measured = Some((key.clone(), None));
            s.store.editor().map(|e| e.mosaic_rule())
        }?;
        let frames = frames.to_vec();
        let _ = std::thread::Builder::new().name("reading a roll's measure".into()).spawn(move || {
            let measured = frames.iter().any(|p| {
                let path = std::path::Path::new(p);
                oni_settings::sidecar::read(path, mosaic(path), false).is_ok_and(|l| measured_or_slide(&l.settings))
            });
            super::super::with_shell(move |c| c.measured_landed(&key, measured));
        });
        None
    }

    fn measured_landed(&self, key: &str, measured: bool) {
        {
            let mut s = self.state.borrow_mut();
            match &mut s.org.measured {
                Some((held, slot)) if held == key => *slot = Some(measured),
                _ => return,
            }
        }
        self.publish_info();
    }
}

/// A frame the roll card counts as needing no measure: measured with its roll, or a slide.
pub(crate) fn measured_or_slide(s: &oni_settings::PipelineSettings) -> bool {
    s.roll_reference.is_some() || s.mode == oni_settings::ConversionMode::Positive
}

/// Whether a folder's roll asks for Prepare Roll as its card would say: a photo lies in it, out of
/// the ONI Trash, and none is measured or a slide. Reads the disk; a measured frame ends the look.
pub(in crate::app) fn unprepared(folder: &std::path::Path) -> bool {
    let Ok(listing) = std::fs::read_dir(folder) else { return false };
    let mut held = false;
    for path in listing.flatten().map(|e| e.path()) {
        let hidden = path.file_name().is_none_or(|n| n.to_string_lossy().starts_with('.'));
        if hidden || !oni_library::library::is_importable(&path) || !path.is_file() {
            continue;
        }
        let Ok(loaded) = oni_settings::sidecar::read(&path, oni_decode::decodes_via_mosaic(&path), false) else { continue };
        if loaded.trashed_at.is_some() {
            continue;
        }
        if measured_or_slide(&loaded.settings) {
            return false;
        }
        held = true;
    }
    held
}

#[cfg(test)]
mod slide_tests {
    use super::measured_or_slide;
    use oni_settings::{ConversionMode, PipelineSettings};

    #[test]
    fn a_slide_never_asks_its_roll_for_a_measure() {
        assert!(measured_or_slide(&PipelineSettings::rest(ConversionMode::Positive, true)));
        // Twin: an unmeasured negative still raises the banner.
        assert!(!measured_or_slide(&PipelineSettings::rest(ConversionMode::Negative, true)));
    }
}
