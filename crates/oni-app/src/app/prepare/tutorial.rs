//! The step's tutorial above its controls: its clip's frames written into one texture at the clip's
//! own pace, the picture redrawn by a revision as the canvas is.

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
    /// Plays the step's clip while the panel shows it and nothing is measured, as Swift's `progress ==
    /// nil`; another step, or none, stops the one playing.
    pub(in crate::app) fn play_tutorial(&self, step: Option<usize>) {
        let wanted = step.filter(|s| tutorial::clip(*s).is_some());
        if self.state.borrow().prepare.tutorial.as_ref().map(|p| p.step) == wanted {
            return;
        }
        let playing = wanted.and_then(|step| Some(Playing { step, frames: Frames::new(tutorial::clip(step)?)?, texture: None, revision: 0, timer: Timer::default() }));
        let playing = std::mem::replace(&mut self.state.borrow_mut().prepare.tutorial, playing).is_some();
        match wanted {
            Some(_) => self.tutorial_frame(),
            None if playing => {
                if let Some(w) = self.window() {
                    w.global::<Preparing>().set_tutorial(Image::default());
                }
            }
            None => {}
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
