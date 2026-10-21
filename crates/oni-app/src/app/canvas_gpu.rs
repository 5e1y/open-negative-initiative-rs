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
        if due
            && let Some(picture) = &self.uncounted
            && let Some(view) = picture.view()
            && self.kernel.count(&self.device, &self.queue, view, picture.extent().w, picture.extent().h, self.screen, || with_shell(|c| c.publish_histogram()))
        {
            self.uncounted = None;
            self.counted_at = Some(Instant::now());
        }
    }

    /// How long a picture waiting for its count waits for the image's cadence; none when none waits.
    pub(super) fn count_wait(&self) -> Option<Duration> {
        self.uncounted.as_ref()?;
        Some(self.counted_at.map_or(Duration::ZERO, |at| crate::timing::IMAGE_INTERVAL.saturating_sub(at.elapsed())))
    }
}
