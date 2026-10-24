//! `GalleryHistory.selfCheck`'s scan: every gesture of the store that writes several photos at once
//! lays one gallery step, directly or through a helper that does, so ⌘Z always answers for it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The gestures on the selection or the roll, each owing ⌘Z one gallery step.
const BATCH: [&str; 13] = [
    "reset_frames",
    "apply_mode",
    "apply_preset",
    "auto_balance",
    "apply_flat_field",
    "join_roll",
    "set_film",
    "paste_settings",
    "paste_framing",
    "reset_to_default",
    "move_to_trash",
    "restore_from_trash",
    "land_roll",
];

/// Each method of the store's sources, by name, with its body up to the next declaration.
fn methods(texts: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for text in texts {
        let mut current: Option<(String, String)> = None;
        for line in text.lines() {
            let trimmed = line.trim_start();
            let declared = line.starts_with("    ") && !line.starts_with("     ") && trimmed.contains("fn ")
                && (trimmed.starts_with("fn ") || trimmed.starts_with("pub"));
            if declared || line.starts_with('}') {
                if let Some((name, body)) = current.take() {
                    out.insert(name, body);
                }
            }
            if declared {
                let name = trimmed.split("fn ").nth(1).unwrap_or("").split(['(', '<']).next().unwrap_or("").to_owned();
                current = Some((name, String::new()));
            } else if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        if let Some((name, body)) = current {
            out.insert(name, body);
        }
    }
    out
}

/// The batch gestures whose body never reaches a gallery record, through any chain of the store's own.
fn unrecorded(texts: &[String]) -> Vec<&'static str> {
    let methods = methods(texts);
    let mut recording: BTreeSet<&str> =
        methods.iter().filter(|(_, b)| b.contains("self.gallery.record")).map(|(n, _)| n.as_str()).collect();
    loop {
        let more: Vec<&str> = methods
            .iter()
            .filter(|(n, b)| !recording.contains(n.as_str()) && recording.iter().any(|r| b.contains(&format!("self.{r}("))))
            .map(|(n, _)| n.as_str())
            .collect();
        if more.is_empty() {
            break;
        }
        recording.extend(more);
    }
    BATCH.iter().copied().filter(|g| !recording.contains(g)).collect()
}

fn store_sources() -> Vec<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut texts = vec![std::fs::read_to_string(src.join("store.rs")).unwrap()];
    for entry in std::fs::read_dir(src.join("store")).unwrap().filter_map(Result::ok) {
        if entry.path().extension().is_some_and(|e| e == "rs") {
            texts.push(std::fs::read_to_string(entry.path()).unwrap());
        }
    }
    texts
}

#[test]
fn every_batch_gesture_of_the_store_lays_a_gallery_step() {
    let texts = store_sources();
    let names = methods(&texts);
    let absent: Vec<&str> = BATCH.iter().copied().filter(|g| !names.contains_key(*g)).collect();
    assert!(absent.is_empty(), "gestures no longer in the store: {absent:?}");
    assert_eq!(unrecorded(&texts), Vec::<&str>::new());
    // Twin: the same sources with every record call taken out name every gesture.
    let stripped: Vec<String> =
        texts.iter().map(|t| t.lines().filter(|l| !l.contains("gallery.record")).collect::<Vec<_>>().join("\n")).collect();
    assert_eq!(unrecorded(&stripped), BATCH.to_vec());
}
