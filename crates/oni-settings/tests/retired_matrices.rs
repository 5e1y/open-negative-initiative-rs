//! A sidecar written while a camera matrix shaped the decode takes that matrix's move on its film's
//! base into its gains, at its first read; written back, it is never moved again.

use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings, codec};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MATRIX: [[f64; 3]; 3] = [[1.1242, -0.1665, -0.0092], [0.0277, 0.9493, -0.0627], [0.0104, -0.0133, 0.8966]];

fn aged(original: &Path, version: i64) {
    let path = sidecar::url_for(original);
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["version"] = serde_json::json!(version);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn a_sidecar_from_under_a_camera_matrix_keeps_its_look_through_its_gains_once() {
    let dir = std::env::temp_dir().join(format!("oni-retired-matrices-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (matched, other): (PathBuf, PathBuf) = (dir.join("P1070001.RW2"), dir.join("DSC_0001.ARW"));
    let held = matched.clone();
    oni_settings::retired_matrices::install(Some(Arc::new(move |p: &Path| (p == held).then_some(MATRIX))));
    let mut edited = PipelineSettings::rest(ConversionMode::Negative, true);
    edited.print.exposure = 0.4;
    edited.gains.stops = [0.1, 0.0, -0.2];
    for original in [&matched, &other] {
        std::fs::write(original, b"raw").unwrap();
        sidecar::write(&edited, original, WriteOptions::default()).unwrap();
        aged(original, codec::MATRICES_RETIRED_VERSION - 1);
    }
    let read = sidecar::read(&matched, true, true).unwrap().settings;
    let by = oni_settings::retired_matrices::on_base(&edited, &MATRIX).unwrap();
    assert!(by.iter().all(|v| v.abs() > 0.01), "{by:?}");
    assert!((0..3).all(|c| (read.gains.stops[c] - edited.gains.stops[c] - by[c]).abs() < 1e-6), "{:?}", read.gains);
    // Twin: a camera without a matrix keeps its gains.
    assert_eq!(sidecar::read(&other, true, true).unwrap().settings, edited);
    // Written back, the file holds the moved gains and a later read moves them no further.
    sidecar::write(&read, &matched, WriteOptions::default()).unwrap();
    assert_eq!(sidecar::read(&matched, true, true).unwrap().settings, read);
    oni_settings::retired_matrices::install(None);
    let _ = std::fs::remove_dir_all(&dir);
}
