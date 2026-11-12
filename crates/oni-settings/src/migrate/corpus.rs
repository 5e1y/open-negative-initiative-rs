//! The corpus: real sidecars of every era, scrubbed, each brought to this format in a scratch copy
//! with its original's facts, then read back to the reading its era's reader gave, text for text.

use super::{Facts, M3, Outcome, Refusal, file_with};
use crate::document;
use crate::geometry::Size;
use crate::prepare;
use crate::sidecar::{self, Loaded};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a sidecar's original says, as `<name>.facts.json` keeps it.
#[derive(Deserialize, Clone)]
pub(super) struct Kept {
    pub mosaic: bool,
    pub matrix: Option<M3>,
    pub scan: Option<(f64, f64)>,
}

impl Kept {
    pub fn facts<R>(&self, then: impl FnOnce(&Facts) -> R) -> R {
        let matrix = || Ok(self.matrix);
        let scan = |_: &crate::PipelineSettings| -> Result<Size, Refusal> { self.scan.ok_or(Refusal::ScanUnknown) };
        then(&Facts { mosaic: super::Mosaic::Known(self.mosaic), matrix: &matrix, scan: &scan })
    }
}

pub(super) fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Every sidecar under `folder`, one level of eras below it or none.
pub(super) fn sidecars(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for path in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                stack.push(path);
            } else if name.ends_with(".json") && !name.ends_with(".facts.json") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn stem(sidecar: &Path) -> String {
    sidecar.to_string_lossy().strip_suffix(".json").unwrap().to_owned()
}

pub(super) fn facts(sidecar: &Path) -> Kept {
    let path = format!("{}.facts.json", stem(sidecar));
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

pub(super) fn golden(sidecar: &Path) -> String {
    let path = format!("{}.reading.txt", stem(sidecar));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub(super) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-corpus-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `bytes` laid as the sidecar of an empty original in its own `Roll` folder under `dir`, as the
/// scrubbed drafts name their base's folder; the original's path.
pub(super) fn laid(dir: &Path, sidecar: &Path, bytes: &[u8]) -> PathBuf {
    let folder = dir.join(sidecar.file_name().unwrap()).join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    let original = folder.join(stem(sidecar).rsplit('/').next().unwrap());
    std::fs::write(&original, b"").unwrap();
    std::fs::write(sidecar::url_for(&original), bytes).unwrap();
    original
}

/// The reading of `original`'s sidecar as it now stands, its draft from its folder's file.
pub(super) fn reading(original: &Path, mosaic: bool) -> String {
    match sidecar::read(original, mosaic) {
        Ok(loaded) => said(&loaded, original),
        Err(e) => format!("refused: {e}\n"),
    }
}

pub(super) fn said(loaded: &Loaded, original: &Path) -> String {
    let drafts = prepare::read(original.parent().unwrap());
    let name = original.file_name().unwrap().to_string_lossy().into_owned();
    let draft = drafts.iter().find_map(|d| d.frames.get(&name).map(|m| (d, *m)));
    loaded.said(draft)
}

/// `bytes` brought to this format beside an empty original, then read: the outcome and the reading.
fn migrated(dir: &Path, sidecar: &Path, bytes: &[u8], kept: &Kept) -> (PathBuf, Result<Outcome, Refusal>, String) {
    let original = laid(dir, sidecar, bytes);
    let outcome = kept.facts(|f| file_with(&original, f));
    let reading = reading(&original, kept.mosaic);
    (original, outcome, reading)
}

fn first_apart(got: &str, want: &str) -> String {
    got.lines().zip(want.lines()).find(|(a, b)| a != b).map(|(a, b)| format!("{a} / {b}")).unwrap_or_default()
}

#[test]
fn every_corpus_file_migrates_and_reads_back_as_its_golden() {
    let dir = scratch("goldens");
    let files = sidecars(&corpus());
    assert_eq!(files.len(), 40, "the corpus holds forty sidecars");
    let mut wrong = Vec::new();
    for path in &files {
        let source = std::fs::read(path).unwrap();
        let (original, outcome, got) = migrated(&dir, path, &source, &facts(path));
        if got != golden(path) {
            wrong.push(format!("{}: {}", path.display(), first_apart(&got, &golden(path))));
        }
        let side = sidecar::url_for(&original);
        let bytes = std::fs::read(&side).unwrap();
        match outcome {
            Ok(Outcome::Migrated { .. }) => {
                // A fixed point: the text decodes and encodes back to itself.
                let again = document::encode(&document::decode(&bytes).unwrap()).unwrap();
                assert_eq!(again, bytes, "{}", path.display());
                let date = std::fs::metadata(&side).unwrap().modified().unwrap();
                assert_eq!(facts(path).facts(|f| file_with(&original, f)), Ok(Outcome::Current), "{}", path.display());
                assert_eq!((std::fs::read(&side).unwrap(), std::fs::metadata(&side).unwrap().modified().unwrap()), (bytes, date));
            }
            // A refused or newer file is byte for byte what it was.
            _ => assert_eq!(bytes, source, "{}: {outcome:?}", path.display()),
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_broken_or_newer_corpus_file_is_refused_with_its_reason() {
    let dir = scratch("broken");
    let mut seen = Vec::new();
    for path in sidecars(&corpus().join("broken")) {
        let source = std::fs::read(&path).unwrap();
        let (_, outcome, _) = migrated(&dir, &path, &source, &facts(&path));
        match outcome {
            Err(Refusal::Unreadable(_)) => seen.push("unreadable"),
            Err(Refusal::Newer(23)) => seen.push("newer"),
            Ok(Outcome::Migrated { .. }) => seen.push("migrated"),
            other => panic!("{}: {other:?}", path.display()),
        }
    }
    seen.sort();
    assert!(seen.contains(&"unreadable") && seen.contains(&"newer"), "{seen:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each Scanner-face frame of the corpus with its keypad as stored.
fn keypads() -> Vec<(PathBuf, Value)> {
    let read = |p: &PathBuf| serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).ok();
    let moved = |d: &Value| d["settings"]["scanner"].as_object().is_some_and(|s| s.values().any(|v| v.as_i64().is_some_and(|n| n != 0)));
    let files = sidecars(&corpus()).into_iter().filter(|p| !p.starts_with(corpus().join("broken")));
    files.filter_map(|p| Some((p.clone(), read(&p)?))).filter(|(_, d)| moved(d)).collect()
}

#[test]
fn twin_a_reshape_that_drops_the_half_steps_or_swaps_two_letters_is_caught() {
    let dir = scratch("twin-keypad");
    let halved: Vec<(PathBuf, Value)> = keypads().into_iter().filter(|(_, d)| d["settings"]["scanner"].get("halves").is_some()).collect();
    assert!(halved.len() >= 5, "the five sidecars with half steps, and their copies at 21");
    for (path, mut document) in halved {
        document["settings"]["scanner"].as_object_mut().unwrap().remove("halves");
        let (_, _, got) = migrated(&dir, &path, &serde_json::to_vec(&document).unwrap(), &facts(&path));
        assert_ne!(got, golden(&path), "{}", path.display());
    }
    let mut swapped = 0;
    for (path, mut document) in keypads() {
        let scanner = document["settings"]["scanner"].as_object_mut().unwrap();
        let (d, c) = (scanner.get("d").cloned().unwrap_or(Value::from(0)), scanner.get("c").cloned().unwrap_or(Value::from(0)));
        if d == c {
            continue;
        }
        scanner.insert("d".into(), c);
        scanner.insert("c".into(), d);
        let (_, _, got) = migrated(&dir, &path, &serde_json::to_vec(&document).unwrap(), &facts(&path));
        assert_ne!(got, golden(&path), "{}", path.display());
        swapped += 1;
    }
    assert!(swapped > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twin_a_field_written_under_another_name_is_no_fixed_point() {
    let dir = scratch("twin-name");
    let path = corpus().join("19-rust/040-P1073163.RW2.json");
    let (original, _, _) = migrated(&dir, &path, &std::fs::read(&path).unwrap(), &facts(&path));
    let text = String::from_utf8(std::fs::read(sidecar::url_for(&original)).unwrap()).unwrap();
    assert!(text.contains("\"exifLine\""), "a line set by hand");
    let renamed = text.replace("\"exifLine\"", "\"exif_line\"");
    let decoded = document::decode(renamed.as_bytes()).unwrap();
    assert_ne!(document::encode(&decoded).unwrap(), renamed.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twin_a_golden_with_one_digit_changed_is_caught() {
    let dir = scratch("twin-digit");
    let file = corpus().join("19-plain/347-DSC_6452.NEF.json");
    let golden = golden(&file);
    let at = golden.find(|c: char| c.is_ascii_digit() && c != '9').unwrap();
    let mut changed = golden.clone();
    changed.replace_range(at..at + 1, "9");
    let (_, _, got) = migrated(&dir, &file, &std::fs::read(&file).unwrap(), &facts(&file));
    assert_eq!(got, golden);
    assert_ne!(got, changed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twin_a_facts_file_without_its_matrix_fails_on_the_gains() {
    let dir = scratch("twin-matrix");
    let file = corpus().join("19-plain/347-DSC_6452.NEF.json");
    let golden = golden(&file);
    let mut bare = facts(&file);
    assert!(bare.matrix.take().is_some(), "an edited negative of a camera the table holds");
    let (_, _, got) = migrated(&dir, &file, &std::fs::read(&file).unwrap(), &bare);
    let first = golden.lines().zip(got.lines()).position(|(a, b)| a != b).expect("the matrix moves the gains");
    let gains = golden.find("gains: Gains").unwrap();
    assert!(golden.lines().take(first).map(|l| l.len() + 1).sum::<usize>() > gains, "the first line apart is in the gains");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Each kind of sidecar found on the machine the corpus was drawn from stays represented.
#[test]
fn the_corpus_holds_every_kind_of_sidecar() {
    let documents: Vec<Value> = sidecars(&corpus()).iter().filter_map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).ok()).collect();
    let settings = |d: &Value| d.get("settings").cloned().unwrap_or(Value::Null);
    let count = |test: &dyn Fn(&Value) -> bool| documents.iter().filter(|d| test(d)).count();
    let kinds: [(&str, &dyn Fn(&Value) -> bool); 13] = [
        ("version 5", &|d| d["version"] == 5),
        ("version 14", &|d| d["version"] == 14),
        ("version 19", &|d| d["version"] == 19),
        ("version 20", &|d| d["version"] == 20),
        ("version 21", &|d| d["version"] == 21),
        ("a roll draft", &|d| d.get("rollDraft").is_some()),
        ("scanner halves", &|d| settings(d)["scanner"].get("halves").is_some()),
        ("the Scanner face", &|d| settings(d)["face"] == "scanner"),
        ("the Frontier", &|d| settings(d)["scannerMachine"] == "frontier"),
        ("a slide", &|d| settings(d)["mode"] == "positive"),
        ("monochrome", &|d| settings(d)["mode"] == "monochrome"),
        ("a contact sheet", &|d| settings(d).get("sheet").is_some()),
        ("a per-frame roll", &|d| settings(d).get("film").is_some()),
    ];
    for (kind, test) in kinds {
        assert!(count(test) > 0, "no sidecar with {kind}");
    }
    let painted = |d: &Value| settings(d)["corrections"].as_array().is_some_and(|c| c.iter().any(|c| c["stroke"].as_array().is_some_and(|s| !s.is_empty())));
    let cropped = |d: &Value| painted(d) && settings(d)["geometry"]["crop"] != serde_json::json!([[0, 0], [1, 1]]);
    assert!(count(&painted) >= 3 && count(&cropped) >= 1, "dust strokes, one cropped");
}

/// The corpus is public: no date in it says when a person imported or trashed a photo.
#[test]
fn the_corpus_dates_no_ones_work() {
    let fixed = |d: &Value| ["importedAt", "trashedAt"].iter().all(|k| d.get(*k).is_none_or(|v| v == "2000-01-01T00:00:00Z" || v == "2000-01-02T00:00:00Z"));
    let documents: Vec<(PathBuf, Value)> = sidecars(&corpus()).into_iter().filter_map(|p| Some((p.clone(), serde_json::from_slice(&std::fs::read(&p).ok()?).ok()?))).collect();
    let dated: Vec<&PathBuf> = documents.iter().filter(|(_, d)| !fixed(d)).map(|(p, _)| p).collect();
    assert!(dated.is_empty(), "scripts/corpus-scrub leaves a date in {dated:?}");
    assert!(documents.iter().any(|(_, d)| d.get("trashedAt").is_some()), "a trashed photo stays represented");
    // Twin: a date of the day the photo was imported is caught.
    assert!(!fixed(&serde_json::json!({ "importedAt": "2026-09-21T15:32:38Z" })));
}

/// The wide corpus: every sidecar of the machine it was drawn from, kept outside the repository and
/// read only; each brought to this format in a scratch copy.
#[test]
fn the_wide_corpus_migrates_to_its_goldens() {
    let (Some(corpus), Some(goldens)) = (std::env::var_os("ONI_SIDECAR_CORPUS"), std::env::var_os("ONI_SIDECAR_GOLDENS")) else {
        println!("SKIPPED the_wide_corpus_migrates_to_its_goldens: set ONI_SIDECAR_CORPUS and ONI_SIDECAR_GOLDENS");
        return;
    };
    let (goldens, dir) = (PathBuf::from(goldens), scratch("wide"));
    let files = sidecars(Path::new(&corpus));
    let mut wrong = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().strip_suffix(".json").unwrap().to_owned();
        let want = std::fs::read_to_string(goldens.join(format!("{name}.reading.txt"))).unwrap();
        let kept = facts(path);
        let (original, outcome, got) = migrated(&dir, path, &std::fs::read(path).unwrap(), &kept);
        if got != want {
            wrong.push(format!("{name}: {}", first_apart(&got, &want)));
        }
        if matches!(outcome, Ok(Outcome::Migrated { .. })) {
            assert_eq!(kept.facts(|f| file_with(&original, f)), Ok(Outcome::Current));
        }
    }
    assert!(wrong.is_empty(), "{} of {}: {wrong:#?}", wrong.len(), files.len());
    println!("wide corpus: {} sidecars migrated to their goldens", files.len());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The calls Rust hands to each system's own libm, whose last bit differs from one system to the next.
const SYSTEM_MATHS: [&str; 22] = [
    "powf", "powi", "exp", "exp2", "exp_m1", "ln", "ln_1p", "log", "log2", "log10", "sin", "cos", "tan", "sin_cos", "asin", "acos",
    "atan", "atan2", "sinh", "cosh", "tanh", "cbrt",
];

/// Each line of `sources` calling the system's maths, past the `#[cfg(test)]` module each file ends on.
fn system_maths(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (path, text) in sources {
        let lines: Vec<&str> = text.lines().collect();
        let end = lines.windows(2).position(|w| w[0].trim() == "#[cfg(test)]" && w[1].starts_with("mod ") && w[1].ends_with('{'));
        for (n, line) in lines[..end.unwrap_or(lines.len())].iter().enumerate() {
            let calls = |m: &&str| line.contains(&format!(".{m}(")) || ["f32", "f64"].iter().any(|t| line.contains(&format!("{t}::{m}")));
            if SYSTEM_MATHS.iter().any(calls) {
                out.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
            }
        }
    }
    out
}

/// The crate's sources, the files of test modules declared `mod name;` left out.
fn crate_sources() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for path in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let tests: Vec<PathBuf> = files
        .iter()
        .flat_map(|(path, text)| {
            let lines: Vec<&str> = text.lines().collect();
            let folder = if path.ends_with("lib.rs") { path.with_file_name("") } else { path.with_extension("") };
            let declared = lines.windows(2).filter(|w| w[0].trim() == "#[cfg(test)]").filter_map(|w| w[1].strip_prefix("mod ")?.strip_suffix(';'));
            declared.map(|name| folder.join(format!("{name}.rs"))).collect::<Vec<_>>()
        })
        .collect();
    files.into_iter().filter(|(path, _)| !tests.contains(path)).collect()
}

/// A sidecar reads the same on every system only through maths that round alike everywhere: a fit
/// turns one last bit into the third decimal of a print, so the reader calls `libm` alone.
#[test]
fn the_reader_rounds_alike_on_every_system() {
    let sources = crate_sources();
    assert!(sources.iter().any(|(p, _)| p.ends_with("migrate/lighting.rs")) && !sources.iter().any(|(p, _)| p.ends_with("migrate/corpus.rs")));
    let found = system_maths(&sources);
    assert!(found.is_empty(), "the system's maths in the reader: {found:#?}");
    // Twin: the system's `powf` planted in a function, and `exp2` passed by name, are caught; a test module's are not.
    let planted = concat!(
        "fn lit(x: f32) -> f32 {\n    x.powf(2.2)\n}\n",
        "fn gains(s: [f32; 3]) -> [f32; 3] {\n    s.map(f32::exp2)\n}\n",
        "#[cfg(test)]\nmod tests {\n    fn f(x: f32) -> f32 {\n        x.ln()\n    }\n}\n",
    );
    assert_eq!(system_maths(&[(PathBuf::from("planted.rs"), planted.into())]).len(), 2);
}
