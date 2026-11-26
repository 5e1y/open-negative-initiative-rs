//! The year shown: read off the disk on another thread, laid as the strip's scenes, its photos landing
//! as they are read, the film advanced and wound back, and the summary closed.

use super::{ACCOUNT, HASHTAG, NAME, PRINT_SIDE, SAMPLE, SHOWN_YEAR_KEY, SLOW, Stamp, Story, WALL_SIDE, offer_due};
use crate::app::{Ctl, image, with_shell};
use crate::wrapped::share::CARD;
use crate::wrapped::{Kind, SWATCHES, Scene, cover, gather, scenes};
use crate::{Wrapped, WrappedKind, WrappedLine, WrappedScene, WrappedSwatch};
use oni_library::journal::Journal;
use oni_library::wrapped::{Darkroom, Frames, Moment, Palette, today};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::Ordering;

/// How large a scene's photo is wanted: filling a card, or whole with its long side at a size.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Need {
    Card,
    Long(usize),
}

impl Need {
    fn of(kind: Kind) -> Need {
        match kind {
            Kind::Title | Kind::Hero | Kind::Summary => Need::Card,
            Kind::Print | Kind::Palette => Need::Long(PRINT_SIDE),
            Kind::Mosaic => Need::Long(WALL_SIDE),
        }
    }
}

impl Ctl {
    /// `year` shown at once: told from its start when it is read already, else over the words that say
    /// it is being read, which its first scene replaces.
    pub fn show_wrapped(self: &Rc<Self>, year: i32) {
        // A year shown in its own December, by the offer, the menu or the welcome, is the offer made.
        if year == today().0 && offer_due(today(), self.shown_year()) {
            self.remember(SHOWN_YEAR_KEY, year.into());
        }
        let ready = self.state.borrow_mut().year.story.as_mut().filter(|s| s.year == year).map(|story| (story.index, story.sharing) = (0, false)).is_some();
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        g.set_index(0);
        g.set_sharing(false);
        g.set_notice(SharedString::default());
        g.set_playing(true);
        if !ready {
            self.state.borrow_mut().year.wanted = Some(year);
            g.set_year(year.to_string().into());
            g.set_scenes(ModelRc::default());
        }
        g.set_loading(!ready);
        g.set_shown(true);
        self.publish_bar();
        if !ready {
            self.read_wrapped(year);
        }
    }

    /// `year` read ahead of its showing, so a click finds it told.
    pub fn ready_wrapped(self: &Rc<Self>, year: i32) {
        self.read_wrapped(year);
    }

    /// `year` read off this thread: the journal, the library's rows and their sidecars, their thumbnails'
    /// colours. The ONI Trash's photos count for nothing. A year read while nothing has changed is not read again.
    fn read_wrapped(self: &Rc<Self>, year: i32) {
        let (rows, support, stamp) = {
            let mut s = self.state.borrow_mut();
            if s.year.reading == Some(year) || s.year.story.as_ref().is_some_and(|story| story.year == year) {
                return;
            }
            let rows = s.index.as_ref().map(|i| i.all()).unwrap_or_default();
            let stamp: Stamp = (digest(&rows), journal_bytes(&s.support));
            if let Some(told) = s.year.read.as_ref().filter(|(was, told)| *was == stamp && told.year == year).map(|(_, told)| told.clone()) {
                drop(s);
                return self.wrapped_read(stamp, told);
            }
            s.year.reading = Some(year);
            (rows, s.support.clone(), stamp)
        };
        // The person's own journal even under a bench, whose gestures a scratch one takes: a year is only read.
        let journal = Journal::in_support(&support);
        let sample = std::env::var_os(SAMPLE).is_some();
        let slow = std::env::var(SLOW).ok().and_then(|ms| ms.parse().ok()).map(std::time::Duration::from_millis);
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
                let (read, tones) = gather::tones(&support, &frames.paths);
                let palette = Palette::of(&tones, SWATCHES);
                step("colours", &mut since);
                let (records, names) = journal.read();
                let darkroom = Darkroom::build(&records, &names, year, &hidden, |time| Moment::local(Moment::journal_unix(time)));
                step("journal", &mut since);
                oni_library::wrapped::Year::new(year, darkroom, frames, palette, &read)
            };
            let read = format!("{:.0} ms ({})", asked.elapsed().as_secs_f64() * 1000.0, steps.join(", "));
            crate::timing::line(&format!("wrapped: {} frames of {} and {} thumbnails read in {read}", told.frames.count, shown.len(), told.palette.sampled));
            if let Some(slow) = slow {
                std::thread::sleep(slow);
            }
            with_shell(move |c| c.wrapped_read(stamp, told));
        });
    }

    /// A year read: kept for the next asking, and told now when its showing waited for it.
    fn wrapped_read(self: &Rc<Self>, stamp: Stamp, told: oni_library::wrapped::Year) {
        let wanted = {
            let mut s = self.state.borrow_mut();
            if s.year.reading == Some(told.year) {
                s.year.reading = None;
            }
            s.year.read = Some((stamp, told.clone()));
            s.year.wanted
        };
        match wanted {
            Some(year) if year == told.year => self.present_wrapped(told),
            Some(other) => self.read_wrapped(other),
            None => {}
        }
    }

    /// The year told from its first scene, its photos landing as they are read and developed.
    fn present_wrapped(self: &Rc<Self>, told: oni_library::wrapped::Year) {
        self.state.borrow_mut().year.wanted = None;
        let Some(w) = self.window() else { return };
        let story = scenes(&told);
        let g = w.global::<Wrapped>();
        g.set_year(told.year.to_string().into());
        g.set_hashtag(HASHTAG.into());
        g.set_account(ACCOUNT.into());
        g.set_name(NAME.into());
        g.set_sharing(false);
        g.set_notice(SharedString::default());
        g.set_index(0);
        g.set_still(false);
        g.set_playing(true);
        g.set_scenes(ModelRc::new(VecModel::from(story.iter().map(face).collect::<Vec<_>>())));
        g.set_showing(g.get_showing() + 1);
        g.set_loading(false);
        g.set_shown(true);
        // Every photo a scene shows: its place, its path, the print that names its thumbnail (the year's
        // frames carry theirs, the opened folder's the rest), and how large its scene wants it.
        let prints: HashMap<&str, &str> = told.frames.paths.iter().map(|(p, f)| (p.as_str(), f.as_str())).collect();
        let photos: Vec<(usize, usize, String, Option<String>, Need)> = {
            let s = self.state.borrow();
            let print = |path: &String| prints.get(path.as_str()).map(|f| f.to_string()).or_else(|| s.fingerprints.get(path).cloned());
            let shown = |(i, scene): (usize, &Scene)| scene.photos.iter().enumerate().map(|(j, p)| (i, j, p.clone(), print(p), Need::of(scene.kind))).collect::<Vec<_>>();
            story.iter().enumerate().flat_map(shown).collect()
        };
        let (support, screen, previews, showing) = {
            let mut s = self.state.borrow_mut();
            let owed = photos.iter().map(|(scene, slot, ..)| (*scene, *slot)).collect();
            s.year.story = Some(Story { year: told.year, scenes: story, index: 0, sharing: false, owed });
            (s.support.clone(), s.screen, s.previews.clone(), s.year.told.clone())
        };
        self.publish_bar();
        let engine = self.engine.clone();
        let mine = showing.fetch_add(1, Ordering::Relaxed) + 1;
        // The thumbnails on disk first: each lights its scene, and stands in a wall, a print or a slice
        // until the photo is developed at the size it is drawn. A photo that fills a card is laid only
        // once developed that large, never a thumbnail stretched; a frame never converted gives its scan.
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let mut owed = Vec::new();
            // Each scene is lit by its first photo, as soon as one picture of it is read.
            let mut lit = HashSet::new();
            let mut light = |scene: usize, slot: usize, rgba: &[u8], width: usize, height: usize| {
                if slot == 0 && lit.insert(scene) {
                    let glow = crate::wrapped::backdrop::blurred(rgba, width, height);
                    let ink = oni_library::wrapped::palette::dominant(rgba, width, height).and_then(oni_library::wrapped::palette::vivid);
                    with_shell(move |c| c.wrapped_light(mine, scene, glow, ink));
                }
            };
            for (scene, slot, path, fingerprint, need) in photos {
                let thumbnail = fingerprint.and_then(|f| crate::thumbnails::load(&support, &f, screen));
                let shape = thumbnail.as_ref().map_or(f32::NAN, |(_, width, height)| *width as f32 / (*height).max(1) as f32);
                let side = match need {
                    Need::Card => cover::side(shape, CARD),
                    Need::Long(side) => side,
                };
                owed.push((scene, slot, path, need, side));
                if let Some((rgba, width, height)) = thumbnail {
                    light(scene, slot, &rgba, width, height);
                    if need != Need::Card {
                        with_shell(move |c| c.wrapped_photo(mine, scene, slot, image(&rgba, width, height)));
                    }
                }
            }
            let mut printed: HashMap<(String, Need), (Vec<u8>, usize, usize)> = HashMap::new();
            for (scene, slot, path, need, side) in owed {
                if showing.load(Ordering::Relaxed) != mine {
                    return;
                }
                let key = (path, need);
                if !printed.contains_key(&key) {
                    let made = crate::app::workers::print(&engine, &previews, &key.0, screen, side).or_else(|| crate::app::workers::scanned(&key.0, screen));
                    let fitted = match need {
                        Need::Card => made.and_then(|(rgba, width, height)| cover::filling(&rgba, width, height, CARD)),
                        Need::Long(_) => made,
                    };
                    if let Some(print) = fitted {
                        printed.insert(key.clone(), print);
                    }
                }
                // Developed or not, the photo is owed no more: its card can be taken as it stands.
                let print = printed.get(&key).cloned();
                if let Some((rgba, width, height)) = &print {
                    light(scene, slot, rgba, *width, *height);
                }
                with_shell(move |c| c.wrapped_developed(mine, scene, slot, print));
            }
        });
    }

    /// A photo developed as large as its scene draws it, laid in its place; none where it could not be.
    fn wrapped_developed(&self, showing: usize, scene: usize, slot: usize, print: Option<(Vec<u8>, usize, usize)>) {
        if self.state.borrow().year.told.load(Ordering::Relaxed) != showing {
            return;
        }
        if let Some(story) = self.state.borrow_mut().year.story.as_mut() {
            story.owed.remove(&(scene, slot));
        }
        if let Some((rgba, width, height)) = print {
            self.wrapped_photo(showing, scene, slot, image(&rgba, width, height));
        }
    }

    /// Whether a photo of `scene` is still being developed: its card waits for it.
    pub fn wrapped_owes(&self, scene: usize) -> bool {
        self.state.borrow().year.story.as_ref().is_some_and(|story| story.owed.iter().any(|(of, _)| *of == scene))
    }

    fn wrapped_photo(&self, showing: usize, scene: usize, slot: usize, picture: Image) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        if self.state.borrow().year.told.load(Ordering::Relaxed) != showing {
            return;
        }
        use slint::Model;
        if let Some(face) = g.get_scenes().row_data(scene) {
            face.photos.set_row_data(slot, picture);
        }
    }

    /// A scene's light: its first photo's backdrop, and an ink in that photo's hue when it has one.
    fn wrapped_light(&self, showing: usize, scene: usize, glow: Option<(Vec<u8>, usize, usize)>, ink: Option<[u8; 3]>) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        if self.state.borrow().year.told.load(Ordering::Relaxed) != showing {
            return;
        }
        use slint::Model;
        let scenes = g.get_scenes();
        let Some(mut face) = scenes.row_data(scene) else { return };
        if let Some((rgba, width, height)) = glow {
            face.backdrop = image(&rgba, width, height);
        }
        if let Some([red, green, blue]) = ink {
            face.accent = slint::Color::from_rgb_u8(red, green, blue);
        }
        scenes.set_row_data(scene, face);
    }

    /// The film advanced or wound back a frame; past the last scene every card is laid out to share,
    /// and past that the summary closes.
    pub(super) fn wrapped_turn(&self, by: i32) {
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

    pub(super) fn close_wrapped(&self) {
        // Over the welcome, it only steps aside: the mystery gift keeps its cards.
        self.state.borrow_mut().year.wanted = None;
        if let Some(w) = self.window().filter(|w| w.global::<crate::Help>().get_welcome_shown()) {
            let g = w.global::<Wrapped>();
            g.set_shown(false);
            g.set_loading(false);
            g.set_sharing(false);
            w.invoke_refocus();
            self.publish_bar();
            return self.welcome_after_wrapped();
        }
        self.forget_wrapped();
    }

    /// The year told forgotten, its cards and the photos still coming for them.
    pub(in crate::app) fn forget_wrapped(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.year.story = None;
            s.year.told.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(w) = self.window() {
            let g = w.global::<Wrapped>();
            g.set_shown(false);
            g.set_loading(false);
            g.set_sharing(false);
            g.set_scenes(ModelRc::default());
            g.set_year(SharedString::default());
            w.invoke_refocus();
        }
        self.publish_bar();
        if crate::wrapped::alone() {
            let _ = slint::quit_event_loop();
        }
    }
}

/// A scene as the strip draws it: its words, its numbers, and a place for each photo and for the light
/// its first photo gives, filled as they are read.
fn face(scene: &Scene) -> WrappedScene {
    use crate::wrapped::Kind as K;
    WrappedScene {
        kind: match scene.kind {
            K::Title => WrappedKind::Title,
            K::Hero => WrappedKind::Hero,
            K::Print => WrappedKind::Print,
            K::Mosaic => WrappedKind::Mosaic,
            K::Palette => WrappedKind::Palette,
            K::Summary => WrappedKind::Summary,
        },
        eyebrow: scene.eyebrow.as_str().into(),
        figure: scene.figure.as_str().into(),
        count: scene.count.unwrap_or(0) as i32,
        unit: scene.unit.as_str().into(),
        size: scene.size,
        headline: scene.headline.as_str().into(),
        note: oni_i18n::spoken(&scene.note).into(),
        photos: ModelRc::new(VecModel::from(vec![Image::default(); scene.photos.len()])),
        backdrop: Image::default(),
        accent: slint::Color::from_argb_u8(0, 0, 0, 0),
        lines: ModelRc::new(VecModel::from(scene.lines.iter().map(|l| WrappedLine { label: l.label.as_str().into(), value: l.value.as_str().into() }).collect::<Vec<_>>())),
        swatches: ModelRc::new(VecModel::from(scene.swatches.iter().map(|(rgb, share)| WrappedSwatch { ink: slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]), share: *share }).collect::<Vec<_>>())),
    }
}

/// The rows as a year reads them, in one number: a photo added, removed, trashed, rated or given a roll
/// changes it.
fn digest(rows: &[oni_library::index::Entry]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in rows {
        (&row.path, &row.fingerprint, std::mem::discriminant(&row.label), row.trashed_at.is_some(), row.sidecar_format).hash(&mut hasher);
        let roll = |film: &oni_settings::Film| (film.name.clone(), film.stock.clone(), film.camera_body.clone(), film.shot_date.map(|date| date.unix_seconds().to_bits()));
        row.film.as_ref().map(roll).hash(&mut hasher);
    }
    hasher.finish()
}

/// The journal's bytes on the disk: they grow with every gesture noted.
fn journal_bytes(support: &std::path::Path) -> u64 {
    let Ok(files) = std::fs::read_dir(support.join("Journal")) else { return 0 };
    files.flatten().filter_map(|file| file.metadata().ok()).map(|data| data.len()).sum()
}
