//! `CorrectionCache`: each dust correction's patch, generated once at full resolution and kept in
//! the caches folder under its key, so a render needs no photo identity to find it.

use oni_pipeline::corrections::PatchKey;
use std::path::{Path, PathBuf};

/// This instance's folder of patches, under the caches folder every running ONI shares: one of its
/// own, named after the support folder that one instance at a time holds, so a quit purges its own.
pub fn caches() -> PathBuf {
    instance(&crate::system::caches(), &crate::timing::support())
}

/// The folder under `caches` of the instance holding `support`, named by the path's FNV-1a hash,
/// which names no folder of the person's.
fn instance(caches: &Path, support: &Path) -> PathBuf {
    let hash = support.as_os_str().as_encoded_bytes().iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    caches.join("Instances").join(format!("{hash:016x}"))
}

/// A patch's file: its correction's id and what was upstream of it, so a change there finds none.
pub fn file(caches: &Path, key: PatchKey) -> PathBuf {
    caches.join("Corrections").join(format!("{}-{:016x}.tiff", key.id, key.upstream))
}

/// Whether the patch of `c` under the frame's settings `s` is on disk.
pub fn made(caches: &Path, s: &oni_settings::PipelineSettings, c: &oni_settings::frame::Correction) -> bool {
    file(caches, PatchKey::of(s, c)).exists()
}

/// Written where the render looks for it, in single float.
pub fn write(caches: &Path, key: PatchKey, width: u32, height: u32, pixels: &[[f32; 4]]) -> Result<(), String> {
    let path = file(caches, key);
    std::fs::create_dir_all(path.parent().expect("a folder")).map_err(|e| e.to_string())?;
    let data: &[f32] = bytemuck::cast_slice(pixels);
    oni_settings::atomic::cached_with(&path, |file| {
        let mut out = std::io::BufWriter::new(file);
        tiff::encoder::TiffEncoder::new(&mut out)
            .and_then(|mut e| e.write_image::<tiff::encoder::colortype::RGBA32Float>(width, height, data))
            .map_err(std::io::Error::other)?;
        std::io::Write::flush(&mut out)
    })
    .map_err(|e| e.to_string())
}

/// `CorrectionCache.purgeAll`, run at quit on this instance's folder: each of its patches is generated
/// again when its frame opens or is written to a file, and the folder goes once empty.
pub fn purge(caches: &Path) {
    let _ = std::fs::remove_dir_all(caches.join("Corrections"));
    let _ = std::fs::remove_dir(caches);
}

/// The patch's pixels and size; `None` while it has not been generated.
pub fn read(caches: &Path, key: PatchKey) -> Option<(u32, u32, Vec<[f32; 4]>)> {
    crate::flat_fields::read_tiff(&file(caches, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::id::Id;

    fn key(id: &str, upstream: u64) -> PatchKey {
        PatchKey { id: Id::parse(id).unwrap(), upstream }
    }

    #[test]
    fn a_patch_is_found_under_its_correction_and_what_was_upstream_of_it() {
        let at = key("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B", 0x2a);
        let caches = std::env::temp_dir().join(format!("oni-patches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&caches);
        assert!(read(&caches, at).is_none(), "not generated yet");
        std::fs::create_dir_all(caches.join("Corrections")).unwrap();
        let path = file(&caches, at);
        assert!(path.ends_with("Corrections/0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B-000000000000002a.tiff"), "{}", path.display());
        let out = std::fs::File::create(&path).unwrap();
        let pixels: Vec<f32> = (0..6).flat_map(|i| [i as f32 / 6.0, 0.5, 0.25, 1.0]).collect();
        tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(3, 2, &pixels).unwrap();
        let (w, h, got) = read(&caches, at).unwrap();
        assert_eq!((w, h), (3, 2));
        assert!((got[4][0] - 4.0 / 6.0).abs() < 1e-6, "row 1 column 1 in place");
        // Twins: another correction's id, or the same one under other settings upstream, finds nothing.
        assert!(read(&caches, key("11111111-2222-3333-4444-555555555555", 0x2a)).is_none());
        assert!(read(&caches, PatchKey { upstream: 0x2b, ..at }).is_none());
        let _ = std::fs::remove_dir_all(&caches);
    }

    #[test]
    fn a_quit_purges_its_own_patches_and_leaves_another_instance_s() {
        let root = std::env::temp_dir().join(format!("oni-patches-instances-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (mine, theirs) = (instance(&root, Path::new("/a/Open Negative Initiative")), instance(&root, Path::new("/b/scratch support")));
        let id = key("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B", 7);
        for folder in [&mine, &theirs] {
            write(folder, id, 2, 2, &[[0.5; 4]; 4]).unwrap();
        }
        assert_eq!(instance(&root, Path::new("/a/Open Negative Initiative")), mine, "a launch finds its own folder again");
        purge(&mine);
        assert!(read(&mine, id).is_none() && !mine.exists(), "the quitting instance's patches and folder go");
        assert!(read(&theirs, id).is_some(), "another instance's patch went with them");
        // Twin: two instances on one folder, as before, lose each other's patches at a quit.
        let shared = instance(&root, Path::new("/b/scratch support"));
        purge(&shared);
        assert!(read(&theirs, id).is_none(), "a purge of the shared folder left the patch");
        let _ = std::fs::remove_dir_all(&root);
    }
}
