//! `ContactSheet`: several frames laid as one picture, six to a row, in Core Image's frame (origin
//! bottom left); each frame fitted in its cell, nothing painted between them.

use crate::reader::{Copy, Reader};
use crate::window::MEASURE_WINDOW_INSET;
use oni_settings::cg::{Rect, Size};
use oni_settings::frame::ContactSheetGrid;
use oni_settings::settings::roll_settings::{self, Field};
use oni_settings::{Film, PipelineSettings};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Six per row, because a strip of film is cut into six frames.
pub const COLUMNS: i64 = 6;

/// The gap between pictures and around them, one number so the margin cannot drift from the gutter.
pub const GAP: f64 = 0.04;

/// The largest sheet in pixels: Core Image renders up to 2^24 in one pass and tiles past it.
pub const MAX_PIXELS: f64 = 16_777_216.0;

/// The widest cell the decode side is cut for: a wider batch lays smaller, never upscaled.
pub const WIDEST_CELL: f64 = 2.0;

pub const ROLL_FRAMES: i64 = 36;

/// Derived from `MAX_PIXELS`, so the two meet exactly at a full roll.
pub fn roll_tile() -> f64 {
    tile_side(f64::MAX, rows(ROLL_FRAMES), WIDEST_CELL)
}

pub fn rows(frames: i64) -> i64 {
    (frames.max(1) + COLUMNS - 1) / COLUMNS
}

pub fn tile_side(wanted: f64, rows: i64, aspect: f64) -> f64 {
    let r = rows.max(1) as f64;
    let columns = COLUMNS as f64;
    let across = columns + GAP * (columns + 1.0);
    let down = r / aspect.max(0.05) + GAP * (r + 1.0);
    wanted.min((MAX_PIXELS / (across * down).max(1.0)).sqrt())
}

/// The batch's median aspect, so a roll fits with no letterboxing.
pub fn aspect(sizes: &[(usize, usize)]) -> f64 {
    let mut ratios: Vec<f64> = sizes.iter().map(|&(w, h)| if h > 0 { w as f64 / h as f64 } else { 1.0 }).collect();
    ratios.sort_by(f64::total_cmp);
    match ratios.get(ratios.len() / 2) {
        Some(r) => r.max(0.05),
        None => 1.0,
    }
}

/// The picture fitted inside its cell, never filled, so the framing shown is the export's.
pub fn fitted(size: (usize, usize), cell: &Rect) -> Rect {
    let (w, h) = (size.0 as f64, size.1 as f64);
    if !(w > 0.0 && h > 0.0) {
        return *cell;
    }
    let scale = (cell.width / w).min(cell.height / h);
    let (fw, fh) = (w * scale, h * scale);
    Rect::new(cell.x + cell.width / 2.0 - fw / 2.0, cell.y + cell.height / 2.0 - fh / 2.0, fw, fh)
}

/// Where one frame landed: the slot the grid reserved, and the rectangle the picture occupies in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub path: String,
    pub cell: Rect,
    pub frame: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub tiles: Vec<Tile>,
    pub width: usize,
    pub height: usize,
    pub lead: String,
    pub grid: ContactSheetGrid,
}

/// `ContactSheet.laid`'s geometry: whole pixels, then walked down until the sheet fits the cap;
/// `windows` are each frame's own measuring window in its crop, the inset one where absent.
pub fn laid(pictures: &[(String, (usize, usize))], lead: &str, wanted: f64, windows: &[Option<Rect>]) -> Option<Layout> {
    if pictures.is_empty() {
        return None;
    }
    let rows = rows(pictures.len() as i64);
    let shape = aspect(&pictures.iter().map(|p| p.1).collect::<Vec<_>>());
    let columns = COLUMNS as f64;
    let laid_out = |side: f64| {
        let high = (side / shape).floor().max(1.0);
        let margin = (side * GAP).round().max(1.0);
        let size = (columns * side + (columns + 1.0) * margin, rows as f64 * high + (rows as f64 + 1.0) * margin);
        (Size { width: side, height: high }, margin, size)
    };
    let mut solved = laid_out(tile_side(wanted, rows, shape).floor().max(1.0));
    while solved.2.0 * solved.2.1 > MAX_PIXELS && solved.0.width > 1.0 {
        solved = laid_out(solved.0.width - 1.0);
    }
    let (tile, margin, (width, height)) = solved;
    let tiles: Vec<Tile> = pictures
        .iter()
        .enumerate()
        .map(|(index, (path, size))| {
            let column = index as i64 % COLUMNS;
            // Row 0 at the top, Core Image's origin at the bottom: the gallery's order kept in the file.
            let row = rows - 1 - index as i64 / COLUMNS;
            let cell = Rect::new(
                margin + column as f64 * (tile.width + margin),
                margin + row as f64 * (tile.height + margin),
                tile.width,
                tile.height,
            );
            Tile { path: path.clone(), cell, frame: fitted(*size, &cell) }
        })
        .collect();
    let i = MEASURE_WINDOW_INSET;
    let untouched = Rect::new(i, i, 1.0 - 2.0 * i, 1.0 - 2.0 * i);
    let reads = tiles
        .iter()
        .enumerate()
        .map(|(index, t)| {
            let own = windows.get(index).copied().flatten().unwrap_or(untouched);
            let f = &t.frame;
            Rect::new((f.x + own.x * f.width) / width, (f.y + own.y * f.height) / height, own.width * f.width / width, own.height * f.height / height)
        })
        .collect();
    let grid = ContactSheetGrid { columns: COLUMNS, rows, frames: pictures.len() as i64, tile, margin, windows: Some(reads) };
    Some(Layout { tiles, width: width as usize, height: height as usize, lead: lead.to_string(), grid })
}

/// A frame's pixels, rows top first, premultiplied.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

/// The sheet's pixels, rows top first, on a clear ground: each picture scaled into its frame and
/// sampled bilinearly, a pixel taken where its centre falls inside the frame.
pub fn composed(layout: &Layout, pictures: &[Picture]) -> Vec<[f32; 4]> {
    let (w, h) = (layout.width, layout.height);
    let mut out = vec![[0.0f32; 4]; w * h];
    for (tile, picture) in layout.tiles.iter().zip(pictures) {
        let (pw, ph) = (picture.width, picture.height);
        if pw == 0 || ph == 0 {
            continue;
        }
        let f = &tile.frame;
        let scale = f.width / pw as f64;
        let texel = |x: i64, y: i64| picture.pixels[y.clamp(0, ph as i64 - 1) as usize * pw + x.clamp(0, pw as i64 - 1) as usize];
        let (x0, x1) = ((f.x - 0.5).ceil().max(0.0) as usize, ((f.x + f.width - 0.5).ceil().max(0.0) as usize).min(w));
        let (y0, y1) = ((f.y - 0.5).ceil().max(0.0) as usize, ((f.y + f.height - 0.5).ceil().max(0.0) as usize).min(h));
        for up in y0..y1 {
            // The picture's own row, counted from its top, at this pixel's centre.
            let v = ph as f64 - (up as f64 + 0.5 - f.y) / scale - 0.5;
            let (vy, fy) = (v.floor(), (v - v.floor()) as f32);
            let row = h - 1 - up;
            for x in x0..x1 {
                let u = (x as f64 + 0.5 - f.x) / scale - 0.5;
                let (ux, fx) = (u.floor(), (u - u.floor()) as f32);
                let (ux, vy) = (ux as i64, vy as i64);
                let (a, b, c, d) = (texel(ux, vy), texel(ux + 1, vy), texel(ux, vy + 1), texel(ux + 1, vy + 1));
                let px = &mut out[row * w + x];
                for k in 0..4 {
                    let top = a[k] * (1.0 - fx) + b[k] * fx;
                    let low = c[k] * (1.0 - fx) + d[k] * fx;
                    px[k] = top * (1.0 - fy) + low * fy;
                }
            }
        }
    }
    out
}

/// `ContactSheet.tile`: the head of the chain on a frame, `Pipeline.framed` alone, reduced to `side`.
pub fn tile(r: &mut Reader, copy: Copy, s: &PipelineSettings, side: f64) -> Option<Picture> {
    let e = copy.source.extent();
    let full = copy.full_width.unwrap_or(f64::from(e.w));
    let scale = if full > 0.0 { f64::from(e.w) / full } else { 1.0 };
    let mut framed = r.framed(copy.source, s, scale)?;
    let f = framed.extent();
    let longest = f64::from(f.w.max(f.h));
    if longest > side {
        framed = framed.lanczos(side / longest);
    }
    let f = framed.extent();
    if f.w == 0 || f.h == 0 {
        return None;
    }
    let pixels = r.graph.render(&framed, f, None).ok()?;
    Some(Picture { width: f.w as usize, height: f.h as usize, pixels })
}

/// `ContactSheet.settings(inheriting:)`: the roll's window and Roll Settings and its own grid, a
/// slide roll's measured gains its rest; the flat-field stays off, each frame carrying its own.
pub fn settings(donor: &PipelineSettings, grid: ContactSheetGrid, film: Option<Film>, mosaic: bool) -> PipelineSettings {
    let mut sheet = PipelineSettings::rest(donor.mode, mosaic);
    sheet.film = film;
    sheet.sheet = Some(grid);
    let every: BTreeSet<Field> = Field::ALL.into_iter().collect();
    let mut sheet = roll_settings::copied(&every, donor, &sheet);
    sheet.roll_reference = donor.roll_reference.clone();
    sheet.measured_gains = donor.measured_gains;
    sheet
}

/// `CharacterSet.whitespaces`: spaces and tabs, never a line's end.
fn spaces(c: char) -> bool {
    c == '\t' || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

/// `ContactSheet.name(forRoll:on:)`: the roll's name, else the day `day` spells.
pub fn name(film: Option<&Film>, day: &str) -> String {
    match film.map(|f| f.name.trim_matches(spaces)).filter(|n| !n.is_empty()) {
        Some(n) => format!("{n} — contact sheet"),
        None => format!("{day} — contact sheet"),
    }
}

/// `ContactSheet.safe`: a name that cannot climb out of its folder nor hide an extension, and that
/// every system can hold.
pub fn safe(stem: &str) -> String {
    let cleaned = oni_library::file_names::portable(&stem.replace('.', "-"));
    let cleaned = cleaned.trim_matches(spaces);
    if cleaned.is_empty() { "contact sheet".into() } else { cleaned.into() }
}

/// `ContactSheet.destination(besides:named:)`: beside the lead, numbered past every name taken.
pub fn destination(lead: &Path, stem: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let folder = lead.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = safe(stem);
    let mut candidate = folder.join(format!("{stem}.tif"));
    let mut number = 2;
    while exists(&candidate) {
        candidate = folder.join(format!("{stem} {number}.tif"));
        number += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(w: usize, h: usize, value: f32) -> Picture {
        Picture { width: w, height: h, pixels: vec![[value, 0.5, 0.25, 1.0]; w * h] }
    }

    fn eight() -> (Vec<(String, (usize, usize))>, Vec<Picture>) {
        let shapes = [(90, 60), (60, 90), (60, 60), (120, 40), (90, 60), (60, 90), (60, 60), (120, 40)];
        let made = shapes.iter().enumerate().map(|(i, s)| (format!("f{i}"), *s)).collect();
        let pictures = shapes.iter().enumerate().map(|(i, s)| plate(s.0, s.1, (i + 1) as f32 / 10.0)).collect();
        (made, pictures)
    }

    fn at(layout: &Layout, pixels: &[[f32; 4]], x: f64, y: f64) -> [f32; 4] {
        let row = layout.height - 1 - y as usize;
        pixels[row * layout.width + x as usize]
    }

    /// `ContactSheet.selfCheck`'s grid: six to a row, one gap everywhere, fitted, the gutter clear.
    #[test]
    fn the_sheet_lays_six_to_a_row_fitted_on_one_gap_with_nothing_painted_between() {
        let (made, pictures) = eight();
        let sheet = laid(&made, "f0", 60.0, &[]).unwrap();
        assert_eq!(sheet.tiles.iter().map(|t| t.path.clone()).collect::<Vec<_>>(), made.iter().map(|m| m.0.clone()).collect::<Vec<_>>());
        let top: Vec<_> = sheet.tiles.iter().filter(|t| t.cell.y == sheet.tiles[0].cell.y).collect();
        assert_eq!(top.len(), 6, "six frames to a row");
        assert!(sheet.tiles[6].cell.y < sheet.tiles[0].cell.y, "the seventh opens a second row, under the first");
        let cell = sheet.tiles[0].cell;
        let edge = sheet.tiles.iter().map(|t| t.cell.y).fold(f64::MAX, f64::min);
        assert!((sheet.height as f64 - (2.0 * cell.height + 3.0 * edge)).abs() < 2.0, "as tall as its rows need");
        let band_v = sheet.tiles[0].cell.y - (sheet.tiles[6].cell.y + sheet.tiles[6].cell.height);
        let band_h = sheet.tiles[1].cell.x - (sheet.tiles[0].cell.x + sheet.tiles[0].cell.width);
        assert!((band_v - band_h).abs() < 1e-6, "{band_v} vertical against {band_h}");
        let left = sheet.tiles.iter().map(|t| t.cell.x).fold(f64::MAX, f64::min);
        let right = sheet.width as f64 - sheet.tiles.iter().map(|t| t.cell.x + t.cell.width).fold(0.0, f64::max);
        let above = sheet.height as f64 - sheet.tiles.iter().map(|t| t.cell.y + t.cell.height).fold(0.0, f64::max);
        assert!((left - right).abs() < 1e-6 && (above - left).abs() < 1e-6, "{left} {right} {above}");
        assert!((sheet.width as f64 - (6.0 * cell.width + 7.0 * left)).abs() < 1.0, "the sheet is exactly its grid");
        assert!((band_h - left).abs() < 1e-6, "the gutter is the margin");

        let drift = made.iter().zip(&sheet.tiles).map(|(m, t)| (t.frame.width / t.frame.height - m.1.0 as f64 / m.1.1 as f64).abs()).fold(0.0, f64::max);
        assert!(drift < 1e-6, "every picture keeps its own aspect: {drift:e}");
        // Twin: filling would square all eight, and six are not square.
        assert_eq!(sheet.tiles.iter().filter(|t| (t.frame.width - t.frame.height).abs() > 1e-6).count(), 6);
        assert!(sheet.tiles.iter().all(|t| crate::window::contains(&crate::window::inset(&t.cell, -1e-9, -1e-9), &t.frame)));

        let pixels = composed(&sheet, &pictures);
        for (i, t) in sheet.tiles.iter().enumerate() {
            let centre = at(&sheet, &pixels, (t.frame.x + t.frame.width / 2.0).round(), (t.frame.y + t.frame.height / 2.0).round());
            assert!((centre[0] - (i + 1) as f32 / 10.0).abs() < 1e-4, "frame {i} reads {centre:?}");
        }
        let gutter = at(&sheet, &pixels, (cell.x + cell.width).round(), (cell.y + cell.height / 2.0).round());
        assert!(gutter[3] < 1e-4, "nothing is painted in the gutter: {gutter:?}");
        // Twin: a pixel inside a frame is opaque, so the gutter's reading is the gutter's.
        let inside = at(&sheet, &pixels, (cell.x + cell.width / 2.0).round(), (cell.y + cell.height / 2.0).round());
        assert!(inside[3] > 0.999);
        assert!(sheet.tiles[0].cell.y > sheet.tiles.last().unwrap().cell.y, "the first frame sits above the last");
        let first = at(&sheet, &pixels, (sheet.tiles[0].frame.x + 5.0).round(), (sheet.tiles[0].frame.y + 5.0).round());
        let row_top = sheet.height - 1 - (sheet.tiles[0].frame.y + 5.0).round() as usize;
        assert!(row_top < sheet.height / 2 && (first[0] - 0.1).abs() < 1e-4, "the first frame is in the file's top half");
    }

    /// `ContactSheet.selfCheck`'s cap: a roll lands on the ceiling, two hundred stay under it.
    #[test]
    fn the_cap_holds_and_nothing_is_dropped_to_hold_it() {
        let roll: Vec<_> = (0..ROLL_FRAMES).map(|i| (format!("r{i}"), (90, 60))).collect();
        let full = laid(&roll, "r0", roll_tile(), &[]).unwrap();
        let pixels = (full.width * full.height) as f64;
        let tile = full.tiles[0].cell.width;
        assert!(pixels > MAX_PIXELS * 0.95 && pixels <= MAX_PIXELS, "{:.1} Mpx at {tile} px a tile", pixels / 1e6);
        // Twin, found rather than tabulated: the smallest tile over the laid one breaches.
        let at = |t: f64| {
            let (r, m) = (rows(ROLL_FRAMES) as f64, (t * GAP).round().max(1.0));
            (6.0 * t + 7.0 * m) * (r * (t / 1.5).floor() + (r + 1.0) * m)
        };
        assert!((1..=64).any(|s| at(tile + f64::from(s)) > MAX_PIXELS));
        let many: Vec<_> = (0..200).map(|i| (format!("p{i}"), (90, 60))).collect();
        let big = laid(&many, "p0", roll_tile(), &[]).unwrap();
        assert!((big.width * big.height) as f64 <= MAX_PIXELS && big.tiles.len() == 200);
        let (made, _) = eight();
        assert_eq!(laid(&made, "f0", 60.0, &[]).unwrap().tiles[0].cell.width, 60.0, "a short sheet keeps the tile it asked for");
        assert!(laid(&[], "", 60.0, &[]).is_none());
    }

    /// Each frame is read through its own window laid where the frame was, the inset one by default.
    #[test]
    fn each_frame_is_read_through_its_own_window_where_it_was_laid() {
        let made = vec![("held".to_string(), (300, 200))];
        let clear = Rect::new(0.5, 0.0, 0.5, 1.0);
        let windowed = laid(&made, "held", 300.0, &[Some(clear)]).unwrap();
        let blind = laid(&made, "held", 300.0, &[]).unwrap();
        let f = windowed.tiles[0].frame;
        let (w, h) = (windowed.width as f64, windowed.height as f64);
        let read = windowed.grid.windows.as_ref().unwrap()[0];
        assert!((read.x - (f.x + 0.5 * f.width) / w).abs() < 1e-12 && (read.height - f.height / h).abs() < 1e-12, "{read:?}");
        // Twin: no window of its own reads the frame inset by a tenth.
        let inset = blind.grid.windows.as_ref().unwrap()[0];
        assert!((inset.x - (f.x + 0.1 * f.width) / w).abs() < 1e-12 && inset != read);
        let reads = crate::window::sheet_reads(&windowed.grid, &Rect::new(0.0, 0.0, w, h));
        assert_eq!(reads.len(), 1);
        assert!(reads[0].x >= f.x + 0.5 * f.width - 1.0 && reads[0].x + reads[0].width <= f.x + f.width + 1e-9);
    }

    /// `ContactSheet.selfCheck`'s inheritance: the roll's reference, gains, levels, ceiling, mode and
    /// grid, never its flat-field; the same copy onto a photograph lays the mask.
    #[test]
    fn a_sheet_stands_on_its_rolls_window_its_frames_flat_field_never_laid_twice() {
        let mut donor = PipelineSettings::default();
        donor.mode = oni_settings::ConversionMode::Positive;
        donor.gains.stops = [0.4, 0.0, -0.3];
        donor.measured_gains = Some([0.4, 0.0, -0.3]);
        donor.levels.red.white = 2.2;
        donor.density_ceiling = 2.4;
        donor.flat_field = oni_settings::frame::FlatField { mask: oni_settings::frame::FlatFieldMask::Custom, custom: Some("3f9a0c7d51e2b846".into()), amount: 1.0, ..Default::default() };
        donor.roll_reference = Some(oni_settings::frame::RollReference { id: oni_settings::Id::new(), stops: [0.0; 3], black: [0.2; 3], white: [0.6; 3], base: None, median: None });
        let grid = ContactSheetGrid { columns: 6, rows: 1, frames: 4, tile: Size { width: 300.0, height: 200.0 }, margin: 10.0, windows: None };
        let s = settings(&donor, grid.clone(), None, false);
        assert!(s.roll_reference == donor.roll_reference && s.gains == donor.gains && s.levels == donor.levels);
        assert_eq!(s.measured_gains, donor.measured_gains, "a slide sheet rests on its roll's measured gains");
        assert!(s.density_ceiling == donor.density_ceiling && s.mode == donor.mode && s.sheet == Some(grid));
        assert_eq!(s.flat_field, oni_settings::frame::FlatField::default());
        let photo = roll_settings::copied(&BTreeSet::from([Field::FlatField]), &donor, &PipelineSettings::default());
        assert_eq!(photo.flat_field, donor.flat_field, "twin: onto a photograph the same copy lays the mask");
    }

    /// `ContactSheet.selfCheck`'s naming: the roll's name, else its day; numbered, never climbing out.
    #[test]
    fn a_sheet_is_named_by_its_roll_or_day_and_lands_in_its_folder_numbered() {
        let portra = Film::new("Portra 400");
        assert_eq!(name(Some(&portra), "2026-05-07"), "Portra 400 — contact sheet");
        assert_eq!(name(None, "2026-05-07"), "2026-05-07 — contact sheet");
        assert_eq!(name(Some(&Film::new("   ")), "2026-05-07"), name(None, "2026-05-07"), "a blank name falls back on the day");
        let lead = Path::new("/rolls/one/sheet.tif");
        let taken = |p: &Path| p == Path::new("/rolls/one/sheet.tif");
        assert_eq!(destination(lead, "sheet", taken), PathBuf::from("/rolls/one/sheet 2.tif"));
        let hostile = destination(lead, "../../etc/passwd", |_| false);
        assert_eq!(hostile.parent(), Some(Path::new("/rolls/one")), "{hostile:?}");
        assert_eq!(hostile.file_name().unwrap(), "------etc-passwd.tif");
        // Twin: unsanitised, the same name leaves the folder.
        let raw = Path::new("/rolls/one").join("../../etc/passwd.tif");
        assert_ne!(raw.components().filter(|c| matches!(c, std::path::Component::ParentDir)).count(), 0);
        assert_eq!(safe(" . "), "-");
        assert_eq!(safe("   "), "contact sheet");
        // A roll name Windows refuses: quotes, a backslash, a device name.
        assert_eq!(safe("Portra \"pushed\" — contact sheet"), "Portra -pushed- — contact sheet");
        assert_eq!(safe("..\\x"), "---x");
        assert_eq!(safe("CON"), "CON-");
    }
}
