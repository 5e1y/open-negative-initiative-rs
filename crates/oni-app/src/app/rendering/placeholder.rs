//! A frame switched to shows its gallery thumbnail at once, fitted, until its own render lands.

use super::*;

/// The full size of a photo seen only as a picture of `size`: its shape at the last photo's long side,
/// a roll's frames sharing a camera; the picture's own size when none was seen.
pub(super) fn guessed_full(size: (f64, f64), last: Option<(f64, f64)>) -> (f64, f64) {
    let Some(last) = last else { return size };
    let k = last.0.max(last.1) / size.0.max(size.1).max(1.0);
    (size.0 * k, size.1 * k)
}

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
        unclipped(&w);
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
            unclipped(&w);
            w.global::<Canvas>().set_frame(Image::default());
        }
    }
}

/// A picture of its own on the canvas, shown whole: only the display texture is clipped.
fn unclipped(w: &MainWindow) {
    w.global::<Canvas>().set_frame_clip_width(0);
    w.global::<Canvas>().set_frame_clip_height(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_lies_where_the_zoom_will_put_its_photo() {
        let (view, zoomed) = ((1000.0, 800.0), Viewport { scale: Some(1.0), offset: (0.0, 0.0) });
        let full = (6048.0, 4032.0);
        let thumbnail = guessed_full((960.0, 640.0), Some(full));
        assert_eq!(zoomed.placement(thumbnail, view).image, zoomed.placement(full, view).image);
        // A portrait frame after a landscape one keeps the long side.
        assert_eq!(guessed_full((640.0, 960.0), Some(full)), (4032.0, 6048.0));
        // Twin: the thumbnail's own size, zoomed, would show a tiny picture.
        assert_ne!(zoomed.placement((960.0, 640.0), view).image, zoomed.placement(full, view).image);
        assert_eq!(guessed_full((960.0, 640.0), None), (960.0, 640.0));
    }
}
