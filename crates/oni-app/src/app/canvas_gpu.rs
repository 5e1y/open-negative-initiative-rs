//! The canvas's GPU: the histogram counted on the frame just encoded, at the image's cadence.

use super::*;

/// The whole picture the histogram counts next; a sheet's clear gaps are no picture's.
pub(super) struct Counted {
    pub(super) picture: oni_graph::Source,
    pub(super) sheet: bool,
}

impl Gpu {
    /// The display kernel's picture of `view` in `kept` when it has `size`, else in a new texture: that
    /// texture, and the image to show (a new texture's, or the `drawn` corner read back) or why Slint refused it.
    pub(super) fn encode(&self, kept: Option<&wgpu::Texture>, size: (u32, u32), view: &wgpu::TextureView, k: &Params, drawn: (u32, u32)) -> (wgpu::Texture, Result<Option<Image>, String>) {
        let reused = kept.filter(|t| (t.width(), t.height()) == size).cloned();
        let fresh = reused.is_none();
        let target = reused.unwrap_or_else(|| display::Gpu::target(&self.device, size.0, size.1));
        let imported = if fresh && !self.reads_back { Image::try_from(target.clone()).map(Some).map_err(|e| e.to_string()) } else { Ok(None) };
        self.kernel.run(&self.device, &self.queue, view, k, &target);
        let shown = imported.map(|image| image.or_else(|| self.read_back(&target, drawn)));
        (target, shown)
    }

    /// The `size` corner of `target` as a picture, on a window that cannot take the GPU's textures;
    /// none where Slint draws the texture itself.
    pub(super) fn read_back(&self, target: &wgpu::Texture, size: (u32, u32)) -> Option<Image> {
        if !self.reads_back {
            return None;
        }
        display::read_back(&self.device, &self.queue, target, size.0, size.1).map(|bytes| image(&bytes, size.0 as usize, size.1 as usize))
    }

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
            && let Some(Counted { picture, sheet }) = &self.uncounted
            && let Some(view) = picture.view()
            && self.kernel.count(&self.device, &self.queue, view, (picture.extent().w, picture.extent().h), self.screen, *sheet, || with_shell(|c| c.publish_histogram()))
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
