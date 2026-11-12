//! A folder's pass touches only ONI's sidecars, asks an original nothing a step does not need, never
//! converts another file's settings with this file's facts, and a plain read writes nothing.

use oni_settings::migrate::{self, Facts, Mosaic, Outcome, Refusal, Standing, chain, reshape};
use oni_settings::sidecar::{self, url_for};
use oni_settings::{ConversionMode, Document, PipelineSettings};
use std::cell::Cell;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-sweep-pass-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An edited negative of a mosaic RAW, stored at `version`: one a camera's former matrix moves.
fn edited_at(original: &Path, version: i64) -> Vec<u8> {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.4;
    s.gains.stops = [0.1, 0.0, -0.2];
    let bytes = serde_json::to_vec_pretty(&reshape::as_stored(&Document::of(s), version)).unwrap();
    std::fs::write(url_for(original), &bytes).unwrap();
    bytes
}

fn importable(p: &Path) -> bool {
    p.extension().is_some_and(|e| ["nef", "tif"].contains(&e.to_string_lossy().to_lowercase().as_str()))
}

const MATRIX: migrate::M3 = [[1.1, -0.05, -0.05], [-0.02, 1.04, -0.02], [0.0, -0.1, 1.1]];

#[test]
fn a_pass_touches_only_a_sidecar_beside_a_photo_with_a_sidecars_marks() {
    let dir = scratch("scope");
    // Another program's settings beside its own file, and a photo's namesake JSON without the marks.
    let foreign_tool = br#"{"name":"my app","settings":{"theme":"dark","fontSize":14},"recent":["a","b"]}"#;
    std::fs::write(dir.join("tool"), b"binary").unwrap();
    std::fs::write(dir.join("tool.json"), foreign_tool).unwrap();
    std::fs::write(dir.join("DSC_0002.NEF"), b"").unwrap();
    let unmarked = br#"{"version":19,"settings":{"theme":"dark"}}"#;
    std::fs::write(dir.join("DSC_0002.NEF.json"), unmarked).unwrap();
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, b"").unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.2;
    std::fs::write(url_for(&photo), serde_json::to_vec(&reshape::as_stored(&Document::of(s), 21)).unwrap()).unwrap();
    let pass = migrate::folder(&dir, &importable, &|| true).unwrap();
    assert_eq!(std::fs::read(dir.join("tool.json")).unwrap(), foreign_tool, "another program's file untouched");
    assert_eq!(std::fs::read(dir.join("DSC_0002.NEF.json")).unwrap(), unmarked, "no sidecar's marks, untouched");
    // Twin: the photo's own sidecar is brought by the same pass.
    assert_eq!((pass.migrated, pass.refused.len()), (1, 0), "{pass:?}");
    assert_eq!(migrate::standing(&std::fs::read(url_for(&photo)).unwrap()), Standing::Current);
    // Twin: the converter alone takes the program's file for a sidecar, which the pass's rule prevents.
    let answering = Facts { mosaic: Mosaic::Known(false), matrix: &|| Ok(None), scan: &|_| Err(Refusal::ScanUnknown) };
    assert!(migrate::converted(foreign_tool, &answering).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_format_that_asks_no_fact_never_asks_the_original_and_the_oldest_ask_how_it_decodes() {
    let dir = scratch("lazy");
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, b"").unwrap();
    edited_at(&photo, chain::MATRICES_RETIRED_VERSION);
    let asked = Cell::new(0);
    let ask = || {
        asked.set(asked.get() + 1);
        Err(Refusal::Away)
    };
    let facts = Facts { mosaic: Mosaic::Asked(&ask), matrix: &|| Err(Refusal::Away), scan: &|_| Err(Refusal::Away) };
    assert_eq!(migrate::file_with(&photo, &facts), Ok(Outcome::Migrated { from: chain::MATRICES_RETIRED_VERSION }));
    assert_eq!(asked.get(), 0, "a format 20 sidecar asks its original nothing, so a cloud original stays in the cloud");
    // Twin: a format under 15 asks how the file decodes, and an original away leaves it as it was.
    let early = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/early/105-P1072740.RW2.json");
    let old = std::fs::read(&early).unwrap();
    std::fs::write(url_for(&photo), &old).unwrap();
    assert_eq!(migrate::file_with(&photo, &facts), Err(Refusal::Away));
    assert_eq!(asked.get(), 1);
    assert_eq!(std::fs::read(url_for(&photo)).unwrap(), old);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_read_writes_nothing_and_facts_never_asked_never_say_a_camera_had_no_matrix() {
    let dir = scratch("read-only");
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, b"").unwrap();
    let old = edited_at(&photo, chain::MATRICES_RETIRED_VERSION - 1);
    // No facts installed, as in a tool: the read shows the edits read-only and writes nothing.
    let read = sidecar::read(&photo, true).unwrap();
    assert_eq!((read.writable, read.held_back.clone()), (false, Some(Refusal::MatrixUnknown)));
    assert_eq!(std::fs::read(url_for(&photo)).unwrap(), old, "a read writes nothing");
    assert_eq!(migrate::file(&photo, Some(true)), Err(Refusal::MatrixUnknown), "never converted as a camera with no matrix");
    assert_eq!(std::fs::read(url_for(&photo)).unwrap(), old);
    // Twin: a table that answers converts it, the matrix kept in the gains.
    let facts = Facts { mosaic: Mosaic::Known(true), matrix: &|| Ok(Some(MATRIX)), scan: &|_| Err(Refusal::ScanUnknown) };
    assert_eq!(migrate::file_with(&photo, &facts), Ok(Outcome::Migrated { from: chain::MATRICES_RETIRED_VERSION - 1 }));
    assert_ne!(sidecar::read(&photo, true).unwrap().settings.gains, read.settings.gains);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A little-endian TIFF whose EXIF IFD holds `date`, then 200 KB of `fill`, as tests/foreign_sidecar.rs lays one.
fn shot(date: &str, fill: u8) -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    out.extend(8u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(0x8769u16.to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    out.extend(26u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(0x9003u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(20u32.to_le_bytes());
    out.extend(44u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(date.as_bytes());
    out.push(0);
    out.extend((0..200_000u32).map(|i| fill.wrapping_mul((i % 251) as u8)));
    out
}

#[test]
fn another_files_sidecar_is_never_converted_with_this_files_facts() {
    let dir = scratch("foreign");
    let photo = dir.join("DSC_0001.TIF");
    std::fs::write(&photo, shot("2024:05:01 10:00:00", 3)).unwrap();
    edited_at(&photo, chain::MATRICES_RETIRED_VERSION - 1);
    let mut stored: serde_json::Value = serde_json::from_slice(&std::fs::read(url_for(&photo)).unwrap()).unwrap();
    stored["originalPrint"] = sidecar::identity::of(&photo).unwrap().into();
    // The frame deleted, another body's namesake copied in: the sidecar is now another file's.
    std::fs::write(&photo, shot("2026:10:25 09:00:00", 5)).unwrap();
    let old = serde_json::to_vec_pretty(&stored).unwrap();
    std::fs::write(url_for(&photo), &old).unwrap();
    let facts = Facts { mosaic: Mosaic::Known(true), matrix: &|| Ok(Some(MATRIX)), scan: &|_| Err(Refusal::ScanUnknown) };
    assert_eq!(migrate::file_with(&photo, &facts), Err(Refusal::Foreign));
    assert_eq!(std::fs::read(url_for(&photo)).unwrap(), old, "left whole, to be set aside at the photo's first write");
    // Twin: the same sidecar beside its own photo takes this camera's matrix.
    stored.as_object_mut().unwrap().remove("originalPrint");
    std::fs::write(url_for(&photo), serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
    assert!(matches!(migrate::file_with(&photo, &facts), Ok(Outcome::Migrated { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_laying_its_own_settings_needs_no_matrix_and_keeps_the_envelope() {
    let dir = scratch("whole");
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, b"").unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.4;
    let mut stored = Document::of(s);
    stored.label = oni_settings::Medal::Gold;
    std::fs::write(url_for(&photo), serde_json::to_vec(&reshape::as_stored(&stored, chain::MATRICES_RETIRED_VERSION - 1)).unwrap()).unwrap();
    // A measure or a Reset lays settings of its own: no camera matrix is asked of them.
    let mut laid = PipelineSettings::rest(ConversionMode::Negative, true);
    laid.print.exposure = -0.3;
    sidecar::write(&laid, &photo, Default::default()).unwrap();
    let read = sidecar::read(&photo, true).unwrap();
    assert_eq!((read.settings, read.label, read.writable), (laid, oni_settings::Medal::Gold, true));
    assert_eq!(migrate::standing(&std::fs::read(url_for(&photo)).unwrap()), Standing::Current);
    // Twin: a label keeps the stored settings, which a matrix unknown holds back: refused, never written uncompensated.
    let other = dir.join("DSC_0002.NEF");
    std::fs::write(&other, b"").unwrap();
    let old = edited_at(&other, chain::MATRICES_RETIRED_VERSION - 1);
    assert!(matches!(sidecar::set_label(oni_settings::Medal::Silver, &other, true), Err(sidecar::Error::Held(Refusal::MatrixUnknown))));
    assert_eq!(std::fs::read(url_for(&other)).unwrap(), old);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_prepare_file_that_does_not_parse_never_blocks_a_frames_own_write() {
    let dir = scratch("drafts");
    let photo = dir.join("DSC_0001.NEF");
    std::fs::write(&photo, b"").unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.exposure = 0.2;
    let mut stored = reshape::as_stored(&Document::of(s.clone()), 21);
    let draft = serde_json::json!({ "id": oni_settings::Id::new(), "step": "flags", "film": "negative", "capture": "camera", "ticked": true, "flagged": false });
    stored["rollDraft"] = draft;
    std::fs::write(url_for(&photo), serde_json::to_vec(&stored).unwrap()).unwrap();
    let broken = b"{\"version\": 1, \"drafts\": [";
    std::fs::write(oni_settings::prepare::path(&dir), broken).unwrap();
    s.print.exposure = 0.5;
    sidecar::write(&s, &photo, Default::default()).unwrap();
    assert_eq!(sidecar::read(&photo, true).unwrap().settings.print.exposure, 0.5, "the frame's own edit written");
    assert_eq!(std::fs::read(dir.join(".oni-prepare (unreadable).json")).unwrap(), broken, "the broken file set aside whole");
    assert_eq!(oni_settings::prepare::read(&dir).len(), 1, "the frame's draft kept in a new file");
    // Twin: a Prepare file the disk refuses to read is still never written over, and blocks the write.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let at = oni_settings::prepare::path(&dir);
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000)).unwrap();
        let other = dir.join("DSC_0002.NEF");
        std::fs::write(&other, b"").unwrap();
        std::fs::write(url_for(&other), serde_json::to_vec(&stored).unwrap()).unwrap();
        let refused = sidecar::write(&s, &other, Default::default());
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(refused, Err(sidecar::Error::Held(Refusal::Drafts(_)))), "{refused:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
