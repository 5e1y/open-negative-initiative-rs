//! The gifts' films, made by `crates/oni-app/welcome-motion` (`scripts/welcome-assets`): each one file of its frames
//! end to end, each a WebP read by a decoder in pure Rust, and a list of how long each holds and how many bytes it takes. A film is decoded as it
//! plays, a few frames ahead, off the interface's thread: none is ever held whole in memory.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The films, by the gift they show: their frames, then their holds and lengths.
const FILMS: [(usize, &[u8], &str); 4] = [
    (0, include_bytes!("../../../../oni-ui/ui/welcome/film-interface.webps"), include_str!("../../../../oni-ui/ui/welcome/film-interface.txt")),
    (1, include_bytes!("../../../../oni-ui/ui/welcome/film-collection.webps"), include_str!("../../../../oni-ui/ui/welcome/film-collection.txt")),
    (2, include_bytes!("../../../../oni-ui/ui/welcome/film-huge.webps"), include_str!("../../../../oni-ui/ui/welcome/film-huge.txt")),
    (3, include_bytes!("../../../../oni-ui/ui/welcome/film-wrapped.webps"), include_str!("../../../../oni-ui/ui/welcome/film-wrapped.txt")),
];
/// How many frames are kept decoded from the one aimed at: at 25 frames a second, a sixth of a second.
const AHEAD: usize = 4;
/// What a frame is reckoned to cost to decode before one has been timed, in seconds.
const FIRST_COST: f32 = 0.04;

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

/// The frames worth decoding when the film stands at `t` and a frame takes `cost` to decode: the one that will be
/// due once it is decoded, then the next ones. Aimed at the frame due now, a decoder slower than the film would only
/// ever finish frames already past.
fn aimed(cut: &Cut, t: f32, cost: f32) -> Vec<usize> {
    let first = cut.index(t + cost, false);
    (0..=AHEAD.min(cut.frames.len() - 1)).map(|k| (first + k) % cut.frames.len()).collect()
}

/// How far frame `to` comes after frame `from`, the film looping.
fn after(from: usize, to: usize, count: usize) -> usize {
    (to + count - from) % count
}

/// Whether a decoded frame is worth keeping when `due` is due: one still to come up to the last frame aimed at, or the
/// latest one past, which shows while the next come.
fn kept(i: usize, due: usize, aimed: &[usize], past: Option<usize>, count: usize) -> bool {
    let horizon = aimed.last().map_or(0, |last| after(due, *last, count));
    after(due, i, count) <= horizon || Some(i) == past
}

/// The frame to show when `due` is due, among those decoded: it if it is ready, else the latest one already past it,
/// so a slow decoder skips frames and the film keeps its time; none if every decoded frame is still to come.
fn pick(decoded: impl Iterator<Item = usize>, due: usize, count: usize) -> Option<usize> {
    decoded.filter(|i| after(*i, due, count) < count / 2).min_by_key(|i| after(*i, due, count))
}

/// What the screen asks: where the film stands and when it was asked, or the frame it holds still.
#[derive(Clone, Copy)]
enum Ask {
    Playing(f32, Instant),
    Still(usize),
}

/// What the interface asks and what the decoders have ready; none asked while the film is off screen.
struct Slots {
    ask: Option<Ask>,
    decoded: Vec<(usize, SharedPixelBuffer<Rgba8Pixel>)>,
    /// The frames being decoded, and what one has lately cost, in seconds.
    busy: Vec<usize>,
    cost: f32,
    /// Set once the reel is dropped, so its decoders end.
    over: bool,
}

impl Slots {
    /// The frames to decode now, those aimed at, and the frames decoded worth keeping.
    fn wanted(&self, cut: &Cut) -> (Vec<usize>, Vec<usize>) {
        let n = cut.frames.len();
        match self.ask {
            None => (Vec::new(), Vec::new()),
            Some(Ask::Still(i)) => (vec![i], vec![i]),
            Some(Ask::Playing(t, at)) => {
                let now = t + at.elapsed().as_secs_f32();
                let (due, aimed) = (cut.index(now, false), aimed(cut, now, self.cost));
                let past = pick(self.decoded.iter().map(|(i, _)| *i), due, n);
                let keep = self.decoded.iter().map(|(i, _)| *i).filter(|i| kept(*i, due, &aimed, past, n)).collect();
                (aimed, keep)
            }
        }
    }
}

/// A film ready to play: the frame it shows, and decoders on threads of their own that keep the next few ready.
pub struct Reel {
    pub gift: usize,
    cut: Arc<Cut>,
    slots: Arc<(Mutex<Slots>, Condvar)>,
    shown: Option<usize>,
    resting: bool,
}

/// Every film that reads, each with its decoders; one that cannot be read is left out.
pub fn reels() -> Vec<Reel> {
    FILMS.iter().filter_map(|(gift, bytes, list)| Reel::start(Cut::read(*gift, bytes, list)?, bytes)).collect()
}

/// A film's decoders: a few, so a frame's decoding on a slower processor still keeps up, never all the cores.
fn decoders() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(2).clamp(1, 3)
}

/// One decoder: it takes the first frame wanted that no one has ready or under way, decodes it, times it.
fn decoding(cut: &Cut, bytes: &[u8], shared: &(Mutex<Slots>, Condvar)) {
    let (lock, wake) = shared;
    let frame = cut.starts.get(1).map_or(0.04, |s| s - cut.starts[0]).max(0.01);
    loop {
        let next = {
            let Ok(mut s) = lock.lock() else { return };
            loop {
                if s.over {
                    return;
                }
                let (aimed, keep) = s.wanted(cut);
                s.decoded.retain(|(i, _)| keep.contains(i));
                if let Some(i) = aimed.into_iter().find(|i| s.decoded.iter().all(|(d, _)| d != i) && !s.busy.contains(i)) {
                    s.busy.push(i);
                    break i;
                }
                // The film moves on by itself: look again within half a frame.
                let Ok((waited, _)) = wake.wait_timeout(s, Duration::from_secs_f32(frame / 2.0)) else { return };
                s = waited;
            }
        };
        let began = Instant::now();
        let decoded = decode(cut, bytes, next);
        let Ok(mut s) = lock.lock() else { return };
        s.busy.retain(|i| *i != next);
        s.cost = 0.7 * s.cost + 0.3 * began.elapsed().as_secs_f32();
        // Kept while it is still to come or the latest one past: a slow decoder's frame is shown late, never dropped.
        let Some(frame) = decoded else { return };
        s.decoded.push((next, frame));
        let (_, keep) = s.wanted(cut);
        s.decoded.retain(|(i, _)| keep.contains(i));
    }
}

impl Reel {
    fn start(cut: Cut, bytes: &'static [u8]) -> Option<Reel> {
        let cut = Arc::new(cut);
        let slots = Arc::new((Mutex::new(Slots { ask: None, decoded: Vec::new(), busy: Vec::new(), cost: FIRST_COST, over: false }), Condvar::new()));
        for _ in 0..decoders() {
            let (film, shared) = (cut.clone(), slots.clone());
            std::thread::Builder::new().name("welcome film".into()).spawn(move || decoding(&film, bytes, &shared)).ok()?;
        }
        Some(Reel { gift: cut.gift, cut, slots, shown: None, resting: true })
    }

    /// The frame to show at `t` seconds into the film when it differs from the one last given: the frame due if it is
    /// decoded, else the latest one past; none while nothing newer is ready. It never waits on a decoder.
    pub fn at(&mut self, t: f32, still: bool) -> Option<Image> {
        self.resting = false;
        let (lock, wake) = &*self.slots;
        let mut s = lock.lock().ok()?;
        let count = self.cut.frames.len();
        let due = self.cut.index(t, still);
        s.ask = Some(if still { Ask::Still(due) } else { Ask::Playing(t, Instant::now()) });
        wake.notify_all();
        let chosen = pick(s.decoded.iter().map(|(i, _)| *i), due, count)?;
        if self.shown.is_some_and(|shown| shown == chosen || after(chosen, shown, count) < count / 2) {
            return None;
        }
        let frame = s.decoded.iter().find(|(i, _)| *i == chosen)?.1.clone();
        self.shown = Some(chosen);
        Some(Image::from_rgba8(frame))
    }

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
            (s.ask, s.decoded) = (None, Vec::new());
        }
        wake.notify_all();
    }
}

impl Drop for Reel {
    fn drop(&mut self) {
        let (lock, wake) = &*self.slots;
        if let Ok(mut s) = lock.lock() {
            s.over = true;
        }
        wake.notify_all();
    }
}

#[cfg(test)]
mod tests;
