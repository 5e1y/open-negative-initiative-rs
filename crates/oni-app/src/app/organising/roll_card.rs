//! The info sidebar's roll card, beyond Swift: a folder of photos is a roll, its stock writes the
//! dye family it names on every frame, the card reads the dyes its frames unmix, and an unmeasured
//! roll says so.

use super::*;
use crate::app::roll_tab::{FAMILIES, family_name};
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::dye_family;
use oni_settings::PipelineSettings;

/// What the roll card reads off its frames: whether one is measured, and the dyes they unmix in
/// the Dyes tabs' order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Frames {
    pub measured: bool,
    pub dyes: Vec<SeparationFamily>,
}

impl Frames {
    fn read(settings: impl Iterator<Item = PipelineSettings>) -> Frames {
        let mut out = Frames::default();
        for s in settings {
            out.measured |= measured_or_slide(&s);
            if let Some(family) = unmixed(&s).filter(|f| !out.dyes.contains(f)) {
                out.dyes.push(family);
            }
        }
        out.dyes.sort_by_key(|f| FAMILIES.iter().position(|g| g == f));
        out
    }
}

/// The family a frame's render unmixes, as the pipeline runs it: none in black and white, on a
/// slide, on RGB passes or at a dose of zero.
fn unmixed(s: &PipelineSettings) -> Option<SeparationFamily> {
    if s.mode.is_monochrome() || oni_pipeline::conversion::separation_dose(s) <= 0.0 {
        return None;
    }
    s.separation_family.or(SeparationFamily::resting(s.mode))
}

impl State {
    /// What of the open photo the roll card reads, in memory: its dyes and whether it is measured.
    /// A hand moving them moves its roll's with them.
    pub(in crate::app) fn open_bearing(&self) -> String {
        let s = self.store.settings();
        format!("{:?}", self.open.as_ref().map(|p| (p, s.separation_family, unmixed(s), measured_or_slide(s))))
    }

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

/// The card's words under a roll's stock, then its fix key's: the dyes the frames unmix, else a warning
/// and the stock's dyes offered when they differ; both empty while the frames read or none unmixes.
pub(super) fn dyes_note(stock: Option<&str>, held: Option<&[SeparationFamily]>) -> (String, String) {
    let Some(held) = held.filter(|h| !h.is_empty()) else { return Default::default() };
    match stock.and_then(dye_family) {
        Some(named) if held != [named] => (
            tr!("Frames set to {family} dyes, but this film stock has {named} dyes", family = names(held.iter().filter(|f| **f != named)), named = family_name(named)),
            tr!("Use {named} Dyes", named = family_name(named)),
        ),
        _ => (tr!("Dyes: {family}", family = names(held.iter())), String::new()),
    }
}

fn names<'a>(families: impl Iterator<Item = &'a SeparationFamily>) -> String {
    families.map(|f| family_name(*f)).collect::<Vec<_>>().join(", ")
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
        self.lay_dyes(&members, family, tr("Set Film Stock"));
    }

    /// The card's fix: the dyes the roll's stock names written again on every frame of the roll.
    pub(in crate::app) fn stock_dyes_laid(self: &Rc<Self>) {
        let Some(roll) = self.state.borrow().roll() else { return };
        let Some(family) = roll.film.stock.as_deref().and_then(dye_family) else { return };
        let members = match &roll.marker {
            Some(folder) => self.state.borrow().folder_frames(folder),
            None => roll.paths,
        };
        self.lay_dyes(&members, family, &tr!("Use {named} Dyes", named = family_name(family)));
    }

    /// `family` written on `members` as one gallery step, a family picked by hand overwritten, the dose kept.
    fn lay_dyes(self: &Rc<Self>, members: &[String], family: SeparationFamily, label: &str) {
        self.gesture_as(label, members, |store| store.lay_roll(members, None, |s| dyed(s, Some(family)), label));
    }

    /// A roll marker a gesture, an undo or a redo wrote: the folder's roll in the gallery and the navigator.
    pub(in crate::app) fn marker_landed(self: &Rc<Self>, folder: &str, film: Option<Film>) {
        let made = film.is_some();
        self.roll_written(folder, film);
        self.state.borrow_mut().nav.roll_edited(folder, made);
        self.lay_archive();
        self.publish_navigator();
    }

    /// What `roll`'s frames say, read off this thread once per roll and per bearing of the open
    /// photo, which counts by its settings in memory, and again once a write may have changed them;
    /// `None` while a first read runs. A slide is never measured, so a roll holding one asks for no measure.
    pub(super) fn roll_frames(&self, roll: &str, frames: &[String]) -> Option<Frames> {
        let (key, mosaic, open, last) = {
            let mut s = self.state.borrow_mut();
            let key = (roll.to_string(), s.open_bearing());
            if let Some((held, read)) = &s.org.frames
                && *held == key
                && !s.org.frames_due
            {
                return read.clone();
            }
            // The roll's last read stays on its card while the next runs, so a medal never blanks it.
            let last = s.org.frames.as_ref().filter(|(held, _)| held.0 == key.0).and_then(|(_, read)| read.clone());
            s.org.frames = Some((key.clone(), last.clone()));
            s.org.frames_due = false;
            let open = s.open.clone().map(|p| (p, s.store.settings().clone()));
            (key, s.store.editor().map(|e| e.mosaic_rule())?, open, last)
        };
        let frames = frames.to_vec();
        let _ = std::thread::Builder::new().name("reading a roll's frames".into()).spawn(move || {
            let settings = frames.iter().filter_map(|p| match &open {
                Some((at, settings)) if at == p => Some(settings.clone()),
                _ => {
                    let path = std::path::Path::new(p);
                    oni_settings::sidecar::read(path, mosaic(path), false).ok().map(|l| l.settings)
                }
            });
            let read = Frames::read(settings);
            super::super::with_shell(move |c| c.frames_landed(&key, read));
        });
        last
    }

    fn frames_landed(&self, key: &(String, String), read: Frames) {
        {
            let mut s = self.state.borrow_mut();
            match &mut s.org.frames {
                Some((held, slot)) if held == key => *slot = Some(read),
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
mod tests {
    use super::{Frames, dyes_note, measured_or_slide};
    use oni_settings::blocks::SeparationFamily::{self, FujiC41, GenericC41, KodakC41};
    use oni_settings::{ConversionMode, PipelineSettings};

    fn negative(family: Option<SeparationFamily>) -> PipelineSettings {
        PipelineSettings { separation_family: family, ..PipelineSettings::rest(ConversionMode::Negative, true) }
    }

    #[test]
    fn the_card_reads_the_dyes_the_frames_unmix_in_the_tabs_order() {
        let slide = PipelineSettings { separation_family: Some(SeparationFamily::E6), ..PipelineSettings::rest(ConversionMode::Positive, true) };
        let grey = PipelineSettings { separation_family: Some(FujiC41), ..PipelineSettings::rest(ConversionMode::Monochrome, true) };
        let read = Frames::read([negative(None), negative(Some(KodakC41)), negative(Some(KodakC41)), slide, grey].into_iter());
        assert_eq!(read.dyes, [KodakC41, GenericC41], "a frame naming none unmixes its mode's; a slide and a grey none");
        assert!(read.measured, "a slide asks for no measure");
        // Twin: a frame dosed at zero by hand unmixes nothing, and no slide leaves the roll unmeasured.
        let zero = PipelineSettings { separation: 0.0, separation_by_hand: true, ..negative(Some(FujiC41)) };
        assert_eq!(Frames::read([zero, negative(Some(KodakC41))].into_iter()), Frames { measured: false, dyes: vec![KodakC41] });
    }

    #[test]
    fn the_card_warns_of_dyes_off_the_stocks_and_offers_the_stocks() {
        let fujicolor = Some("fujifilm-fujicolor");
        let quiet = |text: &str| (text.to_string(), String::new());
        assert_eq!(dyes_note(fujicolor, Some(&[FujiC41])), quiet("Dyes: Fuji"));
        // Twin: frames on Kodak under a Fuji stock warn, naming both, and offer the stock's.
        let warned = ("Frames set to Kodak dyes, but this film stock has Fuji dyes".to_string(), "Use Fuji Dyes".to_string());
        assert_eq!(dyes_note(fujicolor, Some(&[KodakC41])), warned);
        assert_eq!(dyes_note(fujicolor, Some(&[KodakC41, FujiC41])), warned, "only the frames off the stock's are named");
        assert_eq!(dyes_note(None, Some(&[GenericC41])), quiet("Dyes: Generic"));
        assert_eq!(dyes_note(fujicolor, None), quiet(""), "nothing is said while the frames read");
        assert_eq!(dyes_note(fujicolor, Some(&[])), quiet(""), "nor when no frame unmixes");
    }

    #[test]
    fn a_slide_never_asks_its_roll_for_a_measure() {
        assert!(measured_or_slide(&PipelineSettings::rest(ConversionMode::Positive, true)));
        // Twin: an unmeasured negative still raises the banner.
        assert!(!measured_or_slide(&PipelineSettings::rest(ConversionMode::Negative, true)));
    }
}
