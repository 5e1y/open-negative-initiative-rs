//! The workspace's shape: no source file past `LIMIT` lines, so each stays one part an agent reads
//! whole. The files listed under `GRANDFATHERED` may only shrink until they are split.

use std::path::{Path, PathBuf};

const LIMIT: usize = 400;

/// Files already past the limit, each at its size today: a ceiling that only comes down, never a target.
const GRANDFATHERED: &[(&str, usize)] = &[
    ("crates/oni-app/src/app.rs", 519),
    ("crates/oni-app/src/bin/oni-tokens/tokens.rs", 639),
    ("crates/oni-app/src/editors/mod.rs", 559),
    ("crates/oni-app/src/editors/tests.rs", 466),
    ("crates/oni-app/src/export.rs", 424),
    ("crates/oni-app/tests/window/extended.rs", 409),
    ("crates/oni-core/src/parameters.rs", 445),
    ("crates/oni-decode/src/container.rs", 465),
    ("crates/oni-decode/tests/swift_checks.rs", 466),
    ("crates/oni-export/tests/files.rs", 470),
    ("crates/oni-graph/tests/graph.rs", 407),
    ("crates/oni-library/src/project.rs", 551),
    ("crates/oni-localsend/src/receiver.rs", 582),
    ("crates/oni-measure/src/registration.rs", 432),
    ("crates/oni-pipeline/tests/swift_checks.rs", 728),
    ("crates/oni-settings/src/json.rs", 521),
    ("crates/oni-settings/src/lighting_catch_up.rs", 504),
    ("crates/oni-settings/src/tone.rs", 479),
    ("crates/oni-settings/tests/codec_checks.rs", 735),
    ("crates/oni-ui/ui/app.slint", 431),
];

/// Each source file under `crates/` past its ceiling, with its line count.
fn breaches(root: &Path) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "target" && name != "libraw" {
                    stack.push(path);
                }
                continue;
            }
            if !["rs", "slint", "wgsl"].iter().any(|e| path.extension().is_some_and(|x| x == *e)) {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let lines = std::fs::read_to_string(&path).map_or(0, |t| t.lines().count());
            if lines > ceiling(&rel) {
                out.push((rel, lines));
            }
        }
    }
    out.sort();
    out
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
