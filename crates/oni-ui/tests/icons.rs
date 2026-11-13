//! `DSIcon.selfCheck`: every icon the interface names is a file of the tree it is drawn from, and
//! the gear names the app's Settings alone.

use std::path::{Path, PathBuf};

/// Each `.slint` file under `dir` with its text.
fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "slint") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push((path, text));
        }
    }
}

fn ui() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("ui"), &mut files);
    files
}

#[test]
fn every_icon_the_interface_names_is_present() {
    // Each `@image-url`, resolved against its own file.
    let icons: Vec<PathBuf> = ui()
        .iter()
        .flat_map(|(path, text)| text.split("@image-url(\"").skip(1).filter_map(|r| r.split('"').next()).map(|p| path.parent().unwrap().join(p)).collect::<Vec<_>>())
        .collect();
    let missing: Vec<&PathBuf> = icons.iter().filter(|p| !p.is_file()).collect();
    assert!(icons.len() > 30 && missing.is_empty(), "{} named, missing {missing:?}", icons.len());
    // Twin: a name with no file is caught.
    let ghost = icons[0].with_file_name("no-such-icon.svg");
    assert!(!ghost.is_file());
}

/// The files drawing the gear, by name.
fn gears(files: &[(PathBuf, String)]) -> Vec<String> {
    files.iter().filter(|(_, t)| t.contains("Icons.settings")).map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
}

#[test]
fn the_gear_names_the_apps_settings_alone() {
    let mut files = ui();
    assert_eq!(gears(&files), ["toolbar.slint"], "a block's options wear their own glyph");
    let options = files.iter().filter(|(_, t)| t.contains("Icons.options")).count();
    assert!(options >= 2, "the panel's and Prepare the Roll's options keys: {options}");
    // Twin: a block's key drawn with the gear is caught.
    files.push((PathBuf::from("parts.slint"), "icon: Icons.settings;".into()));
    assert_ne!(gears(&files), ["toolbar.slint"]);
}
