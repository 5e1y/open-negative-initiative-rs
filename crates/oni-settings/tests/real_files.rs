//! The real sidecars: roll #8 (written by the current format) and two Lumix frames from version 14.

use oni_settings::codec::{self, Source};
use oni_settings::json::semantic_difference;
use oni_settings::sidecar::{self, ExifMetadata, RollDraft, WriteOptions};
use oni_settings::{ConversionMode, Date, Face, Id, Medal, PipelineSettings, ScannerMachine, roll_folder};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn sidecars(folder: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.ends_with(".NEF.json") || name.ends_with(".RW2.json")
        })
        .collect();
    out.sort();
    out
}

/// The sidecar re-encoded as `Sidecar.write` lays it: the envelope read back, the settings decoded.
fn rewritten(path: &Path, mosaic: bool) -> String {
    let data = std::fs::read(path).unwrap();
    let source = Source { mosaic: &|| mosaic, plain: &|| !mosaic };
    let decoded = codec::decode_with(&data, &source, true).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    codec::encode(&decoded.settings, decoded.imported_at, Some(decoded.label), decoded.exif, decoded.roll_draft, decoded.trashed_at)
        .unwrap()
}

fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// The sidecar as Swift writes it: without the key this app adds for the original it was written for.
fn swift_form(text: &str) -> String {
    let own = format!("  \"{}\" : ", sidecar::identity::KEY);
    text.split_inclusive('\n').filter(|line| !line.starts_with(&own)).collect()
}

#[test]
fn every_roll_8_sidecar_round_trips() {
    let Some(roll) = oni_fixtures::raw_dir("every_roll_8_sidecar_round_trips") else { return };
    let files = sidecars(&roll);
    assert_eq!(files.len(), 38, "roll #8 holds 38 sidecars");
    let mut identical = 0;
    for path in &files {
        let original = swift_form(&std::fs::read_to_string(path).unwrap());
        assert_eq!(value(&original)["version"], 19);
        let written = rewritten(path, true);
        if let Some(why) = semantic_difference(&value(&original), &value(&written)) {
            panic!("{}: {why}", path.display());
        }
        if written == original {
            identical += 1;
        } else {
            let line = original.lines().zip(written.lines()).position(|(a, b)| a != b);
            eprintln!("{}: first differing line {line:?}", path.display());
        }
    }
    eprintln!("roll #8: {} of {} sidecars byte-identical", identical, files.len());
    assert_eq!(identical, files.len());
}

/// Every key path where two documents part, numbers compared within an f32 epsilon.
fn differences(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = format!("{path}/{k}");
                match (x.get(k), y.get(k)) {
                    (Some(u), Some(v)) => differences(u, v, &p, out),
                    (Some(_), None) => out.push(format!("-{p}")),
                    (None, Some(_)) => out.push(format!("+{p}")),
                    (None, None) => {}
                }
            }
        }
        _ => {
            if semantic_difference(a, b).is_some() {
                out.push(format!("~{path}"));
            }
        }
    }
}

#[test]
fn twin_a_moved_field_in_a_roll_8_sidecar_is_seen() {
    let Some(roll) = oni_fixtures::raw_dir("twin_a_moved_field_in_a_roll_8_sidecar_is_seen") else { return };
    let path = roll.join("DSC_6442.NEF.json");
    let original = swift_form(&std::fs::read_to_string(&path).unwrap());
    let data = original.as_bytes();
    let decoded = codec::decode(data).unwrap();
    let encode = |settings: &PipelineSettings, label: Medal| {
        codec::encode(settings, decoded.imported_at, Some(label), decoded.exif.clone(), decoded.roll_draft.clone(), decoded.trashed_at)
            .unwrap()
    };
    // A gain moved by a thousandth of a stop, far past rounding.
    let mut moved = decoded.settings.clone();
    moved.gains.stops[0] += 1e-3;
    let why = semantic_difference(&value(&original), &value(&encode(&moved, decoded.label))).expect("a moved gain is seen");
    assert!(why.contains("gains"), "{why}");
    // A moved enum, a moved label, a dropped optional.
    let mut face = decoded.settings.clone();
    face.face = if face.face == Face::Pro { Face::Scanner } else { Face::Pro };
    assert!(semantic_difference(&value(&original), &value(&encode(&face, decoded.label))).is_some());
    assert!(semantic_difference(&value(&original), &value(&encode(&decoded.settings, Medal::Gold))).is_some());
    let mut dropped = decoded.settings.clone();
    assert!(dropped.geometry.measure_window.is_some());
    dropped.geometry.measure_window = None;
    let mut paths = Vec::new();
    differences(&value(&original), &value(&encode(&dropped, decoded.label)), "", &mut paths);
    assert_eq!(paths, ["-/settings/geometry/measureWindow"]);
    // Within one f32 ulp is rounding: the semantic check forgives it, the byte check does not.
    let mut ulp = decoded.settings.clone();
    ulp.gains.stops[0] = f32::from_bits(ulp.gains.stops[0].to_bits() + 1);
    let text = encode(&ulp, decoded.label);
    assert_ne!(text, original);
    assert!(semantic_difference(&value(&original), &value(&text)).is_none());
}

#[test]
fn neighbouring_f32s_spelt_shortest_are_rounding() {
    // An f32 and its neighbour one ulp past it, each spelt shortest, agree; twin: two ulps past it.
    assert_eq!(semantic_difference(&json!(-1.0909756), &json!(-1.0909758)), None);
    assert!(semantic_difference(&json!(-1.0909756), &json!(-1.0909759)).is_some());
}

#[test]
fn the_version_14_lumix_frames_read_the_migrations_they_are_due() {
    let Some(lumix) = oni_fixtures::lumix_dir("the_version_14_lumix_frames_read_the_migrations_they_are_due") else { return };
    let mut files = sidecars(&lumix);
    // The two calibration frames; the folder may hold other scans beside them.
    files.retain(|p| p.ends_with("P1072392.RW2.json") || p.ends_with("P1072404.RW2.json"));
    assert_eq!(files.len(), 2);
    for path in &files {
        let original = value(&std::fs::read_to_string(path).unwrap());
        assert_eq!(original["version"], 14);
        let written = value(&rewritten(path, true));
        let mut paths = Vec::new();
        differences(&original, &written, "", &mut paths);
        let mut expected = vec![
            // Retired keys the decoder no longer reads, folded at rest.
            "-/settings/curves",
            "-/settings/look",
            "-/settings/print/blacks",
            "-/settings/print/huntStrength",
            "-/settings/print/mids",
            "-/settings/print/whites",
            // Fields added since, laid in at their default.
            "+/settings/face",
            "+/settings/paperCorrection",
            "+/settings/primariesConverted",
            "+/settings/scanner",
            "+/settings/scannerFinish",
            // The Frontier the frame knew, and the literature's separation for an untouched 18 %.
            "+/settings/scannerMachine",
            "~/settings/separation",
            "~/version",
        ];
        expected.sort();
        paths.sort();
        assert_eq!(paths, expected, "{}", path.display());
        assert_eq!(written["settings"]["separation"], json!(0.6));
        assert_eq!(written["settings"]["scannerMachine"], json!("frontier"));
        assert_eq!(written["settings"]["primariesConverted"], json!(false));
        // Twin: the same document read beside a scan file keeps its converted primaries.
        let plain = value(&rewritten(path, false));
        assert_eq!(plain["settings"]["primariesConverted"], json!(true));
        // A second pass is a fixed point, byte for byte.
        let data = rewritten(path, true);
        let again = codec::decode(data.as_bytes()).unwrap();
        let text = codec::encode(&again.settings, again.imported_at, Some(again.label), again.exif, again.roll_draft, again.trashed_at)
            .unwrap();
        assert_eq!(text, data);
    }
    let decoded = codec::decode(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(decoded.settings.scanner_machine, ScannerMachine::Frontier);
}

#[test]
fn the_roll_8_marker_reads_and_writes_back() {
    let Some(roll) = oni_fixtures::raw_dir("the_roll_8_marker_reads_and_writes_back") else { return };
    let bytes = std::fs::read(roll.join(".oni-roll.json")).unwrap();
    let film = roll_folder::decode(&bytes).expect("the roll marker decodes");
    assert_eq!(film.name, "Roll #8");
    assert_eq!(film.stock.as_deref(), Some("fujifilm-fujicolor"));
    assert_eq!(film.id.to_string(), "BC402E4A-035E-4A3A-B279-FB7E35D1DDEA");
    let written = roll_folder::encode(&film);
    let original: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(semantic_difference(&original, &value(&written)).is_none(), "{written}");
    assert!(!written.contains('\n') && !written.contains(": "), "compact: {written}");
    assert_eq!(roll_folder::roll(&roll), Some(film.clone()));
    // Twin: a renamed roll is another document.
    let mut renamed = film.clone();
    renamed.name = "Roll #9".into();
    assert!(semantic_difference(&original, &value(&roll_folder::encode(&renamed))).is_some());
    // Every roll #8 frame's sidecar reads its film from the folder, never from itself.
    for path in sidecars(&roll) {
        assert_eq!(codec::envelope(&std::fs::read(&path).unwrap()).unwrap().film, None);
    }
}

// MARK: - The sidecar on disk

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A roll #8 sidecar beside an empty stand-in as a write there leaves it, with no medal, the camera's
/// lines and a draft: the sidecar is the truth, and a write needs an original beside it.
fn copied(roll: &Path, dir: &Path, frame: &str) -> (PathBuf, String) {
    let text = swift_form(&std::fs::read_to_string(roll.join(format!("{frame}.json"))).unwrap());
    let original = dir.join(frame);
    std::fs::write(&original, b"").unwrap();
    std::fs::write(sidecar::url_for(&original), &text).unwrap();
    let loaded = sidecar::read(&original, true, true).unwrap();
    let exif = ExifMetadata { camera: "NIKON CORPORATION NIKON Z f".into(), ..Default::default() };
    let options = WriteOptions {
        label: Some(Medal::None),
        exif: loaded.exif.is_none().then_some(exif),
        roll_draft: loaded.roll_draft.is_none().then(|| Some(base_draft())),
        ..Default::default()
    };
    sidecar::write(&loaded.settings, &original, options).unwrap();
    (original.clone(), std::fs::read_to_string(sidecar::url_for(&original)).unwrap())
}

/// The draft Swift leaves on a frame of a roll prepared up to its base step.
fn base_draft() -> RollDraft {
    serde_json::from_value(json!({
        "capture": "camera", "film": "negative", "flagged": false, "id": Id::new().to_string(), "passOrder": "bgr",
        "flatField": { "amount": 1, "mask": "None", "temperature": 0, "tint": 0 },
        "step": "base", "ticked": true, "window": [[0.1, 0.1], [0.8, 0.8]],
    }))
    .unwrap()
}

#[test]
fn a_sidecar_read_and_written_back_is_the_same_file() {
    let Some(roll) = oni_fixtures::raw_dir("a_sidecar_read_and_written_back_is_the_same_file") else { return };
    let dir = scratch("rewrite");
    let (original, text) = copied(&roll, &dir, "DSC_6442.NEF");
    assert_eq!(sidecar::url_for(&original), dir.join("DSC_6442.NEF.json"));
    let loaded = sidecar::read(&original, true, true).unwrap();
    assert!(loaded.writable && !loaded.is_new);
    assert!(loaded.roll_draft.is_some() && loaded.exif.is_some());
    // The settings alone are written: the envelope is read back from the file.
    sidecar::write(&loaded.settings, &original, WriteOptions::default()).unwrap();
    assert_eq!(std::fs::read_to_string(sidecar::url_for(&original)).unwrap(), text);
    // Twin: an update that changes nothing writes nothing, one that changes a gain writes it.
    assert!(!sidecar::update(&original, true, |s| s.clone()).unwrap());
    assert!(sidecar::update(&original, true, |s| {
        let mut s = s.clone();
        s.gains.stops[1] = 0.25;
        s
    })
    .unwrap());
    let after = std::fs::read_to_string(sidecar::url_for(&original)).unwrap();
    assert_ne!(after, text);
    assert_eq!(value(&after)["exif"], value(&text)["exif"]);
    assert_eq!(value(&after)["settings"]["gains"]["stops"][1], json!(0.25));
}

#[test]
fn label_trash_and_draft_touch_nothing_else_and_keep_the_revision() {
    let Some(roll) = oni_fixtures::raw_dir("label_trash_and_draft_touch_nothing_else_and_keep_the_revision") else { return };
    let dir = scratch("envelope");
    let (original, text) = copied(&roll, &dir, "DSC_6442.NEF");
    let path = sidecar::url_for(&original);
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    let revision = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    file.set_modified(revision).unwrap();
    drop(file);
    let modified = || std::fs::metadata(&path).unwrap().modified().unwrap();

    sidecar::set_label(Medal::Gold, &original, true).unwrap();
    let labelled = value(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(labelled["label"], json!("gold"));
    assert_eq!(modified(), revision);
    let mut paths = Vec::new();
    differences(&value(&text), &labelled, "", &mut paths);
    assert_eq!(paths, ["~/label"]);

    let stamp = Date::from_unix_seconds(1_760_000_000.0);
    assert!(sidecar::set_trashed(Some(stamp), &original, true).unwrap());
    let trashed = value(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(trashed["trashedAt"], json!(stamp.iso8601()));
    assert_eq!(modified(), revision);
    assert!(sidecar::set_trashed(None, &original, true).unwrap());
    sidecar::set_label(Medal::None, &original, true).unwrap();
    // Back where it started, byte for byte.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    sidecar::set_roll_draft(None, &original, true).unwrap();
    let cleared = value(&std::fs::read_to_string(&path).unwrap());
    assert!(cleared.get("rollDraft").is_none());
    // Twin: the cleared draft was really there.
    assert!(value(&text).get("rollDraft").is_some());
}

#[test]
fn a_newer_format_opens_read_only_and_is_never_rewritten() {
    let Some(roll) = oni_fixtures::raw_dir("a_newer_format_opens_read_only_and_is_never_rewritten") else { return };
    let dir = scratch("future");
    let (original, text) = copied(&roll, &dir, "DSC_6433.NEF");
    let path = sidecar::url_for(&original);
    let future = text.replacen("\"version\" : 19", "\"version\" : 20", 1);
    assert_ne!(future, text);
    std::fs::write(&path, &future).unwrap();
    let loaded = sidecar::read(&original, true, true).unwrap();
    assert!(!loaded.writable);
    assert!(!sidecar::update(&original, true, |s| PipelineSettings { density: 0.4, ..s.clone() }).unwrap());
    assert!(matches!(sidecar::set_label(Medal::Gold, &original, true), Err(sidecar::Error::Newer)), "the refusal is said");
    assert!(!sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &original, true).unwrap());
    assert!(matches!(sidecar::write(&loaded.settings, &original, Default::default()), Err(sidecar::Error::Newer)));
    assert_eq!(sidecar::flush(&PipelineSettings::default(), Some(&original), Some(&loaded.settings), loaded.writable), None);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), future);
    // Twin: today's version is writable.
    std::fs::write(&path, &text).unwrap();
    assert!(sidecar::read(&original, true, true).unwrap().writable);
}

#[test]
fn an_absent_sidecar_is_a_new_photo_and_an_unreadable_one_an_error() {
    let dir = scratch("absent");
    let original = dir.join("DSC_9999.NEF");
    std::fs::write(&original, b"").unwrap();
    let loaded = sidecar::read(&original, true, true).unwrap();
    assert!(loaded.is_new && loaded.writable);
    assert_eq!(loaded.settings, PipelineSettings::rest(ConversionMode::Negative, true));
    assert!(!sidecar::should_write(&loaded.settings, &original));
    sidecar::set_roll_draft(None, &original, true).unwrap();
    assert!(!sidecar::url_for(&original).exists(), "a cleared draft seeds no sidecar");
    // Twin: a moved setting is worth writing, and once written a return to rest is too.
    let mut moved = loaded.settings.clone();
    moved.density = 0.2;
    assert!(sidecar::should_write(&moved, &original));
    assert_eq!(sidecar::flush(&moved, Some(&original), Some(&loaded.settings), true), None);
    assert!(sidecar::url_for(&original).exists());
    assert!(sidecar::should_write(&loaded.settings, &original));
    let back = sidecar::read(&original, true, true).unwrap();
    assert_eq!(back.settings, moved);
    assert!(!back.is_new);

    std::fs::write(sidecar::url_for(&original), "not JSON").unwrap();
    assert!(sidecar::read(&original, true, true).is_err());
    assert!(sidecar::set_trashed(Some(Date::from_unix_seconds(1.0)), &original, true).is_err());
    assert_eq!(std::fs::read_to_string(sidecar::url_for(&original)).unwrap(), "not JSON");
    sidecar::remove(&original).unwrap();
    assert!(!sidecar::url_for(&original).exists());
    sidecar::remove(&original).unwrap();
}
