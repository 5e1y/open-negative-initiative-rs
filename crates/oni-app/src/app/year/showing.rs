//! The year shown: read off the disk on another thread, laid as the strip's scenes, its photos landing
//! as they are read, the film advanced and wound back, and the summary closed.

use super::{ACCOUNT, HASHTAG, NAME, PRINTS_A_SCENE, PRINT_SIDE, SAMPLE, SHOWN_YEAR_KEY, Story, offer_due};
use crate::app::{Ctl, image, with_shell};
use crate::wrapped::{SWATCHES, Scene, gather, scenes};
use crate::{Wrapped, WrappedKind, WrappedLine, WrappedScene, WrappedSwatch};
use oni_library::journal::Journal;
use oni_library::wrapped::{Darkroom, Frames, Moment, Palette, today};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::Ordering;

impl Ctl {
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
            with_shell(move |c| c.present_wrapped(told));
        });
    }

    fn present_wrapped(self: &Rc<Self>, told: oni_library::wrapped::Year) {
        self.state.borrow_mut().year.reading = false;
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
        g.set_shown(true);
        // Every photo a scene shows: its place, its path, the print that names its thumbnail (the year's
        // frames carry theirs, the opened folder's the rest), and whether its scene shows it large.
        let prints: HashMap<&str, &str> = told.frames.paths.iter().map(|(p, f)| (p.as_str(), f.as_str())).collect();
        let photos: Vec<(usize, usize, String, Option<String>, bool)> = {
            let s = self.state.borrow();
            let print = |path: &String| prints.get(path.as_str()).map(|f| f.to_string()).or_else(|| s.fingerprints.get(path).cloned());
            let shown = |(i, scene): (usize, &Scene)| scene.photos.iter().enumerate().map(|(j, p)| (i, j, p.clone(), print(p), scene.photos.len() <= PRINTS_A_SCENE)).collect::<Vec<_>>();
            story.iter().enumerate().flat_map(shown).collect()
        };
        let (support, screen, previews, showing) = {
            let mut s = self.state.borrow_mut();
            s.year.story = Some(Story { year: told.year, scenes: story, index: 0, sharing: false });
            (s.support.clone(), s.screen, s.previews.clone(), s.year.told.clone())
        };
        self.publish_bar();
        let engine = self.engine.clone();
        let mine = showing.fetch_add(1, Ordering::Relaxed) + 1;
        // The thumbnails on disk first; then the larger photos developed again as prints, and those no
        // thumbnail showed made as a thumbnail is, its scan for a frame never converted; each lands in
        // its place as it is made, and a thumbnail stands where no print can be.
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
            for (scene, slot, path, fingerprint, large) in photos {
                let thumbnail = fingerprint.and_then(|f| crate::thumbnails::load(&support, &f, screen));
                if large || thumbnail.is_none() {
                    owed.push((scene, slot, path, if large { PRINT_SIDE } else { crate::thumbnails::SIDE }, thumbnail.is_some()));
                }
                if let Some((rgba, width, height)) = thumbnail {
                    light(scene, slot, &rgba, width, height);
                    with_shell(move |c| c.wrapped_photo(mine, scene, slot, image(&rgba, width, height)));
                }
            }
            let mut printed: HashMap<(String, usize), (Vec<u8>, usize, usize)> = HashMap::new();
            for (scene, slot, path, side, shown) in owed {
                if showing.load(Ordering::Relaxed) != mine {
                    return;
                }
                let key = (path, side);
                let scan = || (!shown).then(|| crate::app::workers::scanned(&key.0, screen)).flatten();
                if !printed.contains_key(&key)
                    && let Some(print) = crate::app::workers::print(&engine, &previews, &key.0, screen, side).or_else(scan)
                {
                    printed.insert(key.clone(), print);
                }
                if let Some((rgba, width, height)) = printed.get(&key).cloned() {
                    light(scene, slot, &rgba, width, height);
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

    /// A scene's light: its first photo's backdrop, and an ink in that photo's hue when it has one.
    fn wrapped_light(&self, showing: usize, scene: usize, glow: Option<(Vec<u8>, usize, usize)>, ink: Option<[u8; 3]>) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        if !g.get_shown() || self.state.borrow().year.told.load(Ordering::Relaxed) != showing {
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
