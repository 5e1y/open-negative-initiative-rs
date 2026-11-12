//! De-dust: the brush, its strokes, the patches and the corrections list on the canvas.

use super::*;

pub(super) mod live;
mod auto;
mod dust;
mod hands;
mod models;
mod originals;

pub(in crate::app) use dust::Dust;
use models::{held, same_row};

impl Ctl {
    // MARK: - De-dust

    /// The De-dust pane open on a photo in the editor: a drag on the canvas paints.
    pub(super) fn painting(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 4 && self.state.borrow().open.is_some() && !self.together_shown()
    }

    /// The brush, the corrections' rows and the canvas's marks from the open frame; a stroke's
    /// moves skip it and grow the stroke's path alone.
    pub(super) fn publish_dedust(&self) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Canvas>();
        // Tiles, marks and strokes show in De-dust alone: a notch or a zoom elsewhere builds none of them.
        if !self.painting() {
            c.set_painting(false);
            self.publish_found(None);
            c.set_space_held(false);
            return;
        }
        let frame = self.image_frame(false);
        type Shape = (Vec<(f64, f64)>, f64);
        let (rows, marks, radius, stroke, rendered, sources, lines, waiting) = {
            let mut s = self.state.borrow_mut();
            let corrections = s.store.settings().corrections.clone();
            for c in &corrections {
                if !s.dust.numbers.contains_key(&c.id) {
                    let n = s.dust.next_number;
                    s.dust.numbers.insert(c.id, n);
                    s.dust.next_number += 1;
                }
            }
            if let Some(f) = frame {
                s.painting_stroke.relaid(f);
            }
            if s.dust.shown.is_some_and(|id| !corrections.iter().any(|c| c.id == id)) {
                s.dust.shown = None;
            }
            let s = &*s;
            // A heal waiting on the frame's full-resolution build is already under way.
            let healing = s.open_path().is_some_and(|p| s.dust.healing.contains_key(p) || s.building_full.as_deref() == Some(p));
            let upstream = oni_pipeline::corrections::upstream(s.store.settings());
            let key = |c: &oni_settings::frame::Correction| oni_pipeline::corrections::PatchKey { id: c.id, upstream };
            let pending = |c: &oni_settings::frame::Correction| healing && !s.dust.patched.contains(&key(c)) && !s.dust.unhealable.contains(&key(c));
            // Every stroke and source as the canvas shows them, mapped once from the scan.
            let placing = s.placing();
            let on_frame = dust::on_frame(&corrections, |_| (0.0, 0.0), placing.as_ref());
            let copied = dust::on_frame(&corrections, |c| s.dust.sources.get(&c.id).copied().unwrap_or_default(), placing.as_ref());
            let rows: Vec<crate::CorrectionRow> = corrections
                .iter()
                .map(|c| crate::CorrectionRow {
                    label: s.dust.numbers.get(&c.id).map_or(tr("Correction").to_string(), |n| tr!("Correction {n}", n = n)).into(),
                    number: s.dust.numbers.get(&c.id).map_or(0, |&n| n as i32),
                    pending: pending(c),
                    fallback: s.dust.fallback.contains(&c.id),
                    original: originals::original(c.id),
                })
                .collect();
            // One correction shown alone, its source with it; else every marker and no source at all.
            let shown = |c: &oni_settings::frame::Correction| s.dust.shown.is_none_or(|id| id == c.id);
            let sourced = |c: &&oni_settings::frame::Correction| s.dust.shown == Some(c.id);
            let rendered = s.full_size;
            let marks: Vec<(f64, f64, f64, f64, usize)> = match (frame, rendered) {
                (Some(f), Some(r)) => on_frame
                    .iter()
                    .filter(|c| !c.stroke.is_empty() && shown(c))
                    .map(|c| {
                        let (x, y, wd, ht) = bounding(c, r);
                        (f.0 + x * f.2, f.1 + y * f.3, wd * f.2, ht * f.3, s.dust.numbers.get(&c.id).copied().unwrap_or(0))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // `correctionMarks`: the line from the stroke shown to its source, and a ring where it was copied from.
            let sources: Vec<(f64, f64, f64, f64, usize)> = match (frame, rendered) {
                (Some(f), Some(r)) => copied
                    .iter()
                    .filter(sourced)
                    .filter(|c| s.dust.sources.contains_key(&c.id))
                    .map(|c| {
                        let (x, y, wd, ht) = bounding(c, r);
                        (f.0 + x * f.2, f.1 + y * f.3, wd * f.2, ht * f.3, 0)
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let centre = |c: &oni_settings::frame::Correction| live::bounding_centre(c.stroke.iter().map(|p| (p.x, p.y)));
            let lines: Vec<((f64, f64), (f64, f64))> = on_frame
                .iter()
                .zip(&copied)
                .filter(|(c, _)| sourced(c) && s.dust.sources.contains_key(&c.id))
                .filter_map(|(c, from)| Some((centre(c)?, centre(from)?)))
                .collect();
            // A stroke stays painted until its patch lands, so the dust is never shown bare meanwhile.
            let waiting: Vec<Shape> = on_frame.iter().filter(|c| pending(c)).map(|c| (c.stroke.iter().map(|p| (p.x, p.y)).collect(), c.radius)).collect();
            (rows, marks, s.brush, SharedString::from(s.painting_stroke.commands()), rendered, sources, lines, waiting)
        };
        let d = w.global::<crate::Dedust>();
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        d.set_fraction(((f64::from(radius) - low) / (high - low)) as f32);
        let text: SharedString = crate::fields::number(f64::from(radius), 0, false, crate::fields::Unit::Pixels).into();
        d.set_readout(text.clone());
        d.set_value_text(text);
        if let Some(rows) = held(&d.get_rows(), rows, same_row) {
            d.set_rows(rows);
        }
        // No correction shown, a removal or another photo since: no tile stays under the hand.
        if self.state.borrow().dust.shown.is_none() {
            d.set_hovered_row(-1);
        }
        c.set_painting(true);
        self.publish_found(frame);
        let picture = frame.unwrap_or_default();
        c.set_picture_x(picture.0 as f32);
        c.set_picture_y(picture.1 as f32);
        c.set_picture_width(picture.2 as f32);
        c.set_picture_height(picture.3 as f32);
        // `strokeLineWidth`: the radius in full-resolution pixels as a width on the canvas.
        let width = |radius: f64| match (frame, rendered) {
            (Some(f), Some(r)) => (radius * 2.0 * f.2 / r.0.max(1.0)) as f32,
            _ => 0.0,
        };
        c.set_brush_diameter(width(f64::from(radius)));
        c.set_stroke_width(width(f64::from(radius)));
        c.set_stroke(stroke);
        let laid = |list: Vec<(f64, f64, f64, f64, usize)>| -> Vec<Mark> {
            list.into_iter().map(|(x, y, wd, ht, number)| Mark { x: x as f32, y: y as f32, width: wd as f32, height: ht as f32, number: number as i32 }).collect()
        };
        if let Some(marks) = held(&c.get_marks(), laid(marks), Mark::eq) {
            c.set_marks(marks);
        }
        if let Some(sources) = held(&c.get_source_marks(), laid(sources), Mark::eq) {
            c.set_source_marks(sources);
        }
        let path = live::traced;
        let shapes = |list: &[Shape]| match frame {
            Some(f) => list.iter().map(|(points, radius)| (path(points, f).into(), width(*radius))).collect::<Vec<(SharedString, f32)>>(),
            None => Vec::new(),
        };
        if let Some(waiting) = held(&c.get_pending_strokes(), shapes(&waiting), |a, b| a == b) {
            c.set_pending_strokes(waiting);
        }
        let line_commands = frame.map(|f| lines.iter().map(|(a, b)| live::dashed(*a, *b, f)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
        c.set_source_lines(line_commands.into());
    }

    /// A press or a move on the canvas: a point on the picture joins the stroke, and the canvas's
    /// path grows by that one segment, so a move costs the same at any length of stroke.
    pub(super) fn paint_at(&self, x: f32, y: f32, start: bool) {
        let Some(w) = self.window() else { return };
        let Some(f) = self.image_frame(false) else { return };
        let commands = {
            let mut s = self.state.borrow_mut();
            if start {
                s.painting_stroke.clear();
            }
            let p = ((f64::from(x) - f.0) / f.2, (f64::from(y) - f.1) / f.3);
            let on = (0.0..=1.0).contains(&p.0) && (0.0..=1.0).contains(&p.1);
            (on && s.painting_stroke.push(p, f)).then(|| SharedString::from(s.painting_stroke.commands()))
        };
        if let Some(commands) = commands {
            w.global::<Canvas>().set_stroke(commands);
        }
    }

    /// `appendCorrection`: a stroke becomes a correction kept on the scan, a click a dot of the brush's size;
    /// the write asks the render, which starts its patch and shows it pending before the next frame.
    pub(super) fn paint_end(self: &Rc<Self>) {
        let (mut stroke, radius, placing) = {
            let mut s = self.state.borrow_mut();
            (s.painting_stroke.take(), s.brush, s.placing())
        };
        let (Some(&(x, y)), Some(placing)) = (stroke.last(), placing) else { return self.publish_dedust() };
        if stroke.len() == 1 {
            stroke.push((x + DOT, y));
        }
        let correction = oni_settings::frame::Correction {
            id: oni_settings::id::Id::new(),
            stroke: stroke.iter().map(|&(x, y)| placing.on_scan(oni_settings::cg::Point { x, y })).collect(),
            radius: f64::from(radius),
        };
        let mut next = self.state.borrow().store.settings().clone();
        next.corrections.push(correction);
        self.write(Command::Write(Box::new(next)));
    }

    /// `deleteCorrection`: gone from the settings, its patch from the cache and the engine; the
    /// engine forgets it off this thread, as a render may hold it for a while.
    pub(super) fn remove_correction(self: &Rc<Self>, row: usize) {
        let mut next = self.state.borrow().store.settings().clone();
        if row >= next.corrections.len() {
            return;
        }
        let gone = next.corrections.remove(row);
        let key = oni_pipeline::corrections::PatchKey::of(&next, &gone);
        let _ = std::fs::remove_file(crate::patches::file(&crate::patches::caches(), key));
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            if let Ok(mut e) = engine.lock() {
                e.forget_patch(key);
            }
        });
        {
            let mut s = self.state.borrow_mut();
            s.dust.fallback.remove(&gone.id);
            s.dust.patched.retain(|k| k.id != gone.id);
            s.dust.sources.remove(&gone.id);
        }
        self.write(Command::Write(Box::new(next)));
    }

    /// The list's tile under the hand, `row` -1 for none: the canvas shows that correction alone with its source.
    pub(super) fn show_correction(self: &Rc<Self>, row: i32) {
        let id = usize::try_from(row).ok().and_then(|r| self.state.borrow().store.settings().corrections.get(r).map(|c| c.id));
        if std::mem::replace(&mut self.state.borrow_mut().dust.shown, id) != id {
            self.publish_dedust();
        }
    }

    /// `negative.fullIfReady`, built ahead for the open frame alone: as De-dust opens, or once the frame
    /// has stayed on screen, so a stroke's patch and a zoom read it at once (about 390 MB a 24 Mpx frame).
    pub(super) fn ready_full(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        if w.global::<App>().get_gallery() || self.together_shown() {
            return;
        }
        let due = {
            let s = self.state.borrow();
            s.requests.rendered != 0 && ahead(self.painting(), s.opened_at.elapsed(), s.hands.load(Ordering::Relaxed) > 0)
        };
        if due {
            self.ensure_full();
        }
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

impl State {
    /// A correction of the open frame with no patch on disk under its settings nor a failed generation;
    /// a patch seen once is not looked for again, so a notch stats no file.
    pub(super) fn unhealed(&mut self) -> bool {
        let settings = self.store.settings();
        let upstream = oni_pipeline::corrections::upstream(settings);
        let lacking: Vec<oni_pipeline::corrections::PatchKey> = settings
            .corrections
            .iter()
            .filter(|c| !c.stroke.is_empty())
            .map(|c| oni_pipeline::corrections::PatchKey { id: c.id, upstream })
            .filter(|k| !self.dust.patched.contains(k) && !self.dust.unhealable.contains(k))
            .collect();
        let mut caches = None;
        for key in lacking {
            if !crate::patches::file(caches.get_or_insert_with(crate::patches::caches), key).exists() {
                return true;
            }
            self.dust.patched.insert(key);
        }
        false
    }
}

/// A click's dot, in fractions of the frame shown: under a pixel of any scan, as a stroke holds two points.
const DOT: f64 = 1e-4;

/// `boundingFraction`: a stroke on the frame shown, its extent padded by its radius, in fractions of
/// the frame rendered at `full` pixels.
fn bounding(c: &oni_settings::frame::Correction, full: (f64, f64)) -> (f64, f64, f64, f64) {
    let (rx, ry) = (c.radius / full.0, c.radius / full.1);
    let (x0, x1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    (x0 - rx, y0 - ry, x1 - x0 + 2.0 * rx, y1 - y0 + 2.0 * ry)
}

/// A frame looked at this long has its full resolution built; arrows walking a roll pass sooner.
const DWELL: std::time::Duration = std::time::Duration::from_secs(1);

/// Whether the open frame's full resolution is built now: never under a hand, at once for De-dust,
/// else once the frame has been shown `DWELL`.
fn ahead(painting: bool, shown_for: std::time::Duration, held: bool) -> bool {
    !held && (painting || shown_for >= DWELL)
}

#[cfg(test)]
mod ahead_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_frame_stayed_on_builds_its_full_resolution_and_one_walked_past_does_not() {
        assert!(ahead(false, Duration::from_millis(1000), false));
        assert!(!ahead(false, Duration::from_millis(400), false), "twin: a frame the arrows pass builds nothing");
        assert!(ahead(true, Duration::ZERO, false), "De-dust builds at once");
        assert!(!ahead(true, Duration::from_secs(5), true), "never under a hand");
    }
}
