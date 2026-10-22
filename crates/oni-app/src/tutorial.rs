//! Prepare the Roll's tutorials, Swift's `TutorialVideo`: one square loop a step, muted, above its
//! controls. Each is a GIF at 20 fps decoded frame by frame, so every system plays the same file.

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

/// A clip's frames as it plays them, looping: each laid over the picture the last one left.
pub struct Frames {
    clip: &'static [u8],
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
        Some(Frames { clip, decoder, picture: vec![0; size.0 as usize * size.1 as usize * 4], size })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The next picture, RGBA rows, and how long it shows; past the last frame the clip starts again.
    pub fn next(&mut self) -> Option<(&[u8], Duration)> {
        let Some(frame) = self.decoder.read_next_frame().ok()? else {
            self.decoder = decoder(self.clip)?;
            return self.next();
        };
        let width = self.size.0 as usize;
        let (left, top, w) = (usize::from(frame.left), usize::from(frame.top), usize::from(frame.width));
        // Transparent pixels keep what the frames before laid, as each frame writes only what moved.
        for (y, row) in frame.buffer.chunks_exact(w * 4).enumerate() {
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                if pixel[3] > 0 {
                    let at = ((top + y) * width + left + x) * 4;
                    self.picture[at..at + 4].copy_from_slice(pixel);
                }
            }
        }
        let delay = Duration::from_millis(u64::from(frame.delay.max(1)) * 10);
        Some((&self.picture, delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_clip_plays_square_frames_at_twenty_a_second_and_loops() {
        for step in 2..=4 {
            let mut frames = Frames::new(clip(step).expect("a clip")).expect("a GIF");
            let (w, h) = frames.size();
            assert!(w.abs_diff(h) <= 1, "step {step} is square to a pixel: {w} × {h}");
            let first = frames.next().map(|(p, d)| (p.to_vec(), d)).expect("a first frame");
            assert_eq!(first.1, Duration::from_millis(50));
            assert!(first.0.chunks_exact(4).any(|p| p[..3] != first.0[..3]), "step {step}'s first frame is a picture, not a fill");
            let mut count = 1;
            while frames.next().expect("frames go on").0 != first.0.as_slice() {
                count += 1;
                assert!(count < 1000, "step {step} never came back to its first frame");
            }
            // Twin: the loop came back after the whole clip, not at once.
            assert!(count > 20, "step {step} looped after {count} frames");
        }
    }

    #[test]
    fn a_step_without_a_clip_shows_none() {
        assert!(clip(0).is_none() && clip(1).is_none() && clip(5).is_none());
    }
}
