//! The decoder's code as one print, built by `build.rs` into `ONI_DECODE_STAMP`, so a decode kept
//! on disk by a build whose decoding differs is never served.

use std::path::{Path, PathBuf};

/// What a kept decode is made by, from this crate's folder: its Rust, LibRaw's version, ONI's C
/// around it and how both are compiled. A folder covers every file it will ever hold.
pub const PARTS: [&str; 4] = ["src", "../oni-libraw/LIBRAW_VERSION", "../oni-libraw/csrc", "../oni-libraw/build.rs"];

/// Each file under `at`, hidden ones aside: a `.DS_Store` is no code.
fn files(at: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if at.is_file() {
        out.push(at.to_path_buf());
        return Ok(());
    }
    for entry in std::fs::read_dir(at)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with('.') {
            files(&entry.path(), out)?;
        }
    }
    Ok(())
}

/// FNV-1a on 128 bits over each file's name from `root`, written with `/`, its length and its bytes,
/// in the names' order: one tree prints alike in any folder of any machine. A part missing is an error.
pub fn stamp(root: &Path, parts: &[&str]) -> std::io::Result<String> {
    let mut all = Vec::new();
    for part in parts {
        files(&root.join(part), &mut all)?;
    }
    let mut named: Vec<(String, PathBuf)> = all
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let name = relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            (name, path)
        })
        .collect();
    named.sort();
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for (name, path) in &named {
        let bytes = std::fs::read(path)?;
        let length = (bytes.len() as u64).to_le_bytes();
        for b in name.as_bytes().iter().chain(&[0]).chain(&length).chain(&bytes) {
            h ^= u128::from(*b);
            h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
        }
    }
    Ok(format!("{h:032x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crate folder with nested sources and a sibling's file, under `name` in the temporary folder.
    fn tree(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("oni-decode-stamp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (file, text) in [("crate/src/lib.rs", "mod neutral;"), ("crate/src/neutral.rs", "const K: f32 = 5000.0;"), ("crate/src/m3/mul.rs", "fn mul() {}"), ("other/VERSION", "0.22.2"), ("other/unlisted", "x")] {
            let at = root.join(file);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        root
    }

    const LISTED: [&str; 2] = ["src", "../other/VERSION"];

    #[test]
    fn one_byte_changed_anywhere_listed_changes_the_stamp() {
        let root = tree("byte");
        let krate = root.join("crate");
        let first = stamp(&krate, &LISTED).unwrap();
        assert_eq!(stamp(&krate, &LISTED).unwrap(), first, "an unchanged tree keeps its stamp");
        for file in ["crate/src/m3/mul.rs", "other/VERSION"] {
            let at = root.join(file);
            let kept = std::fs::read(&at).unwrap();
            let mut moved = kept.clone();
            moved[0] ^= 1;
            std::fs::write(&at, moved).unwrap();
            assert_ne!(stamp(&krate, &LISTED).unwrap(), first, "{file} moved");
            std::fs::write(&at, kept).unwrap();
        }
        assert_eq!(stamp(&krate, &LISTED).unwrap(), first);
        // Twin: a file outside the parts, or a hidden one inside them, moves nothing.
        std::fs::write(root.join("other/unlisted"), "y").unwrap();
        std::fs::write(krate.join("src/.DS_Store"), "Finder").unwrap();
        assert_eq!(stamp(&krate, &LISTED).unwrap(), first);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_stamp_names_no_folder_of_the_machine() {
        let (here, there) = (tree("here"), tree("there-elsewhere"));
        assert_eq!(stamp(&here.join("crate"), &LISTED).unwrap(), stamp(&there.join("crate"), &LISTED).unwrap());
        // Twin: a file renamed with the same bytes is another tree.
        std::fs::rename(there.join("crate/src/neutral.rs"), there.join("crate/src/neutrals.rs")).unwrap();
        assert_ne!(stamp(&here.join("crate"), &LISTED).unwrap(), stamp(&there.join("crate"), &LISTED).unwrap());
        assert!(stamp(&here.join("crate"), &["../missing"]).is_err(), "a part not found stops the build");
        let _ = std::fs::remove_dir_all(&here);
        let _ = std::fs::remove_dir_all(&there);
    }

    #[test]
    fn the_built_stamp_covers_today_s_sources() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(env!("ONI_DECODE_STAMP"), stamp(root, &PARTS).unwrap());
        // Twin: the former list of five files prints otherwise, so the built one is not it.
        let five = ["../oni-libraw/csrc/pinned.c", "../oni-libraw/csrc/stream.cpp", "src/lib.rs", "src/source.rs", "src/camera.rs"];
        assert_ne!(env!("ONI_DECODE_STAMP"), stamp(root, &five).unwrap());
    }
}
