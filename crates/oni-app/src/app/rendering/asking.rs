//! Asking a render: the open frame's settings in the slot, its patches healed, its full-resolution decode.

use super::*;
use crate::heal::Made;
use oni_pipeline::corrections::PatchKey;

impl State {
    /// The canvas asked again at the next render though no setting moved, one frame or frames together:
    /// a proof, a table or a measure changed what they show.
    pub(in crate::app) fn render_anew(&mut self) {
        self.ask_view_again();
        self.loupe.anew();
        if let Some(g) = self.together.as_mut() {
            g.anew();
        }
    }

    /// The open frame's view asked again at the next render, a hovered preset's included.
    pub(in crate::app) fn ask_view_again(&mut self) {
        if let Some(f) = self.frame.as_mut() {
            f.requests.requested_view = None;
        }
    }
}

impl Ctl {
    /// Lays the open frame's current settings in the render slot, when they differ from the last asked.
    pub(in crate::app) fn request_render(self: &Rc<Self>) {
        if resizing() {
            return;
        }
        if self.together_shown() {
            return self.render_together();
        }
        self.look_at_plots();
        let size = self.canvas_size();
        let cropping = self.cropping();
        let (job, wants_full, heal) = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some(f) = s.frame.as_ref() else { return };
            let path = f.path.clone();
            let revision = s.store.revision();
            let moved_at = s.moved_at.take();
            let full = f.full.clone();
            let coarse = f.requests.hand.running();
            let view_key = size.map(|size| (s.viewport, size, full.is_some(), f.compare.split.is_some(), cropping, coarse));
            let moved = revision != f.requests.requested || f.requests.rendered == 0;
            if !moved && view_key == f.requests.requested_view {
                return;
            }
            // `NegativeView.live`: while framing the crop is removed, so a crop drag keeps the pixels.
            let mut live = s.live_settings();
            if cropping {
                live.geometry = live.geometry.without_crop();
            }
            let Some(f) = s.frame.as_mut() else { return };
            if f.requests.rendered != 0 && view_key == f.requests.requested_view && f.requests.requested_live.as_ref() == Some(&live) {
                f.requests.requested = revision;
                return;
            }
            // A pan `navigate` already laid asks no render while the picture rendered, its margin around the
            // view included, still covers all the view shows: on a 5K canvas each render is 15 MP or more.
            let laid = |k: &Option<(Viewport, (f64, f64), bool, bool, bool, bool)>| k.map(|(v, size, full, split, crop, coarse)| (v.scale, size, full, split, crop, coarse));
            let covered = size.zip(f.placed).is_some_and(|(size, p)| s.viewport.placement(p.full, size).covered_by((p.x, p.y, p.width, p.height)));
            if !moved && covered && laid(&view_key) == laid(&f.requests.requested_view) && f.requests.requested_live.as_ref() == Some(&live) {
                f.requests.requested_view = view_key;
                return;
            }
            let Some(negative) = f.negative.clone() else { return };
            f.requests.requested = revision;
            f.requests.requested_view = view_key;
            f.requests.requested_live = Some(live.clone());
            let (first, comparing, counted) = (f.requests.rendered == 0, f.compare.split.is_some(), f.requests.plots.contains(&true));
            // Until the frame's first picture is shown, a render is the opening one, timed from the open.
            let (kind, asked) = if first {
                (Kind::Open, s.opened_at)
            } else if moved {
                (Kind::Notch, moved_at.unwrap_or_else(Instant::now))
            } else {
                (Kind::View, Instant::now())
            };
            let view = size.map(|size| View { viewport: s.viewport, size, full: full.clone(), coarse });
            // Each correction's patch is generated as its frame opens, the cache purged at every quit, on the
            // full resolution, built first when it is not; a run under way reads these settings before each patch.
            if let Some(latest) = s.dust.healing.get(&path)
                && let Ok(mut l) = latest.lock()
            {
                *l = Some(s.store.settings().clone());
            }
            let unhealed = !s.dust.healing.contains_key(&path) && s.unhealed();
            let heal = full.clone().filter(|_| unhealed).map(|full| {
                let latest = std::sync::Arc::new(std::sync::Mutex::new(Some(s.store.settings().clone())));
                s.dust.healing.insert(path.clone(), latest.clone());
                (path.clone(), latest, full)
            });
            // On the Settings' choice the full resolution is built as the photo opens; every render reads it once built.
            let wants_full = full.is_none() && (unhealed || render::FULL_DECODE.load(std::sync::atomic::Ordering::Relaxed));
            let reference = comparing.then(|| oni_core::compare::reference(&live));
            // A notch is the hand at work: the input histograms read the small copy until it stops.
            let held = kind == Kind::Notch;
            (Job { revision, path: PathBuf::from(path), negative, settings: live, kind, asked, view, reference, held, counted }, wants_full, heal)
        };
        self.worker.submit(job);
        self.request_loupe();
        if wants_full {
            self.ensure_full();
        }
        if let Some((path, latest, full)) = heal {
            self.heal(path, latest, full);
            self.publish_dedust();
        }
    }

    /// The open frame's full resolution once built; else its build started off this thread, unless one
    /// runs, failed, or the frame's opening decode has not landed. The build's landing asks a render again.
    pub(in crate::app) fn ensure_full(self: &Rc<Self>) -> Option<oni_graph::Source> {
        let asked = {
            let mut s = self.state.borrow_mut();
            let f = s.frame.as_ref()?;
            if let Some(full) = &f.full {
                return Some(full.clone());
            }
            let (path, decoded) = (f.path.clone(), f.negative.is_some());
            if !decoded || s.building_full.is_some() || s.full_failed.contains(&path) {
                return None;
            }
            s.building_full = Some(path.clone());
            (path, render::Reading::of(s.store.settings()))
        };
        self.build_full(asked.0, asked.1);
        self.publish_dedust();
        None
    }

    /// `ensureGenerated` off this thread, on `full`, the frame's own full resolution: each correction without a
    /// patch in turn, picked by `heal::next` from the settings as the hand leaves them, until none is left.
    fn heal(self: &Rc<Self>, path: String, latest: Latest, full: oni_graph::Source) {
        let engine = self.engine.clone();
        let mut unhealable = self.state.borrow().dust.unhealable.clone();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let started = Instant::now();
            let caches = crate::patches::caches();
            let (mut made, mut failed, mut error) = (0, Vec::new(), None);
            // Until nothing is left to heal or the frame is left.
            let settings = || latest.lock().ok().and_then(|l| l.clone());
            crate::heal::ensure(&engine, &full, &caches, &mut unhealable, settings, |one| match one {
                Ok(one) => {
                    made += 1;
                    // Each patch shows as it lands, so a hand painting stroke after stroke sees them come.
                    let at = path.clone();
                    with_shell(move |ctl| ctl.patch_landed(&at, one));
                }
                // Not tried again on every render.
                Err((key, e)) => {
                    failed.push(key);
                    error = Some(e);
                }
            });
            let took = started.elapsed();
            with_shell(move |c| {
                let open = c.state.borrow_mut().frame_for(&path).is_some();
                {
                    let mut s = c.state.borrow_mut();
                    s.dust.healing.remove(&path);
                    s.dust.unhealable.extend(failed);
                }
                if made > 0 {
                    timing::line(&format!("{}: {made} dust patches generated in {:.0} ms", crate::anonymity::token(&path), took.as_secs_f64() * 1000.0));
                }
                if let Some(e) = error {
                    timing::line(&format!("{}: dust patches not generated: {e}", crate::anonymity::token(&path)));
                }
                // Strokes painted as the run ended are healed next.
                if open {
                    c.state.borrow_mut().render_anew();
                    c.request_render();
                }
                // A patch the balance reads landed after its tab was left.
                if open && made > 0 {
                    c.balance_at_rest();
                }
                c.publish_dedust();
            });
        });
    }

    /// One patch on the canvas as soon as it is written, unless its frame closed; one made for a
    /// correction removed meanwhile is dropped, as `deleteCorrection` cancels the generation.
    fn patch_landed(self: &Rc<Self>, path: &str, one: Made) {
        {
            let mut s = self.state.borrow_mut();
            if s.frame_for(path).is_none() {
                s.dust.patched.insert(one.0);
                return;
            }
            let (kept, gone) = landed(vec![one], &s.store.settings().corrections);
            for key in gone {
                let _ = std::fs::remove_file(crate::patches::file(&crate::patches::caches(), key));
            }
            for (key, fell_back, offset) in kept {
                s.dust.patched.insert(key);
                if fell_back {
                    s.dust.fallback.insert(key.id);
                }
                s.dust.sources.insert(key.id, offset);
            }
            s.render_anew();
        }
        self.request_render();
        self.publish_dedust();
    }

    /// The open frame decoded again at full resolution off this thread, then shown where the view
    /// wants it; one decoded under a reading the frame has since left is dropped.
    fn build_full(self: &Rc<Self>, path: String, converted: render::Reading) {
        let engine = self.engine.clone();
        let ticket = crate::decode_gate::FULL.ask();
        std::thread::spawn(move || {
            let reading = converted.clone();
            let built = crate::decode_gate::FULL.run(ticket, || {
                let started = Instant::now();
                let decoded = render::decode_full(Path::new(&path), converted)?;
                let (decoded_in, held) = (started.elapsed(), Instant::now());
                let full = engine.full(decoded);
                timing::line(&format!(
                    "{}: full resolution decoded in {:.0} ms, laid on the GPU in {:.0} ms",
                    crate::anonymity::token(&path),
                    decoded_in.as_secs_f64() * 1000.0,
                    held.elapsed().as_secs_f64() * 1000.0
                ));
                full
            });
            with_shell(move |c| {
                // Back in the gallery the editor's memory was let go: a build landing there is dropped.
                let editing = c.window().is_none_or(|w| !w.global::<App>().get_gallery());
                {
                    let mut s = c.state.borrow_mut();
                    // Another frame's build may hold the mark: only this one's is cleared.
                    if s.building_full.as_deref() == Some(path.as_str()) {
                        s.building_full = None;
                    }
                    let current = editing && s.wants(&path, &reading);
                    match built {
                        Some(Ok(full)) if current => {
                            if let Some(f) = s.frame_for(&path) {
                                f.full = Some(full);
                            }
                        }
                        Some(Ok(_)) | None => {}
                        Some(Err(e)) => {
                            timing::line(&format!("full resolution not built: {e}"));
                            s.full_failed.insert(path);
                        }
                    }
                }
                // Another frame opened meanwhile asks its own full resolution now that this one is done.
                if editing {
                    c.request_render();
                }
            });
        });
    }
}

/// The frame's settings a heal run reads before each patch, `None` once the frame is left.
pub(in crate::app) type Latest = std::sync::Arc<std::sync::Mutex<Option<oni_settings::PipelineSettings>>>;

/// The patches made for corrections the frame still holds, and the keys of those removed meanwhile.
fn landed(made: Vec<Made>, corrections: &[oni_settings::frame::Correction]) -> (Vec<Made>, Vec<PatchKey>) {
    let (kept, gone): (Vec<Made>, Vec<Made>) = made.into_iter().partition(|m| corrections.iter().any(|c| c.id == m.0.id));
    (kept, gone.into_iter().map(|m| m.0).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::frame::Correction;
    use oni_settings::id::Id;

    #[test]
    fn a_patch_made_for_a_correction_removed_meanwhile_does_not_land() {
        let c = |key: PatchKey| Correction { id: key.id, stroke: Vec::new(), radius: 20.0 };
        let (stays, removed) = (PatchKey { id: Id::new(), upstream: 1 }, PatchKey { id: Id::new(), upstream: 1 });
        let made = vec![(stays, false, (0.1, 0.0)), (removed, true, (0.0, 0.2))];
        let (kept, gone) = landed(made.clone(), &[c(stays)]);
        assert_eq!(kept.iter().map(|m| m.0).collect::<Vec<_>>(), vec![stays]);
        assert_eq!(gone, vec![removed]);
        let (kept, gone) = landed(made, &[c(stays), c(removed)]);
        assert_eq!((kept.len(), gone.len()), (2, 0), "twin: with both still there, both land");
    }
}
