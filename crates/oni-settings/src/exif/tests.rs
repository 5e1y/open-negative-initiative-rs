use super::*;
use std::path::PathBuf;

/// A folder of this test's own, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("oni-exif-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const DATE: &str = "2026:06:18 14:32:10";

/// A little or big-endian TIFF whose EXIF IFD holds `date`, built byte by byte.
fn tiff(little: bool, date: &str) -> Vec<u8> {
    let w16 = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
    let w32 = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut out = Vec::new();
    out.extend_from_slice(if little { b"II" } else { b"MM" });
    out.extend_from_slice(&w16(42));
    out.extend_from_slice(&w32(8));
    // IFD0 at 8: one entry, the EXIF pointer to 26.
    out.extend_from_slice(&w16(1));
    out.extend_from_slice(&w16(EXIF_IFD));
    out.extend_from_slice(&w16(4));
    out.extend_from_slice(&w32(1));
    out.extend_from_slice(&w32(26));
    out.extend_from_slice(&w32(0));
    // EXIF IFD at 26: DateTimeOriginal, its 20 bytes at 44.
    out.extend_from_slice(&w16(1));
    out.extend_from_slice(&w16(DATE_TIME_ORIGINAL));
    out.extend_from_slice(&w16(ASCII));
    out.extend_from_slice(&w32(20));
    out.extend_from_slice(&w32(44));
    out.extend_from_slice(&w32(0));
    out.extend_from_slice(date.as_bytes());
    out.push(0);
    out
}

/// A TIFF whose first directory is the EXIF IFD itself, as a CR3's `CMT2` holds it.
fn exif_first(date: &str) -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&DATE_TIME_ORIGINAL.to_le_bytes());
    out.extend_from_slice(&ASCII.to_le_bytes());
    out.extend_from_slice(&20u32.to_le_bytes());
    out.extend_from_slice(&26u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(date.as_bytes());
    out.push(0);
    out
}

fn jpeg(body: &[u8]) -> Vec<u8> {
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xE1];
    jpeg.extend_from_slice(&((body.len() + 8) as u16).to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(body);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);
    jpeg
}

fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

/// A RAF: its magic, the embedded JPEG's offset and length at 84, the JPEG at `at`.
fn raf(date: &str, at: u32) -> Vec<u8> {
    let embedded = jpeg(&tiff(false, date));
    let mut out = b"FUJIFILMCCD-RAW 0201FF383501".to_vec();
    out.resize(84, 0);
    out.extend_from_slice(&at.to_be_bytes());
    out.extend_from_slice(&(embedded.len() as u32).to_be_bytes());
    out.resize(160, 0);
    out.extend_from_slice(&embedded);
    out
}

/// A CR3: `ftyp crx`, then `moov` holding Canon's `uuid` with `CMT1` and `CMT2`.
fn cr3(date: &str, uuid: [u8; 16]) -> Vec<u8> {
    let mut canon = uuid.to_vec();
    canon.extend(bx(b"CMT1", &tiff(true, "1999:01:01 00:00:00")));
    canon.extend(bx(b"CMT2", &exif_first(date)));
    let mut out = bx(b"ftyp", b"crx \0\0\0\x01crx isom");
    out.extend(bx(b"moov", &bx(b"uuid", &canon)));
    out.extend(bx(b"mdat", &[0; 64]));
    out
}

/// A HEIC: `meta` naming an `Exif` item in `iinf`, placed by `iloc` in `mdat`.
fn heic(date: &str, kind: &[u8; 4]) -> Vec<u8> {
    let ftyp = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
    let mut infe = vec![2, 0, 0, 0, 0, 7, 0, 0];
    infe.extend_from_slice(kind);
    let mut iinf = vec![0, 0, 0, 0, 0, 1];
    iinf.extend(bx(b"infe", &infe));
    // The item, past four bytes naming the TIFF header's offset after them: six, an `Exif\0\0`.
    let mut item = 6u32.to_be_bytes().to_vec();
    item.extend_from_slice(b"Exif\0\0");
    item.extend(tiff(false, date));
    let place = |at: u32| {
        let mut iloc = vec![0, 0, 0, 0, 0x44, 0x00, 0, 1, 0, 7, 0, 0, 0, 1];
        iloc.extend_from_slice(&at.to_be_bytes());
        iloc.extend_from_slice(&(item.len() as u32).to_be_bytes());
        iloc
    };
    let meta_of = |iloc: Vec<u8>| {
        let mut meta = vec![0, 0, 0, 0];
        meta.extend(bx(b"iinf", &iinf));
        meta.extend(bx(b"iloc", &iloc));
        bx(b"meta", &meta)
    };
    let before = ftyp.len() + meta_of(place(0)).len() + 8;
    let mut out = ftyp;
    out.extend(meta_of(place(before as u32)));
    out.extend(bx(b"mdat", &item));
    out
}

fn png(date: &str, chunk: &[u8; 4]) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut add = |kind: &[u8; 4], body: &[u8]| {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out.extend_from_slice(&[0; 4]);
    };
    add(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
    add(chunk, &tiff(true, date));
    add(b"IEND", &[]);
    out
}

#[test]
fn a_tiff_and_a_jpeg_give_their_original_date_in_either_byte_order() {
    let scratch = Scratch::new("tiff");
    for little in [true, false] {
        let path = scratch.write(&format!("t{little}.tif"), &tiff(little, DATE));
        assert_eq!(capture_date(&path).as_deref(), Some(DATE));
    }
    let path = scratch.write("t.jpg", &jpeg(&tiff(true, DATE)));
    assert_eq!(capture_date(&path).as_deref(), Some(DATE));

    // Twin: the same bytes with the byte-order mark flipped read nothing sensible.
    let mut flipped = tiff(true, DATE);
    flipped[..2].copy_from_slice(b"MM");
    let path = scratch.write("flipped.tif", &flipped);
    assert_ne!(capture_date(&path).as_deref(), Some(DATE));
    // Twin: an empty file carries no date.
    let path = scratch.write("empty.NEF", b"");
    assert_eq!(capture_date(&path), None);
}

#[test]
fn raf_and_cr3_carry_their_date() {
    let scratch = Scratch::new("raw");
    let path = scratch.write("DSCF0001.RAF", &raf(DATE, 160));
    assert_eq!(capture_date(&path).as_deref(), Some(DATE));
    let path = scratch.write("IMG_0001.CR3", &cr3(DATE, containers_uuid()));
    assert_eq!(capture_date(&path).as_deref(), Some(DATE), "CMT2's, never CMT1's IFD0");
    // Twin: a RAF whose offset misses its JPEG, and a CR3 under another box's uuid, carry none.
    let path = scratch.write("off.RAF", &raf(DATE, 150));
    assert_eq!(capture_date(&path), None);
    let mut other = containers_uuid();
    other[0] ^= 0xFF;
    let path = scratch.write("other.CR3", &cr3(DATE, other));
    assert_eq!(capture_date(&path), None);
}

#[test]
fn a_heic_and_a_png_carry_their_date() {
    let scratch = Scratch::new("heif");
    let path = scratch.write("IMG_0001.HEIC", &heic(DATE, b"Exif"));
    assert_eq!(capture_date(&path).as_deref(), Some(DATE));
    let path = scratch.write("scan.png", &png(DATE, b"eXIf"));
    assert_eq!(capture_date(&path).as_deref(), Some(DATE));
    // Twin: the item typed otherwise, or the chunk named otherwise, is no EXIF.
    let path = scratch.write("mime.HEIC", &heic(DATE, b"mime"));
    assert_eq!(capture_date(&path), None);
    let path = scratch.write("text.png", &png(DATE, b"tEXt"));
    assert_eq!(capture_date(&path), None);
}

fn containers_uuid() -> [u8; 16] {
    [0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48]
}
