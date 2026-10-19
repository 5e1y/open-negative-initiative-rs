//! A frame switched to shows its gallery thumbnail at once, fitted, until its own render lands.

use super::*;

impl Ctl {
    /// The thumbnail of `path` on the canvas, else an empty canvas: never the last photo's picture,
    /// nor Compare's before of it.
    pub(in crate::app) fn show_thumbnail(&self, path: &str) {
        let Some(w) = self.window() else { return };
        let thumbnail = {
            let mut s = self.state.borrow_mut();
            if let Some(g) = s.gpu.as_mut() {
                g.before = None;
            }
            s.fingerprints.get(path).and_then(|f| s.thumbs.get(f))
        };
        w.global::<Canvas>().set_before(Image::default());
        let Some(thumbnail) = thumbnail else { return self.blank_canvas() };
        // The next render imports its texture again, since the canvas now holds the thumbnail.
        if let Some(g) = self.state.borrow_mut().gpu.as_mut() {
            g.target = None;
        }
        let size = thumbnail.size();
        w.global::<Canvas>().set_frame(thumbnail);
        self.lay(None, (size.width as usize, size.height as usize));
    }

    /// The canvas emptied; its texture is let go too, or the next render of the same size would
    /// only rewrite texels the canvas no longer shows.
    pub(in crate::app) fn blank_canvas(&self) {
        if let Some(g) = self.state.borrow_mut().gpu.as_mut() {
            g.target = None;
        }
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_frame(Image::default());
        }
    }
}
