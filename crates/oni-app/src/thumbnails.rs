//! `Thumbnails`: one JPEG per photo in a durable support folder, named by its fingerprint (`key`), and
//! the memory over it. A tile reads that folder; a render runs only in the pass or after an edit.

use crate::display::Screen;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `Thumbnails.side`: the long side in pixels, the only size produced or held.
pub const SIDE: usize = 320;
/// `Thumbnails.renderVersion`, out of every name: a thumbnail is written once, and only an edit or
/// the pass redraws it.
pub const RENDER_VERSION: u32 = 11;
/// `Thumbnails.refreshDelay`: longer than the sidecar's write delay, or the redraw races the write.
pub const REFRESH_DELAY: Duration = Duration::from_millis(2000);
/// `Thumbnails.compression`: not told from 0.8 at a tile's size, and 37 % lighter.
pub const COMPRESSION: f64 = 0.6;
/// Tiles held in memory: 273 KB each at 320 x 213, so about 270 MB, which an 8 GB Mac can spare;
/// one past it is read again from its JPEG.
pub const MEMORY_LIMIT: usize = 1000;
/// `Library.waitingMark`'s photos are not read yet and have no file.
pub const PENDING: &str = oni_library::library::WAITING_MARK;

/// `Thumbnails.directory`: durable, not the purgeable caches, or a large library redraws for hours;
/// kept off a Windows profile that roams.
pub fn directory(support: &Path) -> PathBuf {
    crate::system::local_support(support).join("Thumbnails")
}

/// `fileName(fingerprint:)`, the fingerprint and nothing else, in a name every file system takes: a
/// byte outside letters, digits, `-`, `.` and the space is written `_` and its hex, an EXIF date's `:` too.
pub fn file_name(fingerprint: &str) -> String {
    let mut name = String::with_capacity(fingerprint.len() + 4);
    for byte in fingerprint.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b' ' => name.push(char::from(byte)),
            _ => name.push_str(&format!("_{byte:02x}")),
        }
    }
    name + ".jpg"
}

/// Swift's name, colons kept: Swift writes it and an earlier build did, so it is still read where
/// the system allows it.
fn swift_name(fingerprint: &str) -> String {
    format!("{}.jpg", fingerprint.replace('/', "_"))
}

/// The name a tile is kept and shown under: the fingerprint, or for a print two rows hold (a copy,
/// a backup) the print and the path, so an edit on one copy never shows on the other's tile.
pub fn key(fingerprint: &str, path: &str, copies: &HashSet<String>) -> String {
    if !copies.contains(fingerprint) {
        return fingerprint.to_owned();
    }
    let hash = oni_library::paths::normal(path).bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("{fingerprint}~{hash:016x}")
}

/// The files a row keeps from the sweep: its print's, Swift's name for it, and its own key's.
pub fn claims(fingerprint: &str, key: &str) -> [String; 3] {
    [file_name(fingerprint), swift_name(fingerprint), file_name(key)]
}

/// Where a thumbnail is written.
pub fn file(support: &Path, fingerprint: &str) -> PathBuf {
    directory(support).join(file_name(fingerprint))
}

/// The file a tile reads: the newer of the portable name and Swift's, Windows having only the first.
fn held(support: &Path, fingerprint: &str) -> Option<PathBuf> {
    let portable = file(support, fingerprint);
    let swift = (!cfg!(windows)).then(|| directory(support).join(swift_name(fingerprint))).filter(|s| *s != portable);
    let modified = |p: &Path| std::fs::metadata(p).ok().map(|m| m.modified().unwrap_or(std::time::UNIX_EPOCH));
    match (modified(&portable), swift.as_deref().and_then(|s| modified(s).map(|t| (s, t)))) {
        (Some(mine), Some((s, theirs))) if theirs > mine => Some(s.to_path_buf()),
        (Some(_), _) => Some(portable),
        (None, Some((s, _))) => Some(s.to_path_buf()),
        (None, None) => None,
    }
}

pub fn is_held(support: &Path, fingerprint: &str) -> bool {
    held(support, fingerprint).is_some()
}

/// Both names gone, so neither app shows a tile older than the photo's settings.
pub fn forget(support: &Path, fingerprint: &str) {
    let _ = std::fs::remove_file(file(support, fingerprint));
    if !cfg!(windows) {
        let _ = std::fs::remove_file(directory(support).join(swift_name(fingerprint)));
    }
}

/// The sRGB JPEG of an encoded frame at `COMPRESSION`, alpha dropped.
pub fn jpeg(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let (w, h) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    if width == 0 || height == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    let mut out = Vec::new();
    let quality = (COMPRESSION * 100.0).round() as u8;
    jpeg_encoder::Encoder::new(&mut out, quality).encode(rgba, w, h, jpeg_encoder::ColorType::Rgba).ok()?;
    Some(out)
}

/// Written whole through a sibling of its own (`write_atomically`), so a tile never reads half a
/// file and a full disk leaves nothing behind.
pub fn persist(support: &Path, fingerprint: &str, data: &[u8]) -> std::io::Result<()> {
    let folder = directory(support);
    std::fs::create_dir_all(&folder)?;
    oni_library::write_atomically(&folder.join(file_name(fingerprint)), data)?;
    // Swift's copy is older than this one now: gone, so Swift draws the photo afresh.
    if !cfg!(windows) && swift_name(fingerprint) != file_name(fingerprint) {
        let _ = std::fs::remove_file(folder.join(swift_name(fingerprint)));
    }
    Ok(())
}

/// A held file decoded to RGBA for `screen`, its sRGB carried to the screen's primaries once here
/// so the grid never matches a tile at each frame.
pub fn load(support: &Path, fingerprint: &str, screen: Screen) -> Option<(Vec<u8>, usize, usize)> {
    let data = std::fs::read(held(support, fingerprint)?).ok()?;
    decoded(&data, screen)
}

pub fn decoded(data: &[u8], screen: Screen) -> Option<(Vec<u8>, usize, usize)> {
    let mut decoder = jpeg_decoder::Decoder::new(data);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (w, h) = (usize::from(info.width), usize::from(info.height));
    let rgba: Vec<u8> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        _ => return None,
    };
    Some((to_screen(rgba, screen), w, h))
}

fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.0;
    if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn encoded(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let e = if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (e * 255.0).round() as u8
}

/// sRGB bytes on the screen's primaries; Display P3 shares sRGB's transfer and white.
pub fn to_screen(mut rgba: Vec<u8>, screen: Screen) -> Vec<u8> {
    if screen == Screen::Srgb {
        return rgba;
    }
    // Linear sRGB to linear Display P3, both on D65.
    const M: [[f32; 3]; 3] = [[0.822_462, 0.177_538, 0.0], [0.033_194, 0.966_806, 0.0], [0.017_083, 0.072_397, 0.910_520]];
    let table: Vec<f32> = (0..=255u8).map(linear).collect();
    for p in rgba.chunks_exact_mut(4) {
        let l = [table[usize::from(p[0])], table[usize::from(p[1])], table[usize::from(p[2])]];
        for (c, row) in M.iter().enumerate() {
            p[c] = encoded(row[0] * l[0] + row[1] * l[1] + row[2] * l[2]);
        }
    }
    rgba
}

/// The tiles in memory by photo path, the least recently shown half dropped past `MEMORY_LIMIT`,
/// never what just arrived, which a scroll would fetch again.
pub struct Memory<T> {
    held: HashMap<String, (T, Cell<u64>)>,
    clock: Cell<u64>,
    limit: usize,
}

impl<T: Clone> Memory<T> {
    pub fn new(limit: usize) -> Memory<T> {
        Memory { held: HashMap::new(), clock: Cell::new(0), limit }
    }

    fn tick(&self) -> u64 {
        self.clock.set(self.clock.get() + 1);
        self.clock.get()
    }

    /// The tile, its use noted so a tile on screen outlives one scrolled past.
    pub fn get(&self, path: &str) -> Option<T> {
        let (tile, used) = self.held.get(path)?;
        used.set(self.tick());
        Some(tile.clone())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.held.contains_key(path)
    }

    pub fn len(&self) -> usize {
        self.held.len()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    pub fn insert(&mut self, path: String, tile: T) {
        let used = Cell::new(self.tick());
        self.held.insert(path, (tile, used));
        if self.held.len() <= self.limit {
            return;
        }
        let mut uses: Vec<(u64, String)> = self.held.iter().map(|(p, (_, u))| (u.get(), p.clone())).collect();
        uses.sort_unstable();
        let stale = self.held.len() - self.limit / 2;
        for (_, old) in uses.into_iter().take(stale) {
            self.held.remove(&old);
        }
    }

    pub fn remove(&mut self, path: &str) {
        self.held.remove(path);
    }

    pub fn clear(&mut self) {
        self.held.clear();
    }
}

mod sweep;
pub use sweep::{FRESH, SWEPT, sweep};

#[cfg(test)]
mod criterion;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnails_file_is_its_photos_fingerprint_and_nothing_else() {
        // `Thumbnails.selfCheck`: a date, a size or the version in the name would redraw a library on every edit.
        let name = file_name("4096-2026:06:18 14:32:10");
        assert_eq!(name, "4096-2026_3a06_3a18 14_3a32_3a10.jpg");
        assert_ne!(file_name("9999-2026:06:18 15:00:00"), name);
        assert!(!name.contains(&RENDER_VERSION.to_string()) && !name.contains(&SIDE.to_string()));
        // Twin: a separator in a fingerprint never reaches the name, which would open a folder.
        assert!(!file_name("a/b").trim_end_matches(".jpg").contains('/'));
        // Two fingerprints never share a name: the escape's own `_` is escaped.
        assert_ne!(file_name("a:b"), file_name("a_3ab"));
    }

    #[test]
    fn two_copies_of_one_file_keep_a_tile_each() {
        let copies = HashSet::from(["4096-no-date-9f".to_owned()]);
        let (mac, backup) = (key("4096-no-date-9f", "/Users/a/Roll 8/A.tif", &copies), key("4096-no-date-9f", "/Volumes/T7/Roll 8/A.tif", &copies));
        assert!(mac != backup && mac.starts_with("4096-no-date-9f~"), "{mac} {backup}");
        assert_eq!(key("4096-no-date-9f", "/Users/a/Roll 8/A.tif", &copies), mac, "a copy keeps its name");
        // Twin: a print no other row holds keeps Swift's name, the print alone.
        assert_eq!(key("2048-no-date-1a", "/Users/a/Roll 8/B.tif", &copies), "2048-no-date-1a");
    }

    /// Windows refuses `:` in a name (NTFS reads a stream after it), so a dated photo had no tile there.
    #[test]
    fn a_dated_fingerprint_persists_on_every_platform() {
        let support = std::env::temp_dir().join(format!("oni-app-dated-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
        let dated = "25331200-2026:06:18 14:32:10-9f3a";
        let rgba = vec![200u8; 16 * 16 * 4];
        let data = jpeg(&rgba, 16, 16).unwrap();
        persist(&support, dated, &data).expect("a dated fingerprint persists");
        assert!(is_held(&support, dated) && load(&support, dated, Screen::Srgb).is_some());
        let name = file_name(dated);
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-. _".contains(&b)), "{name}");
        // Twin: the name Swift writes keeps the colons, which every Windows volume refuses.
        assert!(swift_name(dated).contains(':'));
        if !cfg!(windows) {
            // A tile Swift or an earlier build left reads on, and the next persist takes its place.
            let older = "25331200-2025:01:02 03:04:05-77";
            std::fs::write(directory(&support).join(swift_name(older)), &data).unwrap();
            assert!(load(&support, older, Screen::Srgb).is_some(), "Swift's tile still shows");
            assert!(!is_held(&support, "25331200-2025:01:02 03:04:06-77"), "another photo's is not taken");
            persist(&support, older, &data).unwrap();
            assert!(!directory(&support).join(swift_name(older)).exists() && file(&support, older).exists());
            forget(&support, older);
            assert!(!is_held(&support, older));
        }
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn the_redraw_waits_past_the_sidecars_write() {
        // `DSGesture.selfCheck`: a redraw sooner than the write renders settings the disk does not hold yet.
        assert!(REFRESH_DELAY > crate::app::WRITE_DELAY, "{REFRESH_DELAY:?} against {:?}", crate::app::WRITE_DELAY);
        assert_eq!((SIDE, REFRESH_DELAY, crate::app::WRITE_DELAY), (320, Duration::from_millis(2000), Duration::from_millis(400)));
        // Twin: a refresh shorter than the write is refused by the same comparison.
        assert!(!(Duration::from_millis(300) > crate::app::WRITE_DELAY));
    }

    #[test]
    fn a_tile_written_reads_back_and_a_missing_one_reads_nothing() {
        let support = std::env::temp_dir().join(format!("oni-app-thumbs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
        let (w, h) = (SIDE, 213);
        // A smooth ramp, as a photograph is: hard edges spread across channels by chroma subsampling.
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i % w * 255 / w) as u8, 128, 64, 255]).collect();
        let data = jpeg(&rgba, w, h).unwrap();
        persist(&support, "123-no-date-abc", &data).unwrap();
        assert!(is_held(&support, "123-no-date-abc"));
        let (back, bw, bh) = load(&support, "123-no-date-abc", Screen::Srgb).unwrap();
        assert_eq!((bw, bh, back.len()), (w, h, w * h * 4));
        let error = back.chunks_exact(4).zip(rgba.chunks_exact(4)).map(|(a, b)| (i32::from(a[1]) - i32::from(b[1])).abs()).max().unwrap();
        assert!(error <= 12, "a flat channel survives the JPEG within {error}");
        // Twin: another fingerprint finds no file, so a tile waits for the pass instead of showing another photo.
        assert!(load(&support, "124-no-date-abc", Screen::Srgb).is_none());
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn srgb_white_stays_white_and_a_pure_red_moves_inside_display_p3() {
        let white = to_screen(vec![255, 255, 255, 255], Screen::DisplayP3);
        assert!(white[..3].iter().all(|&v| v >= 254), "{white:?}");
        let red = to_screen(vec![255, 0, 0, 255], Screen::DisplayP3);
        assert!(red[0] < 250 && red[1] > 40, "{red:?}");
        // Twin: on an sRGB screen the bytes are left as the file holds them.
        assert_eq!(to_screen(vec![255, 0, 0, 255], Screen::Srgb), vec![255, 0, 0, 255]);
    }

    #[test]
    fn memory_drops_its_oldest_half_and_never_the_newest() {
        let mut m = Memory::new(4);
        for i in 0..5 {
            m.insert(format!("p{i}"), i);
        }
        assert_eq!(m.len(), 2, "past the ceiling the oldest go down to half");
        assert!(m.contains("p4") && m.contains("p3"));
        // A tile shown again outlives newer ones scrolled past.
        let mut r = Memory::new(4);
        for i in 0..4 {
            r.insert(format!("p{i}"), i);
        }
        r.get("p0");
        r.insert("p4".to_string(), 4);
        assert!(r.contains("p0") && r.contains("p4") && !r.contains("p1"), "the shown tile is kept");
        // Twin: a re-insert does not count twice towards the ceiling.
        let mut n = Memory::new(4);
        for _ in 0..10 {
            n.insert("same".to_string(), 1);
        }
        assert_eq!(n.len(), 1);
    }
}
