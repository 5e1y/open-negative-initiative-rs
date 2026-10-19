//! The canvas's GPU: the histogram counted on the frame just encoded, at the image's cadence.

use super::*;

impl Gpu {
    /// The histogram counted on a frame already shown, once its readback has landed; then the
    /// newest frame is counted, if one waits.
    pub(super) fn landed(&mut self) -> Option<(oni_measure::histogram::Histogram, Instant)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let landed = self.kernel.histogram().zip(self.counted_at);
        self.count_due();
        landed
    }

    /// Counts the picture just encoded, at most once an image interval, so a slide's histogram
    /// follows its frames without the GPU counting faster than they come.
    pub(super) fn count_due(&mut self) {
        let due = self.counted_at.is_none_or(|at| at.elapsed() >= crate::timing::IMAGE_INTERVAL);
        if self.uncounted
            && due
            && let Some(Picture::Resident(picture)) = &self.current
            && let Some(view) = picture.view()
            && self.kernel.count(&self.device, &self.queue, view, picture.extent().w, picture.extent().h, self.screen)
        {
            self.uncounted = false;
            self.counted_at = Some(Instant::now());
        }
    }

    /// A count is under way or a frame waits for one.
    pub(super) fn histogram_pending(&self) -> bool {
        self.uncounted || self.kernel.counting()
    }
}
