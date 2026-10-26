//! Beyond Swift, the owner's design: the info sidebar guides photos outside any roll into one, and a
//! roll's stock writes the dye family it names on every frame. Nothing is made without a click.

use super::*;
use oni_library::gallery_history::Marker;
use oni_library::{folders, paths, roll_folder};
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::dye_family;
use oni_settings::PipelineSettings;
use crate::RollGuide;

/// The guide's draft, and the roll shown by key with whether a frame of it is measured, `None` while read.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Guide {
    draft: Draft,
    pub(in crate::app) measured: Option<(String, Option<bool>)>,
}

/// What the guide holds for the folders the selection lies in: the name typed, the stock picked.
#[derive(Default, Debug, Clone, PartialEq)]
struct Draft {
    subject: String,
    name: Option<String>,
    stock: Option<String>,
}

/// The selection the guide stands on, as it would become a roll.
struct Gathered {
    photos: Vec<String>,
    folders: Vec<String>,
    /// Every photo the gallery shows of the one folder is selected: the folder itself becomes the roll.
    whole: bool,
    /// The name New Roll gives: the earliest capture as year, month, day.
    default: String,
}

impl State {
    /// The guide's selection on the archive, none in the projects, the ONI Trash or with nothing picked.
    fn guide_pending(&self) -> Option<Gathered> {
        if self.org.projects_surface || self.trash_shown() || self.picks.anchor.is_none() {
            return None;
        }
        let photos = self.selected();
        if photos.is_empty() {
            return None;
        }
        let picked: HashSet<&String> = photos.iter().collect();
        let mut folders: Vec<String> = photos.iter().map(|p| paths::parent(p)).collect();
        folders.sort();
        folders.dedup();
        let whole = match folders.as_slice() {
            [one] => self
                .archive
                .as_ref()
                .and_then(|a| a.groups.iter().find(|g| g.folder.as_deref().is_some_and(|f| paths::same(f, one))))
                .is_some_and(|g| g.entries.iter().all(|e| picked.contains(&e.path))),
            _ => false,
        };
        let earliest = self.rows.iter().filter(|e| picked.contains(&e.path)).filter_map(|e| e.captured_at).reduce(|a, b| if b < a { b } else { a });
        Some(Gathered { photos, folders, whole, default: roll_folder::dated(earliest.unwrap_or_else(Date::now)) })
    }

    /// The photos a folder's marker gathers: every frame lying in it, passes and trashed photos aside.
    pub(in crate::app) fn folder_frames(&self, folder: &str) -> Vec<String> {
        self.rows
            .iter()
            .filter(|e| e.pass_of.is_none() && e.trashed_at.is_none() && paths::same(&paths::parent(&e.path), folder))
            .map(|e| e.path.clone())
            .collect()
    }
}

/// The family a stock names written on a frame; a black-and-white stock leaves it alone.
fn dyed(settings: &mut PipelineSettings, family: Option<SeparationFamily>) {
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
    /// Kind 0 of the sidebar: the guide while the selection lies outside any roll, else the plain line.
    pub(super) fn publish_guide(&self) {
        let Some(w) = self.window() else { return };
        let g = w.global::<RollGuide>();
        let mut s = self.state.borrow_mut();
        let Some(pending) = s.guide_pending() else {
            g.set_shown(false);
            s.org.guide.draft = Draft::default();
            return;
        };
        let subject = pending.folders.join("\n");
        if s.org.guide.draft.subject != subject {
            s.org.guide.draft = Draft { subject, ..Draft::default() };
        }
        let one = pending.folders.len() == 1;
        let count = format!(
            "{} · {}",
            trn("{n} selected", "{n} selected", pending.photos.len() as u64),
            trn("{n} folder", "{n} folders", pending.folders.len() as u64)
        );
        g.set_count(count.into());
        let name = s.org.guide.draft.name.clone().unwrap_or_else(|| pending.default.clone());
        if g.get_name().as_str() != name {
            g.set_name(name.into());
        }
        g.set_proposed(pending.default.into());
        let kind = film_stock(s.org.guide.draft.stock.as_deref());
        let artwork = kind.and_then(|k| crate::stocks::icon(k.icon_name));
        g.set_stock(kind.map(|k| k.display_name).unwrap_or_default().into());
        g.set_stock_has_artwork(artwork.is_some());
        g.set_stock_artwork(artwork.unwrap_or_default());
        g.set_key(if pending.whole { tr("Make this folder a roll") } else { tr("Move into a new roll") }.into());
        g.set_enabled(one);
        g.set_note(roll_folder::one_folder(&pending.photos).err().unwrap_or_default().into());
        g.set_shown(true);
    }

    pub(super) fn wire_guide(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let g = window.global::<RollGuide>();
        let p = me.clone();
        g.on_name_edited(move |t| p.guide_name_edited(&t));
        let p = me.clone();
        g.on_pick_stock(move || p.guide_pick_stock());
        let p = me.clone();
        g.on_create(move || p.guide_create());
        let p = me.clone();
        window.global::<Library>().on_prepare_shown_roll(move || p.prepare_shown_roll());
    }

    pub(in crate::app) fn guide_name_edited(&self, typed: &str) {
        self.state.borrow_mut().org.guide.draft.name = Some(typed.to_string());
    }

    pub(in crate::app) fn guide_pick_stock(&self) {
        let current = self.state.borrow().org.guide.draft.stock.clone();
        self.open_stock_picker(crate::app::stock_picker::StockTarget::Guide, current.as_deref());
    }

    pub(in crate::app) fn guide_stock_chosen(&self, id: &str) {
        self.state.borrow_mut().org.guide.draft.stock = Some(id.to_string());
        self.publish_info();
    }

    /// The guide's one key: the whole folder made a roll in place, else New Roll on the selection,
    /// its confirm first; both under the name and the stock typed.
    pub(in crate::app) fn guide_create(self: &Rc<Self>) {
        let (pending, guide) = {
            let s = self.state.borrow();
            let Some(pending) = s.guide_pending() else { return };
            (pending, s.org.guide.draft.clone())
        };
        let [folder] = pending.folders.as_slice() else { return };
        let name = guide.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or(pending.default);
        if pending.whole {
            self.make_roll_named(folder, &name, guide.stock);
        } else {
            let targets: HashSet<String> = pending.photos.into_iter().collect();
            self.new_roll_named(&targets, Some((name, guide.stock)));
        }
    }

    /// The folder's marker written with its name and stock, and the stock's dyes on its frames: one
    /// gallery step, no file moved, the folder keeping its own name.
    fn make_roll_named(self: &Rc<Self>, folder: &str, name: &str, stock: Option<String>) {
        let mut film = Film::new(name);
        film.stock = stock;
        let family = film.stock.as_deref().and_then(dye_family);
        let members = self.state.borrow().folder_frames(folder);
        let marker = Marker { folder: folder.to_owned(), before: folders::roll(folder), after: Some(film) };
        let label = tr("Make a Roll");
        self.gesture_as(label, &members, |store| store.lay_roll(&members, Some(marker), |s| dyed(s, family), label));
    }

    /// `setFilmStock`, and beyond Swift the dye family it names on every frame of the roll: one
    /// gallery step with the marker, a family picked by hand overwritten, the dose kept.
    pub(in crate::app) fn set_roll_stock(self: &Rc<Self>, id: &str) {
        let Some(roll) = self.state.borrow().roll() else { return };
        let mut film = roll.film.clone();
        film.stock = Some(id.to_string());
        let family = dye_family(id);
        let label = tr("Set Film Stock");
        match roll.marker.clone() {
            Some(folder) => {
                let members = self.state.borrow().folder_frames(&folder);
                let marker = Marker { folder, before: Some(roll.film.clone()), after: Some(film) };
                self.gesture_as(label, &members, |store| store.lay_roll(&members, Some(marker), |s| dyed(s, family), label));
            }
            None => {
                let frames = roll.paths.clone();
                self.gesture_as(label, &frames, |store| {
                    store.lay_roll(
                        &frames,
                        None,
                        |s| {
                            s.film = Some(film.clone());
                            dyed(s, family);
                        },
                        label,
                    )
                });
            }
        }
        self.state.borrow_mut().org.info_loaded = None;
        self.publish_info();
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
    pub(super) fn roll_measured(&self, key: String, frames: &[String]) -> Option<bool> {
        let mosaic = {
            let mut s = self.state.borrow_mut();
            if let Some((held, measured)) = &s.org.guide.measured
                && *held == key
            {
                return *measured;
            }
            s.org.guide.measured = Some((key.clone(), None));
            s.store.editor().map(|e| e.mosaic_rule())
        }?;
        let frames = frames.to_vec();
        let _ = std::thread::Builder::new().name("reading a roll's measure".into()).spawn(move || {
            let measured = frames.iter().any(|p| {
                let path = std::path::Path::new(p);
                oni_settings::sidecar::read(path, mosaic(path), false).is_ok_and(|l| l.settings.roll_reference.is_some())
            });
            super::super::with_shell(move |c| c.measured_landed(&key, measured));
        });
        None
    }

    fn measured_landed(&self, key: &str, measured: bool) {
        {
            let mut s = self.state.borrow_mut();
            match &mut s.org.guide.measured {
                Some((held, slot)) if held == key => *slot = Some(measured),
                _ => return,
            }
        }
        self.publish_info();
    }

    /// The banner's key: the toolbar's Prepare Roll on every frame of the roll shown.
    pub(in crate::app) fn prepare_shown_roll(self: &Rc<Self>) {
        let Some(roll) = self.state.borrow().roll() else { return };
        self.commit_info_drafts();
        self.roll_of(roll.paths, |c, paths, left| c.enter_roll(&paths, left));
    }
}
