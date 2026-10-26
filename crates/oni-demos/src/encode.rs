//! A clip to a small looping GIF: one palette, the interface's flat colours exact, then each frame
//! only where it changed over the frame before. Same frames in, same bytes out.

use std::collections::{BTreeMap, HashMap};

use color_quant::NeuQuant;

use crate::photo::Rgba;

/// The index no colour takes: a pixel left as the frame before laid it.
const CLEAR: u8 = 255;
/// A colour covering this share of the pixels off the photograph is the interface's, kept to the bit.
const FLAT_SHARE: f64 = 0.002;
const MAX_FLAT: usize = 96;
/// NeuQuant's sampling: 1 reads every pixel, 30 the fastest; 10 is its own advice.
const SAMPLING: i32 = 10;
/// A 4 × 4 ordered dither laid on the interface's colourful shades, never the photograph's, so a rail
/// runs smooth through the few the palette leaves it; by place, so a pixel maps alike on every frame.
const BAYER: [[i16; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// The dither's spread in levels a channel, about a learnt colour's spacing on a rail, and the
/// colourfulness below which a pixel is a grey, undithered so a moving knob costs no dither.
const SPREAD: i16 = 28;
const CHROMA: u8 = 12;
/// The interface's greys, the keys' and the knobs' shades, on a ramp of their own: a learnt colour
/// near a grey would tint it.
const GREYS: usize = 40;

/// Frames of one size, shown each `delay` hundredths of a second, looping; `photo` is where the
/// photograph lies in them.
pub struct Clip {
    pub frames: Vec<Rgba>,
    pub delay: u16,
    pub photo: Region,
}

/// A rectangle of pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    fn holds(&self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// How a clip is encoded: how far a photograph's pixel may stray from what is shown before it is
/// written again, per channel, and how many colours the photograph is given.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub tolerance: u8,
    pub colours: usize,
}

pub fn gif(clip: &Clip, tuning: Tuning) -> Result<Vec<u8>, String> {
    let first = clip.frames.first().ok_or("a clip of no frame")?;
    let (w, h) = (first.width, first.height);
    if clip.frames.iter().any(|f| (f.width, f.height) != (w, h)) {
        return Err("the frames differ in size".into());
    }
    let palette = Palette::learnt(&clip.frames, clip.photo, tuning.colours);
    let mut out = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut out, w as u16, h as u16, &palette.rgb()).map_err(|e| e.to_string())?;
        encoder.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
        let mut shown: Option<Vec<u8>> = None;
        let mut pending: Option<gif::Frame<'static>> = None;
        for frame in &clip.frames {
            let indices = palette.map(frame);
            let next = match shown.as_mut() {
                Some(shown) => changed(&indices, &palette, shown, w, tuning.tolerance),
                None => Some(whole(&indices, w, h)),
            };
            shown.get_or_insert_with(|| indices.clone());
            match next {
                // Nothing new: the frame before shows longer.
                None => pending.iter_mut().for_each(|p| p.delay += clip.delay),
                Some(mut f) => {
                    if let Some(p) = pending.take() {
                        encoder.write_frame(&p).map_err(|e| e.to_string())?;
                    }
                    f.delay = clip.delay;
                    pending = Some(f);
                }
            }
        }
        if let Some(p) = pending {
            encoder.write_frame(&p).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

fn whole(indices: &[u8], w: u32, h: u32) -> gif::Frame<'static> {
    gif::Frame { width: w as u16, height: h as u16, buffer: indices.to_vec().into(), ..gif::Frame::default() }
}

/// The rectangle of what changed, laid over `shown`; `None` when nothing did. Inside it, what kept
/// its place is either transparent or written again as it stands, whichever packs smaller.
fn changed(indices: &[u8], palette: &Palette, shown: &mut [u8], w: u32, tolerance: u8) -> Option<gif::Frame<'static>> {
    let mut mask = vec![CLEAR; indices.len()];
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (i, &index) in indices.iter().enumerate() {
        let (now, was) = (palette.colours[index as usize], palette.colours[shown[i] as usize]);
        let stray = was.iter().zip(now).map(|(a, b)| a.abs_diff(b)).max().unwrap_or(0);
        // The interface is drawn exact, so a knob that moved on leaves no trace on the ground.
        if stray == 0 || (palette.loose(index) && stray <= tolerance) {
            continue;
        }
        mask[i] = index;
        let (x, y) = (i as u32 % w, i as u32 / w);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    if x0 > x1 {
        return None;
    }
    let (fw, fh) = (x1 - x0 + 1, y1 - y0 + 1);
    let (mut clear, mut opaque) = (Vec::with_capacity((fw * fh) as usize), Vec::with_capacity((fw * fh) as usize));
    for y in y0..=y1 {
        for i in (y * w + x0) as usize..=(y * w + x1) as usize {
            if mask[i] != CLEAR {
                shown[i] = mask[i];
            }
            clear.push(mask[i]);
            opaque.push(shown[i]);
        }
    }
    let transparent = packed(&clear) < packed(&opaque);
    Some(gif::Frame {
        left: x0 as u16,
        top: y0 as u16,
        width: fw as u16,
        height: fh as u16,
        buffer: if transparent { clear } else { opaque }.into(),
        transparent: transparent.then_some(CLEAR),
        dispose: gif::DisposalMethod::Keep,
        ..gif::Frame::default()
    })
}

/// The bytes LZW packs `indices` into, as GIF writes them.
fn packed(indices: &[u8]) -> usize {
    weezl::encode::Encoder::new(weezl::BitOrder::Lsb, 8).encode(indices).map_or(usize::MAX, |b| b.len())
}

/// A learnt part of the palette: its network and the index its colours start at.
struct Learnt {
    net: NeuQuant,
    offset: usize,
}

struct Palette {
    colours: Vec<[u8; 3]>,
    flat: HashMap<[u8; 3], u8>,
    /// Where the grey ramp starts.
    ramp: usize,
    /// The photograph's colours, then the interface's gradients and edges, each learnt apart so the
    /// rails' few colours are not lost among the photograph's many.
    photo: Option<Learnt>,
    interface: Option<Learnt>,
    region: Region,
    width: u32,
}

impl Palette {
    fn learnt(frames: &[Rgba], region: Region, room: usize) -> Palette {
        let width = frames[0].width;
        // The interface's flat colours are counted where the photograph is not, so a large even tone
        // of the photograph stays one of its own, free to stray by the tolerance.
        let mut counts: HashMap<[u8; 3], u64> = HashMap::new();
        for f in frames {
            for (i, p) in f.pixels.chunks_exact(4).enumerate() {
                if !region.holds(i as u32 % width, i as u32 / width) {
                    *counts.entry([p[0], p[1], p[2]]).or_default() += 1;
                }
            }
        }
        let total: u64 = counts.values().sum();
        // Ordered by count then colour, so the palette never depends on the map's order.
        let ranked: BTreeMap<(std::cmp::Reverse<u64>, [u8; 3]), ()> = counts.iter().map(|(c, n)| ((std::cmp::Reverse(*n), *c), ())).collect();
        let flat: Vec<[u8; 3]> = ranked.keys().take_while(|(n, _)| n.0 as f64 >= total as f64 * FLAT_SHARE).take(MAX_FLAT).map(|(_, c)| *c).collect();
        let flat_set: HashMap<[u8; 3], u8> = flat.iter().enumerate().map(|(i, c)| (*c, i as u8)).collect();
        // The interface's colourful shades, each weighed by the root of its count: a rail's gradient,
        // many colours a few pixels each, is not lost among fewer covering more; its greys take the ramp.
        let (mut inside, mut outside) = (Vec::new(), BTreeMap::<[u8; 3], u64>::new());
        for f in frames {
            for (i, p) in f.pixels.chunks_exact(4).enumerate() {
                let c = [p[0], p[1], p[2]];
                if flat_set.contains_key(&c) {
                    continue;
                }
                if region.holds(i as u32 % width, i as u32 / width) {
                    inside.extend_from_slice(&[c[0], c[1], c[2], 255]);
                } else if !grey(c) {
                    *outside.entry(c).or_default() += 1;
                }
            }
        }
        let outside: Vec<u8> = outside.iter().flat_map(|(c, n)| std::iter::repeat_n([c[0], c[1], c[2], 255], (*n as f64).sqrt().ceil() as usize).flatten()).collect();
        let mut colours = flat;
        let ramp = colours.len();
        colours.extend((0..GREYS).map(|k| [(k * 255 / (GREYS - 1)) as u8; 3]));
        let room = room.min(CLEAR as usize - colours.len());
        let photo = learn(&inside, room, &mut colours);
        let interface = learn(&outside, CLEAR as usize - colours.len(), &mut colours);
        colours.resize(256, [0, 0, 0]);
        Palette { colours, flat: flat_set, ramp, photo, interface, region, width }
    }

    fn rgb(&self) -> Vec<u8> {
        self.colours.iter().flatten().copied().collect()
    }

    /// Whether `index` is a photograph's colour, which may stray by the tolerance.
    fn loose(&self, index: u8) -> bool {
        let start = self.photo.as_ref().map_or(usize::MAX, |p| p.offset);
        let end = self.interface.as_ref().map_or(CLEAR as usize, |l| l.offset);
        (start..end).contains(&(index as usize))
    }

    fn map(&self, frame: &Rgba) -> Vec<u8> {
        let flat = self.flat.len();
        frame
            .pixels
            .chunks_exact(4)
            .enumerate()
            .map(|(i, p)| {
                let c = [p[0], p[1], p[2]];
                if let Some(index) = self.flat.get(&c) {
                    return *index;
                }
                let (x, y) = (i as u32 % self.width, i as u32 / self.width);
                let inside = self.region.holds(x, y);
                if !inside && grey(c) {
                    let mean = c.iter().map(|v| usize::from(*v)).sum::<usize>() / 3;
                    return (self.ramp + (mean * (GREYS - 1) + 127) / 255) as u8;
                }
                let c = if inside { c } else { dithered(c, x, y) };
                match if inside { self.photo.as_ref().or(self.interface.as_ref()) } else { self.interface.as_ref().or(self.photo.as_ref()) } {
                    Some(l) => (l.offset + l.net.index_of(&[c[0], c[1], c[2], 255])) as u8,
                    None => nearest(&self.colours[..flat], c),
                }
            })
            .collect()
    }
}

/// `room` colours learnt from `pixels` and laid after `colours`; none for too few pixels to learn from.
fn learn(pixels: &[u8], room: usize, colours: &mut Vec<[u8; 3]>) -> Option<Learnt> {
    if pixels.len() < 4 * 256 || room < 2 {
        return None;
    }
    let net = NeuQuant::new(SAMPLING, room, pixels);
    let offset = colours.len();
    colours.extend(net.color_map_rgb().chunks_exact(3).map(|c| [c[0], c[1], c[2]]));
    Some(Learnt { net, offset })
}

/// Whether `c` is a grey: its channels within `CHROMA` of each other.
fn grey(c: [u8; 3]) -> bool {
    c.iter().max().unwrap_or(&0) - c.iter().min().unwrap_or(&0) < CHROMA
}

fn dithered(c: [u8; 3], x: u32, y: u32) -> [u8; 3] {    let offset = (2 * BAYER[(y % 4) as usize][(x % 4) as usize] + 1 - 16) * SPREAD / 32;
    c.map(|v| (i16::from(v) + offset).clamp(0, 255) as u8)
}

fn nearest(colours: &[[u8; 3]], c: [u8; 3]) -> u8 {
    let d = |k: &[u8; 3]| k.iter().zip(c).map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2)).sum::<i32>();
    colours.iter().enumerate().min_by_key(|(_, k)| d(k)).map_or(0, |(i, _)| i as u8)
}
