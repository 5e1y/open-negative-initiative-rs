//! The canvas's GPU: the histogram counted on the frame just encoded, at the image's cadence.

use super::*;

/// The whole picture the histogram counts next; a sheet's clear gaps are no picture's.
pub(super) struct Counted {
    pub(super) picture: oni_graph::Source,
    pub(super) sheet: bool,
}

/// The histogram's count of what the canvas shows: the picture waiting, the one last taken and the
/// one being read back, voided with the canvas's requests so another frame's count never lands.
#[derive(Default)]
pub(in crate::app) struct Counting {
    waiting: Option<Counted>,
    taken: Option<u64>,
    reading: Option<u64>,
}

impl Counting {
    /// `picture` waits for its count, unless it is the one last taken: a picture is known by its id.
    pub(super) fn take(&mut self, picture: oni_graph::Source, sheet: bool) -> bool {
        if self.taken == Some(picture.id()) {
            return false;
        }
        self.taken = Some(picture.id());
        self.waiting = Some(Counted { picture, sheet });
        true
    }

    pub(super) fn due(&self) -> Option<&Counted> {
        self.waiting.as_ref()
    }

    /// The waiting picture's count went to the GPU.
    pub(super) fn started(&mut self) {
        self.reading = self.waiting.take().map(|c| c.picture.id());
    }

    /// A count read back is this canvas's: none once its frame or sheet was left.
    pub(super) fn landed(&mut self) -> bool {
        self.reading.take().is_some()
    }
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

    /// The histogram counted on a frame already shown, once its readback has landed and if it still
    /// describes the canvas; then the newest picture is counted, if one waits.
    pub(super) fn landed(&mut self, counting: &mut Counting) -> Option<(oni_measure::histogram::Histogram, Instant)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let landed = self.kernel.histogram().filter(|_| counting.landed()).zip(self.counted_at);
        self.count_due(counting);
        landed
    }

    /// Counts the picture waiting, at most once an image interval, so a slide's histogram
    /// follows its frames without the GPU counting faster than they come.
    pub(super) fn count_due(&mut self, counting: &mut Counting) {
        let due = self.counted_at.is_none_or(|at| at.elapsed() >= crate::timing::IMAGE_INTERVAL);
        if due
            && let Some(Counted { picture, sheet }) = counting.due()
            && let Some(view) = picture.view()
            && self.kernel.count(&self.device, &self.queue, view, (picture.extent().w, picture.extent().h), self.screen, *sheet, || with_shell(|c| c.publish_histogram()))
        {
            counting.started();
            self.counted_at = Some(Instant::now());
        }
    }

    /// How long a picture waiting for its count waits for the image's cadence; none when none waits.
    pub(super) fn count_wait(&self, counting: &Counting) -> Option<Duration> {
        counting.due()?;
        Some(self.counted_at.map_or(Duration::ZERO, |at| crate::timing::IMAGE_INTERVAL.saturating_sub(at.elapsed())))
    }
}

#[cfg(test)]
mod tests {
    use super::Counting;
    use crate::render::{Decoded, Device, Engine, Linear};

    /// A picture is taken once by its id; a count read back after the canvas's requests were voided
    /// describes another frame and is dropped.
    #[test]
    fn a_count_lands_only_on_the_canvas_that_asked_it() {
        let Ok(device) = Device::open() else {
            eprintln!("no adapter: the count's slot is not exercised");
            return;
        };
        let linear = Linear { width: 4, height: 4, pixels: vec![[0.5; 4]; 16] };
        let picture = Engine::on(&device).prepare(Decoded { linear, full_width: 4.0, undeclared: None }).unwrap().preview;
        let asked = || {
            let mut c = Counting::default();
            assert!(c.take(picture.clone(), false));
            assert!(!c.take(picture.clone(), false), "the same picture is counted once");
            c.started();
            c
        };
        // Twin: the count of the canvas still shown lands.
        assert!(asked().landed());
        // Opening another frame voids the canvas's requests, the count in flight with them.
        let mut requests = crate::app::rendering::Requests { count: asked(), ..Default::default() };
        drop(std::mem::take(&mut requests));
        assert!(!requests.count.landed(), "a frame left drops its count");
    }
}
