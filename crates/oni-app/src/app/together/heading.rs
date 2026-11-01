//! The sheet's heading across its top, each part a column wide: the logo at the left, an EAN-13 of
//! the frame count and the roll's date in the middle, the roll's name at the right, at full scale.

use super::*;
use super::plate::{Anchor, Face, Lettering, Plate};
use resvg::{tiny_skia, usvg};
use swash::scale::ScaleContext;

const LOGO_SVG: &[u8] = include_bytes!("../../../../oni-ui/ui/icons/oni-logo.svg");
/// An EAN's width in modules, its first digit's seven included; its guards' reach below its bars
/// and its digits' size, in modules.
const SPAN: f32 = 102.0;
const GUARDS: f32 = 8.0;
const DIGITS: f32 = 9.0;

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

/// A column of the plate: its left, top, width and height.
type Column = (f32, f32, f32, f32);

/// The heading on a clear plate `width` wide and `HEADER` tall, in three columns a tile wide: the logo
/// at the left, the EAN in the middle, the roll's name at the right, each across its column.
pub(in crate::app) fn drawn(heading: &Heading, width: usize, lettering: Lettering) -> Plate {
    let mut plate = Plate::clear(width, HEADER);
    let mut context = ScaleContext::new();
    // A sheet narrower than three tiles sets the whole heading smaller, centred in its band.
    let column = (TILE as f32).min((width as f32 - 2.0 * GAP as f32) / 3.0).max(1.0);
    let height = HEADER as f32 * column / TILE as f32;
    let top = (HEADER as f32 - height) / 2.0;
    logo(&mut plate, (0.0, top, column, height));
    bars(&mut plate, ((width as f32 - column) / 2.0, top, column, height), &ean(heading.count, &heading.date), lettering, &mut context);
    name(&mut plate, (width as f32 - column, top, column, height), &heading.roll, lettering, &mut context);
    plate
}

/// The logo as wide as `at` allows within its height, against its left edge, in its own colours.
fn logo(plate: &mut Plate, at: Column) {
    let Ok(tree) = usvg::Tree::from_data(LOGO_SVG, &usvg::Options::default()) else { return };
    let size = tree.size();
    let scale = (at.2 / size.width()).min(at.3 / size.height());
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let Some(mut pixmap) = tiny_skia::Pixmap::new(w, h) else { return };
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let (left, top) = (at.0.round() as i64, (at.1 + (at.3 - h as f32) / 2.0).round() as i64);
    for (i, p) in pixmap.pixels().iter().enumerate() {
        let c = p.demultiply();
        let ink = [c.red(), c.green(), c.blue()].map(|v| f32::from(v) / 255.0);
        plate.cover(left + (i % w as usize) as i64, top + (i / w as usize) as i64, f32::from(c.alpha()) / 255.0, ink);
    }
}

/// The EAN across `at` in `lettering`'s ink, its first digit at its left, its guards reaching down
/// to its digits' baseline at the column's bottom; each bar's edges on whole pixels.
fn bars(plate: &mut Plate, at: Column, digits: &[u8; 13], lettering: Lettering, context: &mut ScaleContext) {
    let module = at.2 / SPAN;
    let x0 = at.0 + (SPAN - 95.0) * module;
    let edge = |k: usize| (x0 + k as f32 * module).round() as i64;
    let guard = |k: usize| k < 3 || (45..50).contains(&k) || k >= 92;
    let (top, bars) = (at.1.round() as i64, (at.3 - GUARDS * module).round() as i64);
    for (k, dark) in modules(digits).into_iter().enumerate() {
        if dark {
            plate.rect((edge(k), top, edge(k + 1) - edge(k), if guard(k) { at.3.round() as i64 } else { bars }), lettering.ink);
        }
    }
    let (baseline, size) = (at.1 + at.3, DIGITS * module);
    let mut digit = |m: f32, d: u8, anchor| plate.text((x0 + m * module, baseline), &d.to_string(), size, lettering, anchor, context);
    digit(-1.0, digits[0], Anchor::End);
    for i in 0..6 {
        digit(3.0 + 7.0 * i as f32 + 3.5, digits[1 + i], Anchor::Centre);
        digit(50.0 + 7.0 * i as f32 + 3.5, digits[7 + i], Anchor::Centre);
    }
}

/// `text` across `at`, against its right edge, its capitals centred in its height; a short one is
/// set no taller than its descenders still fit.
fn name(plate: &mut Plate, at: Column, text: &str, lettering: Lettering, context: &mut ScaleContext) {
    let Some(face) = Face::of(lettering.weight) else { return };
    let (cap, descent) = face.heights(1.0);
    let size = (at.2 / face.width(text, 1.0).max(1e-3)).min(at.3 / (cap + 2.0 * descent));
    plate.text((at.0 + at.2, at.1 + (at.3 + cap * size) / 2.0), text, size, lettering, Anchor::End, context);
}

#[cfg(test)]
mod tests {
    use super::super::plate::{ORANGE, WHITE};
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

    /// The first and last columns of `plate` holding ink in `xs`.
    fn span(plate: &Plate, xs: std::ops::Range<usize>) -> (usize, usize) {
        let inked: Vec<usize> = columns(plate, 0..plate.height).into_iter().filter(|x| xs.contains(x)).collect();
        (inked[0], *inked.last().unwrap())
    }

    #[test]
    fn the_heading_sets_the_logo_the_ean_and_the_roll_each_across_a_column() {
        let heading = Heading { roll: "2026-10-30".into(), count: 38, date: "20261030".into() };
        // Five columns a tile wide, the middle one from 2108 to 3132, the last from 4216.
        let width = 5 * TILE + 4 * GAP;
        let plate = drawn(&heading, width, WHITE);
        let (first, last) = span(&plate, 0..TILE + GAP);
        assert!(first < 4 && last > TILE - 8, "the logo across the first column: {first}..{last}");
        let (first, last) = span(&plate, 2 * (TILE + GAP)..3 * (TILE + GAP));
        assert!(first < 2108 + 12 && (last as i64 - 3131).abs() <= 1, "the EAN across the middle one: {first}..{last}");
        let (first, last) = span(&plate, 4 * (TILE + GAP)..width);
        assert!(first < 4216 + 12 && last > width - 12, "the roll across the last one: {first}..{last}");
        let bars = (0..plate.height).filter(|&y| plate.pixels[y * width + 2108 + 1024 - 2][3] > 0.5).count();
        assert_eq!(bars, HEADER, "the end guard runs the band's height");
        let lit = |p: &Plate| p.pixels.iter().filter(|p| p[3] > 0.9 && p[1] > 0.9).count();
        assert!(lit(&plate) > 100_000, "set in white");
        // Twin: in the numbers' orange, the EAN and the roll show no white, the logo alone does.
        assert!(lit(&drawn(&heading, width, ORANGE)) < lit(&plate) / 2);
        // Twin: a short name is no taller than its band, a sheet of one frame sets all three smaller.
        let short = drawn(&Heading { roll: "R8".into(), ..heading.clone() }, width, WHITE);
        let rows: Vec<usize> = (0..HEADER).filter(|&y| (4216..width).any(|x| short.pixels[y * width + x][3] > 0.5)).collect();
        assert!(rows[0] > 8 && *rows.last().unwrap() < HEADER - 8, "rows {}..{}", rows[0], rows.last().unwrap());
        let one = drawn(&heading, TILE, WHITE);
        let third = (TILE - 2 * GAP) / 3;
        assert!(span(&one, 0..third + GAP).1 < third + 2 && span(&one, TILE - third..TILE).0 > TILE - third - 2, "three thirds side by side");
    }
}
