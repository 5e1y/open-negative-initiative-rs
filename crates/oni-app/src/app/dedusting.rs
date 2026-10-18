//! De-dust: the brush, its strokes, the patches and the corrections list on the canvas.

use super::*;

impl Ctl {
    // MARK: - De-dust

    /// The De-dust pane open on a photo in the editor: a drag on the canvas paints.
    pub(super) fn painting(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 4 && self.state.borrow().open.is_some()
    }

    /// `toggleCorrecting`: into the hidden pane, or out to the panel the hand was in, never the crop.
    pub(super) fn toggle_correcting(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let panel = w.global::<PanelState>();
        let tab = panel.get_tab();
        let next = if tab == 4 {
            let last = self.state.borrow().last_panel;
            if last == 1 || last == 4 { 2 } else { last }
        } else {
            self.state.borrow_mut().last_panel = tab;
            4
        };
        panel.set_tab(next);
        w.global::<Bar>().set_dedust(next == 4);
        self.tab_opened(next);
        self.publish_dedust();
    }

    /// The brush, the corrections' rows and the canvas's marks from the open frame.
    pub(super) fn publish_dedust(&self) {
        let Some(w) = self.window() else { return };
        let painting = self.painting();
        let frame = self.image_frame(false);
        let caches = crate::patches::caches();
        let (rows, marks, radius, stroke, rendered, traces, lines) = {
            let mut s = self.state.borrow_mut();
            let corrections = s.store.settings().corrections.clone();
            for c in &corrections {
                if !s.numbers.contains_key(&c.id) {
                    let n = s.next_number;
                    s.numbers.insert(c.id, n);
                    s.next_number += 1;
                }
            }
            let healing = s.healing.is_some() && s.healing.as_deref() == s.open_path();
            let rows: Vec<crate::CorrectionRow> = corrections
                .iter()
                .map(|c| crate::CorrectionRow {
                    label: s.numbers.get(&c.id).map_or("Correction".to_string(), |n| format!("Correction {n}")).into(),
                    pending: healing && !crate::patches::file(&caches, c.id).exists(),
                    fallback: s.fallback.contains(&c.id),
                })
                .collect();
            let rendered = s.full_size;
            let marks: Vec<(f64, f64, f64, f64, usize)> = match (frame, rendered) {
                (Some(f), Some(r)) => corrections
                    .iter()
                    .filter(|c| !c.stroke.is_empty())
                    .map(|c| {
                        // `boundingFraction`: the stroke's extent padded by its radius, in fractions.
                        let (rx, ry) = (c.radius / r.0, c.radius / r.1);
                        let (x0, x1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
                        let (y0, y1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                        let (x, y, wd, ht) = (x0 - rx, y0 - ry, x1 - x0 + 2.0 * rx, y1 - y0 + 2.0 * ry);
                        (f.0 + x * f.2, f.1 + y * f.3, wd * f.2, ht * f.3, s.numbers.get(&c.id).copied().unwrap_or(0))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // `correctionMarks`: the line from each stroke to its source, and the copied stroke itself.
            let traces: Vec<(Vec<(f64, f64)>, f64)> = corrections
                .iter()
                .filter_map(|c| {
                    let o = *s.sources.get(&c.id)?;
                    Some((c.stroke.iter().map(|p| (p.x + o.0, p.y + o.1)).collect(), c.radius))
                })
                .collect();
            let lines: Vec<((f64, f64), (f64, f64))> = corrections
                .iter()
                .filter_map(|c| {
                    let o = s.sources.get(&c.id)?;
                    let n = c.stroke.len().max(1) as f64;
                    let centre = (c.stroke.iter().map(|p| p.x).sum::<f64>() / n, c.stroke.iter().map(|p| p.y).sum::<f64>() / n);
                    Some((centre, (centre.0 + o.0, centre.1 + o.1)))
                })
                .collect();
            (rows, marks, s.brush, s.painting_stroke.clone(), rendered, traces, lines)
        };
        let d = w.global::<crate::Dedust>();
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        d.set_fraction(((f64::from(radius) - low) / (high - low)) as f32);
        let text: SharedString = format!("{radius:.0} px").into();
        d.set_readout(text.clone());
        d.set_value_text(text);
        d.set_rows(ModelRc::new(VecModel::from(rows)));
        let c = w.global::<Canvas>();
        c.set_painting(painting);
        // `strokeLineWidth`: the radius in full-resolution pixels as a width on the canvas.
        let width = match (frame, rendered) {
            (Some(f), Some(r)) => f64::from(radius) * 2.0 * f.2 / r.0.max(1.0),
            _ => 0.0,
        };
        c.set_brush_diameter(width as f32);
        c.set_stroke_width(width as f32);
        let commands = match frame {
            Some(f) if stroke.len() > 1 => stroke
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} {:.2} {:.2}", if i == 0 { "M" } else { "L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3))
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        c.set_stroke(commands.into());
        let marks: Vec<Mark> = marks
            .into_iter()
            .map(|(x, y, wd, ht, number)| Mark { x: x as f32, y: y as f32, width: wd as f32, height: ht as f32, number: number as i32 })
            .collect();
        c.set_marks(ModelRc::new(VecModel::from(marks)));
        let path = |points: &[(f64, f64)], f: (f64, f64, f64, f64)| {
            let mut out = String::new();
            for (i, p) in points.iter().enumerate() {
                out.push_str(&format!("{} {:.2} {:.2} ", if i == 0 { "M" } else { "L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3));
            }
            if points.len() == 1 {
                out.push_str(&format!("L {:.2} {:.2}", f.0 + points[0].0 * f.2, f.1 + points[0].1 * f.3));
            }
            out
        };
        let (shapes, line_commands) = match (frame, rendered) {
            (Some(f), Some(r)) => (
                traces
                    .iter()
                    .map(|(points, radius)| (path(points, f).into(), (radius * 2.0 * f.2 / r.0.max(1.0)) as f32))
                    .collect::<Vec<(SharedString, f32)>>(),
                lines.iter().map(|(a, b)| path(&[*a, *b], f)).collect::<Vec<_>>().join(" "),
            ),
            _ => (Vec::new(), String::new()),
        };
        c.set_source_shapes(ModelRc::new(VecModel::from(shapes)));
        c.set_source_lines(line_commands.into());
        c.set_source_line_width(1.5);
    }

    /// A point of the canvas in fractions of the rendered frame, when it falls on the picture.
    pub(super) fn on_picture(&self, x: f32, y: f32) -> Option<(f64, f64)> {
        let f = self.image_frame(false)?;
        let (fx, fy) = ((f64::from(x) - f.0) / f.2, (f64::from(y) - f.1) / f.3);
        ((0.0..=1.0).contains(&fx) && (0.0..=1.0).contains(&fy)).then_some((fx, fy))
    }

    pub(super) fn paint_hover(&self, x: f32, y: f32, hover: bool) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Canvas>();
        c.set_brush_shown(hover && self.on_picture(x, y).is_some());
        c.set_brush_x(x);
        c.set_brush_y(y);
    }

    pub(super) fn paint_at(&self, x: f32, y: f32, start: bool) {
        let point = self.on_picture(x, y);
        {
            let mut s = self.state.borrow_mut();
            if start {
                s.painting_stroke.clear();
            }
            if let Some(p) = point {
                s.painting_stroke.push(p);
            }
        }
        self.paint_hover(x, y, true);
        self.publish_dedust();
    }

    /// `appendCorrection`: a stroke of two points at least becomes a correction, its patch generated next.
    pub(super) fn paint_end(self: &Rc<Self>) {
        let (stroke, radius) = {
            let mut s = self.state.borrow_mut();
            (std::mem::take(&mut s.painting_stroke), s.brush)
        };
        if stroke.len() > 1 {
            let correction = oni_settings::frame::Correction {
                id: oni_settings::id::Id::new(),
                stroke: stroke.iter().map(|&(x, y)| oni_settings::cg::Point { x, y }).collect(),
                radius: f64::from(radius),
            };
            let mut next = self.state.borrow().store.settings().clone();
            next.corrections.push(correction);
            self.write(Command::Write(Box::new(next)));
        }
        self.publish_dedust();
    }

    /// `deleteCorrection`: gone from the settings, its patch from the cache and the engine.
    pub(super) fn remove_correction(self: &Rc<Self>, row: usize) {
        let mut next = self.state.borrow().store.settings().clone();
        if row >= next.corrections.len() {
            return;
        }
        let gone = next.corrections.remove(row);
        let _ = std::fs::remove_file(crate::patches::file(&crate::patches::caches(), gone.id));
        if let Ok(mut e) = self.engine.lock() {
            e.forget_patch(gone.id);
        }
        self.state.borrow_mut().fallback.remove(&gone.id);
        self.write(Command::Write(Box::new(next)));
    }

    /// The brush slider, held as `DSSlider` holds a value: a double press resets it to the default.
    pub(super) fn brush_press(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let double = s.brush_pressed_at.is_some_and(|t| t.elapsed() < system::double_click());
        s.brush_pressed_at = Some(Instant::now());
        if double {
            s.brush = oni_settings::frame::Correction::DEFAULT_RADIUS as f32;
            s.brush_start = None;
            return true;
        }
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        s.brush_start = Some(((f64::from(s.brush) - low) / (high - low)) as f32);
        false
    }

    pub(super) fn brush_drag(&self, translation: f32, span: f32, fine: bool) {
        {
            let mut s = self.state.borrow_mut();
            let Some(start) = s.brush_start else { return };
            let f = track::clamped(track::asked(start, translation, span, fine));
            let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
            s.brush = (low + f64::from(f) * (high - low)) as f32;
        }
        self.publish_dedust();
    }

    /// A frame on the canvas held, moved and let go: one undo step, constrained on every event.
    pub(super) fn frame_grabbed(self: &Rc<Self>, window: bool, handle: i32) {
        let Some(handle) = Handle::from_index(handle) else { return };
        let cropping = self.cropping();
        let Some(frame) = self.image_frame(cropping || !window) else { return };
        let mut s = self.state.borrow_mut();
        let g = s.store.settings().geometry.clone();
        let from = if window { frames::window_shown(&g, cropping) } else { g.crop };
        s.frame_gesture = Some((window, handle, from, (frame.2, frame.3)));
        s.store.hold();
    }

    pub(super) fn frame_dragged(self: &Rc<Self>, dx: f32, dy: f32) {
        let Some(image) = self.image_size() else { return };
        let cropping = self.cropping();
        let Some((window, handle, from, frame)) = self.state.borrow().frame_gesture.filter(|_| !self.state.borrow().frame_cancelled) else { return };
        let proposed = frames::apply(handle, from, frame, (f64::from(dx), f64::from(dy)));
        let g = self.state.borrow().store.settings().geometry.clone();
        let next = if window {
            Geometry { measure_window: Some(frames::window_stored(&g, frames::bounded(proposed), cropping)), ..g }
        } else {
            frames::constrained_crop(&g, handle, proposed, image)
        };
        self.write_geometry(next);
        self.publish_framing();
    }

    /// `CropOverlay`'s Esc: the held frame back where the press found it, the drag spent.
    pub(super) fn cancel_frame_drag(self: &Rc<Self>) -> bool {
        let cropping = self.cropping();
        let (window, from) = {
            let mut s = self.state.borrow_mut();
            let Some((window, _, from, _)) = s.frame_gesture.filter(|_| !s.frame_cancelled) else { return false };
            s.frame_cancelled = true;
            (window, from)
        };
        let g = self.state.borrow().store.settings().geometry.clone();
        let back = if window { Geometry { measure_window: Some(frames::window_stored(&g, from, cropping)), ..g } } else { Geometry { crop: from, ..g } };
        self.write_geometry(back);
        self.publish_framing();
        true
    }

    pub(super) fn frame_released(self: &Rc<Self>) {
        self.state.borrow_mut().frame_cancelled = false;
        let held = self.state.borrow_mut().frame_gesture.take().is_some();
        if held {
            self.state.borrow_mut().store.release();
            self.publish(false);
        }
    }
}
