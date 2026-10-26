//! A profile file as an export reads it: a clear pixel converted to black, and a file replaced at
//! its path read again. Each against its twin.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use moxcms::{ColorProfile, ProfileClass};
use oni_colour::{OutputProfile, Proof, Quantised};

fn write_matrix(dir: &Path, name: &str, mut profile: ColorProfile, class: ProfileClass) -> PathBuf {
    profile.profile_class = class;
    let path = dir.join(name);
    fs::write(&path, profile.encode().unwrap()).unwrap();
    path
}

#[test]
fn a_clear_pixel_converts_to_black_through_a_profile() {
    let dir = common::sandbox("clear");
    let srgb = OutputProfile::new(write_matrix(&dir, "sRGB.icc", ColorProfile::new_srgb(), ProfileClass::DisplayDevice).to_string_lossy());
    let filled = |alpha: f32| {
        move |_: usize, _: usize, out: &mut [f32]| {
            for p in out.chunks_exact_mut(4) {
                p.copy_from_slice(&[0.8, 0.3, 0.1, alpha]);
            }
        }
    };
    let Some(Quantised::Sixteen(clear)) = srgb.converted(4, 3, 16, filled(0.0)) else { panic!() };
    assert!(clear.chunks(4).all(|p| p[..3] == [0, 0, 0] && p[3] == 65535), "{:?}", &clear[..4]);
    // Twin: the same colour opaque converts to itself, far from black.
    let Some(Quantised::Sixteen(opaque)) = srgb.converted(4, 3, 16, filled(1.0)) else { panic!() };
    assert!(opaque.chunks(4).all(|p| p[0] > 50000 && p[1] > 20000), "{:?}", &opaque[..4]);
}

#[test]
fn a_profile_replaced_at_its_path_is_read_again() {
    let dir = common::sandbox("replaced");
    let file = dir.join("Lab Lustre.icc");
    common::write_stand_in(&file, "Lab Lustre", 0.03, None, false);
    let paper = OutputProfile::new(file.to_string_lossy());
    let (space, cube) = (paper.loaded().unwrap(), Proof::cube(&paper, false).unwrap());
    // Twin: untouched, the file's parse and cube are the ones held.
    assert!(std::sync::Arc::ptr_eq(&space, &paper.loaded().unwrap()));
    assert!(std::sync::Arc::ptr_eq(&cube, &Proof::cube(&paper, false).unwrap()));
    common::write_stand_in(&file, "Lab Lustre", 0.08, None, false);
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    fs::File::options().write(true).open(&file).unwrap().set_modified(later).unwrap();
    assert_ne!(paper.loaded().unwrap().data, space.data, "the lab's new profile is the one converted into");
    assert_ne!(*Proof::cube(&paper, false).unwrap(), *cube, "and the one the canvas proofs");
}
