//! The sheet's heading across its top: the logo at the left, an EAN-13 of the frame count and the
//! roll's date in the middle, the roll's name at the right, set at the sheet's full scale.

use super::*;
use super::plate::{Anchor, Face, Lettering, Plate};
use resvg::{tiny_skia, usvg};
use swash::scale::ScaleContext;

const LOGO_SVG: &[u8] = include_bytes!("../../../../oni-ui/ui/icons/oni-logo.svg");
/// The logo's height; an EAN module's width, its bars' height, its guards' reach below them to its
/// digits' baseline and their size, in pixels of the full sheet.
const LOGO: f32 = 48.0;
const MODULE: i64 = 3;
const BARS: i64 = 72;
const GUARDS: i64 = 20;
const DIGITS: f32 = 26.0;
/// The room kept between the roll's name and the EAN: a longer name is set smaller.
const CLEAR: f32 = 60.0;

/// What the heading says: the roll's name, the frames on the sheet, the roll's date as YYYYMMDD.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Heading {
    pub roll: String,
    pub count: usize,
    pub date: String,
}

impl Ctl {
    /// The heading of `paths`: their rolls' names in order, a frame on none named by its folder; the
    /// first roll's date, else the first frame's capture, else today's.
    pub(in crate::app) fn heading_of(&self, paths: &[String]) -> Heading {
        let s = self.state.borrow();
        let wanted: HashSet<&str> = paths.iter().map(String::as_str).collect();
        // The first entry of a path wins, as a search finds it.
        let entries: HashMap<&str, &oni_library::Entry> =
            s.archive.as_ref().map(|a| a.entries().into_iter().rev().filter(|e| wanted.contains(e.path.as_str())).map(|e| (e.path.as_str(), e)).collect()).unwrap_or_default();
        let film = |p: &str| entries.get(p).and_then(|e| e.film.as_ref()).filter(|f| !f.name.trim().is_empty());
        let mut names: Vec<String> = Vec::new();
        for p in paths {
            let folder = oni_library::paths::parent(p);
            let name = film(p).map_or_else(|| Path::new(&folder).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(folder.clone()), |f| f.name.trim().to_owned());
            if !names.contains(&name) {
                names.push(name);
            }
        }
        let date = paths.iter().find_map(|p| film(p).and_then(|f| f.shot_date)).or_else(|| paths.first().and_then(|p| entries.get(p.as_str())?.captured_at));
        Heading { roll: names.join(" · "), count: paths.len(), date: day(date.unwrap_or_else(oni_settings::Date::now)) }
    }
}

/// A date's day as YYYYMMDD.
fn day(date: oni_settings::Date) -> String {
    date.iso8601().chars().filter(char::is_ascii_digit).take(8).collect()
}

/// The left half's codes (L); the right half's are their complements (R), the even ones (G) the R
/// codes reversed.
const L: [u8; 10] = [0b0001101, 0b0011001, 0b0010011, 0b0111101, 0b0100011, 0b0110001, 0b0101111, 0b0111011, 0b0110111, 0b0001011];
/// Which of the left half's six digits take G, by the first digit, the leftmost first.
const PARITY: [u8; 10] = [0b000000, 0b001011, 0b001101, 0b001110, 0b010011, 0b011001, 0b011100, 0b010101, 0b010110, 0b011010];

fn r(d: u8) -> u8 {
    !L[usize::from(d)] & 0x7f
}

fn g(d: u8) -> u8 {
    (0..7).fold(0, |code, k| code << 1 | (r(d) >> k & 1))
}

/// The EAN-13 of `count` on four digits then `date` (YYYYMMDD), its check digit last.
pub(in crate::app) fn ean(count: usize, date: &str) -> [u8; 13] {
    let text = format!("{:04}{date}", count.min(9999));
    let mut digits = [0u8; 13];
    for (d, c) in digits.iter_mut().zip(text.bytes().filter(u8::is_ascii_digit).chain(std::iter::repeat(b'0')).take(12)) {
        *d = c - b'0';
    }
    let sum: u32 = digits[..12].iter().enumerate().map(|(i, &d)| u32::from(d) * if i % 2 == 0 { 1 } else { 3 }).sum();
    digits[12] = ((10 - sum % 10) % 10) as u8;
    digits
}

/// The 95 modules of an EAN-13, dark as true: its guards, the left half in the parities its first
/// digit names, the right half in R.
fn modules(digits: &[u8; 13]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(95);
    let mut put = |code: u8, n: u32| bits.extend((0..n).rev().map(|k| code >> k & 1 == 1));
    put(0b101, 3);
    for (i, &d) in digits[1..7].iter().enumerate() {
        let even = PARITY[usize::from(digits[0])] >> (5 - i) & 1 == 1;
        put(if even { g(d) } else { L[usize::from(d)] }, 7);
    }
    put(0b01010, 5);
    for &d in &digits[7..] {
        put(r(d), 7);
    }
    put(0b101, 3);
    bits
}

/// The heading on a clear plate `width` wide and `HEADER` tall: the logo against its left edge, the
/// EAN centred, the roll's name against its right edge, smaller when it would reach the EAN.
pub(in crate::app) fn drawn(heading: &Heading, width: usize, lettering: Lettering) -> Plate {
    let mut plate = Plate::clear(width, HEADER);
    let mut context = ScaleContext::new();
    logo(&mut plate, (HEADER as f32 - LOGO) / 2.0);
    let half = bars(&mut plate, &ean(heading.count, &heading.date), lettering, &mut context);
    if let Some(face) = Face::of(lettering.weight) {
        let room = width as f32 / 2.0 - half - CLEAR;
        let size = (NUMBER as f32).min(NUMBER as f32 * room / face.width(&heading.roll, NUMBER as f32).max(1.0)).max(8.0);
        let baseline = face.centred(0.0, HEADER as f32, size);
        plate.text((width as f32, baseline), &heading.roll, size, lettering, Anchor::End, &mut context);
    }
    plate
}

/// The logo `LOGO` tall from `top`, against the plate's left edge, in its own colours.
fn logo(plate: &mut Plate, top: f32) {
    let Ok(tree) = usvg::Tree::from_data(LOGO_SVG, &usvg::Options::default()) else { return };
    let size = tree.size();
    let scale = LOGO / size.height();
    let (w, h) = ((size.width() * scale).ceil() as u32, LOGO.ceil() as u32);
    let Some(mut pixmap) = tiny_skia::Pixmap::new(w, h) else { return };
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    for (i, p) in pixmap.pixels().iter().enumerate() {
        let c = p.demultiply();
        let ink = [c.red(), c.green(), c.blue()].map(|v| f32::from(v) / 255.0);
        plate.cover((i % w as usize) as i64, top.round() as i64 + (i / w as usize) as i64, f32::from(c.alpha()) / 255.0, ink);
    }
}

/// The EAN centred on the plate in `lettering`'s ink, its guards reaching down to its digits'
/// baseline; half the block's width, its first digit included.
fn bars(plate: &mut Plate, digits: &[u8; 13], lettering: Lettering, context: &mut ScaleContext) -> f32 {
    let x0 = (plate.width as i64 - 95 * MODULE) / 2;
    let top = (HEADER as i64 - BARS - GUARDS) / 2;
    let guard = |k: usize| k < 3 || (45..50).contains(&k) || k >= 92;
    for (k, dark) in modules(digits).into_iter().enumerate() {
        if dark {
            plate.rect((x0 + k as i64 * MODULE, top, MODULE, BARS + if guard(k) { GUARDS } else { 0 }), lettering.ink);
        }
    }
    let baseline = (top + BARS + GUARDS) as f32;
    let at = |module: f32| x0 as f32 + module * MODULE as f32;
    let mut digit = |module: f32, d: u8| plate.text((at(module), baseline), &d.to_string(), DIGITS, lettering, Anchor::Centre, context);
    digit(-4.5, digits[0]);
    for i in 0..6 {
        digit(3.0 + 7.0 * i as f32 + 3.5, digits[1 + i]);
        digit(50.0 + 7.0 * i as f32 + 3.5, digits[7 + i]);
    }
    plate.width as f32 / 2.0 - at(-9.0)
}

#[cfg(test)]
mod tests {
    use super::super::plate::ORANGE;
    use super::*;

    /// The digits read back from the modules by the tables, and each left digit's parity, G as true.
    fn read(bits: &[bool]) -> (Vec<u8>, Vec<bool>) {
        let code = |at: usize| bits[at..at + 7].iter().fold(0u8, |c, b| c << 1 | u8::from(*b));
        let mut digits = Vec::new();
        let mut even = Vec::new();
        for i in 0..6 {
            let c = code(3 + 7 * i);
            let d = (0..10).find(|&d| L[d as usize] == c || g(d) == c).expect("a left code");
            even.push(g(d) == c);
            digits.push(d);
        }
        for i in 0..6 {
            let c = code(50 + 7 * i);
            digits.push((0..10).find(|&d| r(d) == c).expect("a right code"));
        }
        (digits, even)
    }

    #[test]
    fn an_ean_carries_the_count_then_the_date_and_its_check_digit() {
        assert_eq!(ean(38, "20261030"), [0, 0, 3, 8, 2, 0, 2, 6, 1, 0, 3, 0, 7]);
        // The check digit of a published code: 400638133393 checks with 1.
        let mut known = ean(4006, "38133393");
        assert_eq!(known[12], 1);
        // Twin: a digit changed no longer checks.
        known[3] = 7;
        let sum: u32 = known[..12].iter().enumerate().map(|(i, &d)| u32::from(d) * if i % 2 == 0 { 1 } else { 3 }).sum();
        assert_ne!((10 - sum % 10) % 10, u32::from(known[12]));
    }

    #[test]
    fn the_modules_read_back_their_digits_in_their_first_digit_s_parities() {
        let digits = ean(4006, "38133393");
        let bits = modules(&digits);
        assert_eq!(bits.len(), 95);
        assert_eq!((&bits[..3], &bits[45..50], &bits[92..]), (&[true, false, true][..], &[false, true, false, true, false][..], &[true, false, true][..]), "the guards");
        let (back, even) = read(&bits);
        assert_eq!(back, digits[1..].to_vec());
        // A 4 opens LGLLGG.
        assert_eq!(even, [false, true, false, false, true, true]);
        // Twin: a first digit 0 sets every left digit in L.
        let zero = modules(&ean(38, "20261030"));
        assert!(read(&zero).1.iter().all(|e| !e));
    }

    /// The columns of `plate` holding any ink in the rows `rows`.
    fn columns(plate: &Plate, rows: std::ops::Range<usize>) -> Vec<usize> {
        (0..plate.width).filter(|&x| rows.clone().any(|y| plate.pixels[y * plate.width + x][3] > 0.5)).collect()
    }

    #[test]
    fn the_heading_lays_the_logo_left_the_ean_centred_and_the_roll_right() {
        let heading = Heading { roll: "Roll #8".into(), count: 38, date: "20261030".into() };
        let width = 3000;
        let plate = drawn(&heading, width, ORANGE);
        let logo = columns(&plate, 30..90);
        assert!(logo[0] < 4, "the logo against the left edge: {}", logo[0]);
        let white = plate.pixels.iter().filter(|p| p[3] > 0.9 && p[2] > 0.9).count();
        assert!(white > 500, "the logo in its own white: {white} pixels");
        // The bars span 95 modules about the middle; the name ends at the right edge.
        let middle = columns(&plate, 20..40);
        let bars: Vec<usize> = middle.iter().copied().filter(|&x| x > 1000 && x < 2000).collect();
        assert!((bars[0] as i64 - (1500 - 142)).abs() <= 3 && (*bars.last().unwrap() as i64 - (1500 + 142)).abs() <= 3, "{}..{}", bars[0], bars.last().unwrap());
        assert!(*columns(&plate, 30..90).last().unwrap() > width - 8, "the roll's name against the right edge");
        // Twin: a name too long for its half is set smaller, clear of the EAN.
        let long = Heading { roll: "A very long roll name that would run into the barcode".into(), ..heading };
        let crowded = drawn(&long, width, ORANGE);
        let named: Vec<usize> = columns(&crowded, 30..90).into_iter().filter(|&x| x > 1500 + 142).collect();
        assert!(named[0] as f32 >= 1500.0 + 142.0 + CLEAR - 4.0, "the name starts {} px from the middle", named[0] as i64 - 1500);
    }
}
