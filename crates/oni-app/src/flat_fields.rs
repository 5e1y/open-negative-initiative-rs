//! `CustomFlatFields`: the masks loaded by hand, kept in the support folder as float TIFFs named by
//! their content, read once per id with their peaks and laid on every frame that names them.

use oni_pipeline::flat_field::Mask;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One mask in `masks.json`: its content id, the name it shows, and whether it is made of passes.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    #[serde(rename = "perChannel", default)]
    pub per_channel: Option<bool>,
}

pub fn folder(support: &Path) -> PathBuf {
    support.join("FlatFields")
}

fn file(support: &Path, id: &str) -> PathBuf {
    folder(support).join(format!("{id}.tiff"))
}

/// The list as the Swift app wrote it; empty when there is none or it does not read.
pub fn entries(support: &Path) -> Vec<Entry> {
    std::fs::read(folder(support).join("masks.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// `CustomFlatFields.source`: the mask read with no colour conversion, the stored values already
/// the working space's, and its peaks, `CIAreaMaximum` over the whole mask; `None` once the file is gone.
pub fn read(support: &Path, id: &str) -> Option<Mask> {
    let mut decoder = tiff::decoder::Decoder::new(std::io::BufReader::new(std::fs::File::open(file(support, id)).ok()?)).ok()?;
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
    let peaks = pixels.iter().fold([f32::MIN; 3], |m, p| [m[0].max(p[0]), m[1].max(p[1]), m[2].max(p[2])]);
    let per_channel = entries(support).iter().any(|e| e.id == id && e.per_channel == Some(true));
    Some(Mask { width, height, pixels, peaks, per_channel })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(support: &Path, id: &str, pixels: &[f32], width: u32, height: u32) {
        std::fs::create_dir_all(folder(support)).unwrap();
        let out = std::fs::File::create(file(support, id)).unwrap();
        let mut encoder = tiff::encoder::TiffEncoder::new(out).unwrap();
        encoder.write_image::<tiff::encoder::colortype::RGBA32Float>(width, height, pixels).unwrap();
    }

    #[test]
    fn a_mask_reads_with_its_peaks_and_its_passes_flag() {
        let support = std::env::temp_dir().join(format!("oni-flat-fields-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&support);
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
