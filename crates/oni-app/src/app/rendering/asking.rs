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
            let view = size.map(|size| View { viewport: s.viewport, size, full, settled });
            // The cache is purged at every quit, so each correction's patch is generated as its frame opens.
            let heal = (!s.healing.contains(&path) && s.unhealed()).then(|| {
                s.healing.insert(path.clone());
                (path.clone(), s.store.settings().clone())
            });
            let reference = s.split.map(|_| oni_core::compare::reference(&live));
            // A notch is the hand at work: the input histograms read the small copy until it stops.
            let held = kind == Kind::Notch;
            (Job { revision, path: PathBuf::from(path), negative, settings: live, kind, asked, view, reference, held }, build, heal)
        };
        self.worker.submit(job);
        self.request_loupe();
        if let Some((path, converted)) = build {
            self.build_full(path, converted);
        }
        if let Some((path, settings)) = heal {
            self.heal(path, settings);
            self.publish_dedust();
        }
    }

    /// `ensureGenerated` for each correction of the open frame that has no patch yet, one after the
    /// other on the full-resolution frame, so each source holds the patches before it healed.
    pub(in crate::app) fn heal(self: &Rc<Self>, path: String, settings: oni_settings::PipelineSettings) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let caches = crate::patches::caches();
            let lacking = |c: &oni_settings::frame::Correction| !c.stroke.is_empty() && !crate::patches::file(&caches, c.id).exists();
            let mut made = Vec::new();
            let failed = (|| -> Result<(), String> {
                let decoded = render::decode_full(Path::new(&path), render::Reading::of(&settings))?;
                let full = engine.full(decoded)?;
                for c in settings.corrections.iter().filter(|c| lacking(c)) {
                    let (region, size) = engine.lock()?.search_region(&full, &settings, c)?;
                    let patch = crate::heal::generate(&region, c, size.0, size.1);
                    crate::patches::write(&caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
                    engine.lock()?.forget_patch(c.id);
                    made.push((c.id, patch.fallback, patch.offset));
                }
                Ok(())
            })()
            .err();
            // A correction left without its patch by a failure is not generated again on every render.
            let unhealable: Vec<oni_settings::id::Id> =
                if failed.is_some() { settings.corrections.iter().filter(|c| lacking(c)).map(|c| c.id).collect() } else { Vec::new() };
            with_shell(move |c| {
                let open = c.state.borrow().open_path() == Some(path.as_str());
                {
                    let mut s = c.state.borrow_mut();
                    s.healing.remove(&path);
                    s.unhealable.extend(unhealable);
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
                    timing::line(&format!("{}: {} dust patches generated", stem(&path), made.len()));
                    if open {
                        c.state.borrow_mut().requested_view = None;
                        c.request_render();
                    }
                }
                if let Some(e) = failed {
                    timing::line(&format!("{}: dust patches not generated: {e}", stem(&path)));
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
                    stem(&path),
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
