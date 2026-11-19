//! A photo's grain seed is its file's: kept through a rename and a move, another for another photo or a
//! byte changed where the print reads; each verdict with a twin.

use oni_settings::sidecar::identity;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-seed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 200 KB of bytes that tell one photo from another by `salt`.
fn photo(salt: u32) -> Vec<u8> {
    (0..200_000u32).map(|i| (i.wrapping_mul(2_654_435_761).wrapping_add(salt.wrapping_mul(97)) >> 13) as u8).collect()
}

#[test]
fn a_renamed_or_moved_photo_keeps_its_seed_and_another_photo_has_its_own() {
    let dir = scratch("kept");
    let (first, moved, renamed, other) = (dir.join("DSC_0001.NEF"), dir.join("roll").join("DSC_0001.NEF"), dir.join("Kept.NEF"), dir.join("DSC_0002.NEF"));
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::write(&first, photo(1)).unwrap();
    std::fs::write(&other, photo(2)).unwrap();
    let seed = identity::seed(&first).expect("a readable file has a seed");
    std::fs::copy(&first, &moved).unwrap();
    std::fs::rename(&first, &renamed).unwrap();
    assert_eq!(identity::seed(&moved), Some(seed), "a move keeps the seed");
    assert_eq!(identity::seed(&renamed), Some(seed), "a rename keeps the seed");
    assert_ne!(identity::seed(&other), Some(seed), "another photo shares the seed");
    // Twin: a seed salted with the file's name loses the photo's grain at a rename.
    let named = |p: &Path| identity::seed(p).map(|s| [s[0] ^ p.file_name().unwrap().len() as u32, s[1]]);
    assert_ne!(named(&renamed), named(&moved), "the twin keeps the seed through a rename");
    assert_eq!(identity::seed(&dir.join("absent.NEF")), None, "a file unread has no seed");
}

#[test]
fn a_byte_changed_where_the_print_reads_moves_the_seed() {
    let dir = scratch("byte");
    let (path, bytes) = (dir.join("P1000001.RW2"), photo(3));
    std::fs::write(&path, &bytes).unwrap();
    let seed = identity::seed(&path).unwrap();
    // The block at a half of the file, which the print reads.
    let mut changed = bytes.clone();
    changed[100_005] ^= 0x40;
    std::fs::write(&path, &changed).unwrap();
    assert_ne!(identity::seed(&path), Some(seed), "a byte in a read block keeps the seed");
    // A byte none of the three blocks reads is no other photo.
    let mut unread = bytes.clone();
    unread[10] ^= 0x40;
    std::fs::write(&path, &unread).unwrap();
    assert_eq!(identity::seed(&path), Some(seed));
}
