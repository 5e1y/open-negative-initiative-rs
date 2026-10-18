//! The canvas's render loop: asking a render, landing it, the full-resolution region, zoom, compare.

use super::*;

impl Ctl {
    // MARK: - Rendering

    /// Lays the open frame's current settings in the render slot, when they differ from the last asked.
    pub(super) fn request_render(self: &Rc<Self>) {
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
            let build = (wants_full && full.is_none() && s.building_full.as_deref() != Some(path.as_str())).then(|| {
                s.building_full = Some(path.clone());
                (path.clone(), render::Reading::of(s.store.settings()))
            });
            let view = size.map(|size| View { viewport: s.viewport, size, full, settled });
            // The cache is purged at every quit, so each correction's patch is generated as its frame opens.
            let caches = crate::patches::caches();
            let stored = s.store.settings().clone();
            let unhealed = stored.corrections.iter().any(|c| !c.stroke.is_empty() && !crate::patches::file(&caches, c.id).exists());
            let heal = (unhealed && s.healing.as_deref() != Some(path.as_str())).then(|| {
                s.healing = Some(path.clone());
                (path.clone(), stored)
            });
            // `NegativeView.referenceSettings`: the bar's reset frame, the dust still healed.
            let reference = s.split.map(|_| {
                let mut reference = oni_core::parameters::imported(&live);
                reference.corrections = live.corrections.clone();
                reference
            });
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
    pub(super) fn heal(self: &Rc<Self>, path: String, settings: oni_settings::PipelineSettings) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let caches = crate::patches::caches();
            let made = (|| -> Result<Vec<(oni_settings::id::Id, bool, (f64, f64))>, String> {
                let decoded = render::decode_full(Path::new(&path), render::Reading::of(&settings))?;
                let full = engine.lock()?.full(decoded)?;
                let mut made = Vec::new();
                for c in &settings.corrections {
                    if c.stroke.is_empty() || crate::patches::file(&caches, c.id).exists() {
                        continue;
                    }
                    let (region, size) = engine.lock()?.search_region(&full, &settings, c)?;
                    let patch = crate::heal::generate(&region, c, size.0, size.1);
                    crate::patches::write(&caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
                    engine.lock()?.forget_patch(c.id);
                    made.push((c.id, patch.fallback, patch.offset));
                }
                Ok(made)
            })();
            with_shell(move |c| {
                c.state.borrow_mut().healing = None;
                match made {
                    Ok(m) if m.is_empty() => {}
                    Ok(m) => {
                        let n = m.len();
                        for (id, fell_back, offset) in m {
                            let mut s = c.state.borrow_mut();
                            if fell_back {
                                s.fallback.insert(id);
                            }
                            s.sources.insert(id, offset);
                        }
                        timing::line(&format!("{}: {n} dust patches generated", stem(&path)));
                        if c.state.borrow().open_path() == Some(path.as_str()) {
                            c.state.borrow_mut().requested_view = None;
                            c.request_render();
                        }
                    }
                    Err(e) => timing::line(&format!("{}: dust patches not generated: {e}", stem(&path))),
                }
                c.publish_dedust();
            });
        });
    }

    /// The canvas in drawable pixels, once laid out.
    pub(super) fn canvas_size(&self) -> Option<(f64, f64)> {
        let w = self.window()?;
        let (c, scale) = (w.global::<Canvas>(), f64::from(w.window().scale_factor()));
        let size = (f64::from(c.get_width()) * scale, f64::from(c.get_height()) * scale);
        (size.0 >= 1.0 && size.1 >= 1.0).then_some(size)
    }

    /// The open frame decoded again at full resolution off this thread, then shown where the view wants it.
    pub(super) fn build_full(self: &Rc<Self>, path: String, converted: render::Reading) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let built = render::decode_full(Path::new(&path), converted).and_then(|d| engine.lock()?.full(d));
            with_shell(move |c| {
                let open = c.state.borrow().open_path() == Some(path.as_str());
                {
                    let mut s = c.state.borrow_mut();
                    s.building_full = None;
                    match built {
                        Ok(full) if open => s.full = Some((path, full)),
                        Ok(_) => {}
                        Err(e) => timing::line(&format!("full resolution not built: {e}")),
                    }
                }
                if open {
                    c.request_render();
                }
            });
        });
    }

    /// Applies a navigation at once on the preview, then asks again once it has settled.
    pub(super) fn navigate(self: &Rc<Self>, change: impl FnOnce(&mut Viewport, (f64, f64), (f64, f64))) {
        let Some(size) = self.canvas_size() else { return };
        {
            let mut s = self.state.borrow_mut();
            let Some(full) = s.full_size else { return };
            let before = s.viewport;
            change(&mut s.viewport, full, size);
            if s.viewport == before {
                return;
            }
            s.navigated_at = Some(Instant::now());
        }
        self.request_render();
        let me = Rc::downgrade(self);
        // The timer marks the view settled itself: it can fire a hair before the debounce reads as elapsed.
        self.settle.start(TimerMode::SingleShot, viewport::SETTLE, move || {
            if let Some(me) = me.upgrade() {
                me.state.borrow_mut().navigated_at = None;
                me.request_render();
            }
        });
    }

    /// The bar's − / +: the zoom ladder in the editor.
    pub(super) fn step_zoom(self: &Rc<Self>, by: i32) {
        let rung = {
            let s = self.state.borrow();
            let (Some(full), Some(size)) = (s.full_size, self.canvas_size()) else { return };
            Viewport::rung(s.viewport.effective_scale(full, size), s.viewport.is_fitted(), by)
        };
        if let Some(rung) = rung {
            self.navigate(|v, full, size| v.step(rung, full, size));
        }
    }

    /// A render arrived: stale ones are dropped, the GPU path encodes its texture into the canvas,
    /// the software renderer shows the bytes encoded on the render thread.
    pub(super) fn deliver(self: &Rc<Self>, done: Result<Done, String>) {
        let Some(w) = self.window() else { return };
        let done = match done {
            Ok(d) => d,
            Err(e) => {
                // The canvas never keeps another photo's picture under this one's name.
                w.global::<Canvas>().set_frame(Image::default());
                w.global::<Canvas>().set_busy(false);
                w.global::<Plots>().set_output(Image::default());
                self.state.borrow_mut().histogram_dirty = false;
                self.say(&format!("Not rendered: {e}"));
                return;
            }
        };
        {
            let s = self.state.borrow();
            let current = s.open_path().map(PathBuf::from);
            if current.as_ref() != Some(&done.path) || done.revision < s.rendered {
                return;
            }
        }
        let shown = Shown { kind: done.kind, asked: done.asked, pipeline: done.pipeline, frame: stem(&done.path.to_string_lossy()) };
        let (width, height) = (done.width, done.height);
        self.lay(done.placed, (width, height));
        if let Some((before, placed)) = done.before {
            self.encode_before(before, placed);
        }
        let on_gpu = {
            let mut s = self.state.borrow_mut();
            s.rendered = done.revision.max(1);
            s.renders += 1;
            if let Some(h) = done.histogram {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            if let Some(i) = done.inputs {
                s.inputs = Some(i);
                s.inputs_drawn = None;
            }
            s.gpu.is_some()
        };
        self.publish_inputs();
        if on_gpu {
            let whole = done.placed.is_none_or(|p| p.whole);
            self.encode(done.picture, shown, whole);
            w.window().request_redraw();
        } else {
            if let Picture::Pixels { rgba8, .. } = done.picture {
                w.global::<Canvas>().set_frame(image(&rgba8, width, height));
            }
            self.shown(shown);
        }
        let status = {
            let s = self.state.borrow();
            let name = s.open_path().map(stem).unwrap_or_default();
            match s.placed {
                Some(p) => format!("{name} · {:.0} × {:.0}", p.full.0, p.full.1),
                None => format!("{name} · {width} × {height}"),
            }
        };
        w.global::<Canvas>().set_busy(false);
        w.global::<Canvas>().set_status(status.into());
    }

    /// Lays the render where the viewport puts it; a render with no place fills the canvas, fitted.
    pub(super) fn lay(&self, placed: Option<Placed>, size: (usize, usize)) {
        let Some(w) = self.window() else { return };
        let scale = f64::from(w.window().scale_factor());
        let rect = match (placed, self.canvas_size()) {
            (Some(p), _) => (p.x, p.y, p.width, p.height),
            (None, Some(view)) => Viewport::default().placement((size.0 as f64, size.1 as f64), view).image,
            (None, None) => return,
        };
        {
            let mut s = self.state.borrow_mut();
            s.placed = placed;
            if let Some(p) = placed {
                s.full_size = Some(p.full);
            }
        }
        let c = w.global::<Canvas>();
        c.set_frame_x((rect.0 / scale) as f32);
        c.set_frame_y((rect.1 / scale) as f32);
        c.set_frame_width((rect.2 / scale) as f32);
        c.set_frame_height((rect.3 / scale) as f32);
        let bubble = {
            let s = self.state.borrow();
            match (placed, self.canvas_size()) {
                (Some(p), Some(size)) => {
                    let current = s.viewport.effective_scale(p.full, size);
                    let fitted = s.viewport.is_fitted();
                    Some((viewport::zoom_label(current, p.at_full_resolution), Viewport::rung(current, fitted, -1).is_some(), Viewport::rung(current, fitted, 1).is_some()))
                }
                _ => None,
            }
        };
        let (label, out, inward) = bubble.unwrap_or_default();
        c.set_zoom_label(label.into());
        c.set_can_zoom_out(out);
        c.set_can_zoom_in(inward);
        self.publish_framing();
        self.publish_dedust();
    }

    /// Compare's before into its own texture, laid where its view puts it.
    pub(super) fn encode_before(&self, before: oni_graph::Source, placed: Option<Placed>) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let (imported, rect) = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(g) = s.gpu.as_mut() else { return };
            let Some(view) = before.view().cloned() else { return };
            let e = before.extent();
            let reuse = g.before.as_ref().filter(|(t, _)| t.width() == e.w && t.height() == e.h).map(|(t, _)| t.clone());
            let (target, imported) = match reuse {
                Some(t) => (t, None),
                None => {
                    let t = display::Gpu::target(&g.device, e.w, e.h);
                    (t.clone(), Image::try_from(t).ok())
                }
            };
            let k = zebra(&s, Params::new(screen, e.w, e.h, s.gpu.as_ref().map_or(0, |g| g.seed)), placed, e.w, canvas);
            let Some(g) = s.gpu.as_mut() else { return };
            g.kernel.run(&g.device, &g.queue, &view, &k, &target);
            if g.before.as_ref().is_none_or(|(_, kept)| kept.id() != before.id()) {
                s.befores += 1;
            }
            s.before_placed = placed;
            let Some(g) = s.gpu.as_mut() else { return };
            g.before = Some((target, before));
            (imported, placed)
        };
        let c = w.global::<Canvas>();
        if let Some(image) = imported {
            c.set_before(image);
        }
        if let Some(p) = rect {
            let scale = f64::from(w.window().scale_factor());
            c.set_before_x((p.x / scale) as f32);
            c.set_before_y((p.y / scale) as f32);
            c.set_before_width((p.width / scale) as f32);
            c.set_before_height((p.height / scale) as f32);
        }
    }

    /// `ScreenGamut`'s stripes on and off; while on the canvas is encoded again at their pace,
    /// from the render it holds, so the stripes move and the pipeline never runs for them.
    pub(super) fn toggle_screen_gamut(self: &Rc<Self>) {
        let on = {
            let mut s = self.state.borrow_mut();
            s.screen_gamut = !s.screen_gamut;
            s.screen_gamut
        };
        if on {
            let me = Rc::downgrade(self);
            let pace = Duration::from_secs_f64(1.0 / f64::from(oni_colour::gamut::FRAMES_PER_SECOND));
            self.stripes.start(TimerMode::Repeated, pace, move || {
                if let Some(me) = me.upgrade() {
                    me.restripe();
                }
            });
        } else {
            self.stripes.stop();
        }
        self.restripe();
    }

    /// The canvas, and Compare's before, encoded again from the renders they hold.
    pub(super) fn restripe(&self) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        {
            let s = self.state.borrow();
            let screen = s.screen;
            let Some(g) = s.gpu.as_ref() else { return };
            if let (Some(Picture::Resident(source)), Some(target), Some(view)) = (&g.current, &g.target, g.current.as_ref().and_then(picture_view)) {
                let e = source.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, &view, &k, target);
            }
            if let Some((target, before)) = &g.before
                && let Some(view) = before.view()
            {
                let e = before.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.before_placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, view, &k, target);
            }
        }
        w.window().request_redraw();
    }

    /// Compare Before/After on and off, the split at half the canvas.
    pub(super) fn toggle_compare(self: &Rc<Self>) {
        let split = {
            let mut s = self.state.borrow_mut();
            s.split = if s.split.is_some() { None } else { Some(0.5) };
            s.split
        };
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_split(split.unwrap_or(-1.0));
        }
        self.request_render();
    }

    /// On a wgpu renderer the display kernel writes the canvas texture, which the next frame draws.
    pub(super) fn encode(&self, picture: Picture, shown: Shown, whole: bool) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let image = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(g) = s.gpu.as_mut() else { return };
            let uploaded;
            let (source, wd, ht) = match &picture {
                Picture::Resident(source) => {
                    let e = source.extent();
                    (source.view().expect("a resident render is one texture").clone(), e.w, e.h)
                }
                Picture::Pixels { linear, .. } => {
                    let (w, h) = (linear.width as u32, linear.height as u32);
                    uploaded = display::upload(&g.device, &g.queue, &linear.pixels, w, h);
                    (uploaded.create_view(&Default::default()), w, h)
                }
            };
            // The texture is imported once per size; a new render only rewrites its texels.
            let (target, imported) = match &g.target {
                Some(t) if t.width() == wd && t.height() == ht => (t.clone(), None),
                _ => {
                    let t = display::Gpu::target(&g.device, wd, ht);
                    g.target = Some(t.clone());
                    (t.clone(), Image::try_from(t).ok())
                }
            };
            g.seed = g.seed.wrapping_add(1);
            let seed = g.seed;
            let k = zebra(&s, Params::new(screen, wd, ht, seed), s.placed, wd, canvas);
            let Some(g) = s.gpu.as_mut() else { return };
            g.kernel.run(&g.device, &g.queue, &source, &k, &target);
            g.current = Some(picture);
            // A region is not the picture: its histogram would describe what the zoom shows.
            g.uncounted = whole;
            (imported, shown)
        };
        if let (Some(image), _) = &image {
            w.global::<Canvas>().set_frame(image.clone());
        }
        self.shown(image.1);
    }

    /// A render reached the screen: its line, and a notch's place in the centiles.
    pub(super) fn shown(&self, shown: Shown) {
        let end_to_end = shown.asked.elapsed();
        timing::render(shown.kind, &shown.frame, shown.pipeline, end_to_end);
        if shown.kind == Kind::View {
            self.state.borrow_mut().views.record(end_to_end, Instant::now());
        }
        if shown.kind == Kind::Notch {
            let report = {
                let mut s = self.state.borrow_mut();
                s.notches.record(end_to_end, Instant::now());
                s.notches.due()
            };
            if let Some(report) = report {
                timing::line(&report);
            }
        }
    }

    /// The histogram and the load counter, on their own pace so a notch touches neither.
    pub(super) fn pulse(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // A tab opened or a full frame landed reaches the loupe at this pace when no render asks.
        self.request_loupe();
        let (histogram, loading) = {
            let mut s = self.state.borrow_mut();
            if let Some(h) = s.gpu.as_mut().and_then(Gpu::landed) {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            let histogram = if s.histogram_dirty {
                s.histogram_dirty = false;
                s.histogram.clone()
            } else {
                None
            };
            (histogram, s.load_bar())
        };
        if let Some(h) = histogram {
            let (pw, ph) = HISTOGRAM;
            w.global::<Plots>().set_output(premultiplied(&h.raster(pw, ph), pw, ph));
        }
        if w.global::<App>().get_gallery() {
            w.global::<Library>().set_loading(loading.unwrap_or_default().into());
        }
    }

    /// The levels' and the manual curves' plots over their input distributions, each with its own
    /// channel in focus; drawn again only when a reading lands or a channel tab moves.
    pub(crate) fn publish_inputs(&self) {
        let Some(w) = self.window() else { return };
        let focus = {
            let g = self.editors.gestures.borrow();
            (g.levels_channel().histogram_channel(), g.curve_channel().histogram_channel())
        };
        let inputs = {
            let mut s = self.state.borrow_mut();
            if s.inputs_drawn == Some(focus) {
                return;
            }
            s.inputs_drawn = Some(focus);
            s.inputs.clone()
        };
        let (pw, ph) = HISTOGRAM;
        let plots = w.global::<Plots>();
        match inputs {
            Some(i) => {
                plots.set_levels(premultiplied(&render::InputHistograms::raster(&i.levels, focus.0, pw, ph), pw, ph));
                plots.set_curves(premultiplied(&render::InputHistograms::raster(&i.curves, focus.1, pw, ph), pw, ph));
            }
            None => {
                plots.set_levels(Image::default());
                plots.set_curves(Image::default());
            }
        }
    }

    /// The deferred write, `WRITE_DELAY` after the last change; a hand still on a slider puts it off
    /// until its release, which asks again.
    pub(super) fn schedule_write(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.writer.start(TimerMode::SingleShot, WRITE_DELAY, move || {
            let Some(me) = me.upgrade() else { return };
            let failure = {
                let mut s = me.state.borrow_mut();
                if s.slides.iter().any(|sl| sl.live.is_some()) {
                    return;
                }
                s.last_change = None;
                s.store.flush()
            };
            me.journal_settled();
            match failure {
                Some(message) => me.say(&message),
                None => me.schedule_redraw(),
            }
        });
    }

    /// Writes the open frame's sidecar now when the disk differs.
    pub(crate) fn flush(&self) {
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.say(&message);
        }
    }

    pub(super) fn say(&self, text: &str) {
        self.state.borrow_mut().status = text.to_string();
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_status(text.into());
        }
    }
}
