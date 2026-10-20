//! `CorrectionCache`: each dust correction's patch, generated once at full resolution and kept in
//! the caches folder under its correction's id, so a render needs no photo identity to find it.

use oni_settings::id::Id;
use std::path::{Path, PathBuf};

/// The app's caches folder, `AppFolders.caches`'s one route.
pub fn caches() -> PathBuf {
    crate::system::caches()
}

pub fn file(caches: &Path, id: Id) -> PathBuf {
    caches.join("Corrections").join(format!("{id}.tiff"))
}

/// Written where the render looks for it, in single float; the Swift app reads it as its own.
pub fn write(caches: &Path, id: Id, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<(), String> {
    let path = file(caches, id);
    std::fs::create_dir_all(path.parent().expect("a folder")).map_err(|e| e.to_string())?;
    let partial = path.with_extension("tiff.partial");
    let out = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let data: &[f32] = bytemuck::cast_slice(pixels);
    tiff::encoder::TiffEncoder::new(std::io::BufWriter::new(out))
        .and_then(|mut e| e.write_image::<tiff::encoder::colortype::RGBA32Float>(width, height, data))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())
}

/// `CorrectionCache.purgeAll`, run at quit: every patch is generated again the next time its frame opens.
pub fn purge(caches: &Path) {
    let _ = std::fs::remove_dir_all(caches.join("Corrections"));
}

/// The patch's pixels and size; `None` while it has not been generated.
pub fn read(caches: &Path, id: Id) -> Option<(u32, u32, Vec<[f32; 4]>)> {
    crate::flat_fields::read_tiff(&file(caches, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_is_found_under_its_corrections_id_as_swift_names_it() {
        let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
        let caches = std::env::temp_dir().join(format!("oni-patches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caches);
        assert!(read(&caches, id).is_none(), "not generated yet");
        std::fs::create_dir_all(caches.join("Corrections")).unwrap();
        let path = file(&caches, id);
        assert!(path.ends_with("Corrections/0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B.tiff"), "{}", path.display());
        let out = std::fs::File::create(&path).unwrap();
        let pixels: Vec<f32> = (0..6).flat_map(|i| [i as f32 / 6.0, 0.5, 0.25, 1.0]).collect();
        tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(3, 2, &pixels).unwrap();
        let (w, h, got) = read(&caches, id).unwrap();
        assert_eq!((w, h), (3, 2));
        assert!((got[4][0] - 4.0 / 6.0).abs() < 1e-6, "row 1 column 1 in place");
        // Twin: another correction's id finds nothing.
        assert!(read(&caches, Id::parse("11111111-2222-3333-4444-555555555555").unwrap()).is_none());
        let _ = std::fs::remove_dir_all(&caches);
    }
}
