//! A render landing: stale ones dropped, the rest encoded into the canvas where the viewport lays it.

use super::*;

impl Ctl {
    /// A render arrived: stale ones are dropped, the GPU path encodes its texture into the canvas,
    /// the software renderer shows the bytes encoded on the render thread.
    pub(in crate::app) fn deliver(self: &Rc<Self>, delivery: Delivery) {
        let done = match delivery {
            Delivery::Render(done) => done,
            Delivery::Inputs { path, inputs } => return self.inputs_landed(&path, inputs),
        };
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
            s.gpu.is_some()
        };
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

    /// The input distributions of a render already shown; another photo's are dropped.
    fn inputs_landed(&self, path: &Path, inputs: InputHistograms) {
        {
            let mut s = self.state.borrow_mut();
            if s.open_path().map(Path::new) != Some(path) {
                return;
            }
            s.inputs = Some(inputs);
            s.inputs_drawn = None;
        }
        self.publish_inputs();
    }

    /// Lays the render where the viewport puts it; a render with no place fills the canvas, fitted.
    pub(in crate::app) fn lay(&self, placed: Option<Placed>, size: (usize, usize)) {
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

    /// On a wgpu renderer the display kernel writes the canvas texture, which the next frame draws.
    pub(in crate::app) fn encode(&self, picture: Picture, shown: Shown, whole: bool) {
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
            g.count_due();
            (imported, shown)
        };
        self.watch_histogram();
        if let (Some(image), _) = &image {
            w.global::<Canvas>().set_frame(image.clone());
        }
        self.shown(image.1);
    }

    /// A render reached the screen: its line, and a notch's place in the centiles.
    pub(in crate::app) fn shown(&self, shown: Shown) {
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
}
