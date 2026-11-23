//! The info sidebar's roll card, beyond Swift: a folder of photos is a roll, its stock is written on every frame, the
//! card reads the stock its frames' negatives are read through, and an unmeasured roll says so.

use super::*;
use crate::app::roll_tab::dye_set_name;
use oni_settings::dyes::dye_set;
use oni_settings::{DyeSet, PipelineSettings};

/// What the roll card reads off its frames: whether one is measured, and the stocks their negatives are read through,
/// in the stocks' order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Frames {
    pub measured: bool,
    pub sets: Vec<DyeSet>,
}

impl Frames {
    fn read(settings: impl Iterator<Item = PipelineSettings>) -> Frames {
        let mut out = Frames::default();
        for s in settings {
            out.measured |= measured_or_slide(&s);
            if let Some(set) = read_through(&s).filter(|set| !out.sets.contains(set)) {
                out.sets.push(set);
            }
        }
        out.sets.sort_by_key(|s| DyeSet::all().iter().position(|t| t == s));
        out
    }
}

/// The scanned stock Lighting reads a frame's negative back through, as the pipeline runs it: a colour negative's, none
/// in black and white, on a slide or on a digital photo, whose virtual film is its own.
fn read_through(s: &PipelineSettings) -> Option<DyeSet> {
    match oni_pipeline::negative::Film::of(s) {
        oni_pipeline::negative::Film::Dyes(set) => Some(set),
        _ => None,
    }
}

impl State {
    /// What of the open photo the roll card reads, in memory: its stock and whether it is measured.
    /// A hand moving them moves its roll's with them.
    pub(in crate::app) fn open_bearing(&self) -> String {
        let s = self.store.settings();
        format!("{:?}", self.open_path().map(|p| (p, s.dye_set, read_through(s), measured_or_slide(s))))
    }

    /// The photos a folder's marker gathers: every frame lying in it, passes and trashed photos aside.
    pub(in crate::app) fn folder_frames(&self, folder: &str) -> Vec<String> {
        self.rows()
            .iter()
            .filter(|e| e.pass_of.is_none() && e.trashed_at.is_none() && oni_library::paths::same(&oni_library::paths::parent(&e.path), folder))
            .map(|e| e.path.clone())
            .collect()
    }
}

/// The scanned stock a roll's stock names written on a frame; a black-and-white stock or a slide's leaves it alone.
pub(in crate::app) fn dyed(settings: &mut PipelineSettings, stock: Option<&str>) {
    if let Some(set) = stock.and_then(dye_set) {
        settings.dye_set = set;
    }
}

/// The card's words under a roll's stock, then its fix key's: the stock the frames are read through, else a warning and
/// the roll's stock offered when they differ; both empty while the frames read or none is read through a stock.
pub(super) fn dyes_note(stock: Option<&str>, held: Option<&Frames>) -> (String, String) {
    let Some(held) = held.filter(|h| !h.sets.is_empty()) else { return Default::default() };
    let names = |sets: &mut dyn Iterator<Item = &DyeSet>| sets.map(|s| dye_set_name(*s)).collect::<Vec<_>>().join(", ");
    match stock.and_then(dye_set) {
        Some(named) if held.sets != [named] => (
            tr!("Frames read as {family}, but this film stock is {named}", family = names(&mut held.sets.iter().filter(|s| **s != named)), named = dye_set_name(named)),
            tr!("Read as {named}", named = dye_set_name(named)),
        ),
        _ => (tr!("Frames read as {stocks}", stocks = names(&mut held.sets.iter())), String::new()),
    }
}

impl Ctl {
    /// `setFilmStock`, and beyond Swift the stock it names on every frame of the roll: one gallery step with the
    /// marker.
    pub(in crate::app) fn set_roll_stock(self: &Rc<Self>, id: &str) {
        self.update_roll(tr("Set Film Stock"), |f| f.stock = Some(id.to_string()));
    }

    /// The stock of a roll New Roll just made is written on the frames moved in.
    pub(in crate::app) fn dyes_follow(self: &Rc<Self>, folder: &str, stock: &str) {
        if dye_set(stock).is_none() {
            return;
        }
        let members = self.state.borrow().folder_frames(folder);
        self.lay_dyes(&members, stock, tr("Set Film Stock"));
    }

    /// The card's fix: the roll's stock written again on every frame of the roll.
    pub(in crate::app) fn stock_dyes_laid(self: &Rc<Self>) {
        let Some(roll) = self.state.borrow().roll() else { return };
        let Some(stock) = roll.film.stock.clone() else { return };
        let Some(set) = dye_set(&stock) else { return };
        let named = dye_set_name(set);
        let members = match &roll.marker {
            Some(folder) => self.state.borrow().folder_frames(folder),
            None => roll.paths,
        };
        self.lay_dyes(&members, &stock, &tr!("Read as {named}", named = named));
    }

    /// The scanned stock `stock` names written on `members` as one gallery step.
    fn lay_dyes(self: &Rc<Self>, members: &[String], stock: &str, label: &str) {
        self.gesture_as(label, members, |store| store.lay_roll(members, None, |s| dyed(s, Some(stock)), label));
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
            let open = s.open_path().map(str::to_string).map(|p| (p, s.store.settings().clone()));
            (key, s.store.mosaic_rule(), open, last)
        };
        let frames = frames.to_vec();
        let _ = std::thread::Builder::new().name("reading a roll's frames".into()).spawn(move || {
            let settings = frames.iter().filter_map(|p| match &open {
                Some((at, settings)) if at == p => Some(settings.clone()),
                _ => {
                    let path = std::path::Path::new(p);
                    oni_settings::sidecar::read(path, mosaic(path)).ok().map(|l| l.settings)
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

/// A frame the roll card counts as needing no measure: measured with its roll, a slide or a digital photo.
pub(crate) fn measured_or_slide(s: &oni_settings::PipelineSettings) -> bool {
    s.roll_reference.is_some() || !s.mode.is_measured()
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
        let Ok(loaded) = oni_settings::sidecar::read(&path, oni_decode::decodes_via_mosaic(&path)) else { continue };
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
    use oni_settings::{ConversionMode, DyeSet, PipelineSettings};

    fn negative(set: DyeSet) -> PipelineSettings {
        PipelineSettings { dye_set: set, ..PipelineSettings::rest(ConversionMode::Negative, true) }
    }

    #[test]
    fn the_card_reads_the_stocks_the_frames_are_read_through_in_their_order() {
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        let grey = PipelineSettings { dye_set: DyeSet::of("fujifilm-c200"), ..PipelineSettings::rest(ConversionMode::Monochrome, true) };
        let read = Frames::read([negative(DyeSet::of("kodak-gold-200")), negative(DyeSet::of("generic-c41")), negative(DyeSet::of("generic-c41")), slide, grey].into_iter());
        assert_eq!(read.sets, [DyeSet::of("generic-c41"), DyeSet::of("kodak-gold-200")], "a slide and a grey are read through none");
        assert!(read.measured, "a slide asks for no measure");
        // Twin: a digital photo reads its own virtual film, whatever stock it names.
        let digital = PipelineSettings { dye_set: DyeSet::of("fujifilm-c200"), ..PipelineSettings::rest(ConversionMode::Digital, true) };
        assert_eq!(Frames::read([digital, negative(DyeSet::of("kodak-gold-200"))].into_iter()).sets, [DyeSet::of("kodak-gold-200")]);
        // Twin: no slide, the negatives unmeasured, leaves the roll unmeasured.
        assert!(!Frames::read([negative(DyeSet::of("kodak-gold-200"))].into_iter()).measured);
    }

    #[test]
    fn the_card_warns_of_frames_off_the_rolls_stock_and_offers_it() {
        let fujicolor = Some("fujifilm-fujicolor");
        let quiet = |text: &str| (text.to_string(), String::new());
        let on = |sets: &[DyeSet]| Frames { sets: sets.to_vec(), ..Frames::default() };
        assert_eq!(dyes_note(fujicolor, Some(&on(&[DyeSet::of("fujifilm-c200")]))), quiet("Frames read as Fujifilm C200"));
        // Twin: frames on Gold under a Fujicolor stock warn, naming both, and offer the stock's.
        let warned = ("Frames read as Kodak Gold 200, but this film stock is Fujifilm C200".to_string(), "Read as Fujifilm C200".to_string());
        assert_eq!(dyes_note(fujicolor, Some(&on(&[DyeSet::of("kodak-gold-200")]))), warned);
        assert_eq!(dyes_note(fujicolor, Some(&on(&[DyeSet::of("kodak-gold-200"), DyeSet::of("fujifilm-c200")]))), warned, "only the frames off the stock's are named");
        assert_eq!(dyes_note(None, Some(&on(&[DyeSet::of("generic-c41")]))), quiet("Frames read as Generic C-41"));
        assert_eq!(dyes_note(fujicolor, None), quiet(""), "nothing is said while the frames read");
        assert_eq!(dyes_note(fujicolor, Some(&on(&[]))), quiet(""), "nor when no frame is read through a stock");
    }

    #[test]
    fn a_stock_writes_its_measured_dyes_and_black_and_white_none() {
        let mut s = negative(DyeSet::of("kodak-gold-200"));
        super::dyed(&mut s, Some("fujifilm-superia"));
        assert_eq!(s.dye_set, DyeSet::of("fujifilm-xtra-400"));
        super::dyed(&mut s, Some("lucky-color200"));
        assert_eq!(s.dye_set, DyeSet::of("generic-c41"), "a stock no profile measures lays the generic set");
        // Twin: a black-and-white stock leaves the set as it was.
        let before = s.clone();
        super::dyed(&mut s, Some("ilford-hp5"));
        assert_eq!(s, before);
    }

    #[test]
    fn a_slide_never_asks_its_roll_for_a_measure() {
        assert!(measured_or_slide(&PipelineSettings::rest(ConversionMode::Positive, true)));
        // Twin: an unmeasured negative still raises the banner.
        assert!(!measured_or_slide(&PipelineSettings::rest(ConversionMode::Negative, true)));
    }
}
