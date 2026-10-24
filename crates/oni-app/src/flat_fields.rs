//! `CustomFlatFields`: the masks loaded by hand, kept as float TIFFs named by their content and laid
//! on every frame naming them; `FlatField.remember`, the three sliders a hand last left, per machine.

use crate::preferences::Preferences;
use oni_i18n::tr;
use oni_library::index::LibraryIndex;
use oni_pipeline::flat_field::Mask;
use oni_settings::frame::{FlatField, RgbPasses};
use oni_settings::sidecar;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// One mask in `masks.json`: its content id, the name it shows, and whether it is made of passes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(rename = "perChannel", default, skip_serializing_if = "Option::is_none")]
    pub per_channel: Option<bool>,
}

/// `CustomFlatFields.Failure`, in Swift's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Undecodable,
    Unwritable,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Failure::Undecodable => tr("This file could not be decoded as a picture."),
            Failure::Unwritable => tr("The mask could not be saved in the app's folder."),
        }
    }
}

/// A decode at the preview's side: width, height and working-light pixels, row 0 at the top.
pub type Picture = (usize, usize, Vec<[f32; 4]>);

pub fn folder(support: &Path) -> PathBuf {
    support.join("FlatFields")
}

fn index(support: &Path) -> PathBuf {
    folder(support).join("masks.json")
}

fn file(support: &Path, id: &str) -> PathBuf {
    folder(support).join(format!("{id}.tiff"))
}

/// `CustomFlatFields.presets`: where a photograph taken as a mask goes once it leaves the library.
pub fn presets(support: &Path) -> PathBuf {
    support.join("FlatField Presets")
}

/// The list as the Swift app wrote it; empty when there is none or it does not read, one that does
/// not read set aside before the next write.
pub fn entries(support: &Path) -> Vec<Entry> {
    oni_library::read_json(&index(support)).unwrap_or_default()
}

/// Through a sibling and a rename, so a reader never meets half a list.
fn write_entries(support: &Path, list: &[Entry]) -> bool {
    serde_json::to_vec(list).is_ok_and(|data| oni_library::write_atomically(&index(support), &data).is_ok())
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The first 8 bytes of the content's SHA-256 in hex; a triplet's pass names enter it, so its
/// mask is not its lead pass's.
pub fn id_of(source: &[u8], passes: Option<&RgbPasses>) -> String {
    let mut hash = Sha256::new();
    hash.update(source);
    if let Some(p) = passes {
        hash.update(format!("rgb:{}|{}|{}", p.red, p.green, p.blue).as_bytes());
    }
    hash.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// `RGBPasses.named(by:)`: the passes a lead's sidecar names; a sidecar naming none is not parsed.
pub fn passes_named_by(lead: &Path) -> Option<RgbPasses> {
    let data = std::fs::read(sidecar::url_for(lead)).ok()?;
    let key = b"\"rgbPasses\"";
    if !data.windows(key.len()).any(|w| w == key) {
        return None;
    }
    sidecar::read(lead, oni_decode::decodes_via_mosaic(lead), false).ok()?.settings.rgb_passes
}

/// `CustomFlatFields.load`: decoded once by `decode` at the preview's side and kept as a half-float
/// TIFF in working light under its content id; a file loaded before returns its entry. Disk and decode work.
pub fn load(support: &Path, source: &Path, decode: impl FnOnce(&Path) -> Option<Picture>) -> Result<Entry, Failure> {
    let bytes = std::fs::read(source).map_err(|_| Failure::Undecodable)?;
    let passes = passes_named_by(source);
    let id = id_of(&bytes, passes.as_ref());
    drop(bytes);
    let mut list = entries(support);
    if let Some(known) = list.iter().find(|e| e.id == id)
        && file(support, &id).exists()
    {
        return Ok(known.clone());
    }
    let (width, height, pixels) = decode(source).ok_or(Failure::Undecodable)?;
    std::fs::create_dir_all(folder(support)).map_err(|_| Failure::Unwritable)?;
    // No metadata: a RAW's EXIF, serial number included, has no business in a mask.
    let icc = oni_colour::working::working_profile().encode().unwrap_or_default();
    let tiff = oni_export::tiff::encoded_half(width, height, &pixels, &icc).ok_or(Failure::Unwritable)?;
    // Through a sibling and a rename: a mask cut short by a crash would be taken whole by the next load.
    oni_library::write_atomically(&file(support, &id), &tiff).map_err(|_| Failure::Unwritable)?;
    let entry = Entry { id: id.clone(), name: stem(source), per_channel: passes.map(|_| true) };
    list.retain(|e| e.id != id);
    list.push(entry.clone());
    if !write_entries(support, &list) {
        return Err(Failure::Unwritable);
    }
    Ok(entry)
}

/// Whether `path` is the very file a mask was loaded from: its name first, which costs nothing,
/// then its content's hash, which reads the file only when a name matched.
pub fn is_mask_source(support: &Path, path: &Path) -> bool {
    let stem = stem(path);
    let named: Vec<Entry> = entries(support).into_iter().filter(|e| e.name == stem).collect();
    if named.is_empty() {
        return false;
    }
    let Ok(data) = std::fs::read(path) else { return false };
    let id = id_of(&data, None);
    named.iter().any(|e| e.id == id)
}

/// A mask's name changed in the list; its id, hence every frame naming it, stays as it was.
pub fn rename(support: &Path, id: &str, name: &str) {
    let mut list = entries(support);
    let Some(entry) = list.iter_mut().find(|e| e.id == id) else { return };
    entry.name = name.to_string();
    write_entries(support, &list);
}

pub fn forget(support: &Path, ids: &HashSet<String>) {
    let list: Vec<Entry> = entries(support).into_iter().filter(|e| !ids.contains(&e.id)).collect();
    for id in ids {
        let _ = std::fs::remove_file(file(support, id));
    }
    write_entries(support, &list);
}

/// How a move reaches the disk; a check swaps them to force a move across volumes.
pub struct Moving<'a> {
    pub rename: fn(&Path, &Path) -> std::io::Result<()>,
    pub copy: fn(&Path, &Path) -> std::io::Result<()>,
    pub trash: &'a dyn Fn(&Path) -> std::io::Result<()>,
}

impl<'a> Moving<'a> {
    /// The system's rename and a copy handed to the drive, then `trash`, the app's discard.
    pub fn through(trash: &'a dyn Fn(&Path) -> std::io::Result<()>) -> Moving<'a> {
        Moving { rename: |from, to| std::fs::rename(from, to), copy: synced_copy, trash }
    }
}

/// `Library.moveOut`: the original moved into `folder` under a free name, its sidecar sent to the
/// system Trash and its index row cleared; the one case where the app moves an original, once confirmed.
pub fn move_out(path: &str, folder: &Path, index: Option<&LibraryIndex>, how: &Moving) -> Option<PathBuf> {
    let url = Path::new(path);
    std::fs::create_dir_all(folder).ok()?;
    let mut target = folder.join(url.file_name()?);
    let extension = url.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let mut rank = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("{} {rank}.{extension}", stem(url)));
        rank += 1;
    }
    moved(url, &target, how).ok()?;
    let settings = sidecar::url_for(url);
    if settings.exists() {
        let _ = (how.trash)(&settings);
    }
    if let Some(index) = index {
        let _ = index.remove(path);
    }
    Some(target)
}

/// `FileManager.moveItem`, safer across volumes: a copy handed to the drive, read back equal to
/// the original byte for byte, and only then the original sent to the system Trash.
fn moved(from: &Path, to: &Path, how: &Moving) -> std::io::Result<()> {
    match (how.rename)(from, to) {
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let checked = (how.copy)(from, to).and_then(|()| {
                if same_content(from, to)? { Ok(()) } else { Err(std::io::Error::other(tr("the copy does not match the original"))) }
            });
            let gone = checked.and_then(|()| (how.trash)(from));
            if gone.is_err() {
                let _ = std::fs::remove_file(to);
            }
            gone
        }
        other => other,
    }
}

/// A copy into a new file, handed to the drive with its folder before it counts.
fn synced_copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut source = std::fs::File::open(from)?;
    let mut copy = std::fs::OpenOptions::new().write(true).create_new(true).open(to)?;
    std::io::copy(&mut source, &mut copy)?;
    copy.sync_all()?;
    // The new name reaches the disk with the folder; a system that cannot sync a folder skips it.
    if let Some(folder) = to.parent()
        && let Ok(folder) = std::fs::File::open(folder)
    {
        let _ = folder.sync_all();
    }
    Ok(())
}

/// Both files read again, chunk by chunk, and compared whole.
fn same_content(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let (mut a, mut b) = (std::io::BufReader::new(std::fs::File::open(a)?), std::io::BufReader::new(std::fs::File::open(b)?));
    let (mut x, mut y) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let n = a.read(&mut x)?;
        if n == 0 {
            return Ok(b.read(&mut y[..1])? == 0);
        }
        match b.read_exact(&mut y[..n]) {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
            other => other?,
        }
        if x[..n] != y[..n] {
            return Ok(false);
        }
    }
}

/// `FlatField.preferenceKey`: the three sliders as a hand last left them, kept across sessions,
/// since a rig is dialled once and every frame scanned on it wants the same dose and colour.
pub const SLIDERS_KEY: &str = "flatFieldSliders";

pub fn remember(preferences: &mut Preferences, f: &FlatField) -> std::io::Result<()> {
    let kept = [f.amount, f.temperature, f.tint].map(f64::from);
    preferences.set(SLIDERS_KEY, serde_json::json!(kept))
}

pub fn remembered(preferences: &Preferences) -> Option<[f64; 3]> {
    let list = preferences.value(SLIDERS_KEY)?.as_array()?;
    if list.len() != 3 {
        return None;
    }
    Some([list[0].as_f64()?, list[1].as_f64()?, list[2].as_f64()?])
}

/// `withRemembered`: a frame whose three sliders were never moved takes the colour last dialled;
/// its Amount stays whole and its mask its own.
pub fn with_remembered(f: &FlatField, kept: Option<[f64; 3]>) -> FlatField {
    let untouched = f.amount == FlatField::default().amount && f.temperature == 0.0 && f.tint == 0.0;
    let Some(kept) = kept.filter(|_| untouched) else { return f.clone() };
    let (low, high) = FlatField::COLOUR_RANGE;
    FlatField { temperature: (kept[1] as f32).clamp(low, high), tint: (kept[2] as f32).clamp(low, high), ..f.clone() }
}

/// `CustomFlatFields.source`: the mask read with no colour conversion, the stored values already
/// the working space's, and its peaks, `CIAreaMaximum` over the whole mask; `None` once the file is gone.
pub fn read(support: &Path, id: &str) -> Option<Mask> {
    let (width, height, pixels) = read_tiff(&file(support, id))?;
    let peaks = pixels.iter().fold([f32::MIN; 3], |m, p| [m[0].max(p[0]), m[1].max(p[1]), m[2].max(p[2])]);
    let per_channel = entries(support).iter().any(|e| e.id == id && e.per_channel == Some(true));
    Some(Mask { width, height, pixels, peaks, per_channel })
}

/// A TIFF the Swift app wrote in working light, half or single float, read as it stands, row 0 at the top.
pub fn read_tiff(path: &Path) -> Option<(u32, u32, Vec<[f32; 4]>)> {
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?)).ok()?;
    let (width, height) = decoder.dimensions().ok()?;
    let channels = match decoder.colortype().ok()? {
        tiff::ColorType::RGBA(_) => 4,
        tiff::ColorType::RGB(_) => 3,
        tiff::ColorType::Gray(_) => 1,
        _ => return None,
    };
    let samples: Vec<f32> = match decoder.read_image().ok()? {
        tiff::decoder::DecodingResult::F16(v) => v.into_iter().map(f32::from).collect(),
        tiff::decoder::DecodingResult::F32(v) => v,
        tiff::decoder::DecodingResult::U16(v) => v.into_iter().map(|s| f32::from(s) / 65535.0).collect(),
        tiff::decoder::DecodingResult::U8(v) => v.into_iter().map(|s| f32::from(s) / 255.0).collect(),
        _ => return None,
    };
    if samples.len() != (width * height) as usize * channels {
        return None;
    }
    let pixels: Vec<[f32; 4]> = samples
        .chunks_exact(channels)
        .map(|c| {
            let v = |i: usize| c[i.min(channels - 1)];
            [v(0), v(1), v(2), if channels == 4 { c[3] } else { 1.0 }]
        })
        .collect();
    Some((width, height, pixels))
}

#[cfg(test)]
mod tests;
