//! The presets folder: one file each, loaded, saved, deleted, shared, taken in.

use super::*;

/// The preset files of a folder, by file name.
pub fn files(directory: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    found.sort();
    found
}

/// `<support>/Presets`, next to `library.json`: presets belong to no photo, so no sidecar holds them.
pub fn directory(support: &Path) -> PathBuf {
    support.join("Presets")
}

/// The shelf as the app reads it: the single-file list taken over first, then every preset.
pub fn load_shelf(support: &Path) -> Vec<Preset> {
    migrate(&legacy_file(support), &directory(support));
    load(&directory(support))
}

/// Every readable preset, sorted by name then id; unreadable files are skipped.
pub fn load(directory: &Path) -> Vec<Preset> {
    let mut out: Vec<Preset> = files(directory).iter().filter_map(|f| decode(&std::fs::read(f).ok()?)).collect();
    out.sort_by(|a, b| oni_settings::collation::case_insensitive(&a.name, &b.name).then(a.id.to_string().cmp(&b.id.to_string())));
    out
}

/// Writes one preset, then removes any other file with its id, which makes a rename safe.
pub fn save(preset: &Preset, directory: &Path) -> io::Result<()> {
    saved(preset, directory, &oni_settings::atomic::renamed)
}

/// `save` through its own rename, which a check refuses.
pub(super) fn saved(preset: &Preset, directory: &Path, rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let mut preset = preset.clone();
    preset.name = writable_name(&preset, directory);
    let path = directory.join(file_name(&preset));
    let text = encode(&preset).map_err(io::Error::other)?;
    oni_settings::atomic::written(&path, text.as_bytes(), rename)?;
    for other in files(directory).into_iter().filter(|f| *f != path) {
        // A stale twin that refuses to go leaves the save standing: the file just written is the preset.
        if std::fs::read(&other).ok().and_then(|d| decode(&d)).is_some_and(|p| p.id == preset.id) {
            let _ = std::fs::remove_file(&other);
        }
    }
    Ok(())
}

/// Hands every file carrying the preset's id to `discard`, which sends it to the system's trash;
/// by identifier, so a file renamed by hand goes too. A file that refuses is left where it is.
pub fn delete(preset: &Preset, directory: &Path, discard: &mut dyn FnMut(&Path) -> io::Result<()>) {
    for file in files(directory) {
        if std::fs::read(&file).ok().and_then(|d| decode(&d)).is_some_and(|p| p.id == preset.id) {
            let _ = discard(&file);
        }
    }
}

/// Takes preset files in from anywhere, as `adopting` does; unreadable files are skipped.
pub fn importing(paths: &[PathBuf], directory: &Path) -> Vec<Preset> {
    adopting(paths.iter().filter_map(|p| decode(&std::fs::read(p).ok()?)).collect(), directory)
}

/// What Share Preset… writes: the preset's own file.
pub fn shareable(preset: &Preset) -> Result<String, String> {
    encode(preset)
}

/// Takes presets in and never overwrites: one held already is skipped, one sharing an id with a
/// different preset comes in under an id of its own.
pub fn adopting(incoming: Vec<Preset>, directory: &Path) -> Vec<Preset> {
    let mut held = load(directory);
    let mut taken = Vec::new();
    for mut candidate in incoming {
        if held.iter().any(|h| h.same_look(&candidate)) {
            continue;
        }
        if held.iter().any(|h| h.id == candidate.id) {
            candidate.id = Id::new();
        }
        if save(&candidate, directory).is_ok() {
            held.push(candidate.clone());
            taken.push(candidate);
        }
    }
    taken
}
