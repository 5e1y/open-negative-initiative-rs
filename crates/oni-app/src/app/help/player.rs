//! The one clip player, Prepare the Roll's demos and the welcome's films: a GIF decoded on a thread of
//! its own, one frame queued, each written into one texture and announced by a revision.

use crate::app::{Ctl, Gpu, SHELL};
use crate::tutorial::Frames;
use slint::{Image, SharedPixelBuffer, Timer, TimerMode};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::time::Duration;

/// How soon the player looks again when the decoder has not kept up.
const RETRY: Duration = Duration::from_millis(5);

/// How a clip plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Pace {
    /// Once, resting on its last frame.
    Once,
    /// From its start again for as long as it shows.
    Loop,
    /// Its last frame alone, still: Reduce Motion.
    Still,
}

/// A frame as the decoder hands it: its pixels when it moved, else none, and how long it shows.
struct Decoded {
    pixels: Option<Vec<u8>>,
    delay: Duration,
}

pub(in crate::app) struct Player {
    frames: Receiver<Decoded>,
    /// Shown frames' buffers handed back, so the decoder allocates none after its first two.
    spare: Sender<Vec<u8>>,
    size: (u32, u32),
    texture: Option<wgpu::Texture>,
    revision: i32,
    timer: Timer,
    /// The seat's own step, which takes the next frame and lays it where the seat shows it.
    due: fn(&Ctl),
}

/// A frame to show: the image when it is a new one (the first, or each when read back), and the
/// revision that redraws the texture rewritten in place.
pub(in crate::app) struct Shown {
    pub image: Option<Image>,
    pub revision: i32,
}

impl Player {
    /// `clip` played at `pace`, its frames asked by `due`, which the seat calls once to start.
    /// Dropping the player ends the decoder at its next frame.
    pub(in crate::app) fn start(clip: &'static [u8], pace: Pace, due: fn(&Ctl)) -> Option<Player> {
        let size = Frames::new(clip)?.size();
        let (sender, frames) = sync_channel(1);
        let (spare, spares) = channel();
        std::thread::Builder::new().name("clip".into()).spawn(move || decode(clip, pace, &sender, &spares)).ok()?;
        Some(Player { frames, spare, size, texture: None, revision: 0, timer: Timer::default(), due })
    }

    /// The decoder's next frame written for the window and the next ask timed by its delay; none
    /// decoded yet, a short wait; none moved, nothing to redraw; none left, the clip rests.
    pub(in crate::app) fn step(&mut self, gpu: &Gpu) -> Option<Shown> {
        let decoded = match self.frames.try_recv() {
            Ok(decoded) => decoded,
            Err(TryRecvError::Empty) => {
                self.ask(RETRY);
                return None;
            }
            Err(TryRecvError::Disconnected) => return None,
        };
        self.ask(decoded.delay);
        let pixels = decoded.pixels?;
        let (width, height) = self.size;
        let image = if gpu.reads_back {
            Some(Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&pixels, width, height)))
        } else {
            let fresh = self.texture.is_none();
            let texture = self.texture.get_or_insert_with(|| target(&gpu.device, width, height));
            let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) };
            gpu.queue.write_texture(texture.as_image_copy(), &pixels, layout, texture.size());
            if fresh { Image::try_from(texture.clone()).ok() } else { None }
        };
        let _ = self.spare.send(pixels);
        self.revision = self.revision.wrapping_add(1);
        Some(Shown { image, revision: self.revision })
    }

    fn ask(&self, after: Duration) {
        let due = self.due;
        self.timer.start(TimerMode::SingleShot, after, move || {
            if let Some(me) = SHELL.with(|s| s.borrow().upgrade()) {
                due(&me);
            }
        });
    }
}

/// Decodes `clip` at `pace` into `sender` until the clip ends or the player is dropped.
fn decode(clip: &'static [u8], pace: Pace, sender: &SyncSender<Decoded>, spares: &Receiver<Vec<u8>>) {
    let hand = |picture: &[u8], delay: Duration, moved: bool| {
        let pixels = moved.then(|| {
            let mut pixels = spares.try_recv().unwrap_or_default();
            pixels.clear();
            pixels.extend_from_slice(picture);
            pixels
        });
        sender.send(Decoded { pixels, delay }).is_ok()
    };
    while let Some(mut frames) = Frames::new(clip) {
        if pace == Pace::Still {
            while frames.next().is_some() {}
            hand(frames.picture(), Duration::ZERO, true);
            return;
        }
        while let Some((picture, delay, moved)) = frames.next() {
            if !hand(picture, delay, moved) {
                return;
            }
        }
        if pace == Pace::Once {
            return;
        }
    }
}

/// A texture the clip's frames are written into and Slint samples: `Image::try_from` takes this format.
fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("clip"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

#[cfg(test)]
mod tests;
