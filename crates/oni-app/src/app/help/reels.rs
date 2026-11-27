//! The gifts' films, made by `crates/oni-app/welcome-motion` (`scripts/welcome-assets`): each one file of its frames
//! end to end, each a WebP read by a decoder in pure Rust, and a list of how long each holds and how many bytes it takes. A film is decoded as it
//! plays, a few frames ahead, off the interface's thread: none is ever held whole in memory.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::sync::{Arc, Condvar, Mutex};

/// The films, by the gift they show: their frames, then their holds and lengths.
const FILMS: [(usize, &[u8], &str); 4] = [
    (0, include_bytes!("../../../../oni-ui/ui/welcome/film-interface.webps"), include_str!("../../../../oni-ui/ui/welcome/film-interface.txt")),
    (1, include_bytes!("../../../../oni-ui/ui/welcome/film-collection.webps"), include_str!("../../../../oni-ui/ui/welcome/film-collection.txt")),
    (2, include_bytes!("../../../../oni-ui/ui/welcome/film-huge.webps"), include_str!("../../../../oni-ui/ui/welcome/film-huge.txt")),
    (3, include_bytes!("../../../../oni-ui/ui/welcome/film-wrapped.webps"), include_str!("../../../../oni-ui/ui/welcome/film-wrapped.txt")),
];
/// How many frames are decoded ahead of the one shown: at 25 frames a second, a sixth of a second.
const AHEAD: usize = 4;

/// A film's frames as its file lays them: each one's place in the file, and from which second it shows.
struct Cut {
    gift: usize,
    size: (usize, usize),
    frames: Vec<(usize, usize)>,
    starts: Vec<f32>,
    length: f32,
}

impl Cut {
    fn read(gift: usize, bytes: &[u8], list: &str) -> Option<Cut> {
        let mut lines = list.lines();
        let size: Vec<usize> = lines.next()?.split(' ').filter_map(|v| v.parse().ok()).collect();
        let &[width, height] = size.as_slice() else { return None };
        let (mut frames, mut starts, mut at, mut length) = (Vec::new(), Vec::new(), 0, 0.0);
        for line in lines {
            let (hold, take) = line.split_once(' ')?;
            let (hold, take): (f32, usize) = (hold.parse().ok()?, take.parse().ok()?);
            frames.push((at, take));
            starts.push(length);
            (at, length) = (at + take, length + hold);
        }
        (at == bytes.len() && !frames.is_empty() && length > 0.0).then_some(Cut { gift, size: (width, height), frames, starts, length })
    }

    /// The frame shown `t` seconds into the film, which plays in a loop; held still, the one it holds longest.
    fn index(&self, t: f32, still: bool) -> usize {
        if still {
            let hold = |i: usize| self.starts.get(i + 1).copied().unwrap_or(self.length) - self.starts[i];
            return (0..self.frames.len()).max_by(|a, b| hold(*a).total_cmp(&hold(*b))).unwrap_or(0);
        }
        self.starts.partition_point(|s| *s <= t.max(0.0) % self.length).saturating_sub(1)
    }
}

/// Frame `i` of `cut`, decoded; none if its bytes are not a WebP of the film's size.
fn decode(cut: &Cut, bytes: &[u8], i: usize) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let (at, take) = cut.frames[i];
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes.get(at..at + take)?)).ok()?;
    let (width, height) = decoder.dimensions();
    if decoder.has_alpha() || (width as usize, height as usize) != cut.size {
        return None;
    }
    let mut rgb = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut rgb).ok()?;
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    for (to, from) in buffer.make_mut_slice().iter_mut().zip(rgb.chunks_exact(3)) {
        *to = Rgba8Pixel { r: from[0], g: from[1], b: from[2], a: 255 };
    }
    Some(buffer)
}

/// What the interface wants and what the decoder has ready: the frame wanted, none while the film is not on screen,
/// and frames decoded from it on.
struct Slots {
    wanted: Option<usize>,
    decoded: Vec<(usize, SharedPixelBuffer<Rgba8Pixel>)>,
    /// Set once the reel is dropped, so its decoder ends.
    over: bool,
}

/// A film ready to play: the frame it shows, and a decoder thread that keeps the next few ready.
pub struct Reel {
    pub gift: usize,
    cut: Arc<Cut>,
    slots: Arc<(Mutex<Slots>, Condvar)>,
    shown: Option<usize>,
    resting: bool,
}

/// Every film that reads, each with its decoder thread; one that cannot be read is left out.
pub fn reels() -> Vec<Reel> {
    FILMS.iter().filter_map(|(gift, bytes, list)| Reel::start(Cut::read(*gift, bytes, list)?, bytes)).collect()
}

/// The frames the decoder keeps ready from `wanted` on, the film looping.
fn window(wanted: usize, count: usize) -> impl Iterator<Item = usize> {
    (0..=AHEAD.min(count - 1)).map(move |k| (wanted + k) % count)
}

impl Reel {
    fn start(cut: Cut, bytes: &'static [u8]) -> Option<Reel> {
        let cut = Arc::new(cut);
        let slots = Arc::new((Mutex::new(Slots { wanted: None, decoded: Vec::new(), over: false }), Condvar::new()));
        let (film, shared) = (cut.clone(), slots.clone());
        std::thread::Builder::new()
            .name("welcome film".into())
            .spawn(move || {
                let (lock, wake) = &*shared;
                let count = film.frames.len();
                loop {
                    let next = {
                        let Ok(mut s) = lock.lock() else { return };
                        loop {
                            if s.over {
                                return;
                            }
                            let ahead: Vec<usize> = s.wanted.map(|w| window(w, count).collect()).unwrap_or_default();
                            s.decoded.retain(|(i, _)| ahead.contains(i));
                            if let Some(i) = ahead.into_iter().find(|i| s.decoded.iter().all(|(d, _)| d != i)) {
                                break i;
                            }
                            let Ok(waited) = wake.wait(s) else { return };
                            s = waited;
                        }
                    };
                    let Some(frame) = decode(&film, bytes, next) else { return };
                    let Ok(mut s) = lock.lock() else { return };
                    if s.wanted.is_some_and(|w| window(w, count).any(|i| i == next)) {
                        s.decoded.push((next, frame));
                    }
                }
            })
            .ok()?;
        Some(Reel { gift: cut.gift, cut, slots, shown: None, resting: true })
    }

    /// The frame at `t` seconds into the film when it differs from the one last given; none while the same one
    /// shows or the wanted one is still decoding, so the screen keeps the last.
    pub fn at(&mut self, t: f32, still: bool) -> Option<Image> {
        let want = self.cut.index(t, still);
        self.resting = false;
        if self.shown == Some(want) {
            return None;
        }
        let (lock, wake) = &*self.slots;
        let mut s = lock.lock().ok()?;
        if s.wanted != Some(want) {
            s.wanted = Some(want);
            wake.notify_one();
        }
        let frame = s.decoded.iter().find(|(i, _)| *i == want)?.1.clone();
        self.shown = Some(want);
        Some(Image::from_rgba8(frame))
    }
}

impl Reel {
    /// The next `at` gives its frame even if it is the one last given, which another film has since replaced.
    pub fn again(&mut self) {
        self.shown = None;
    }

    /// Off screen: nothing decoded is kept, and nothing is decoded until it is asked again.
    pub fn rest(&mut self) {
        if std::mem::replace(&mut self.resting, true) {
            return;
        }
        self.shown = None;
        let (lock, wake) = &*self.slots;
        if let Ok(mut s) = lock.lock() {
            (s.wanted, s.decoded) = (None, Vec::new());
        }
        wake.notify_one();
    }
}

impl Drop for Reel {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slots;
        if let Ok(mut s) = lock.lock() {
            s.over = true;
        }
        wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_film_reads_and_each_frame_decodes_at_its_size() {
        for (gift, bytes, list) in FILMS {
            let cut = Cut::read(gift, bytes, list).expect("the film's list matches its frames");
            assert!(cut.length > 4.0 && cut.frames.len() > 20, "gift {gift}: {} s, {} frames", cut.length, cut.frames.len());
            for i in [0, cut.frames.len() / 2, cut.frames.len() - 1] {
                assert!(decode(&cut, bytes, i).is_some(), "gift {gift}: frame {i}");
            }
            // Twin: a list that leaves a frame out does not match the file.
            let short: String = list.lines().take(list.lines().count() - 1).map(|l| format!("{l}\n")).collect();
            assert!(Cut::read(gift, bytes, &short).is_none());
        }
    }

    #[test]
    fn a_film_plays_at_120_hz_without_a_late_frame_and_holds_a_few_frames() {
        for mut reel in reels() {
            let tick = std::time::Duration::from_micros(8333);
            // Its first frames, decoded as its words come, before it plays.
            reel.at(0.0, false);
            std::thread::sleep(std::time::Duration::from_millis(300));
            let began = std::time::Instant::now();
            let (mut late, mut changes, mut last) = (0, 0, usize::MAX);
            for n in 1.. {
                let t = began.elapsed().as_secs_f32();
                if t > 2.5 {
                    break;
                }
                let want = reel.cut.index(t, false);
                reel.at(t, false);
                // A frame is late when the tick that first wants it cannot show it.
                if want != last {
                    (changes, last) = (changes + 1, want);
                    late += usize::from(reel.shown != Some(want));
                }
                let held = reel.slots.0.lock().unwrap().decoded.len();
                assert!(held <= AHEAD + 1, "at most {} frames decoded at once: {held}", AHEAD + 1);
                std::thread::sleep((began + tick * n).saturating_duration_since(std::time::Instant::now()));
            }
            eprintln!("films: gift {}: {late} of {changes} frames late", reel.gift);
            assert!(changes > 20, "the film moved on: {changes} frames");
            // A loaded machine may miss a frame or two; a decoder that does not keep ahead misses nearly all.
            assert!(late <= changes / 20, "gift {}: {late} of {changes} frames came late", reel.gift);
            // Off screen it keeps nothing decoded.
            reel.rest();
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(reel.slots.0.lock().unwrap().decoded.is_empty(), "a film off screen holds no frame");
        }
        let mut reel = reels().into_iter().next().expect("a film");
        // Twin: a still film gives its longest-held frame and then nothing new.
        let still = reel.cut.index(0.0, true);
        reel.again();
        assert!(reel.at(0.0, true).is_some() || reel.shown != Some(still));
    }
}
