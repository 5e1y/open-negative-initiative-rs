//! Presets of earlier versions: a file of format 21 and under read through the settings' chain and
//! brought to this format in place, the single-file list of earlier versions taken over once.

use super::*;
use oni_settings::atomic;

/// The old per-tab groups, read off what applying them did then.
pub(super) fn legacy_rows(group: &str) -> &'static [&'static str] {
    match group {
        "convert" => &["gains", "levelsPerChannel"],
        "look" => &["levelsGlobal", "curves", "spectrogram", "saturation"],
        "detail" => &["chroma", "sharpen"],
        _ => &[],
    }
}

/// A preset file of format 21 and under: its settings through the chain, its block beside them or
/// at the top, its retired rows renamed; a newer one is skipped.
pub(super) fn earlier(data: &[u8], object: &Value) -> Option<Preset> {
    let settings = oni_settings::migrate::document(data).ok()?.document.settings;
    let meta = object.get("preset").filter(|m| m.is_object()).unwrap_or(object);
    let name = meta.get("name")?.as_str()?.trim_matches(is_blank).to_string();
    if name.is_empty() {
        return None;
    }
    let id = meta.get("id").and_then(Value::as_str).and_then(Id::parse).unwrap_or_else(|| Id::of_bytes(data));
    let parameters = match meta.get("parameters").and_then(Value::as_array) {
        // Rows naming stages that no longer render go; the two saturation zones are one curve.
        Some(written) => parameters::without_retired_methods(
            written
                .iter()
                .filter_map(Value::as_str)
                .filter(|p| !["contrast", "fade", "toneBalance", "colorCast"].contains(p))
                .map(|p| if p == "saturation.shadows" || p == "saturation.highlights" { "saturation" } else { p })
                .map(str::to_string)
                .collect(),
        ),
        None => meta
            .get("groups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .flat_map(|g| legacy_rows(g).iter().map(|r| r.to_string()))
            .collect(),
    };
    let tint = meta.get("tint").and_then(Value::as_str).and_then(Tint::from_raw).unwrap_or_default();
    let note = meta.get("note").and_then(Value::as_str).unwrap_or_default().to_string();
    Some(Preset { id, name, parameters, tint, note, settings })
}

/// Every preset file of `directory` an earlier version wrote, rewritten in this format: read back
/// and compared before one rename, left as it was on any refusal. How many were brought.
pub fn brought(directory: &Path) -> usize {
    let mut count = 0;
    for path in files(directory) {
        let Ok(old) = std::fs::read(&path) else { continue };
        let named = serde_json::from_slice::<Value>(&old).ok().is_some_and(|v| v.get("format").and_then(Value::as_str) == Some(FORMAT));
        let version = oni_settings::document::version_of(&old).unwrap_or(0);
        if (named && version == VERSION) || version > VERSION {
            continue;
        }
        let Some(preset) = decode(&old) else { continue };
        let Ok(text) = encode(&preset) else { continue };
        let verify = |back: &[u8]| decode(back).as_ref() == Some(&preset);
        let unmoved = || std::fs::read(&path).is_ok_and(|now| now == old);
        if atomic::write_verified(&path, text.as_bytes(), &verify, &unmoved, true).is_ok() {
            count += 1;
        }
    }
    count
}

/// The single-file list of earlier versions, archived whole by the takeover; it holds afterwards
/// only what the takeover could not save.
pub fn legacy_file(support: &Path) -> PathBuf {
    support.join("presets.json")
}

/// Takes the single-file list over. Only adds: an id on the shelf is left as the person keeps it,
/// and what fails to save goes back to the list for the next pass alone.
pub fn migrate(legacy: &Path, directory: &Path) -> usize {
    taken_over(legacy, directory, &atomic::renamed, &atomic::write_atomically)
}

/// `migrate` through its own rename and write, which a check refuses. The list leaves its place in
/// one rename before any preset is taken, so a refusal later can never bring a taken one back.
pub(in crate::presets) fn taken_over(legacy: &Path, directory: &Path, rename: &dyn Fn(&Path, &Path) -> io::Result<()>, write: &dyn Fn(&Path, &[u8]) -> io::Result<()>) -> usize {
    // The first list stays whole as the archive. A later one, written back by a pass or again by an
    // earlier version, is read then removed: the archive keeps the first.
    let archive = beside(legacy, "migrated");
    let staging = beside(legacy, "taking");
    let from = if archive.exists() { &staging } else { &archive };
    if rename(legacy, from).is_err() {
        return 0;
    }
    let list: Vec<Value> = std::fs::read(from).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default();
    if *from == staging {
        let _ = std::fs::remove_file(&staging);
    }
    let held: Vec<Id> = load(directory).into_iter().map(|p| p.id).collect();
    let mut migrated = 0;
    let mut failed = Vec::new();
    for element in &list {
        let Some(preset) = serde_json::to_vec(element).ok().and_then(|b| decode(&b)) else { continue };
        if held.contains(&preset.id) {
            continue;
        }
        match save(&preset, directory) {
            Ok(()) => migrated += 1,
            Err(_) => failed.push(element),
        }
    }
    if !failed.is_empty() {
        // A refused write leaves them in the archive alone.
        if let Ok(bytes) = serde_json::to_vec(&failed) {
            let _ = write(legacy, &bytes);
        }
    }
    migrated
}

/// `<legacy>.<suffix>`, beside it.
fn beside(legacy: &Path, suffix: &str) -> PathBuf {
    let mut name = legacy.as_os_str().to_owned();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}
