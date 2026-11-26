//! The year on film: each gesture noted in the journal, and "Show My Year on Film" read back off the
//! journal, the library and its thumbnails as a strip of scenes, each one a card that can be saved.

use super::{Ctl, image, with_shell};
use crate::wrapped::{SWATCHES, Scene, gather, scenes, share};
use crate::{Wrapped, WrappedKind, WrappedLine, WrappedScene, WrappedSwatch};
use oni_i18n::{tr, trn};
use oni_library::journal::{Journal, Kind};
use oni_library::wrapped::{Darkroom, Frames, Moment, Palette, today};
use oni_settings::{Medal, PipelineSettings};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// `@AppStorage("wrappedShownYear")`: December offers the year once.
pub const SHOWN_YEAR_KEY: &str = "wrappedShownYear";
/// A full sample year instead of the journal's, to look at the summary before living one.
pub const SAMPLE: &str = "ONI_WRAPPED_SAMPLE";
/// What the strip's edge prints after the year, and who signs a frame: marks, the same in every language.
const EDGE: &str = "SAFETY FILM";
/// What a posted card is tagged with, and the account it names: on every card, in every language.
pub const HASHTAG: &str = "#madewithoni";
pub const ACCOUNT: &str = "@open.negative.initiative";
/// A print's long side, enough for a frame as large as a tall window draws it.
const PRINT_SIDE: usize = 1024;
/// A scene showing more photos than this shows them small: their thumbnails are enough.
const PRINTS_A_SCENE: usize = 4;

/// Whether December's offer is still to make: in December, for a year not shown yet.
pub fn offer_due((year, month): (i32, u32), shown: i64) -> bool {
    month == 12 && shown < i64::from(year)
}

/// The year "Show My Year on Film" tells: the one under way, or in January the one just ended.
pub fn told_year((year, month): (i32, u32)) -> i32 {
    if month == 1 { year - 1 } else { year }
}

#[derive(Default)]
pub struct Year {
    /// `JournalEdits.last`: each photo's settings at its last settled edit.
    last: HashMap<String, PipelineSettings>,
    /// The summary while it shows: the year it tells, its scenes and the one in the gate.
    story: Option<Story>,
    /// A showing asked, its year still being read off the disk.
    reading: bool,
    /// Counts the showings and their ends: a photo landing for a summary since closed is dropped.
    told: Arc<AtomicUsize>,
    /// The launch's offer, waiting for the opened folder's rows as Swift's waits for `fillOpened`.
    offer_due: bool,
}

/// The summary while it shows: the year it tells, its scenes, the one in the gate, and whether the
/// story is over and its cards laid out to share.
struct Story {
    year: i32,
    scenes: Vec<Scene>,
    index: usize,
    sharing: bool,
}

fn medal_rank(medal: Medal) -> u16 {
    match medal {
        Medal::None => 0,
        Medal::Bronze => 1,
        Medal::Silver => 2,
        Medal::Gold => 3,
    }
}

/// The year's journal in `support`; a bench drags the owner's real roll, so its gestures go to a
/// scratch journal instead, as Swift's checks write under a stand-in folder.
pub(super) fn journal_in(support: &std::path::Path) -> std::sync::Arc<Journal> {
    if crate::bench::requested().is_some() {
        return Journal::in_support(&std::env::temp_dir().join(format!("oni-bench-journal-{}", std::process::id())));
    }
    Journal::in_support(support)
}

impl Ctl {
    fn journal(&self) -> std::sync::Arc<Journal> {
        journal_in(&self.state.borrow().support)
    }

    fn note(&self, kind: Kind, subject: Option<&str>, detail: u16, value: u32, name: Option<&str>) {
        self.journal().note(kind, subject, detail, value, name, oni_settings::Date::now());
    }

    /// `load(_:)`: the photo opened, its settings the ground its first edit is read against.
    pub(super) fn journal_opened(&self, path: &str) {
        self.note(Kind::Opened, Some(path), 0, 0, None);
        let settings = self.state.borrow().store.settings().clone();
        self.state.borrow_mut().year.last.insert(path.to_string(), settings);
    }

    /// `JournalEdits.settled`: one record per settings row the gesture moved, never one per notch.
    pub(super) fn journal_settled(&self) {
        let (path, settings, before) = {
            let s = self.state.borrow();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let settings = s.store.settings().clone();
            let before = s.year.last.get(&path).cloned();
            (path, settings, before)
        };
        if let Some(before) = before.filter(|b| *b != settings) {
            for (rank, row) in oni_core::parameters::all().iter().enumerate() {
                let mut probe = before.clone();
                (row.carry)(&settings, &mut probe);
                if probe != before {
                    self.note(Kind::Edited, Some(&path), rank as u16, 0, None);
                }
            }
        }
        self.state.borrow_mut().year.last.insert(path, settings);
    }

    pub(super) fn journal_medal(&self, paths: &[String], medal: Medal) {
        for path in paths {
            self.note(Kind::Medal, Some(path), medal_rank(medal), 0, None);
        }
    }

    pub(super) fn journal_preset(&self, paths: &[String], name: &str) {
        for path in paths {
            self.note(Kind::Preset, Some(path), 0, 0, Some(name));
        }
    }

    /// `.pasted` and `.rolled` count the photos the gesture reached.
    pub(super) fn journal_count(&self, kind: Kind, count: usize) {
        self.note(kind, None, 0, count as u32, None);
    }

    pub(super) fn journal_balanced(&self) {
        self.note(Kind::Balanced, None, 0, 0, None);
    }

    pub(super) fn wire_wrapped(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let w = window.global::<Wrapped>();
        let p = me.clone();
        w.on_forward(move || p.wrapped_turn(1));
        let p = me.clone();
        w.on_back(move || p.wrapped_turn(-1));
        let p = me.clone();
        w.on_close(move || p.close_wrapped());
        let p = me.clone();
        w.on_save_all(move || p.save_wrapped(None));
        let p = me.clone();
        w.on_save_card(move |scene| p.save_wrapped(Some(scene.max(0) as usize)));
        let p = me.clone();
        w.on_copy_card(move |scene| p.copy_wrapped(scene.max(0) as usize));
        w.on_counted(|n| crate::wrapped::figure(n.max(0) as usize).into());
    }

    /// The launch's offer once the opened folder is read, at once when none is being read.
    pub fn offer_wrapped_when_read(self: &Rc<Self>) {
        let reading = {
            let s = self.state.borrow();
            s.opened().is_some() && !s.read_done()
        };
        if reading {
            self.state.borrow_mut().year.offer_due = true;
        } else {
            self.offer_wrapped();
        }
    }

    /// The launch's offer made now if it waited for the rows just read.
    pub(super) fn wrapped_after_read(self: &Rc<Self>) {
        let due = std::mem::take(&mut self.state.borrow_mut().year.offer_due);
        if due {
            self.offer_wrapped();
        }
    }

    fn shown_year(&self) -> i64 {
        self.state.borrow().preferences.value(SHOWN_YEAR_KEY).and_then(serde_json::Value::as_i64).unwrap_or(0)
    }

    /// In December, once a year, never under a bench, and never at the launch that shows the welcome,
    /// which gives the year itself.
    pub fn offer_wrapped(self: &Rc<Self>) {
        if offer_due(today(), self.shown_year()) && !self.welcome_due() {
            self.show_wrapped(today().0);
        }
    }

    /// `year` read off this thread: the journal, the library's rows and their sidecars, their thumbnails'
    /// colours. The ONI Trash's photos count for nothing.
    pub fn show_wrapped(self: &Rc<Self>, year: i32) {
        // A year shown in its own December, by the offer, the menu or the welcome, is the offer made.
        if year == today().0 && offer_due(today(), self.shown_year()) {
            self.remember(SHOWN_YEAR_KEY, year.into());
        }
        let (rows, support) = {
            let mut s = self.state.borrow_mut();
            if s.year.story.is_some() || s.year.reading {
                return;
            }
            s.year.reading = true;
            (s.index.as_ref().map(|i| i.all()).unwrap_or_default(), s.support.clone())
        };
        // The person's own journal even under a bench, whose gestures a scratch one takes: a year is only read.
        let journal = Journal::in_support(&support);
        let sample = std::env::var_os(SAMPLE).is_some();
        std::thread::spawn(move || {
            let asked = std::time::Instant::now();
            let mut steps = Vec::new();
            let mut step = |name: &str, since: &mut std::time::Instant| {
                steps.push(format!("{name} {:.0}", since.elapsed().as_secs_f64() * 1000.0));
                *since = std::time::Instant::now();
            };
            let mut since = asked;
            let hidden: HashSet<String> = oni_library::index::trashed(&rows).into_iter().map(|e| e.path).collect();
            let shown = oni_library::index::shown(&rows);
            let paths: Vec<String> = shown.iter().map(|e| e.path.clone()).collect();
            let markers = oni_library::folders::rolls(&paths);
            step("rolls", &mut since);
            let told = if sample {
                crate::wrapped::sample::year(year, gather::frames(&shown, &markers, None), &support)
            } else {
                let frames = Frames::build(&gather::frames(&shown, &markers, Some(year)), year, Moment::local);
                step("sidecars", &mut since);
                let palette = Palette::of(&gather::tones(&support, &frames.paths), SWATCHES);
                step("colours", &mut since);
                let (records, names) = journal.read();
                let darkroom = Darkroom::build(&records, &names, year, &hidden, |time| Moment::local(Moment::journal_unix(time)));
                step("journal", &mut since);
                oni_library::wrapped::Year::new(year, darkroom, frames, palette)
            };
            let read = format!("{:.0} ms ({})", asked.elapsed().as_secs_f64() * 1000.0, steps.join(", "));
            crate::timing::line(&format!("wrapped: {} frames of {} and {} thumbnails read in {read}", told.frames.count, shown.len(), told.palette.sampled));
            with_shell(move |c| c.present_wrapped(told));
        });
    }

    fn present_wrapped(self: &Rc<Self>, told: oni_library::wrapped::Year) {
        self.state.borrow_mut().year.reading = false;
        let Some(w) = self.window() else { return };
        let story = scenes(&told);
        let g = w.global::<Wrapped>();
        g.set_year(told.year.to_string().into());
        g.set_edge(format!("ONI {}   {EDGE}", told.year).into());
        g.set_hashtag(HASHTAG.into());
        g.set_account(ACCOUNT.into());
        g.set_sharing(false);
        g.set_notice(SharedString::default());
        g.set_index(0);
        g.set_still(false);
        g.set_playing(true);
        g.set_scenes(ModelRc::new(VecModel::from(story.iter().map(face).collect::<Vec<_>>())));
        g.set_showing(g.get_showing() + 1);
        g.set_shown(true);
        // A photo's thumbnail is named by its print: the year's frames carry theirs, the opened folder's the rest.
        let prints: HashMap<&str, &str> = told.frames.paths.iter().map(|(p, f)| (p.as_str(), f.as_str())).collect();
        let wanted: Vec<(usize, usize, String)> = {
            let s = self.state.borrow();
            let print = |path: &String| prints.get(path.as_str()).map(|f| f.to_string()).or_else(|| s.fingerprints.get(path).cloned());
            story.iter().enumerate().flat_map(|(i, scene)| scene.photos.iter().enumerate().map(move |(j, p)| (i, j, p))).filter_map(|(i, j, p)| print(p).map(|f| (i, j, f))).collect()
        };
        let large: Vec<(usize, usize, String)> =
            story.iter().enumerate().filter(|(_, scene)| scene.photos.len() <= PRINTS_A_SCENE).flat_map(|(i, scene)| scene.photos.iter().enumerate().map(move |(j, p)| (i, j, p.clone()))).collect();
        let (support, screen, previews, showing) = {
            let mut s = self.state.borrow_mut();
            s.year.story = Some(Story { year: told.year, scenes: story, index: 0, sharing: false });
            (s.support.clone(), s.screen, s.previews.clone(), s.year.told.clone())
        };
        self.publish_bar();
        let engine = self.engine.clone();
        let mine = showing.fetch_add(1, Ordering::Relaxed) + 1;
        // The thumbnails on disk first; then the scenes' larger photos developed again as prints, each
        // thumbnail standing until its print lands.
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let current = || showing.load(Ordering::Relaxed) == mine;
            for (scene, slot, fingerprint) in wanted {
                let Some((rgba, width, height)) = crate::thumbnails::load(&support, &fingerprint, screen) else { continue };
                with_shell(move |c| c.wrapped_photo(mine, scene, slot, image(&rgba, width, height)));
            }
            let mut printed: HashMap<String, (Vec<u8>, usize, usize)> = HashMap::new();
            for (scene, slot, path) in large {
                if !current() {
                    return;
                }
                if !printed.contains_key(&path)
                    && let Some(print) = super::workers::print(&engine, &previews, &path, screen, PRINT_SIDE)
                {
                    printed.insert(path.clone(), print);
                }
                if let Some((rgba, width, height)) = printed.get(&path).cloned() {
                    with_shell(move |c| c.wrapped_photo(mine, scene, slot, image(&rgba, width, height)));
                }
            }
        });
    }

    fn wrapped_photo(&self, showing: usize, scene: usize, slot: usize, picture: Image) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        if !g.get_shown() || self.state.borrow().year.told.load(Ordering::Relaxed) != showing {
            return;
        }
        use slint::Model;
        if let Some(face) = g.get_scenes().row_data(scene) {
            face.photos.set_row_data(slot, picture);
        }
    }

    /// The film advanced or wound back a frame; past the last scene every card is laid out to share,
    /// and past that the summary closes.
    fn wrapped_turn(&self, by: i32) {
        let Some((count, index, sharing)) = self.state.borrow().year.story.as_ref().map(|s| (s.scenes.len() as i32, s.index as i32, s.sharing)) else { return };
        let next = index + by;
        if sharing && by > 0 {
            return self.close_wrapped();
        }
        if next < 0 {
            return;
        }
        let share = !sharing && next >= count;
        if let Some(story) = self.state.borrow_mut().year.story.as_mut() {
            story.sharing = share;
            if !sharing && !share {
                story.index = next as usize;
            }
        }
        if let Some(w) = self.window() {
            let g = w.global::<Wrapped>();
            g.set_notice(SharedString::default());
            g.set_sharing(share);
            if !sharing && !share {
                g.set_index(next);
            }
        }
    }

    /// "Save All Cards", or one card: written into the year's folder in Downloads with no question asked,
    /// then shown there, and said under the keys.
    fn save_wrapped(self: &Rc<Self>, scene: Option<usize>) {
        let Some(year) = self.state.borrow().year.story.as_ref().map(|s| s.year) else { return };
        self.wrapped_notice(tr("Saving your cards…"));
        let folder = share::folder(&crate::system::downloads(), year);
        let shown = folder.clone();
        self.save_wrapped_into(&folder, scene, move |written| {
            with_shell(move |c| {
                let notice = match &written {
                    Ok(saved) => {
                        match saved.as_slice() {
                            [one] => crate::system::reveal(one),
                            _ => crate::system::show_folder(&shown),
                        }
                        let cards = trn("{n} card saved", "{n} cards saved", saved.len() as u64);
                        tr!("{cards} in Downloads, in {folder}", cards = cards, folder = oni_i18n::named(&share::folder_name(year)))
                    }
                    Err(e) => tr!("The cards cannot be saved: {e}", e = e),
                };
                c.wrapped_notice(&notice);
            });
        });
    }

    fn wrapped_notice(&self, text: &str) {
        if let Some(w) = self.window() {
            w.global::<Wrapped>().set_notice(oni_i18n::spoken(text).into());
        }
    }

    /// A scene's card, 1080 by 1920, as the window draws it: the card laid alone and told whole at its
    /// full size, the window's own picture taken a view's worth at a time and laid together. The story
    /// is back where it stood before a frame is drawn.
    fn card_picture(&self, scene: usize) -> Result<Vec<u8>, String> {
        let Some(index) = self.state.borrow().year.story.as_ref().map(|s| s.index) else { return Err(tr("the summary is closed").into()) };
        let Some(w) = self.window() else { return Err(tr("the summary is closed").into()) };
        let scale = w.window().scale_factor();
        let g = w.global::<Wrapped>();
        // The view's place in the window's picture, on whole pixels.
        let px = |points: f32| (points * scale).round().max(0.0) as usize;
        let view = ((px(g.get_view_x()), px(g.get_view_y())), (px(g.get_view_width()), px(g.get_view_height())));
        let tiles = share::tiles(share::CARD, view.1);
        if tiles.is_empty() {
            return Err(tr("the window is too small").into());
        }
        g.set_still_width(share::CARD.0 as f32 / scale);
        g.set_still(true);
        g.set_index(scene as i32);
        let mut card = vec![0u8; share::CARD.0 * share::CARD.1 * 4];
        let taken = tiles.iter().try_for_each(|(x, y)| {
            g.set_still_x(-(*x as f32) / scale);
            g.set_still_y(-(*y as f32) / scale);
            let shot = w.window().take_snapshot().map_err(|e| e.to_string())?;
            share::lay(&mut card, share::CARD, (*x, *y), shot.as_bytes(), shot.width() as usize, view);
            Ok(())
        });
        g.set_index(index as i32);
        g.set_still(false);
        taken.map(|()| card)
    }

    /// One scene's card, or every scene's, taken off the window one card a turn of the interface's loop,
    /// so the window stays live; each is brought to sRGB, encoded and written into `folder` off this
    /// thread as it is taken, and `done` told there what was written.
    pub fn save_wrapped_into(self: &Rc<Self>, folder: &Path, scene: Option<usize>, done: impl FnOnce(Result<Vec<PathBuf>, String>) + Send + 'static) {
        let Some((year, count)) = self.state.borrow().year.story.as_ref().map(|s| (s.year, s.scenes.len())) else { return done(Ok(Vec::new())) };
        let mut scenes = scene.map_or(0..count, |one| one.min(count.saturating_sub(1))..(one + 1).min(count));
        let (folder, screen) = (folder.to_path_buf(), self.state.borrow().screen);
        let (cards, taken) = std::sync::mpsc::channel::<Result<(usize, Vec<u8>), String>>();
        std::thread::spawn(move || {
            let made = std::fs::create_dir_all(&folder).map_err(|e| e.to_string());
            done(made.and_then(|()| taken.iter().map(|card| card.and_then(|(scene, card)| card_written(card, &folder, year, scene, screen))).collect()))
        });
        let timer = Rc::new(slint::Timer::default());
        let (me, held) = (Rc::downgrade(self), timer.clone());
        // The sender leaves with the timer's closure: the writer then knows every card was handed over.
        let mut cards = Some(cards);
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(1), move || {
            let next = me.upgrade().zip(scenes.next()).map(|(ctl, scene)| ctl.card_picture(scene).map(|card| (scene, card)));
            // A card that could not be taken is handed over too, as the failure the writer reports.
            let sent = match (next, &cards) {
                (Some(card), Some(to)) => {
                    let taken = card.is_ok();
                    to.send(card).is_ok() && taken
                }
                _ => false,
            };
            if !sent {
                cards = None;
                held.stop();
            }
        });
    }

    /// A scene's card on the pasteboard, to paste anywhere: brought to sRGB off this thread, then laid.
    pub fn copy_wrapped(&self, scene: usize) {
        let screen = self.state.borrow().screen;
        match self.card_picture(scene) {
            Ok(card) => {
                std::thread::spawn(move || {
                    let card = crate::thumbnails::from_screen(card, screen);
                    let forms = share::tiff(&card, share::CARD.0, share::CARD.1).zip(share::dib(&card, share::CARD.0, share::CARD.1));
                    with_shell(move |c| c.card_copied(forms));
                });
            }
            Err(e) => self.wrapped_notice(&tr!("The card cannot be copied: {e}", e = e)),
        }
    }

    fn card_copied(&self, forms: Option<(Vec<u8>, Vec<u8>)>) {
        let Some((tiff, dib)) = forms else { return self.wrapped_notice(&tr!("The card cannot be copied: {e}", e = tr("the card cannot be encoded"))) };
        {
            let mut s = self.state.borrow_mut();
            crate::pasteboard::write_picture(&mut *s.pasteboard, &tiff, &dib);
            s.board_seen = Some(s.pasteboard.change_count());
        }
        self.wrapped_notice(tr("Card copied: paste it where you post."));
    }

    /// The picture the pasteboard holds, which a check reads back.
    pub fn board_picture(&self) -> Option<Vec<u8>> {
        self.state.borrow().pasteboard.data(crate::pasteboard::PICTURE)
    }

    pub(super) fn close_wrapped(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.year.story = None;
            s.year.told.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(w) = self.window() {
            let g = w.global::<Wrapped>();
            g.set_shown(false);
            g.set_sharing(false);
            g.set_scenes(ModelRc::default());
            g.set_year(SharedString::default());
            w.invoke_refocus();
        }
        self.publish_bar();
    }
}

/// A scene's card, taken off a window drawn for `screen`, brought to sRGB and written into `folder`.
fn card_written(card: Vec<u8>, folder: &Path, year: i32, scene: usize, screen: crate::display::Screen) -> Result<PathBuf, String> {
    let card = crate::thumbnails::from_screen(card, screen);
    let data = share::jpeg(&card, share::CARD.0, share::CARD.1).ok_or_else(|| tr("the card cannot be encoded").to_string())?;
    let path = folder.join(share::file_name(year, scene));
    oni_settings::atomic::write_atomically(&path, &data).map_err(|e| e.to_string())?;
    Ok(path)
}

/// A scene as the strip draws it: its words, its numbers, and a place for each photo its thumbnail fills.
fn face(scene: &Scene) -> WrappedScene {
    use crate::wrapped::Kind as K;
    let artwork = scene.stock.as_deref().and_then(crate::stocks::icon);
    WrappedScene {
        kind: match scene.kind {
            K::Title => WrappedKind::Title,
            K::Figure => WrappedKind::Figure,
            K::Bars => WrappedKind::Bars,
            K::Film => WrappedKind::Film,
            K::Lines => WrappedKind::Lines,
            K::Photos => WrappedKind::Photos,
            K::Palette => WrappedKind::Palette,
            K::Persona => WrappedKind::Persona,
            K::Summary => WrappedKind::Summary,
            K::Farewell => WrappedKind::Farewell,
        },
        eyebrow: if scene.kind == K::Farewell { scene.eyebrow.as_str().into() } else { scene.eyebrow.to_uppercase().into() },
        figure: scene.figure.as_str().into(),
        count: scene.count.unwrap_or(0) as i32,
        unit: scene.unit.as_str().into(),
        caption: scene.caption.as_str().into(),
        note: scene.note.as_str().into(),
        photos: ModelRc::new(VecModel::from(vec![Image::default(); scene.photos.len()])),
        lines: ModelRc::new(VecModel::from(scene.lines.iter().map(|l| WrappedLine { label: l.label.as_str().into(), value: l.value.as_str().into(), share: l.share }).collect::<Vec<_>>())),
        bars: ModelRc::new(VecModel::from(scene.bars.clone())),
        lit: scene.lit.map_or(-1, |l| l as i32),
        marks: ModelRc::new(VecModel::from(scene.marks.iter().map(|m| SharedString::from(m.as_str())).collect::<Vec<_>>())),
        swatches: ModelRc::new(VecModel::from(scene.swatches.iter().map(|(rgb, share)| WrappedSwatch { ink: slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]), share: *share }).collect::<Vec<_>>())),
        has_artwork: artwork.is_some(),
        artwork: artwork.unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn january_tells_the_year_just_ended_and_every_other_month_its_own() {
        assert_eq!(told_year((2027, 1)), 2026);
        assert_eq!((told_year((2026, 12)), told_year((2027, 2))), (2026, 2027));
        // Twin: January's own year is not the one told.
        assert_ne!(told_year((2027, 1)), 2027);
    }

    #[test]
    fn december_offers_a_year_once_and_no_other_month_does() {
        assert!(offer_due((2026, 12), 0) && offer_due((2026, 12), 2025));
        assert!(!offer_due((2026, 12), 2026), "shown already");
        // Twin: the same year not yet shown is not offered in November.
        assert!(!offer_due((2026, 11), 0));
    }
}
