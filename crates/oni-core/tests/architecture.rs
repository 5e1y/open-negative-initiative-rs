//! The workspace's shape: no source file past `LIMIT` lines, so each stays one part an agent reads
//! whole. The files listed under `GRANDFATHERED` may only shrink until they are split.

use std::path::{Path, PathBuf};

const LIMIT: usize = 400;

/// Files already past the limit, each at its size today: a ceiling that only comes down, never a target.
const GRANDFATHERED: &[(&str, usize)] = &[
    ("crates/oni-app/src/app.rs", 462),
    ("crates/oni-app/src/editors/mod.rs", 525),
    ("crates/oni-app/src/editors/tests.rs", 466),
    ("crates/oni-app/tests/window/extended.rs", 409),
    ("crates/oni-decode/tests/swift_checks.rs", 466),
    ("crates/oni-export/tests/files.rs", 470),
    ("crates/oni-graph/tests/graph.rs", 404),
    ("crates/oni-localsend/src/receiver.rs", 560),
    ("crates/oni-measure/src/registration.rs", 432),
    ("crates/oni-pipeline/tests/swift_checks.rs", 728),
    ("crates/oni-settings/src/json.rs", 521),
    ("crates/oni-settings/src/lighting_catch_up.rs", 504),
    ("crates/oni-settings/src/tone.rs", 479),
    ("crates/oni-settings/tests/codec_checks.rs", 725),
];

/// A Rust line past this many characters is hard to read whole.
const WIDE: usize = 200;

/// Rust lines past `WIDE` today: a count that only falls, never a target.
const WIDE_LINES: usize = 86;

/// Every source file under `crates/`, as its path from `root` and its text.
fn sources(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                // The 2.5D lab is a sandbox of specimen sheets, never built into the app.
                if name != "target" && name != "libraw" && !path.ends_with("oni-ui/lab") {
                    stack.push(path);
                }
                continue;
            }
            if !["rs", "slint", "wgsl"].iter().any(|e| path.extension().is_some_and(|x| x == *e)) {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&path).unwrap_or_default()));
        }
    }
    out
}

/// Each source file under `crates/` past its ceiling, with its line count.
fn breaches(root: &Path) -> Vec<(String, usize)> {
    let mut out: Vec<_> = sources(root)
        .into_iter()
        .map(|(rel, text)| (rel, text.lines().count()))
        .filter(|(rel, lines)| *lines > ceiling(rel))
        .collect();
    out.sort();
    out
}

/// The Rust lines under `crates/` past `WIDE` characters.
fn wide_lines(root: &Path) -> usize {
    sources(root).iter().filter(|(rel, _)| rel.ends_with(".rs")).flat_map(|(_, text)| text.lines()).filter(|l| l.chars().count() > WIDE).count()
}

fn ceiling(rel: &str) -> usize {
    GRANDFATHERED.iter().find(|(p, _)| *p == rel).map_or(LIMIT, |(_, n)| (*n).max(LIMIT))
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn no_source_file_grows_past_its_ceiling() {
    let found = breaches(&workspace());
    assert!(found.is_empty(), "split these into one file per part: {found:?}");
}

#[test]
fn twin_a_file_past_the_limit_is_caught() {
    let root = std::env::temp_dir().join(format!("oni-architecture-{}", std::process::id()));
    let dir = root.join("crates/demo/src");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("long.rs"), "// line\n".repeat(LIMIT + 1)).unwrap();
    std::fs::write(dir.join("short.rs"), "// line\n".repeat(LIMIT)).unwrap();
    assert_eq!(breaches(&root), [("crates/demo/src/long.rs".to_string(), LIMIT + 1)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_more_lines_run_past_the_width_than_today() {
    let found = wide_lines(&workspace());
    assert!(found <= WIDE_LINES, "{found} lines past {WIDE} characters, {WIDE_LINES} at most: break the new ones");
    assert!(found >= WIDE_LINES, "{found} lines past {WIDE} characters: lower WIDE_LINES to it");
}

#[test]
fn twin_a_line_past_the_width_is_counted() {
    let root = std::env::temp_dir().join(format!("oni-architecture-wide-{}", std::process::id()));
    let dir = root.join("crates/demo/src");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("wide.rs"), format!("{}\n{}\n", "x".repeat(WIDE + 1), "x".repeat(WIDE))).unwrap();
    assert_eq!(wide_lines(&root), 1);
    let _ = std::fs::remove_dir_all(&root);
}
