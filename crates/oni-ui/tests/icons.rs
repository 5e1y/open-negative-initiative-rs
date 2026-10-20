//! `DSIcon.selfCheck`: every icon the interface names is a file of the tree it is drawn from.

use std::path::{Path, PathBuf};

/// Each `@image-url` of the `.slint` files under `dir`, resolved against its own file.
fn named(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            named(&path, out);
        } else if path.extension().is_some_and(|e| e == "slint") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.extend(text.split("@image-url(\"").skip(1).filter_map(|r| r.split('"').next()).map(|p| path.parent().unwrap().join(p)));
        }
    }
}

#[test]
fn every_icon_the_interface_names_is_present() {
    let mut icons = Vec::new();
    named(&Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"), &mut icons);
    let missing: Vec<&PathBuf> = icons.iter().filter(|p| !p.is_file()).collect();
    assert!(icons.len() > 30 && missing.is_empty(), "{} named, missing {missing:?}", icons.len());
    // Twin: a name with no file is caught.
    let ghost = icons[0].with_file_name("no-such-icon.svg");
    assert!(!ghost.is_file());
}
