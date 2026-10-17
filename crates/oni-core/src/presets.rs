//! Named presets: a stance restricted to the rows it was saved from, one JSON file each, written as
//! a settings document plus a `preset` block so the codec's tolerance to absent keys reads it.

use crate::parameters;
use oni_settings::codec::{self, SettingsDocument, VERSION};
use oni_settings::json::{self, Node};
use oni_settings::{Id, PipelineSettings};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

/// A fixed set of card colours: recognition across a shelf needs distinct hues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tint {
    #[default]
    Slate,
    Rust,
    Amber,
    Moss,
    Teal,
    Indigo,
    Plum,
    Rose,
}

impl Tint {
    pub const ALL: [Tint; 8] =
        [Tint::Slate, Tint::Rust, Tint::Amber, Tint::Moss, Tint::Teal, Tint::Indigo, Tint::Plum, Tint::Rose];

    pub fn raw_value(self) -> &'static str {
        ["slate", "rust", "amber", "moss", "teal", "indigo", "plum", "rose"][self as usize]
    }

    pub fn from_raw(raw: &str) -> Option<Tint> {
        Tint::ALL.into_iter().find(|t| t.raw_value() == raw)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: Id,
    pub name: String,
    /// Rows and sliders carried, by name; empty is legal and applies nothing.
    pub parameters: BTreeSet<String>,
    pub tint: Tint,
    pub note: String,
    pub settings: PipelineSettings,
}

impl Preset {
    pub fn new(name: &str, parameters: BTreeSet<String>, tint: Tint, settings: PipelineSettings) -> Preset {
        Preset { id: Id::new(), name: name.to_string(), parameters, tint, note: String::new(), settings }
    }

    /// The card's monogram, derived so a rename cannot leave it stale.
    pub fn abbreviation(&self) -> String {
        let upper = self.name.to_uppercase();
        let kept: String = upper.chars().filter(|c| c.is_alphanumeric()).collect();
        let source = if kept.is_empty() { upper } else { kept };
        source.chars().take(3).collect()
    }

    /// Geometry is protected by being absent from the rows, never by a branch.
    pub fn apply(&self, destination: &PipelineSettings) -> PipelineSettings {
        parameters::apply(&self.parameters, &self.settings, destination)
    }

    /// A copy under a fresh identifier, so the two evolve apart and each keeps its own file.
    pub fn duplicated(&self) -> Preset {
        Preset { id: Id::new(), name: format!("{} copy", self.name), ..self.clone() }
    }

    /// Held already: same identity and the same effect.
    pub fn same_look(&self, other: &Preset) -> bool {
        let neutral = PipelineSettings::default();
        self.id == other.id
            && self.name == other.name
            && self.parameters == other.parameters
            && self.tint == other.tint
            && self.note == other.note
            && self.apply(&neutral) == other.apply(&neutral)
    }
}

/// The old per-tab groups, read off what applying them did then.
fn legacy_rows(group: &str) -> &'static [&'static str] {
    match group {
        "convert" => &["gains", "levelsPerChannel"],
        "look" => &["levelsGlobal", "curves", "spectrogram", "saturation"],
        "detail" => &["chroma", "sharpen"],
        _ => &[],
    }
}

/// The first free `Preset NN`, case and edge whitespace folded so near-duplicates count as taken.
pub fn default_name(names: &[&str]) -> String {
    let taken: BTreeSet<String> = names.iter().map(|n| n.trim().to_lowercase()).collect();
    let mut number = 1;
    while taken.contains(&format!("preset {number:02}")) {
        number += 1;
    }
    format!("Preset {number:02}")
}

/// Longest stem kept from a name, so a pasted paragraph cannot hit the file system's 255-byte cap.
const MAX_STEM: usize = 64;

/// A readable stem plus the id's head; identity is the id inside, so a rename never orphans a file.
pub fn file_name(preset: &Preset) -> String {
    let short: String = preset.id.to_string().replace('-', "").chars().take(8).collect::<String>().to_lowercase();
    format!("{}-{short}.json", slug(&preset.name))
}

fn slug(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        let c = if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' };
        if !(c == '-' && stem.ends_with('-')) {
            stem.push(c);
        }
    }
    let stem: String = stem.trim_matches('-').chars().take(MAX_STEM).collect();
    if stem.is_empty() { "preset".to_string() } else { stem }
}

pub fn encode(preset: &Preset) -> Result<String, json::Error> {
    let document = SettingsDocument {
        version: VERSION,
        imported_at: None,
        label: None,
        exif: None,
        roll_draft: None,
        trashed_at: None,
        settings: preset.settings.clone(),
    };
    let Node::Obj(mut entries) = json::to_node(&document)? else {
        return Err(json::Error("a settings document is an object".into()));
    };
    let meta = Node::Obj(vec![
        ("id".into(), Node::Str(preset.id.to_string())),
        ("name".into(), Node::Str(preset.name.clone())),
        ("parameters".into(), Node::Arr(preset.parameters.iter().cloned().map(Node::Str).collect())),
        ("tint".into(), Node::Str(preset.tint.raw_value().into())),
        ("note".into(), Node::Str(preset.note.clone())),
    ]);
    entries.push(("preset".into(), meta));
    Ok(json::pretty_sorted(&Node::Obj(entries)))
}

/// A preset from a file's bytes; a file from a newer version is skipped rather than half-applied.
pub fn decode(data: &[u8]) -> Option<Preset> {
    let decoded = codec::decode(data).ok().filter(|d| !d.is_from_future())?;
    let object: Value = serde_json::from_slice(data).ok()?;
    let meta = object.get("preset").filter(|m| m.is_object()).unwrap_or(&object);
    let name = meta.get("name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let id = meta.get("id").and_then(Value::as_str).and_then(Id::parse).unwrap_or_default();
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
    Some(Preset { id, name, parameters, tint, note, settings: decoded.settings })
}

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
    // A blank name hides a preset from `load`, so the name on disk is kept instead.
    if preset.name.trim().is_empty() {
        preset.name = load(directory).into_iter().find(|p| p.id == preset.id).map_or("Preset".into(), |p| p.name);
    } else {
        preset.name = preset.name.trim().to_string();
    }
    let path = directory.join(file_name(&preset));
    let text = encode(&preset).map_err(|e| io::Error::other(e.to_string()))?;
    let temporary = directory.join(format!(".{}.tmp", file_name(&preset)));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, &path)?;
    for other in files(directory).into_iter().filter(|f| *f != path) {
        if std::fs::read(&other).ok().and_then(|d| decode(&d)).is_some_and(|p| p.id == preset.id) {
            std::fs::remove_file(&other)?;
        }
    }
    Ok(())
}

/// Hands every file carrying the preset's id to `discard`, which sends it to the system's trash.
pub fn delete(preset: &Preset, directory: &Path, discard: &mut dyn FnMut(&Path) -> io::Result<()>) -> io::Result<()> {
    for file in files(directory) {
        if std::fs::read(&file).ok().and_then(|d| decode(&d)).is_some_and(|p| p.id == preset.id) {
            discard(&file)?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oni-core-presets-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> Preset {
        let mut settings = PipelineSettings::default();
        settings.sharpen.amount = 1.5;
        settings.geometry.angle = 3.0;
        Preset::new("Portra / soft", BTreeSet::from(["sharpen".to_string()]), Tint::Moss, settings)
    }

    #[test]
    fn a_preset_round_trips_through_its_file_and_a_rename_leaves_one_file() {
        let dir = folder("round");
        let mut preset = sample();
        save(&preset, &dir).unwrap();
        assert_eq!(load(&dir), vec![preset.clone()]);
        preset.name = "Renamed".into();
        save(&preset, &dir).unwrap();
        assert_eq!(files(&dir).len(), 1);
        assert_eq!(load(&dir)[0].name, "Renamed");
        // Twin: a duplicate is a second file, with an id of its own.
        save(&preset.duplicated(), &dir).unwrap();
        assert_eq!(files(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_preset_carries_its_rows_and_never_the_framing() {
        let preset = sample();
        let target = PipelineSettings::default();
        let laid = preset.apply(&target);
        assert_eq!((laid.sharpen.amount, laid.geometry.angle), (1.5, 0.0));
        // Twin: with no row ticked nothing moves.
        let empty = Preset { parameters: BTreeSet::new(), ..preset };
        assert_eq!(empty.apply(&target), target);
    }

    #[test]
    fn an_old_preset_reads_its_groups_and_its_retired_rows() {
        let mut doc: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
        doc["name"] = "Old".into();
        doc["groups"] = serde_json::json!(["detail", "nonsense"]);
        let read = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert_eq!(read.parameters, BTreeSet::from(["chroma".to_string(), "sharpen".to_string()]));
        doc["parameters"] = serde_json::json!(["fade", "saturation.shadows", "autoBalance.classic"]);
        let read = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert_eq!(read.parameters, BTreeSet::from(["autoBalance.filters".to_string(), "saturation".to_string()]));
        // Twin: a blank name is no preset.
        doc["name"] = "  ".into();
        assert!(decode(&serde_json::to_vec(&doc).unwrap()).is_none());
    }

    #[test]
    fn names_and_files_are_formed_as_the_shelf_expects() {
        assert_eq!(default_name(&["Preset 01", " preset 02 "]), "Preset 03");
        assert_eq!(default_name(&["Preset 02"]), "Preset 01");
        let p = sample();
        assert!(file_name(&p).starts_with("Portra-soft-"));
        assert_eq!(p.abbreviation(), "POR");
    }
}
