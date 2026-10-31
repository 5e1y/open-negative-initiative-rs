//! The measure: off this thread with its progress and Stop, then landed as one gallery step.

use super::*;

impl Ctl {
    // MARK: - The measure

    /// `measureRoll`: every ticked frame read off this thread after the drafts' writes, the flagged
    /// ones riding without a vote; the base read, the roll combined, every frame written onto it.
    pub(in crate::app) fn measure_roll(self: &Rc<Self>) {
        if self.state.borrow().prepare.film == ConversionMode::Positive {
            return self.prepare_slides();
        }
        let (job, session) = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let live = s.store.settings().clone();
            let r = &mut s.prepare;
            let picked: Vec<Frame> = r.frames.iter().filter(|f| f.ticked).cloned().collect();
            if !picked.iter().any(|f| !f.flagged) || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, picked.len()));
            let (window, mask, pick, film, stop, session) = (r.window, r.mask(), r.base.clone(), r.film, r.stop.clone(), r.session);
            let sheet = SheetPlan::of(&s.rows, s.archive.as_ref(), &picked);
            let screen = s.screen;
            let job: Job = Box::new(move || {
                let paths: Vec<String> = picked.iter().map(|f| f.path.clone()).collect();
                let common = CaptureExposure::common(&paths.iter().map(|p| CaptureExposure::read(Path::new(p))).collect::<Vec<_>>());
                let stops = |path: &str| match (common, CaptureExposure::read(Path::new(path))) {
                    (Some(c), Some(own)) => own.stops() - c.stops(),
                    _ => 0.0,
                };
                let m = crate::measuring::measurer();
                // RGB passes: each band's magnification read once on the first frames, then each frame's shifts on its own preview.
                let triplets: Vec<(String, RgbPasses)> = picked.iter().filter(|f| !f.flagged).filter_map(|f| rgb_passes::named_by(&f.path).map(|p| (f.path.clone(), p))).collect();
                let registrar = crate::passes::Registrar::new(&triplets);
                let mut jobs: Vec<roll::Job> = Vec::new();
                let mut frames = Vec::new();
                let side = oni_measure::sheet::roll_tile();
                let mut tiles: Vec<(String, oni_measure::sheet::Picture)> = Vec::new();
                let mut reads = Vec::new();
                let mut blank = Vec::new();
                // The base read first, so the roll's screen places the black the roll will take.
                let base = pick.and_then(|p| {
                    let bare = if Some(&p.path) == open.as_ref() {
                        live.clone()
                    } else {
                        sidecar::read(Path::new(&p.path), mosaic(&p.path), true).map(|l| l.settings).unwrap_or_default()
                    };
                    let unit = Point::new(p.unit.0, p.unit.1);
                    m?.with(&p.path, &bare, |r, copy| key::base(r, copy, &bare, film, mask.as_ref(), stops(&p.path), unit, p.radius))
                });
                for f in &picked {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let before = if Some(&f.path) == open.as_ref() {
                        live.clone()
                    } else {
                        match sidecar::read(Path::new(&f.path), mosaic(&f.path), true) {
                            Ok(l) if l.writable => l.settings,
                            _ => continue,
                        }
                    };
                    let mut start = roll::start(&before, film, window, mask.as_ref(), stops(&f.path));
                    if let Some(passes) = rgb_passes::named_by(&f.path) {
                        let passes = registrar.registered(&f.path, passes);
                        rgb_passes::hold_one(&f.path, passes.clone());
                        start.rgb_passes = Some(passes);
                    }
                    let job = roll::Job { path: f.path.clone(), before, start, flagged: f.flagged };
                    // Every frame of the measure lays its tile, a flagged one too, on the measuring copy.
                    let start = job.start.clone();
                    let mut drawn = None;
                    let voted = frames.len();
                    if f.flagged {
                        if let Some(tile) = m.and_then(|m| m.with(&f.path, &start, |r, copy| oni_measure::sheet::tile(r, copy, &start, side))) {
                            drawn = trace::Drawn::of(&tile, screen).zip(negative::of(&tile));
                            tiles.push((f.path.clone(), tile));
                            reads.push(oni_measure::window::in_crop(&start.geometry));
                        }
                        jobs.push(job);
                    } else if let Some((frame, tile)) = m.and_then(|m| {
                        m.with(&f.path, &start, |r, copy| Some((key::roll_measured(r, copy, &f.path, &start), oni_measure::sheet::tile(r, copy, &start, side))))
                    }) {
                        // A frame holding nothing to place an end on, a blank one, rides on the roll unvoting as a flagged one does.
                        match frame {
                            Some(frame) => frames.push(frame),
                            None => blank.push(f.path.clone()),
                        }
                        jobs.push(job);
                        if let Some(tile) = tile {
                            drawn = trace::Drawn::of(&tile, screen).zip(negative::of(&tile));
                            tiles.push((f.path.clone(), tile));
                            reads.push(oni_measure::window::in_crop(&start.geometry));
                        }
                    }
                    // The roll as it would combine now, sent once per frame that voted.
                    let roll = (frames.len() > voted).then(|| roll::combined(&frames, film, base)).flatten().map(|r| (trace::Drawn::roll(&frames, &r, film), frames.len()));
                    let landed = trace::Landed { name: oni_library::paths::name(&f.path), frame: drawn, roll };
                    with_shell(move |c| c.measure_progressed(session, landed));
                }
                let stopped = stop.load(Ordering::Relaxed);
                let base = base.filter(|_| !stopped);
                let reference = if stopped { None } else { roll::combined(&frames, film, base) };
                let written: Option<Written> =
                    reference.map(|reference| jobs.iter().map(|j| (j.path.clone(), j.before.clone(), oni_core::balance::written(&reference, &j.start))).collect());
                let sheet = written.as_ref().and_then(|_| sheet.written(&paths, &jobs, &tiles, &reads));
                with_shell(move |c| c.land_measure(session, written, sheet, stopped, &paths, &blank));
            });
            (job, session)
        };
        let _ = session;
        self.state.borrow().prepare.post(job);
        self.trace_cleared();
        self.publish_prepare();
        self.publish_bar();
    }

    /// A frame read: the count moves on, and what it drew is morphed into.
    pub(in crate::app) fn measure_progressed(self: &Rc<Self>, session: u64, landed: trace::Landed) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            if r.session != session {
                return;
            }
            if let Some(p) = r.progress.as_mut() {
                p.0 += 1;
            }
        }
        self.trace_landed(landed);
        self.publish_prepare();
    }

    /// `landRoll`: nothing measured keeps the roll open and says why; else every frame written as one
    /// gallery step, the editor on the first frame unless the open one landed, and what was left out named.
    pub(in crate::app) fn land_measure(self: &Rc<Self>, session: u64, written: Option<Written>, sheet: Option<Sheet>, stopped: bool, asked: &[String], blank: &[String]) {
        let prepared: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            if r.session != session {
                return;
            }
            r.progress = None;
            r.session += 1;
            r.tiles.clear();
            r.card = None;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            r.frames.iter().map(|f| f.path.clone()).collect()
        };
        self.trace_cleared();
        let Some(written) = written.filter(|w| !w.is_empty()) else {
            if let Some(store) = self.state.borrow_mut().store.editor() {
                store.forget_measured();
            }
            if !stopped {
                self.advise(tr("Something Went Wrong"), tr("No frame of this roll could be measured: the files could not be read, or the measuring frame holds nothing to balance."));
            }
            self.publish_prepare();
            self.publish_bar();
            return;
        };
        let (failure, landed) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            let landed = store.land_roll(&written, &prepared);
            let failure = store.failure.take();
            s.last_change = Some(Instant::now());
            s.prepare.draft_id = None;
            s.prepare.shown = false;
            (failure, landed)
        };
        // Adopted, never opened: the editor opens on the roll's first frame below.
        if let (Some(sheet), Some((_, _, donor))) = (sheet, written.first()) {
            self.adopt_sheet(&sheet, donor, false);
        }
        let support = self.state.borrow().support.clone();
        crate::app::year::journal_in(&support).note(oni_library::journal::Kind::Measured, None, 0, landed as u32, None, oni_settings::Date::now());
        let message = failure.or_else(|| left_out(asked, &written, blank));
        let open = self.state.borrow().open.clone();
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(false);
        }
        if written.iter().any(|(p, _, _)| Some(p) == open.as_ref()) {
            self.disarm_window();
        }
        let mut touched: Vec<String> = written.iter().map(|(p, _, _)| p.clone()).collect();
        let passes: Vec<String> = written
            .iter()
            .filter_map(|(lead, _, now)| now.rgb_passes.as_ref().map(|p| (lead, p)))
            .flat_map(|(lead, p)| rgb_passes::paths_beside(p, lead).into_iter().filter(move |f| f != lead))
            .collect();
        touched.extend(passes.iter().cloned());
        self.landed(&touched);
        self.index_passes(&written, &passes);
        if open.is_none() || !written.iter().any(|(p, _, _)| Some(p) == open.as_ref()) {
            let first = written[0].0.clone();
            self.state.borrow_mut().designate(&first);
            self.open_photo(&first);
            self.publish_tiles();
        }
        if let Some(m) = message {
            self.advise(tr("Prepare the Roll"), &m);
        }
        self.publish_prepare();
        self.publish_bar();
    }

    /// `saveRollDraft`: each frame's draft written on the serial thread, after the writes before it.
    pub(in crate::app) fn save_draft(&self) {
        // The open frame on disk first: a draft write reads its settings back, and must not revive older ones.
        if let Some(failure) = self.state.borrow_mut().store.flush() {
            crate::timing::line(&format!("the open frame's settings could not be saved: {failure}"));
        }
        let job = {
            let r = &mut self.state.borrow_mut().prepare;
            let Some(drafts) = r.drafts() else { return };
            let job: Job = Box::new(move || {
                let refused: Vec<String> =
                    drafts.into_iter().filter_map(|(path, draft)| sidecar::set_roll_draft(Some(draft), Path::new(&path), mosaic(&path)).err()).map(|e| oni_library::refusal::said(&e)).collect();
                if let Some(cause) = refused.first().cloned() {
                    let n = refused.len() as u64;
                    with_shell(move |c| c.told(tr("Prepare the Roll"), &trn("One frame's preparation could not be saved: {cause}", "{n} frames' preparation could not be saved: {cause}", n).replace("{cause}", &cause)));
                }
            });
            job
        };
        self.state.borrow().prepare.post(job);
    }

    /// As the window closes: the step's draft written and every queued write waited for, since the
    /// app may quit next; at most `DRAFT_FLUSH`, and never behind a measure, which lands on this thread.
    pub(in crate::app) fn flush_drafts(&self) {
        self.keep_picker();
        if self.state.borrow().prepare.progress.is_some() {
            return;
        }
        self.save_draft();
        self.drain_drafts();
    }

    /// The draft writes already queued, waited for before a gesture moves their frames' folder,
    /// where a later write finds no photo; at most `DRAFT_FLUSH`, and never behind a measure.
    pub(in crate::app) fn drain_drafts(&self) {
        if self.state.borrow().prepare.progress.is_some() {
            return;
        }
        let (done, waited) = mpsc::channel::<()>();
        self.state.borrow().prepare.post(Box::new(move || {
            let _ = done.send(());
        }));
        let _ = waited.recv_timeout(DRAFT_FLUSH);
    }
}

/// The frames a measure could not read, then those that held nothing to read, each by its name.
fn left_out(asked: &[String], written: &Written, blank: &[String]) -> Option<String> {
    let names = |paths: Vec<&String>| paths.iter().map(|p| oni_library::paths::name(p)).collect::<Vec<_>>().join(", ");
    let unread: Vec<&String> = asked.iter().filter(|p| !written.iter().any(|(w, _, _)| w == *p)).collect();
    let empty: Vec<&String> = blank.iter().filter(|p| written.iter().any(|(w, _, _)| w == *p)).collect();
    let lines: Vec<String> = [
        (!unread.is_empty()).then(|| trn("{names} could not be read and was left as it was.", "{names} could not be read and were left as they were.", unread.len() as u64).replace("{names}", &names(unread))),
        (!empty.is_empty()).then(|| trn("{names} holds nothing to measure: it takes the roll's settings without weighing on them.", "{names} hold nothing to measure: they take the roll's settings without weighing on them.", empty.len() as u64).replace("{names}", &names(empty))),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!lines.is_empty()).then(|| lines.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measure_names_the_frames_it_could_not_read_and_those_that_held_nothing() {
        let asked: Vec<String> = ["/r/a.RW2", "/r/b.RW2", "/r/c.RW2"].map(String::from).into();
        let written: Written = asked[..2].iter().map(|p| (p.clone(), PipelineSettings::default(), PipelineSettings::default())).collect();
        let said = left_out(&asked, &written, &asked[1..2]).expect("two frames to name");
        assert!(said.contains("c.RW2 could not be read") && said.contains("b.RW2 holds nothing to measure"), "{said}");
        assert!(!said.contains("a.RW2"));
        // Twin: every frame read and voting, nothing is said.
        assert_eq!(left_out(&asked[..2], &written, &[]), None);
    }
}
