//! The corpus: real sidecars of every era, scrubbed, each read with its original's facts to the
//! reading this format's reader gave when it was frozen, text for text.

use super::{Facts, M3, converted};
use crate::sidecar::Loaded;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a sidecar's original says, as `<name>.facts.json` keeps it.
#[derive(Deserialize, Clone)]
struct Kept {
    mosaic: bool,
    matrix: Option<M3>,
    scan: Option<(f64, f64)>,
}

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Every sidecar under `folder`, one level of eras below it or none.
fn sidecars(folder: &Path) -> Vec<PathBuf> {
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

fn facts(sidecar: &Path) -> Kept {
    let path = format!("{}.facts.json", stem(sidecar));
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

/// The reading of `sidecar` given `kept`, or the reader's refusal, as `examples/reading.rs` prints it.
fn reading(sidecar: &Path, kept: &Kept) -> String {
    let bytes = std::fs::read(sidecar).unwrap();
    let facts = Facts { mosaic: kept.mosaic, matrix: &|| kept.matrix, scan: &|_| kept.scan };
    match converted(&bytes, &facts) {
        Ok(c) => Loaded::of(c).said(),
        Err(e) => format!("refused: {e}\n"),
    }
}

/// The files of `sidecars` whose reading is not their golden in `goldens`, with the first line apart.
fn astray(sidecars: &[PathBuf], golden: impl Fn(&Path) -> PathBuf) -> Vec<String> {
    let mut out = Vec::new();
    for path in sidecars {
        let want = std::fs::read_to_string(golden(path)).unwrap_or_else(|e| panic!("{}: {e}", golden(path).display()));
        let got = reading(path, &facts(path));
        if got != want {
            let line = got.lines().zip(want.lines()).find(|(a, b)| a != b).map(|(a, b)| format!("{a} / {b}")).unwrap_or_default();
            out.push(format!("{}: {line}", path.display()));
        }
    }
    out
}

fn beside(sidecar: &Path) -> PathBuf {
    PathBuf::from(format!("{}.reading.txt", stem(sidecar)))
}

#[test]
fn every_corpus_file_reads_as_its_golden_through_migrate() {
    let files = sidecars(&corpus());
    assert_eq!(files.len(), 40, "the corpus holds forty sidecars");
    let wrong = astray(&files, beside);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn twin_a_golden_with_one_digit_changed_is_caught() {
    let file = corpus().join("19-plain/347-DSC_6452.NEF.json");
    let golden = std::fs::read_to_string(beside(&file)).unwrap();
    let at = golden.find(|c: char| c.is_ascii_digit() && c != '9').unwrap();
    let mut changed = golden.clone();
    changed.replace_range(at..at + 1, "9");
    let got = reading(&file, &facts(&file));
    assert_eq!(got, golden);
    assert_ne!(got, changed);
}

#[test]
fn twin_a_facts_file_without_its_matrix_fails_on_the_gains() {
    let file = corpus().join("19-plain/347-DSC_6452.NEF.json");
    let golden = std::fs::read_to_string(beside(&file)).unwrap();
    let mut bare = facts(&file);
    assert!(bare.matrix.take().is_some(), "an edited negative of a camera the table holds");
    let got = reading(&file, &bare);
    let differing: Vec<&str> = got.lines().zip(golden.lines()).filter(|(a, b)| a != b).map(|(a, _)| a).collect();
    assert!(!differing.is_empty(), "the matrix moves the gains");
    let gains = golden.find("gains: Gains").unwrap();
    let first = golden.lines().zip(got.lines()).position(|(a, b)| a != b).unwrap();
    assert!(golden.lines().take(first).map(|l| l.len() + 1).sum::<usize>() > gains, "the first line apart is in the gains");
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

/// The wide corpus: every sidecar of the machine it was drawn from, kept outside the repository.
#[test]
fn the_wide_corpus_reads_as_its_goldens() {
    let (Some(corpus), Some(goldens)) = (std::env::var_os("ONI_SIDECAR_CORPUS"), std::env::var_os("ONI_SIDECAR_GOLDENS")) else {
        println!("SKIPPED the_wide_corpus_reads_as_its_goldens: set ONI_SIDECAR_CORPUS and ONI_SIDECAR_GOLDENS");
        return;
    };
    let goldens = PathBuf::from(goldens);
    let files = sidecars(Path::new(&corpus));
    let wrong = astray(&files, |p| goldens.join(format!("{}.reading.txt", p.file_name().unwrap().to_string_lossy().strip_suffix(".json").unwrap())));
    assert!(wrong.is_empty(), "{} of {}: {wrong:#?}", wrong.len(), files.len());
    println!("wide corpus: {} sidecars read as their goldens", files.len());
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
