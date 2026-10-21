//! A window resized: the picture shown follows its frame at once, by arithmetic alone, and one render
//! at the new size is asked once the hand rests; nothing renders meanwhile.

use super::*;
use std::cell::Cell;

thread_local! {
    /// The canvas size the picture shown was laid for, and whether a resize is under way.
    static LAID_IN: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
    static RESIZING: Cell<bool> = const { Cell::new(false) };
}

/// The picture was laid for a canvas of `view`, so a resize knows where it stood.
pub(super) fn laid_in(view: Option<(f64, f64)>) {
    LAID_IN.with(|l| l.set(view));
}

/// A resize under way: no render is asked until it rests.
pub(in crate::app) fn resizing() -> bool {
    RESIZING.with(Cell::get)
}

type Rect = (f64, f64, f64, f64);

/// The texture's rectangle `t`, laid within the whole image at `from`, moved to the same share of `to`.
fn followed(t: Rect, from: Rect, to: Rect) -> Rect {
    let k = to.2 / from.2.max(1e-9);
    (to.0 + (t.0 - from.0) * k, to.1 + (t.1 - from.1) * k, t.2 * k, t.3 * k)
}

impl Ctl {
    /// The canvas took a new size: the picture shown is laid again where the view now puts it, the
    /// same texture stretched or moved, and the render at the new size waits for the hand to rest.
    pub(in crate::app) fn resized(self: &Rc<Self>) {
        let Some(view) = self.canvas_size() else { return };
        RESIZING.with(|r| r.set(true));
        let moved = {
            let mut s = self.state.borrow_mut();
            let before = s.viewport;
            if let Some(full) = s.full_size {
                s.viewport.clamp(full, view);
            }
            match (s.placed, LAID_IN.with(Cell::get)) {
                (Some(p), Some(old)) => {
                    let r = followed((p.x, p.y, p.width, p.height), before.placement(p.full, old).image, s.viewport.placement(p.full, view).image);
                    Some(Placed { x: r.0, y: r.1, width: r.2, height: r.3, ..p })
                }
                _ => None,
            }
        };
        if let Some(p) = moved {
            self.lay(Some(p), (0, 0));
        }
        self.publish_framing();
        let me = Rc::downgrade(self);
        self.settle.start(TimerMode::SingleShot, viewport::SETTLE, move || {
            RESIZING.with(|r| r.set(false));
            if let Some(me) = me.upgrade() {
                me.request_render();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texture_follows_its_image_to_the_same_share_of_the_new_frame() {
        // Fitted: the whole image grows from 800 to 1000 wide; the texture covering it follows exactly.
        let (from, to) = ((100.0, 50.0, 800.0, 600.0), (150.0, 40.0, 1000.0, 750.0));
        assert_eq!(followed(from, from, to), to);
        // Zoomed: the image keeps its scale and moves by the half of the growth; a region of it with it.
        let (from, to) = ((-500.0, -300.0, 4000.0, 3000.0), (-400.0, -250.0, 4000.0, 3000.0));
        assert_eq!(followed((0.0, 0.0, 900.0, 700.0), from, to), (100.0, 50.0, 900.0, 700.0));
        // Twin: a region left where it was would sit off its image.
        assert_ne!(followed((0.0, 0.0, 900.0, 700.0), from, to), (0.0, 0.0, 900.0, 700.0));
    }
}
