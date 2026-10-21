//! Asking a render: the open frame's settings in the slot, its patches healed, its full-resolution decode.

use super::*;

impl Ctl {
    /// Lays the open frame's current settings in the render slot, when they differ from the last asked.
    pub(in crate::app) fn request_render(self: &Rc<Self>) {
        let size = self.canvas_size();
        let cropping = self.cropping();
        let (job, build, heal) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let revision = s.store.revision();
            let moved_at = s.moved_at.take();
            let settled = s.navigated_at.is_none_or(|t| t.elapsed() >= viewport::SETTLE);
            let full = s.full.as_ref().filter(|(p, _)| *p == path).map(|(_, f)| f.clone());
            let view_key = size.map(|size| (s.viewport, size, settled, full.is_some(), s.split.is_some(), cropping));
            let moved = revision != s.requested || s.rendered == 0;
            if !moved && view_key == s.requested_view {
                return;
            }
            // `NegativeView.live`: while framing the crop is removed, so a crop drag keeps the pixels.
            let mut live = s.store.settings().clone();
            if let Some(previewed) = s.preset_preview(&live) {
                live = previewed;
            }
            if cropping {
                live.geometry = live.geometry.without_crop();
            }
            if s.rendered != 0 && view_key == s.requested_view && s.requested_live.as_ref() == Some(&live) {
                s.requested = revision;
                return;
            }
            let Some(negative) = s.negative.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()) else { return };
            s.requested = revision;
            s.requested_view = view_key;
            s.requested_live = Some(live.clone());
            // Until the frame's first picture is shown, a render is the opening one, timed from the open.
            let (kind, asked) = if s.rendered == 0 {
                (Kind::Open, s.opened_at)
            } else if moved {
                (Kind::Notch, moved_at.unwrap_or_else(Instant::now))
            } else {
                (Kind::View, Instant::now())
            };
            // Full resolution is built once the view settles past the preview's ratio, never mid-gesture.
            let wants_full = match (size, s.full_size) {
                (Some(size), Some(f)) => settled && s.viewport.needs_full_resolution(negative.size().0 as f64 / negative.full_width, f, size, viewport::NOMINAL_BUDGET),
                _ => false,
            };
            let build = (wants_full && full.is_none() && s.building_full.is_none() && !s.full_failed.contains(&path)).then(|| {
                s.building_full = Some(path.clone());
                (path.clone(), render::Reading::of(s.store.settings()))
            });
            let view = size.map(|size| View { viewport: s.viewport, size, full: full.clone(), settled });
            // The cache is purged at every quit, so each correction's patch is generated as its frame opens;
            // a full resolution being built is waited for, its landing asking again, never decoded twice.
            let building = build.is_some() || s.building_full.as_deref() == Some(path.as_str());
            let heal = (!building && !s.healing.contains(&path) && s.unhealed()).then(|| {
                s.healing.insert(path.clone());
                (path.clone(), s.store.settings().clone(), full.clone())
            });
            let reference = s.split.map(|_| oni_core::compare::reference(&live));
            // A notch is the hand at work: the input histograms read the small copy until it stops.
            let held = kind == Kind::Notch;
            (Job { revision, path: PathBuf::from(path), negative, settings: live, kind, asked, view, reference, held }, build, heal)
        };
        self.worker.submit(job);
        self.request_loupe();
        let pending = build.is_some() || heal.is_some();
        if let Some((path, converted)) = build {
            self.build_full(path, converted);
        }
        if let Some((path, settings, full)) = heal {
            self.heal(path, settings, full);
        }
        if pending {
            self.publish_dedust();
        }
    }

    /// `ensureGenerated` for each correction of the open frame that has no patch yet, one after the
    /// other on the full-resolution frame, so each source holds the patches before it healed. `full`
    /// is the frame's own when built, `fullIfReady`; else it is decoded here and handed back.
    pub(in crate::app) fn heal(self: &Rc<Self>, path: String, settings: oni_settings::PipelineSettings, full: Option<oni_graph::Source>) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let started = Instant::now();
            let caches = crate::patches::caches();
            let lacking = |c: &oni_settings::frame::Correction| !c.stroke.is_empty() && !crate::patches::file(&caches, c.id).exists();
            let (mut made, mut built) = (Vec::new(), None);
            let failed = (|| -> Result<(), String> {
                let (full, decoded) = reused(full, || engine.full(render::decode_full(Path::new(&path), render::Reading::of(&settings))?))?;
                if decoded {
                    built = Some(full.clone());
                }
                // The engine is taken per correction, so the canvas's renders pass between two.
                for c in settings.corrections.iter().filter(|c| lacking(c)) {
                    let (region, size) = engine.lock()?.search_region(&full, &settings, c)?;
                    let patch = crate::heal::generate(&region, c, size.0, size.1);
                    crate::patches::write(&caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
                    engine.lock()?.forget_patch(c.id);
                    made.push((c.id, patch.fallback, patch.offset));
                }
                engine.lock()?.purge();
                Ok(())
            })()
            .err();
            let (took, decoded_here) = (started.elapsed(), built.is_some());
            // A correction left without its patch by a failure is not generated again on every render.
            let unhealable: Vec<oni_settings::id::Id> =
                if failed.is_some() { settings.corrections.iter().filter(|c| lacking(c)).map(|c| c.id).collect() } else { Vec::new() };
            with_shell(move |c| {
                let open = c.state.borrow().open_path() == Some(path.as_str());
                {
                    let mut s = c.state.borrow_mut();
                    s.healing.remove(&path);
                    s.unhealable.extend(unhealable);
                    // `deleteCorrection` cancels the generation: a patch made for a correction removed meanwhile is dropped.
                    if open {
                        let (kept, gone) = landed(made, &s.store.settings().corrections);
                        for id in gone {
                            let _ = std::fs::remove_file(crate::patches::file(&crate::patches::caches(), id));
                        }
                        made = kept;
                    }
                    let current = s.reading.as_ref() == Some(&render::Reading::of(&settings));
                    if let Some(full) = built.filter(|_| open && current && s.full.is_none()) {
                        s.full = Some((path.clone(), full));
                    }
                    for &(id, fell_back, offset) in &made {
                        s.patched.insert(id);
                        if open {
                            if fell_back {
                                s.fallback.insert(id);
                            }
                            s.sources.insert(id, offset);
                        }
                    }
                }
                if !made.is_empty() {
                    let from = if decoded_here { "decoded for them" } else { "the frame's own" };
                    timing::line(&format!("{}: {} dust patches generated in {:.0} ms, full resolution {from}", crate::anonymity::token(&path), made.len(), took.as_secs_f64() * 1000.0));
                }
                // Strokes painted meanwhile are generated next, on the full resolution now kept.
                let more = open && c.state.borrow_mut().unhealed();
                if open && (!made.is_empty() || more) {
                    c.state.borrow_mut().requested_view = None;
                    c.request_render();
                }
                if let Some(e) = failed {
                    timing::line(&format!("{}: dust patches not generated: {e}", crate::anonymity::token(&path)));
                }
                c.publish_dedust();
            });
        });
    }

    /// The open frame decoded again at full resolution off this thread, then shown where the view
    /// wants it; one decoded under a reading the frame has since left is dropped.
    pub(in crate::app) fn build_full(self: &Rc<Self>, path: String, converted: render::Reading) {
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
                {
                    let mut s = c.state.borrow_mut();
                    // Another frame's build may hold the mark: only this one's is cleared.
                    if s.building_full.as_deref() == Some(path.as_str()) {
                        s.building_full = None;
                    }
                    let current = s.open_path() == Some(path.as_str()) && s.reading.as_ref() == Some(&reading);
                    match built {
                        Some(Ok(full)) if current => s.full = Some((path, full)),
                        Some(Ok(_)) | None => {}
                        Some(Err(e)) => {
                            timing::line(&format!("full resolution not built: {e}"));
                            s.full_failed.insert(path);
                        }
                    }
                }
                // Another frame opened meanwhile asks its own full resolution now that this one is done.
                c.request_render();
            });
        });
    }
}

/// The source kept when there is one, else the one `build` makes, and whether it had to.
fn reused<T>(kept: Option<T>, build: impl FnOnce() -> Result<T, String>) -> Result<(T, bool), String> {
    match kept {
        Some(t) => Ok((t, false)),
        None => build().map(|t| (t, true)),
    }
}

type Made = (oni_settings::id::Id, bool, (f64, f64));

/// The patches made for corrections the frame still holds, and the ids of those removed meanwhile.
fn landed(made: Vec<Made>, corrections: &[oni_settings::frame::Correction]) -> (Vec<Made>, Vec<oni_settings::id::Id>) {
    let (kept, gone): (Vec<Made>, Vec<Made>) = made.into_iter().partition(|m| corrections.iter().any(|c| c.id == m.0));
    (kept, gone.into_iter().map(|m| m.0).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::frame::Correction;
    use oni_settings::id::Id;

    #[test]
    fn a_kept_full_resolution_is_reused_and_none_is_decoded() {
        let decodes = std::cell::Cell::new(0);
        let decode = || {
            decodes.set(decodes.get() + 1);
            Ok(7)
        };
        assert_eq!(reused(Some(3), decode), Ok((3, false)));
        assert_eq!(decodes.get(), 0, "a frame's own full resolution decodes nothing");
        assert_eq!(reused(None, decode), Ok((7, true)), "twin: none kept decodes one");
        assert_eq!(decodes.get(), 1);
    }

    #[test]
    fn a_patch_made_for_a_correction_removed_meanwhile_does_not_land() {
        let c = |id| Correction { id, stroke: Vec::new(), radius: 20.0 };
        let (stays, removed) = (Id::new(), Id::new());
        let made = vec![(stays, false, (0.1, 0.0)), (removed, true, (0.0, 0.2))];
        let (kept, gone) = landed(made.clone(), &[c(stays)]);
        assert_eq!(kept.iter().map(|m| m.0).collect::<Vec<_>>(), vec![stays]);
        assert_eq!(gone, vec![removed]);
        let (kept, gone) = landed(made, &[c(stays), c(removed)]);
        assert_eq!((kept.len(), gone.len()), (2, 0), "twin: with both still there, both land");
    }
}
