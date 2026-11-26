//! The small gifts' films, shot in the real app by `welcome_films` (`scripts/welcome-assets`): each one JPEG of
//! its frames one under the other and a list of how long each holds, read off the interface's thread.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// The films, by the gift they show: its frames, then its holds.
const FILMS: [(usize, &[u8], &str); 2] = [
    (0, include_bytes!("../../../../oni-ui/ui/welcome/film-interface.jpg"), include_str!("../../../../oni-ui/ui/welcome/film-interface.txt")),
    (1, include_bytes!("../../../../oni-ui/ui/welcome/film-collection.jpg"), include_str!("../../../../oni-ui/ui/welcome/film-collection.txt")),
];

/// A film's frames as decoded, which cross threads, and how long each holds.
pub struct Decoded {
    gift: usize,
    frames: Vec<SharedPixelBuffer<Rgba8Pixel>>,
    holds: Vec<f32>,
}

/// A film ready to play: its frames, each from the second it starts, and its whole length.
pub struct Reel {
    pub gift: usize,
    frames: Vec<Image>,
    starts: Vec<f32>,
    length: f32,
}

/// Every film decoded; one that cannot be read is left out, and its gift shows its scene instead.
pub fn decoded() -> Vec<Decoded> {
    FILMS.iter().filter_map(|(gift, jpeg, holds)| decode(*gift, jpeg, holds)).collect()
}

fn decode(gift: usize, jpeg: &[u8], holds: &str) -> Option<Decoded> {
    let mut lines = holds.lines();
    let size: Vec<usize> = lines.next()?.split(' ').filter_map(|v| v.parse().ok()).collect();
    let holds: Vec<f32> = lines.filter_map(|l| l.trim().parse().ok()).collect();
    let (&[width, height], false) = (size.as_slice(), holds.is_empty()) else { return None };
    let mut decoder = jpeg_decoder::Decoder::new(jpeg);
    let rgb = decoder.decode().ok()?;
    let info = decoder.info()?;
    if info.pixel_format != jpeg_decoder::PixelFormat::RGB24 || usize::from(info.width) != width || usize::from(info.height) != height * holds.len() {
        return None;
    }
    let frames = rgb
        .chunks_exact(width * height * 3)
        .map(|frame| {
            let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width as u32, height as u32);
            for (to, from) in buffer.make_mut_slice().iter_mut().zip(frame.chunks_exact(3)) {
                *to = Rgba8Pixel { r: from[0], g: from[1], b: from[2], a: 255 };
            }
            buffer
        })
        .collect();
    Some(Decoded { gift, frames, holds })
}

impl Reel {
    pub fn of(decoded: Decoded) -> Reel {
        let starts = decoded.holds.iter().scan(0.0, |at, hold| Some(std::mem::replace(at, *at + hold))).collect();
        Reel { gift: decoded.gift, frames: decoded.frames.into_iter().map(Image::from_rgba8).collect(), starts, length: decoded.holds.iter().sum() }
    }

    /// The frame at `t` seconds into the film, which plays in a loop; held still, the one it holds longest.
    pub fn at(&self, t: f32, still: bool) -> Image {
        let longest = || (0..self.frames.len()).max_by(|a, b| self.hold(*a).total_cmp(&self.hold(*b))).unwrap_or(0);
        let index = if still { longest() } else { self.starts.partition_point(|s| *s <= t.max(0.0) % self.length).saturating_sub(1) };
        self.frames.get(index).cloned().unwrap_or_default()
    }

    fn hold(&self, i: usize) -> f32 {
        self.starts.get(i + 1).copied().unwrap_or(self.length) - self.starts[i]
    }
}
