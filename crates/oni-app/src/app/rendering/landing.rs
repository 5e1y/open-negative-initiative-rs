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
                if self.state.borrow().open_path().map(PathBuf::from) != Some(path.clone()) {
                    return;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                return self.canvas_failed(tr!("{name}: could not be rendered", name = name), false, &e);
            }
        };
        {
            let s = self.state.borrow();
            let current = s.open_path().map(PathBuf::from);
            if current.as_ref() != Some(&done.path) || done.revision < s.requests.rendered {
                return;
            }
        }
        // A render of the open frame that landed is the truth about it: no failure stands over it.
        w.global::<Canvas>().set_failure("".into());
        w.global::<Canvas>().set_convertible(false);
        let shown = Shown { kind: done.kind, asked: done.asked, pipeline: done.pipeline, frame: crate::anonymity::token(&done.path.to_string_lossy()) };
        let e = done.picture.extent();
        let (width, height) = (e.w as usize, e.h as usize);
        self.lay(done.placed, (width, height));
        if let Some((before, placed)) = done.before {
            self.encode_before(before, placed);
        }
        {
            let mut s = self.state.borrow_mut();
            s.requests.rendered = done.revision.max(1);
            s.renders += 1;
        }
        self.encode(done.picture, shown, None);
        w.window().request_redraw();
        // The photo's kept size, the crop's own figure, never the render's reduced extent scaled back up.
        let status = {
            let (name, place) = {
                let s = self.state.borrow();
                let path = s.open_path();
                // Its place on the roll, which a name alone loses on a long one.
                let place = path.and_then(|p| s.archive.as_ref()?.place(p));
                let place = place.map(|(number, count)| format!(" · {}", tr!("Frame {number} of {count}", number = number, count = count)));
                (path.map(stem).unwrap_or_default(), place.unwrap_or_default())
            };
            match self.kept_size() {
                Some((w, h)) => format!("{name}{place} · {w:.0} × {h:.0}"),
                None => format!("{name}{place} · {width} × {height}"),
            }
        };
        w.global::<Canvas>().set_busy(false);
        w.global::<Canvas>().set_status(status.into());
    }

    /// The open frame's decode failed under the reading it still wants: the card says why, and offers
    /// the conversion when the file is a Nikon HE RAW.
    pub(in crate::app) fn decode_failed(self: &Rc<Self>, path: &str, e: String) {
        let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let failure = if e == crate::render::cut_short() { tr!("{name}: cut short", name = name) } else { tr!("{name}: could not be decoded", name = name) };
        let convertible = crate::adobe_dng::is_nikon_he(Path::new(path));
        self.canvas_failed(failure, convertible, &format!("{}: {e}", stem(path)));
    }

    /// The canvas emptied under a failure card, as it never keeps another picture under this frame's name.
    fn canvas_failed(&self, failure: String, convertible: bool, said: &str) {
        let Some(w) = self.window() else { return };
        self.blank_canvas();
        w.global::<Canvas>().set_busy(false);
        w.global::<Canvas>().set_failure(failure.into());
        w.global::<Canvas>().set_convertible(convertible);
        self.clear_output();
        self.say(&tr!("Not rendered: {e}", e = said));
    }

    /// The output histogram emptied, for a frame with no render to count.
    pub(in crate::app) fn clear_output(&self) {
        if let Some(w) = self.window() {
            super::plots::clear(&w, super::plots::Slot::Output);
        }
        self.state.borrow_mut().histogram_dirty = false;
    }

    /// The histogram's picture of the open frame, counted at the image's cadence; another photo's is dropped.
    fn counted_landed(&self, path: &Path, picture: oni_graph::Source) {
        {
            let mut s = self.state.borrow_mut();
            if s.open_path().map(Path::new) != Some(path) {
                return;
            }
            let s = &mut *s;
            if s.requests.count.take(picture, false) {
                s.gpu.count_due(&mut s.requests.count);
            }
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
            if s.requests.inputs.as_ref() == Some(&inputs) {
                return;
            }
            s.requests.inputs = Some(inputs);
            s.requests.inputs_drawn = None;
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
        crate::hz::changed();
        c.set_frame_x(at[0]);
        c.set_frame_y(at[1]);
        c.set_frame_width(at[2]);
        c.set_frame_height(at[3]);
        let bubble = {
            let s = self.state.borrow();
            match (placed, self.canvas_size()) {
                (Some(p), Some(size)) => {
                    let current = s.viewport.effective_scale(p.full, size);
                    let ratio = s.negative.as_ref().filter(|(_, n)| n.full_width > 0.0).map_or(1.0, |(_, n)| f64::from(n.size().0) / n.full_width);
                    let enlarged = viewport::enlarged(current, ratio, p.at_full_resolution);
                    Some((viewport::zoom_label(current), enlarged, s.viewport.rung(-1, p.full, size).is_some(), s.viewport.rung(1, p.full, size).is_some()))
                }
                _ => None,
            }
        };
        let (label, enlarged, out, inward) = bubble.unwrap_or_default();
        // A wheel, a pinch or a double click moves the view outside any menu item: ⌘− and ⌘+ follow it.
        let reach = (c.get_can_zoom_out(), c.get_can_zoom_in()) != (out, inward);
        c.set_zoom_label(label.into());
        c.set_enlarged(enlarged);
        c.set_can_zoom_out(out);
        c.set_can_zoom_in(inward);
        self.publish_framing();
        self.publish_dedust();
        if reach {
            self.publish_menus();
        }
    }

    /// The display kernel writes the canvas texture, which the next frame draws, or which is read back
    /// on a window that cannot take it.
    pub(in crate::app) fn encode(&self, picture: oni_graph::Source, shown: Shown, counted: Option<oni_graph::Source>) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let mut refused: Option<String> = None;
        let image = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let screen = s.screen;
            let g = &mut s.gpu;
            let e = picture.extent();
            let (source, wd, ht) = (picture.view().expect("a resident render is one texture").clone(), e.w, e.h);
            // The texture is imported once per step of size, the render written in its corner and shown
            // clipped, so a pan or a pinch rewrites texels only. One Slint refuses is not kept, so the next
            // render tries again rather than drawing nowhere.
            let (bw, bh) = (wd.div_ceil(TARGET_STEP) * TARGET_STEP, ht.div_ceil(TARGET_STEP) * TARGET_STEP);
            g.seed = g.seed.wrapping_add(1);
            let seed = g.seed;
            let k = grounded(s, zebra(s, Params::new(screen, wd, ht, seed), s.placed, wd, canvas));
            let g = &mut s.gpu;
            let (target, encoded) = g.encode(g.target.as_ref(), (bw, bh), &source, &k, (wd, ht));
            let imported = match encoded {
                Ok(image) => {
                    g.target = Some(target);
                    image
                }
                Err(e) => {
                    refused = Some(e);
                    None
                }
            };
            g.current = Some(picture);
            // The histogram describes the whole picture at one size, never the zoom's part of it.
            if let Some(picture) = counted {
                s.requests.count.take(picture, true);
            }
            s.gpu.count_due(&mut s.requests.count);
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
        crate::hz::changed();
        if let Some(e) = refused {
            timing::line(&format!("the canvas cannot take the render's texture: {e}"));
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                self.say(tr("The picture cannot be shown on this graphics card; the log names why."));
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
