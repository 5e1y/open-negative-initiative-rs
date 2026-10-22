//! A render landing: stale ones dropped, the rest encoded into the canvas where the viewport lays it.

use super::*;

impl Ctl {
    /// A render arrived: stale ones are dropped, the rest encoded into the canvas.
    pub(in crate::app) fn deliver(self: &Rc<Self>, delivery: Delivery) {
        let done = match delivery {
            // Frames edited together own the canvas; the open one's render waits for the group to end.
            Delivery::Render(_) if self.together_shown() => return,
            Delivery::Render(done) => done,
            Delivery::Inputs { path, inputs } => return self.inputs_landed(&path, inputs),
            Delivery::Counted { .. } if self.together_shown() => return,
            Delivery::Counted { path, picture } => return self.counted_landed(&path, picture),
        };
        let Some(w) = self.window() else { return };
        let done = match done {
            Ok(d) => d,
            Err((path, e)) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if self.state.borrow().open_path().map(PathBuf::from) != Some(path) {
                    return;
                }
                // The canvas never keeps another photo's picture under this one's name.
                self.blank_canvas();
                w.global::<Canvas>().set_busy(false);
                w.global::<Canvas>().set_failure(format!("{name}: could not be decoded").into());
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
        let shown = Shown { kind: done.kind, asked: done.asked, pipeline: done.pipeline, frame: crate::anonymity::token(&done.path.to_string_lossy()) };
        let e = done.picture.extent();
        let (width, height) = (e.w as usize, e.h as usize);
        self.lay(done.placed, (width, height));
        if let Some((before, placed)) = done.before {
            self.encode_before(before, placed);
        }
        {
            let mut s = self.state.borrow_mut();
            s.rendered = done.revision.max(1);
            s.renders += 1;
        }
        self.encode(done.picture, shown, None);
        w.window().request_redraw();
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

    /// The histogram's picture of the open frame, counted at the image's cadence; another photo's is dropped.
    fn counted_landed(&self, path: &Path, picture: oni_graph::Source) {
        {
            let mut s = self.state.borrow_mut();
            if s.open_path().map(Path::new) != Some(path) {
                return;
            }
            s.gpu.uncounted = Some(picture);
            s.gpu.count_due();
        }
        self.watch_histogram();
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

    /// Lays the render where the viewport puts it; a picture with no place (a thumbnail waiting for its
    /// render) where the viewport will put the photo, its size guessed from the last photo's.
    pub(in crate::app) fn lay(&self, placed: Option<Placed>, size: (usize, usize)) {
        let Some(w) = self.window() else { return };
        let scale = f64::from(w.window().scale_factor());
        let rect = match (placed, self.canvas_size()) {
            (Some(p), _) => (p.x, p.y, p.width, p.height),
            (None, Some(view)) => {
                let s = self.state.borrow();
                s.viewport.placement(super::placeholder::guessed_full((size.0 as f64, size.1 as f64), s.full_size), view).image
            }
            (None, None) => return,
        };
        resizing::laid_in(self.canvas_size());
        let c = w.global::<Canvas>();
        // A render landing where the last one lay, as every notch's does, moves nothing to publish.
        let at = [rect.0, rect.1, rect.2, rect.3].map(|v| (v / scale) as f32);
        let unmoved = placed.is_some() && self.state.borrow().placed == placed && at == [c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height()];
        if unmoved {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            s.placed = placed;
            if let Some(p) = placed {
                s.full_size = Some(p.full);
            }
        }
        c.set_frame_x(at[0]);
        c.set_frame_y(at[1]);
        c.set_frame_width(at[2]);
        c.set_frame_height(at[3]);
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

    /// The display kernel writes the canvas texture, which the next frame draws, or which is read back
    /// on a window that cannot take it.
    pub(in crate::app) fn encode(&self, picture: oni_graph::Source, shown: Shown, counted: Option<oni_graph::Source>) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let mut refused: Option<String> = None;
        let image = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let g = &mut s.gpu;
            let e = picture.extent();
            let (source, wd, ht) = (picture.view().expect("a resident render is one texture").clone(), e.w, e.h);
            // The texture is imported once per step of size, the render written in its corner and shown
            // clipped, so a pan or a pinch rewrites texels only. One Slint refuses is not kept, so the next
            // render tries again rather than drawing nowhere.
            let (bw, bh) = (wd.div_ceil(TARGET_STEP) * TARGET_STEP, ht.div_ceil(TARGET_STEP) * TARGET_STEP);
            let (target, imported) = match &g.target {
                Some(t) if t.width() == bw && t.height() == bh => (t.clone(), None),
                _ if g.reads_back => {
                    let t = display::Gpu::target(&g.device, bw, bh);
                    g.target = Some(t.clone());
                    (t, None)
                }
                _ => {
                    let t = display::Gpu::target(&g.device, bw, bh);
                    match Image::try_from(t.clone()) {
                        Ok(image) => {
                            g.target = Some(t.clone());
                            (t, Some(image))
                        }
                        Err(e) => {
                            refused = Some(e.to_string());
                            (t, None)
                        }
                    }
                }
            };
            g.seed = g.seed.wrapping_add(1);
            let seed = g.seed;
            let k = zebra(&s, Params::new(screen, wd, ht, seed), s.placed, wd, canvas);
            let g = &mut s.gpu;
            g.kernel.run(&g.device, &g.queue, &source, &k, &target);
            let imported = imported.or_else(|| g.read_back(&target, (wd, ht)));
            g.current = Some(picture);
            // The histogram describes the whole picture at one size, never the zoom's part of it.
            if counted.is_some() {
                g.uncounted = counted;
            }
            g.count_due();
            (imported, shown, (wd, ht))
        };
        self.watch_histogram();
        let c = w.global::<Canvas>();
        if let (Some(image), _, _) = &image {
            c.set_frame(image.clone());
        }
        c.set_frame_clip_width(image.2.0 as i32);
        c.set_frame_clip_height(image.2.1 as i32);
        c.set_frame_revision(c.get_frame_revision().wrapping_add(1));
        if let Some(e) = refused {
            timing::line(&format!("the canvas cannot take the render's texture: {e}"));
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                self.say("The picture cannot be shown on this graphics card; the log names why.");
            }
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

/// The display texture grows and shrinks by this many pixels a side, so a pan or a pinch keeps it.
const TARGET_STEP: u32 = 256;
