//! A frame switched to shows its gallery thumbnail at once, fitted, until its own render lands.

use super::*;

impl Ctl {
    /// The thumbnail of `path` on the canvas; none drawn yet leaves the canvas as it is.
    pub(in crate::app) fn show_thumbnail(&self, path: &str) {
        let Some(w) = self.window() else { return };
        let thumbnail = {
            let mut s = self.state.borrow_mut();
            let Some(t) = s.fingerprints.get(path).and_then(|f| s.thumbs.get(f)) else { return };
            // The next render imports its texture again, since the canvas now holds the thumbnail.
            if let Some(g) = s.gpu.as_mut() {
                g.target = None;
            }
            t
        };
        let size = thumbnail.size();
        w.global::<Canvas>().set_frame(thumbnail);
        self.lay(None, (size.width as usize, size.height as usize));
    }
}
