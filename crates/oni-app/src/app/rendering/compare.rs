//! Compare Before/After: the before encoded into its own texture, the split on the canvas.

use super::*;
use super::landing::stepped;
use crate::bench::chain::Side;

impl Ctl {
    /// A landing's before, encoded while Compare is on: one that came with none leaves no older picture
    /// over the region the after now shows.
    pub(in crate::app) fn land_before(&self, before: Option<oni_graph::Source>) {
        if self.state.borrow().split().is_none() {
            return;
        }
        match before {
            Some(before) => self.encode_before(before),
            None => self.drop_before(),
        }
    }

    /// Compare's before into its own texture, sized by the after's steps: both hold the same region, so
    /// the canvas lays and clips them alike, and a pan moves both in one frame.
    fn encode_before(&self, before: oni_graph::Source) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let imported = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(view) = before.view().cloned() else { return };
            let e = before.extent();
            let k = grounded(&s, zebra(&s, Params::new(screen, e.w, e.h, s.gpu.seed), s.placed(), e.w, canvas));
            let (g, size) = (&s.gpu, stepped((e.w, e.h)));
            let kept = g.before.as_ref().map(|(t, _)| t);
            let made = kept.is_none_or(|t| (t.width(), t.height()) != size);
            let (target, shown) = g.encode(kept, size, &view, &k, (e.w, e.h));
            if g.before.as_ref().is_none_or(|(_, kept)| kept.id() != before.id()) {
                s.befores += 1;
            }
            s.gpu.before = Some((target, before));
            (shown.ok().flatten(), made)
        };
        let c = w.global::<Canvas>();
        if let Some(image) = imported.0 {
            c.set_before(image);
        }
        c.set_before_revision(c.get_before_revision().wrapping_add(1));
        self.note_canvas(imported.1.then_some(Side::Before));
    }

    /// The before let go, its texture and its picture on the canvas.
    pub(in crate::app) fn drop_before(&self) {
        self.state.borrow_mut().gpu.before = None;
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_before(Image::default());
        }
        self.note_canvas(None);
    }

    /// The canvas as this turn leaves it, for a bench: whether the before shown holds the after's region,
    /// and the display texture `made`, if any.
    pub(in crate::app) fn note_canvas(&self, made: Option<Side>) {
        if !crate::bench::chain::recording() {
            return;
        }
        let s = self.state.borrow();
        let after = s.gpu.current.as_ref().map(oni_graph::Source::extent);
        let before = s.gpu.before.as_ref().map(|(_, b)| Some(b.extent()) == after);
        crate::bench::chain::canvas(crate::bench::chain::Canvas { at: None, before, made });
    }

    /// Compare Before/After on and off, the split at half the canvas.
    pub(in crate::app) fn toggle_compare(self: &Rc<Self>) {
        let Some(split) = self.state.borrow_mut().frame.as_mut().map(|f| {
            f.compare.split = if f.compare.split.is_some() { None } else { Some(0.5) };
            f.compare.split
        }) else {
            return;
        };
        self.show_split(split);
        self.request_render();
    }

    /// The split on the canvas, none while Compare is off.
    pub(in crate::app) fn show_split(&self, split: Option<f32>) {
        // The before is a texture the size of the view: Compare off lets it go.
        if split.is_none() {
            self.drop_before();
        }
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_split(split.unwrap_or(-1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stepped;

    /// The textures a run of render extents makes, one each time the size asked differs from the kept one.
    fn made(extents: &[(u32, u32)], size: impl Fn((u32, u32)) -> (u32, u32)) -> usize {
        let mut kept = None;
        extents.iter().filter(|e| kept.replace(size(**e)) != Some(size(**e))).count()
    }

    #[test]
    fn a_pinch_makes_no_more_before_textures_than_after_ones() {
        // A pinch's regions at 100 % on a 3456 × 2234 canvas, each a few pixels off the last.
        let extents: Vec<(u32, u32)> = (0..40).map(|k| (4300 - 13 * k, 2800 - 9 * k)).collect();
        let (after, before) = (made(&extents, stepped), made(&extents, stepped));
        assert!(before <= after && after <= 4, "{before} before textures, {after} after ones");
        // Twin: a before sized to each extent makes a texture at every step of the pinch.
        assert_eq!(made(&extents, |e| e), extents.len());
    }
}
