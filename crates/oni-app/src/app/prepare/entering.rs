//! Entering Prepare the Roll from its three keys, and leaving it.

use super::*;

impl Ctl {
    // MARK: - Entering and leaving

    /// `prepareRoll(propagationTargets)`: one frame brings its folder's own roll, several come as they are.
    pub(in crate::app) fn prepare_roll(self: &Rc<Self>) {
        let targets = self.targets();
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
        let wanted: HashSet<&String> = paths.iter().filter(|p| !roll_membership::is_sheet(p)).collect();
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
            r.scan = None;
            (r.fulls, r.asked_full) = (Vec::new(), None);
            r.asked_scan = None;
            r.shown_path = None;
            r.stop.store(false, Ordering::Relaxed);
            r.base = None;
            r.base_frame = 0;
            r.draft_id = None;
            r.draftless.clear();
            r.shown = true;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            (ordered, r.session)
        };
        let paths: Vec<String> = ordered.into_iter().map(|(p, _)| p).collect();
        let reading = paths.clone();
        std::thread::spawn(move || {
            let captures: Vec<Option<CaptureExposure>> = reading.iter().map(|p| CaptureExposure::read(Path::new(p))).collect();
            with_shell(move |c| {
                {
                    let mut s = c.state.borrow_mut();
                    if s.prepare.session != session {
                        return;
                    }
                    for (path, capture) in reading.iter().zip(captures) {
                        if let Some(f) = s.prepare.frames.iter_mut().find(|f| &f.path == path) {
                            f.capture = capture;
                        }
                    }
                }
                c.publish_prepare();
            });
        });
        // Framings land frame by frame, so nothing waits for the slowest file before the first scan shows.
        std::thread::spawn(move || {
            for (index, path) in paths.into_iter().enumerate() {
                let loaded = sidecar::read(Path::new(&path), mosaic(&path), false).ok();
                let read = loaded.as_ref().map(|l| l.settings.clone()).unwrap_or_default();
                let draft = loaded.and_then(|l| l.roll_draft);
                let aspect = aspect(scan_size(&path), &read.geometry);
                with_shell(move |c| c.land_frame(session, &path, read, draft, index == 0, aspect));
            }
        });
        self.publish_menus();
        self.publish_bar();
        self.publish_prepare();
        w.invoke_refocus();
    }

    /// `landRollFrame`: the frame's framing, its preview asked, the roll's values off the first.
    pub(in crate::app) fn land_frame(self: &Rc<Self>, session: u64, path: &str, read: PipelineSettings, draft: Option<RollDraft>, first: bool, aspect: Option<f64>) {
        {
            let kept = self.remembered_flat_field();
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            if !r.shown || r.session != session {
                return;
            }
            r.read.insert(path.to_string(), read.clone());
            if let Some(source) = r.sources.iter_mut().find(|f| f.path == path) {
                source.crop = read.geometry.crop;
                source.aspect = aspect;
                if read.rgb_passes.is_some() {
                    source.passes = read.rgb_passes.clone();
                }
            }
            let Some(index) = r.frames.iter().position(|f| f.path == path) else { return };
            let f = &mut r.frames[index];
            if read.rgb_passes.is_some() && f.passes.is_none() {
                f.passes = read.rgb_passes.clone();
            }
            f.crop = read.geometry.crop;
            f.aspect = aspect;
            if let Some(p) = &r.previews {
                p.add(path, &read);
            }
            if first {
                if let Some(a) = aspect {
                    r.aspect = a;
                }
                r.window = read.geometry.measure_window.unwrap_or_else(|| oni_measure::window::proposal(&read.geometry.crop, false));
                r.flat = crate::flat_fields::with_remembered(&read.flat_field, kept);
                r.film = read.mode;
                r.capture = if read.rgb_passes.is_some() {
                    Capture::RgbPasses
                } else if read.decoded_from_mosaic {
                    Capture::Camera
                } else {
                    Capture::Scanner
                };
            }
            r.resume(draft.as_ref(), index);
        }
        self.publish_prepare();
    }

    /// `leaveRollReference`: refused while measuring; the draft saved, back where the hand came from.
    pub(in crate::app) fn leave_roll(self: &Rc<Self>) {
        {
            let s = self.state.borrow();
            if !s.prepare.shown || s.prepare.progress.is_some() {
                return;
            }
        }
        self.save_draft();
        self.keep_picker();
        let back = {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            r.shown = false;
            r.session += 1;
            rgb_passes::release();
            r.tiles.clear();
            r.card = None;
            r.scan = None;
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

    /// `takeOff`: a frame opened from the sidebar leaves the roll first, never while it measures.
    pub(in crate::app) fn take_off_roll(self: &Rc<Self>) -> bool {
        let (shown, measuring) = {
            let r = &self.state.borrow().prepare;
            (r.shown, r.progress.is_some())
        };
        if shown && measuring {
            return false;
        }
        if shown {
            self.leave_roll();
        }
        true
    }

    /// `requireRoll`: a frame never measured, opened in the editor, goes to Prepare the Roll on its
    /// selection or its folder; Cancel then leaves for the gallery.
    pub(in crate::app) fn require_roll(self: &Rc<Self>, path: &str) {
        let picked: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let writable = s.store.editor().is_some_and(|e| e.writable());
            let settings = s.store.settings();
            if !s.prepare.require || !writable || settings.roll_reference.is_some() || settings.sheet.is_some() {
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
