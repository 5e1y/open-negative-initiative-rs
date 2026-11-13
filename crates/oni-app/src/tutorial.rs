//! Prepare the Roll's tutorials: one square clip a step, muted, played once when asked. Each is a GIF
//! at 20 fps decoded frame by frame, so every system plays the same file; the welcome's films too.

use std::io::Cursor;
use std::time::Duration;

/// The clip of a step of Prepare the Roll, by its index; a step without one shows none.
pub fn clip(step: usize) -> Option<&'static [u8]> {
    match step {
        2 => Some(include_bytes!("../resources/tutorials/roll-base.gif")),
        3 => Some(include_bytes!("../resources/tutorials/roll-flags.gif")),
        4 => Some(include_bytes!("../resources/tutorials/roll-window.gif")),
        _ => None,
    }
}

/// A clip's frames as it plays them, once: each laid over the picture the last one left.
pub struct Frames {
    decoder: gif::Decoder<Cursor<&'static [u8]>>,
    picture: Vec<u8>,
    size: (u32, u32),
}

fn decoder(clip: &'static [u8]) -> Option<gif::Decoder<Cursor<&'static [u8]>>> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    options.read_info(Cursor::new(clip)).ok()
}

impl Frames {
    pub fn new(clip: &'static [u8]) -> Option<Frames> {
        let decoder = decoder(clip)?;
        let size = (u32::from(decoder.width()), u32::from(decoder.height()));
        Some(Frames { decoder, picture: vec![0; size.0 as usize * size.1 as usize * 4], size })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The picture the frames read so far laid.
    pub fn picture(&self) -> &[u8] {
        &self.picture
    }

    /// The next picture, RGBA rows, how long it shows, and whether it differs from the last; none
    /// past the last frame.
    pub fn next(&mut self) -> Option<(&[u8], Duration, bool)> {
        let frame = self.decoder.read_next_frame().ok()??;
        let width = self.size.0 as usize;
        let (left, top, w) = (usize::from(frame.left), usize::from(frame.top), usize::from(frame.width));
        let mut moved = false;
        // Transparent pixels keep what the frames before laid, as each frame writes only what moved.
        for (y, row) in frame.buffer.chunks_exact(w * 4).enumerate() {
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                if pixel[3] > 0 {
                    let at = ((top + y) * width + left + x) * 4;
                    moved |= self.picture[at..at + 4] != *pixel;
                    self.picture[at..at + 4].copy_from_slice(pixel);
                }
            }
        }
        let delay = Duration::from_millis(u64::from(frame.delay.max(1)) * 10);
        Some((&self.picture, delay, moved))
    }
}

/// A clip of a light bar over a grey ground, one frame per place given, 20 a second: the checks'
/// stand-in for a film. A place repeated is a frame where nothing moves.
pub fn synthetic(width: u16, height: u16, places: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = gif::Encoder::new(&mut bytes, width, height, &[96, 96, 96, 240, 220, 160]).expect("a header");
    for &place in places {
        let bar = (width / 8).max(1);
        let buffer: Vec<u8> = (0..height).flat_map(|_| (0..width).map(move |x| u8::from(x >= place && x < place + bar))).collect();
        let frame = gif::Frame { width, height, delay: 5, buffer: buffer.into(), ..gif::Frame::default() };
        encoder.write_frame(&frame).expect("a frame");
    }
    drop(encoder);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_clip_plays_square_frames_at_twenty_a_second_once() {
        for step in 2..=4 {
            let mut frames = Frames::new(clip(step).expect("a clip")).expect("a GIF");
            let (w, h) = frames.size();
            assert!(w.abs_diff(h) <= 1, "step {step} is square to a pixel: {w} × {h}");
            let first = frames.next().map(|(p, d, _)| (p.to_vec(), d)).expect("a first frame");
            assert_eq!(first.1, Duration::from_millis(50));
            assert!(first.0.chunks_exact(4).any(|p| p[..3] != first.0[..3]), "step {step}'s first frame is a picture, not a fill");
            let mut count = 1;
            while frames.next().is_some() {
                count += 1;
                assert!(count < 1000, "step {step} never ends");
            }
            assert!(frames.next().is_none(), "step {step} rests on its last frame, never starting again");
            // Twin: the clip ends after the whole of it, not at once.
            assert!(count > 20, "step {step} ended after {count} frames");
        }
    }

    #[test]
    fn a_step_without_a_clip_shows_none() {
        assert!(clip(0).is_none() && clip(1).is_none() && clip(5).is_none());
    }
}
