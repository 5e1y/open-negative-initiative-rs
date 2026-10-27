//! The reading's code as one print: every source the balance's reading runs through, so an outcome
//! kept by an older build is never served once that code moves.

use std::path::{Path, PathBuf};

/// Each crate the reading runs through, from the decode to the means, and the app's decode branch.
const READ: [&str; 13] = [
    "src",
    "../oni-pipeline/src",
    "../oni-render/src",
    "../oni-graph/src",
    "../oni-settings/src",
    "../oni-core/src/balance.rs",
    "../oni-decode/src",
    "../oni-decode/csrc",
    "../oni-decode/LIBRAW_VERSION",
    "../oni-app/src/render/decode.rs",
    "../oni-app/src/flat_fields.rs",
    "../oni-app/src/patches.rs",
    "../../Cargo.lock",
];

fn files(at: &Path, out: &mut Vec<PathBuf>) {
    if at.is_file() {
        out.push(at.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(at) else { return };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with('.') {
            files(&entry.path(), out);
        }
    }
}

fn main() {
    let mut all = Vec::new();
    for at in READ {
        println!("cargo:rerun-if-changed={at}");
        files(Path::new(at), &mut all);
    }
    all.sort();
    // FNV-1a on 128 bits, over each file's path and bytes.
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for path in &all {
        let bytes = std::fs::read(path).unwrap_or_default();
        for b in path.to_string_lossy().as_bytes().iter().chain(&bytes) {
            h ^= u128::from(*b);
            h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
        }
    }
    println!("cargo:rustc-env=ONI_READING_CODE={h:032x}");
}
