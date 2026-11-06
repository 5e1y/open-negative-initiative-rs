//! The step's demo under its controls, on the hand's request: its clip's frames written into one
//! texture at the clip's own pace, the picture redrawn by a revision as the canvas is.

use super::super::SHELL;
use super::*;
use crate::tutorial::{self, Frames};
use slint::{SharedPixelBuffer, Timer, TimerMode};

pub(in crate::app) struct Playing {
    step: usize,
    frames: Frames,
    texture: Option<wgpu::Texture>,
    revision: i32,
    timer: Timer,
}

impl Ctl {
    /// Show How: the step's clip from its first frame, played once and resting on its last; pressed
    /// again, the clip shown is put away. Nothing moves beside the scan unasked.
    pub(in crate::app) fn toggle_tutorial(&self) {
        let (step, shown) = {
            let r = &self.state.borrow().prepare;
            (r.step.index(), r.tutorial.is_some())
        };
        self.play_tutorial((!shown).then_some(step));
    }

    /// A clip stays on its own step while nothing is measured; on another step, or none, it goes.
    pub(in crate::app) fn keep_tutorial(&self, step: Option<usize>) {
        if self.state.borrow().prepare.tutorial.as_ref().is_some_and(|p| Some(p.step) != step) {
            self.play_tutorial(None);
        }
    }

    /// The clip of `step` from its start, or none shown.
    fn play_tutorial(&self, step: Option<usize>) {
        let playing = step.and_then(|step| Some(Playing { step, frames: Frames::new(tutorial::clip(step)?)?, texture: None, revision: 0, timer: Timer::default() }));
        let started = playing.is_some();
        let stopped = std::mem::replace(&mut self.state.borrow_mut().prepare.tutorial, playing).is_some();
        if started {
            self.tutorial_frame();
        } else if stopped && let Some(w) = self.window() {
            w.global::<Preparing>().set_tutorial(Image::default());
        }
    }

    /// The clip's next frame on the panel, the one after it asked for when this one has shown its time.
    fn tutorial_frame(&self) {
        let Some(w) = self.window() else { return };
        let (image, revision) = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some(p) = s.prepare.tutorial.as_mut() else { return };
            let (width, height) = p.frames.size();
            let Some((pixels, delay)) = p.frames.next() else { return };
            let g = &s.gpu;
            let image = if g.reads_back {
                Some(Image::from_rgba8(SharedPixelBuffer::clone_from_slice(pixels, width, height)))
            } else {
                let fresh = p.texture.is_none();
                let texture = p.texture.get_or_insert_with(|| target(&g.device, width, height));
                let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) };
                g.queue.write_texture(texture.as_image_copy(), pixels, layout, texture.size());
                if fresh { Image::try_from(texture.clone()).ok() } else { None }
            };
            p.revision = p.revision.wrapping_add(1);
            p.timer.start(TimerMode::SingleShot, delay, || {
                if let Some(me) = SHELL.with(|s| s.borrow().upgrade()) {
                    me.tutorial_frame();
                }
            });
            (image, p.revision)
        };
        let p = w.global::<Preparing>();
        if let Some(image) = image {
            p.set_tutorial(image);
        }
        p.set_tutorial_revision(revision);
    }
}

/// A texture the clip's frames are written into and Slint samples: `Image::try_from` takes this format.
fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tutorial"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
