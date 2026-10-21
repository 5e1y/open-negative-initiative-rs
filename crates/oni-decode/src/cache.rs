//! Full decodes kept on disk: the camera's 16-bit window and the matrix that turns it into floats, so
//! a frame opened again skips LibRaw's demosaic (0.9 s a 24 Mpx frame) for a read and the conversion.

use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

/// Where decodes are kept and the bytes they may take; none keeps nothing.
static PLACE: RwLock<Option<(PathBuf, u64)>> = RwLock::new(None);

const MAGIC: &[u8; 8] = b"ONIDEC1\0";

/// The decoder's own code, so a change to how a file is read never serves an older decode.
static CODE: LazyLock<u64> = LazyLock::new(|| {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for part in [include_str!("../csrc/pinned.c"), include_str!("lib.rs"), include_str!("camera.rs"), include_str!("rendition.rs")] {
        part.hash(&mut h);
    }
    h.finish()
});

/// Keeps full decodes in `dir`, the oldest let go past `cap` bytes.
pub fn keep_in(dir: PathBuf, cap: u64) {
    if let Ok(mut p) = PLACE.write() {
        *p = Some((dir, cap));
    }
}

/// A decode as kept: its window's pixels, three channels a pixel, and how they are turned and converted.
pub(crate) struct Kept {
    pub width: usize,
    pub height: usize,
    pub flip: i32,
    pub factor: usize,
    pub matrix: [[f32; 3]; 3],
    pub raw: Vec<u16>,
}

/// The file's identity and the reading asked of it; `None` when the file cannot be looked at.
pub(crate) fn key(path: &Path, reading: &str) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    let at = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some(format!("{:016x}|{}|{}|{at}|{reading}", *CODE, path.display(), m.len()))
}

fn file(dir: &Path, key: &str) -> PathBuf {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    dir.join(format!("{:016x}.decode", h.finish()))
}

/// The decode kept under `key`, its use noted so the oldest go first.
pub(crate) fn read(key: &str) -> Option<Kept> {
    let (dir, _) = PLACE.read().ok()?.clone()?;
    read_in(&dir, key)
}

fn read_in(dir: &Path, key: &str) -> Option<Kept> {
    let at = file(dir, key);
    let mut f = std::fs::File::open(&at).ok()?;
    let mut head = [0u8; 8];
    f.read_exact(&mut head).ok()?;
    if &head != MAGIC {
        return None;
    }
    let mut word = [0u8; 4];
    let mut next = |f: &mut std::fs::File| f.read_exact(&mut word).ok().map(|_| u32::from_le_bytes(word));
    let length = next(&mut f)? as usize;
    let mut kept_key = vec![0u8; length];
    f.read_exact(&mut kept_key).ok()?;
    if kept_key != key.as_bytes() {
        return None;
    }
    let (width, height, flip, factor) = (next(&mut f)? as usize, next(&mut f)? as usize, next(&mut f)? as i32, next(&mut f)? as usize);
    let mut matrix = [[0.0f32; 3]; 3];
    for v in matrix.iter_mut().flatten() {
        *v = f32::from_bits(next(&mut f)?);
    }
    // Read a row at a time into the samples, so no second copy of the window is ever held.
    let mut raw = Vec::with_capacity(width * height * 3);
    let mut row = vec![0u8; width * 6];
    for _ in 0..height {
        f.read_exact(&mut row).ok()?;
        raw.extend(row.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])));
    }
    let _ = f.set_modified(std::time::SystemTime::now());
    Some(Kept { width, height, flip, factor, matrix, raw })
}

/// Writes a decode's `rows` under `key` straight from the decoder's buffer, then lets the oldest
/// decodes go past the cap; a copy for a writer thread would hold another 145 MB.
pub(crate) fn write<'a>(key: &str, head: &Kept, rows: impl Iterator<Item = &'a [u16]>) {
    let Some((dir, cap)) = PLACE.read().ok().and_then(|p| p.clone()) else { return };
    store(&dir, key, head, rows);
    trim(&dir, cap);
}

/// The decode written whole under a name of its own, then renamed, so a reader never meets half a file;
/// `head`'s samples are not read, `rows` are.
fn store<'a>(dir: &Path, key: &str, kept: &Kept, rows: impl Iterator<Item = &'a [u16]>) {
    let _ = std::fs::create_dir_all(dir);
    let at = file(dir, key);
    let part = at.with_extension("part");
    let written = (|| -> std::io::Result<()> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&part)?);
        out.write_all(MAGIC)?;
        out.write_all(&(key.len() as u32).to_le_bytes())?;
        out.write_all(key.as_bytes())?;
        for v in [kept.width as u32, kept.height as u32, kept.flip as u32, kept.factor as u32] {
            out.write_all(&v.to_le_bytes())?;
        }
        for v in kept.matrix.iter().flatten() {
            out.write_all(&v.to_bits().to_le_bytes())?;
        }
        let mut bytes = Vec::with_capacity(kept.width * 6);
        for row in rows {
            bytes.clear();
            bytes.extend(row.iter().flat_map(|v| v.to_le_bytes()));
            out.write_all(&bytes)?;
        }
        out.flush()?;
        std::fs::rename(&part, &at)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
}

/// The oldest decodes let go until the folder holds `cap` bytes at most.
fn trim(dir: &Path, cap: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut kept: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "decode"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    kept.sort_by_key(|k| std::cmp::Reverse(k.0));
    let mut total = 0;
    for (_, len, decode) in kept {
        total += len;
        if total > cap {
            let _ = std::fs::remove_file(decode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oni-decode-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_decode_kept_reads_back_bit_for_bit_and_only_under_its_own_key() {
        let dir = temporary("round");
        let raw: Vec<u16> = (0..4 * 3 * 3).map(|i| (i * 2731 % 65536) as u16).collect();
        let matrix = [[1.25, -0.5, 0.125], [0.0, 1.0, 0.0], [-0.0625, 0.25, 0.8125]];
        let kept = Kept { width: 4, height: 3, flip: 6, factor: 1, matrix, raw: Vec::new() };
        store(&dir, "frame|a", &kept, raw.chunks(12));
        let back = read_in(&dir, "frame|a").expect("kept");
        assert_eq!((back.width, back.height, back.flip, back.factor, back.matrix), (4, 3, 6, 1, matrix));
        assert_eq!(back.raw, raw);
        // Twin: another reading of the same file finds nothing, and a file whose bytes moved reads otherwise.
        assert!(read_in(&dir, "frame|b").is_none());
        let at = file(&dir, "frame|a");
        let mut bytes = std::fs::read(&at).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x40;
        std::fs::write(&at, bytes).unwrap();
        assert_ne!(read_in(&dir, "frame|a").expect("kept").raw, raw);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_oldest_decodes_go_first_past_the_cap() {
        let dir = temporary("trim");
        let kept = Kept { width: 10, height: 10, flip: 0, factor: 1, matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], raw: Vec::new() };
        for (i, key) in ["old", "mid", "new"].iter().enumerate() {
            let samples = vec![i as u16; 300];
            store(&dir, key, &kept, samples.chunks(30));
            let at = std::time::SystemTime::now() - std::time::Duration::from_secs(100 - 10 * i as u64);
            std::fs::File::options().write(true).open(file(&dir, key)).unwrap().set_modified(at).unwrap();
        }
        let one = std::fs::metadata(file(&dir, "new")).unwrap().len();
        trim(&dir, 2 * one);
        assert!(read_in(&dir, "old").is_none(), "the oldest went");
        assert!(read_in(&dir, "mid").is_some() && read_in(&dir, "new").is_some(), "twin: the two newest stay");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
