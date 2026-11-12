//! Entering Prepare the Roll from its three keys, and leaving it.

use super::*;
use crate::app::Pending;

impl Ctl {
    // MARK: - Entering and leaving

    /// `prepareRoll(propagationTargets)`: one frame brings its folder's own roll, several come as
    /// they are; none picked in the gallery, the roll it shows.
    pub(in crate::app) fn prepare_roll(self: &Rc<Self>) {
        let targets = self.roll_targets();
        self.roll_of(targets, |c, paths, left| c.enter_roll(&paths, left));
    }

    /// `roll(of:)`: the frame's gallery group read off this thread, kept to its own roll.
    pub(in crate::app) fn roll_of(self: &Rc<Self>, targets: Vec<String>, then: impl FnOnce(&Rc<Ctl>, Vec<String>, usize) + Send + 'static) {
        let folder: Option<Vec<String>> = {
            let s = self.state.borrow();
            match targets.as_slice() {
                [picked] => s.org.bands.iter().find(|b| b.entries.iter().any(|e| &e.path == picked)).map(|b| b.entries.iter().map(|e| e.path.clone()).collect()),
                _ => None,
            }
        };
        std::thread::spawn(move || {
            let (paths, left) = match folder {
                Some(folder) => {
                    let own = own_roll(&targets[0], &folder, roll_membership::reference);
                    let left = folder.len() - own.len();
                    (own, left)
                }
                None => (targets, 0),
            };
            with_shell(move |c| then(c, paths, left));
        });
    }

    /// `enterRollReference`: every frame ticked, the film step, then each sidecar read off this thread.
    pub(in crate::app) fn enter_roll(self: &Rc<Self>, paths: &[String], left_out: usize) {
        let Some(w) = self.window() else { return };
        let wanted: HashSet<&String> = paths.iter().collect();
        let (ordered, session) = {
            let mut s = self.state.borrow_mut();
            let ordered: Vec<(String, String)> = s.rows.iter().filter(|e| wanted.contains(&e.path)).map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            if ordered.is_empty() || s.prepare.progress.is_some() {
                return;
            }
            // The roll shown asked again (its own key landing after the automatic entry) keeps its step.
            let same = ordered.len() == s.prepare.sources.len() && s.prepare.sources.iter().all(|f| ordered.iter().any(|(p, _)| *p == f.path));
            if s.prepare.shown && same {
                return;
            }
            let gallery = w.global::<App>().get_gallery();
            s.shelf.hovered = None;
            let r = &mut s.prepare;
            if !r.shown {
                r.back_to_gallery = gallery;
            }
            r.frames = ordered.iter().map(|(p, f)| Frame::new(p, f)).collect();
            r.sources = r.frames.clone();
            (r.pass_offset, r.grouping) = (0, None);
            r.pass_pictures.clear();
            rgb_passes::release();
            r.left_out = left_out;
            r.read.clear();
            r.step = Step::Film;
            r.at = 0;
            r.flat_pick = None;
            r.session += 1;
            r.tiles.clear();
            r.card = None;
            (r.scan, r.unreadable) = (None, None);
            (r.fulls, r.asked_full) = (Vec::new(), None);
            r.asked_scan = None;
            r.shown_path = None;
            r.stop.store(false, Ordering::Relaxed);
            r.base = None;
            r.base_frame = 0;
            r.laid_window = r.window;
            r.draft_id = None;
            r.draftless.clear();
            r.draft_waiting = false;
            r.tutorial_hidden = false;
            r.shown = true;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            (ordered, r.session)
        };
        let paths: Vec<String> = ordered.into_iter().map(|(p, _)| p).collect();
        self.dng_roll_entered(&paths);
        let reading = paths.clone();
        std::thread::spawn(move || {
            let captures: Vec<Option<CaptureExposure>> = reading.iter().map(|p| CaptureExposure::read(Path::new(p))).collect();
            with_shell(move |c| {
                {
                    let mut s = c.state.borrow_mut();
                    if s.prepare.session != session {
                        return;
                    }
                    // On the sources too, which a regrouping lays the frames from.
                    let r = &mut s.prepare;
                    for (path, capture) in reading.iter().zip(captures) {
                        for f in r.sources.iter_mut().chain(r.frames.iter_mut()).filter(|f| &f.path == path) {
                            f.capture = capture;
                        }
                    }
                }
                c.publish_prepare();
            });
        });
        // Framings land frame by frame, so nothing waits for the slowest file before the first scan
        // shows; on the drafts' thread, so a roll left a moment ago is read once its drafts are dropped.
        self.state.borrow().prepare.post(Box::new(move || {
            for (index, path) in paths.into_iter().enumerate() {
                let loaded = sidecar::read(Path::new(&path), mosaic(&path), false).ok();
                let read = loaded.as_ref().map(|l| l.settings.clone()).unwrap_or_default();
                let draft = loaded.and_then(|l| l.roll_draft);
                let aspect = aspect(scan_size(&path), &read.geometry);
                with_shell(move |c| c.land_frame(session, &path, read, draft, index == 0, aspect));
            }
        }));
        self.publish_menus();
        self.publish_bar();
        self.publish_prepare();
        w.invoke_refocus();
    }

    /// `landRollFrame`: the frame's framing, its preview asked, the roll's values off the first.
    pub(in crate::app) fn land_frame(self: &Rc<Self>, session: u64, path: &str, read: PipelineSettings, draft: Option<RollDraft>, first: bool, aspect: Option<f64>) {
        // Only the first frame and a draft group the roll's files.
        let known = if first || draft.is_some() { self.known() } else { passes::Known::new() };
        let (waited, grouped) = {
            let kept = self.remembered_flat_field();
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            if !r.shown || r.session != session {
                return;
            }
            r.read.insert(path.to_string(), read.clone());
            // Prepare the Roll shows and measures each scan whole, whatever its crop (owner's call).
            let whole = uncropped(&read);
            if let Some(source) = r.sources.iter_mut().find(|f| f.path == path) {
                source.crop = whole.geometry.crop;
                source.aspect = aspect;
                if read.rgb_passes.is_some() {
                    source.passes = read.rgb_passes.clone();
                }
            }
            if let Some(f) = r.frames.iter_mut().find(|f| f.path == path) {
                if read.rgb_passes.is_some() && f.passes.is_none() {
                    f.passes = read.rgb_passes.clone();
                }
                f.crop = whole.geometry.crop;
                f.aspect = aspect;
                if let Some(p) = &r.previews {
                    p.add(path, &whole);
                }
            }
            if first {
                if let Some(a) = aspect {
                    r.aspect = a;
                }
                r.window = read.geometry.measure_window.unwrap_or_else(|| oni_settings::geometry::proposal(read.geometry.crop, false));
                r.laid_window = r.window;
                r.flat = crate::flat_fields::with_remembered(&read.flat_field, kept);
                // A slide once read as its own negative comes back as the positive it is.
                r.film = if read.inverted_source { ConversionMode::Positive } else { read.mode };
                let capture = if read.rgb_passes.is_some() {
                    Capture::RgbPasses
                } else if read.decoded_from_mosaic {
                    Capture::Camera
                } else {
                    Capture::Scanner
                };
                r.group_as(capture, r.pass_order, r.pass_offset, &known);
            }
            let resumed = r.resume(draft.as_ref(), path, &known);
            (r.draft_landed(), resumed && r.grouping.is_some())
        };
        if grouped {
            self.read_passes(false);
        }
        if waited {
            self.save_draft();
        }
        self.publish_prepare();
    }

    /// Cancel, Escape and the GALLERY key: the roll left, after asking while it holds a choice.
    pub(in crate::app) fn leave_roll(self: &Rc<Self>) {
        self.take_off_roll(|_| {});
    }

    /// `takeOff`: the roll left, then `then`; asked first while it holds a choice, never while it measures.
    pub(in crate::app) fn take_off_roll(self: &Rc<Self>, then: impl FnOnce(&Rc<Ctl>) + 'static) {
        let (shown, measuring, holds) = {
            let r = &self.state.borrow().prepare;
            (r.shown, r.progress.is_some(), r.holds_choices())
        };
        if !shown {
            return then(self);
        }
        if measuring {
            return;
        }
        let leave = move |c: &Rc<Ctl>| {
            c.drop_roll();
            then(c);
        };
        if !holds {
            return leave(self);
        }
        let message = tr("The base picked, the frames flagged and the measuring window drawn are dropped, and the roll opens again on its first step.");
        self.ask(tr("Discard This Roll's Preparation?"), message, tr("Discard"), true, Pending::Run(Box::new(leave)));
    }

    /// `leaveRollReference`: refused while measuring; nothing kept, back where the hand came from.
    pub(in crate::app) fn drop_roll(self: &Rc<Self>) {
        {
            let s = self.state.borrow();
            if !s.prepare.shown || s.prepare.progress.is_some() {
                return;
            }
        }
        self.discard_draft();
        self.keep_picker();
        let back = {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            r.shown = false;
            r.session += 1;
            rgb_passes::release();
            r.tiles.clear();
            r.card = None;
            (r.scan, r.unreadable) = (None, None);
            (r.fulls, r.asked_full) = (Vec::new(), None);
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            r.back_to_gallery
        };
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(back);
        }
        self.publish_prepare();
        self.publish_tiles();
        self.publish_bar();
        self.request_render();
    }

    /// `requireRoll`: a frame never measured, opened in the editor, goes to Prepare the Roll on its
    /// selection or its folder; Cancel then leaves for the gallery.
    pub(in crate::app) fn require_roll(self: &Rc<Self>, path: &str) {
        let picked: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let writable = s.store.writable();
            let settings = s.store.settings();
            let slide = settings.mode == ConversionMode::Positive;
            if !s.prepare.require || !writable || settings.roll_reference.is_some() || settings.sheet.is_some() || slide {
                return;
            }
            let among = std::mem::take(&mut s.prepare.opened_among);
            if among.contains(path) { s.order.iter().filter(|p| among.contains(*p)).cloned().collect() } else { vec![path.to_string()] }
        };
        let path = path.to_string();
        self.roll_of(picked, move |c, paths, left| {
            let still = {
                let s = c.state.borrow();
                let gallery = c.window().is_none_or(|w| w.global::<App>().get_gallery());
                s.open.as_deref() == Some(path.as_str()) && s.store.settings().roll_reference.is_none() && !gallery && !s.prepare.shown
            };
            if !still {
                return;
            }
            c.enter_roll(&paths, left);
            if c.state.borrow().prepare.shown {
                c.state.borrow_mut().prepare.back_to_gallery = true;
            }
        });
    }
}

/// The scan's pixel size, read off its container's header.
fn scan_size(path: &str) -> Option<(usize, usize)> {
    oni_decode::container::pixel_size(Path::new(path))
}
