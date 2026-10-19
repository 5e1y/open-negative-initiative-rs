//! `CustomFlatFields`: the masks loaded by hand, kept as float TIFFs named by their content and laid
//! on every frame naming them; `FlatField.remember`, the three sliders a hand last left, per machine.

use crate::preferences::Preferences;
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
            Failure::Undecodable => "This file could not be decoded as a picture.",
            Failure::Unwritable => "The mask could not be saved in the app's folder.",
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
    std::fs::write(file(support, &id), tiff).map_err(|_| Failure::Unwritable)?;
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

/// `Library.moveOut`: the original moved into `folder` under a free name, its sidecar and index row
/// cleared; the one case where the app moves an original, and only once a hand has confirmed it.
pub fn move_out(path: &str, folder: &Path, index: Option<&LibraryIndex>) -> Option<PathBuf> {
    let url = Path::new(path);
    std::fs::create_dir_all(folder).ok()?;
    let mut target = folder.join(url.file_name()?);
    let extension = url.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let mut rank = 2;
    while target.symlink_metadata().is_ok() {
        target = folder.join(format!("{} {rank}.{extension}", stem(url)));
        rank += 1;
    }
    moved(url, &target).ok()?;
    let _ = sidecar::remove(url);
    if let Some(index) = index {
        let _ = index.remove(path);
    }
    Some(target)
}

/// `FileManager.moveItem`: a rename, else across volumes a copy whose length is checked before
/// the original goes.
fn moved(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let copied = std::fs::copy(from, to);
            if copied.as_ref().ok() != Some(&std::fs::metadata(from)?.len()) {
                let _ = std::fs::remove_file(to);
                return Err(copied.err().unwrap_or_else(|| std::io::Error::other("the copy came out short")));
            }
            std::fs::remove_file(from)
        }
        other => other,
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
mod tests {
    use super::*;
    use std::cell::Cell;

    fn written(support: &Path, id: &str, pixels: &[f32], width: u32, height: u32) {
        std::fs::create_dir_all(folder(support)).unwrap();
        let out = std::fs::File::create(file(support, id)).unwrap();
        let mut encoder = tiff::encoder::TiffEncoder::new(out).unwrap();
        encoder.write_image::<tiff::encoder::colortype::RGBA32Float>(width, height, pixels).unwrap();
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oni-flat-fields-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A panel falling off from 1 on the right to 0.25 on the left, as a decode would hand it.
    fn ramp() -> Picture {
        let (w, h) = (64, 32);
        let pixels = (0..w * h).map(|i| { let v = 0.25 + 0.75 * (i % w) as f32 / (w - 1) as f32; [v, v, v, 1.0] }).collect();
        (w, h, pixels)
    }

    #[test]
    fn a_mask_reads_with_its_peaks_and_its_passes_flag() {
        let support = scratch("read");
        let pixels: Vec<f32> = (0..4 * 3).flat_map(|i| [0.2 + i as f32 * 0.05, 0.9 - i as f32 * 0.01, 0.4, 1.0]).collect();
        written(&support, "abc", &pixels, 4, 3);
        std::fs::write(folder(&support).join("masks.json"), r#"[{"id":"abc","name":"Panel","perChannel":true},{"id":"def","name":"Other"}]"#).unwrap();
        let mask = read(&support, "abc").expect("the mask reads");
        assert_eq!((mask.width, mask.height, mask.pixels.len()), (4, 3, 12));
        assert!((mask.peaks[0] - 0.75).abs() < 1e-6 && (mask.peaks[1] - 0.9).abs() < 1e-6, "{:?}", mask.peaks);
        assert!(mask.per_channel);
        assert_eq!(entries(&support).len(), 2);
        // Twin: a mask the list does not mark as passes divides on its green, and a gone file reads none.
        written(&support, "def", &pixels, 4, 3);
        assert!(!read(&support, "def").unwrap().per_channel);
        assert!(read(&support, "gone").is_none());
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn the_id_is_the_first_eight_bytes_of_the_contents_sha_256() {
        // SHA-256("abc") = ba7816bf8f01cfea 414140de…, the FIPS 180-2 example.
        assert_eq!(id_of(b"abc", None), "ba7816bf8f01cfea");
        let passes = RgbPasses { red: "R.NEF".into(), green: "G.NEF".into(), blue: "B.NEF".into(), registration: None };
        // Twin: the same bytes leading a triplet take another id, so the two masks never share one.
        assert_ne!(id_of(b"abc", Some(&passes)), id_of(b"abc", None));
        assert_eq!(id_of(b"abc", Some(&passes)).len(), 16);
    }

    /// `FlatField.selfCheck`'s loaded mask: it joins the list under its file's name, loading it
    /// again adds nothing, and the TIFF holds the decode's falloff.
    #[test]
    fn a_loaded_picture_joins_the_list_under_its_files_name_once() {
        let support = scratch("load");
        let source = support.join("Scans/panel.tiff");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"the panel's own bytes").unwrap();
        let decodes = Cell::new(0);
        let decode = |_: &Path| {
            decodes.set(decodes.get() + 1);
            Some(ramp())
        };
        let entry = load(&support, &source, decode).expect("the panel loads");
        assert_eq!((entry.name.as_str(), entry.per_channel), ("panel", None));
        assert_eq!(entry.id, id_of(b"the panel's own bytes", None));
        assert_eq!(entries(&support), [entry.clone()]);
        let text = std::fs::read_to_string(folder(&support).join("masks.json")).unwrap();
        assert!(!text.contains("perChannel"), "an unset flag is left out, as JSONEncoder does: {text}");
        let again = load(&support, &source, |_| -> Option<Picture> { panic!("a known file is not decoded again") }).unwrap();
        assert_eq!((again, entries(&support).len(), decodes.get()), (entry.clone(), 1, 1));
        let (w, h, pixels) = read_tiff(&file(&support, &entry.id)).expect("the half-float TIFF reads");
        assert_eq!((w, h), (64, 32));
        let (dim, bright) = (pixels[16 * 64 + 2][1], pixels[16 * 64 + 61][1]);
        assert!((dim - 0.2738).abs() < 1e-3 && (bright - 0.9762).abs() < 1e-3, "{dim} {bright}");
        // Twin: a file no decoder takes, or no file at all, is undecodable and leaves the list alone.
        let other = support.join("Scans/noise.tiff");
        std::fs::write(&other, b"other bytes").unwrap();
        assert_eq!(load(&support, &other, |_| None), Err(Failure::Undecodable));
        assert_eq!(load(&support, &support.join("gone.tiff"), |_| Some(ramp())), Err(Failure::Undecodable));
        assert_eq!(entries(&support).len(), 1);
        let _ = std::fs::remove_dir_all(&support);
    }

    /// `RGBPasses.selfCheck`'s triplet: the lead taken as a mask is marked made of passes, under an
    /// id of its own; the same file alone is a white-light mask.
    #[test]
    fn a_triplets_lead_is_a_mask_of_passes_under_an_id_of_its_own() {
        let support = scratch("passes");
        let lead = support.join("Scans/B.tif");
        std::fs::create_dir_all(lead.parent().unwrap()).unwrap();
        std::fs::write(&lead, b"lead bytes").unwrap();
        let alone = load(&support, &lead, |_| Some(ramp())).unwrap();
        let mut settings = oni_settings::PipelineSettings::default();
        settings.rgb_passes = Some(RgbPasses { red: "R.tif".into(), green: "G.tif".into(), blue: "B.tif".into(), registration: None });
        sidecar::write(&settings, &lead, Default::default()).expect("the lead's sidecar");
        assert_eq!(passes_named_by(&lead).map(|p| p.red), Some("R.tif".into()));
        let triplet = load(&support, &lead, |_| Some(ramp())).unwrap();
        assert_eq!((triplet.per_channel, alone.per_channel), (Some(true), None));
        assert_ne!(triplet.id, alone.id);
        assert!(read(&support, &triplet.id).unwrap().per_channel && !read(&support, &alone.id).unwrap().per_channel);
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn the_mask_source_is_known_by_its_name_then_its_content() {
        let support = scratch("source");
        let source = support.join("Rig.NEF");
        std::fs::write(&source, b"rig bytes").unwrap();
        let entry = load(&support, &source, |_| Some(ramp())).unwrap();
        assert!(is_mask_source(&support, &source));
        // Twins: the same name over other bytes, and the same bytes under another name, are not it.
        let renamed = support.join("Other.NEF");
        std::fs::write(&renamed, b"rig bytes").unwrap();
        assert!(!is_mask_source(&support, &renamed));
        std::fs::write(&source, b"rewritten").unwrap();
        assert!(!is_mask_source(&support, &source));
        // A rename keeps the id; forgetting drops the entry and its file.
        rename(&support, &entry.id, "Easel");
        assert_eq!(entries(&support)[0], Entry { name: "Easel".into(), ..entry.clone() });
        forget(&support, &HashSet::from([entry.id.clone()]));
        assert!(entries(&support).is_empty() && read(&support, &entry.id).is_none());
        let _ = std::fs::remove_dir_all(&support);
    }

    /// `FlatField.selfCheck`'s remembered sliders.
    #[test]
    fn a_fresh_frame_opens_on_the_colour_last_dialled_and_a_dialled_one_keeps_its_own() {
        let support = scratch("remember");
        let mut preferences = Preferences::load(&support);
        assert_eq!(with_remembered(&FlatField::default(), remembered(&preferences)), FlatField::default());
        let dialled = FlatField { amount: 0.37, temperature: 0.61, tint: -0.23, ..FlatField::default() };
        remember(&mut preferences, &dialled).unwrap();
        let kept = remembered(&Preferences::load(&support));
        let fresh = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
        let opened = with_remembered(&fresh, kept);
        assert_eq!((opened.amount, opened.temperature, opened.tint, opened.mask), (1.0, 0.61, -0.23, fresh.mask));
        // Twin: a frame already dialled keeps its own sliders.
        let own = FlatField { amount: 0.5, ..FlatField::default() };
        assert_eq!(with_remembered(&own, kept), own);
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn a_photo_moved_out_leaves_its_sidecar_and_index_row_and_takes_a_free_name() {
        let root = scratch("move-out");
        let (scans, presets) = (root.join("Scans"), presets(&root));
        std::fs::create_dir_all(&scans).unwrap();
        let photo = scans.join("Panel.NEF");
        std::fs::write(&photo, b"panel bytes").unwrap();
        sidecar::write(&oni_settings::PipelineSettings::default(), &photo, Default::default()).unwrap();
        let path = oni_library::paths::canonical(&photo);
        let index = LibraryIndex::open(&root.join("index.sqlite")).unwrap();
        index.upsert(&oni_library::index::Entry::new(&path, oni_settings::Date::now(), "f")).unwrap();
        std::fs::create_dir_all(&presets).unwrap();
        std::fs::write(presets.join("Panel.NEF"), b"an earlier panel").unwrap();
        let target = move_out(&path, &presets, Some(&index)).expect("moved");
        assert_eq!(target, presets.join("Panel 2.NEF"));
        assert_eq!(std::fs::read(&target).unwrap(), b"panel bytes");
        assert!(!photo.exists() && !sidecar::url_for(&photo).exists() && index.all().is_empty());
        assert_eq!(std::fs::read(presets.join("Panel.NEF")).unwrap(), b"an earlier panel", "the one already there stays");
        // Twin: a path with no file moves nothing and leaves the folder as it was.
        assert!(move_out(&path, &presets, Some(&index)).is_none());
        assert_eq!(std::fs::read_dir(&presets).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The masks the Swift app saved on this machine, read in place and never written.
    #[test]
    fn the_swift_apps_own_masks_read_whole() {
        let support = crate::timing::support();
        let found = entries(&support);
        if found.is_empty() {
            println!("SKIPPED: no custom flat-field in {}", support.display());
            return;
        }
        for e in &found {
            let Some(mask) = read(&support, &e.id) else { panic!("{}: {} does not read", e.id, e.name) };
            assert!(mask.width > 0 && mask.pixels.len() == (mask.width * mask.height) as usize);
            assert!(mask.peaks.iter().all(|p| p.is_finite() && *p > 0.0), "{}: {:?}", e.name, mask.peaks);
        }
    }
}
