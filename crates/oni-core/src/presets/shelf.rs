//! The presets folder: one file each, loaded, saved, deleted, shared, taken in, migrated once.

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

/// The single-file list of earlier versions, read once by the migration and never written.
pub fn legacy_file(support: &Path) -> PathBuf {
    support.join("presets.json")
}

/// The shelf as the app reads it: the single-file list taken over first, then every preset.
pub fn load_shelf(support: &Path) -> Vec<Preset> {
    migrate(&legacy_file(support), &directory(support));
    load(&directory(support))
}

/// Every readable preset, sorted by name then id; unreadable files are skipped.
pub fn load(directory: &Path) -> Vec<Preset> {
    let mut out: Vec<Preset> = files(directory).iter().filter_map(|f| decode(&std::fs::read(f).ok()?)).collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.to_string().cmp(&b.id.to_string())));
    out
}

/// Writes one preset, then removes any other file with its id, which makes a rename safe.
pub fn save(preset: &Preset, directory: &Path) -> io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let mut preset = preset.clone();
    preset.name = writable_name(&preset, directory);
    let path = directory.join(file_name(&preset));
    let text = encode(&preset).map_err(|e| io::Error::other(e.to_string()))?;
    let temporary = directory.join(format!(".{}.tmp", file_name(&preset)));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, &path)?;
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
pub fn shareable(preset: &Preset) -> Result<String, json::Error> {
    encode(preset)
}

/// Takes over the single-file list once, then archives it beside itself; idempotent, the files
/// deriving from ids, and the new files land before the old one moves.
pub fn migrate(legacy: &Path, directory: &Path) -> usize {
    let Some(list) = std::fs::read(legacy).ok().and_then(|d| serde_json::from_slice::<Value>(&d).ok()) else { return 0 };
    let Some(list) = list.as_array() else { return 0 };
    let mut migrated = 0;
    for element in list {
        let Ok(bytes) = serde_json::to_vec(element) else { continue };
        let Some(preset) = decode(&bytes) else { continue };
        if save(&preset, directory).is_ok() {
            migrated += 1;
        }
    }
    let mut archive = legacy.as_os_str().to_owned();
    archive.push(".migrated");
    let archive = PathBuf::from(archive);
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::rename(legacy, &archive);
    migrated
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
