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

    /// The swatch's name, the raw value capitalised.
    pub fn label(self) -> String {
        let raw = self.raw_value();
        raw[..1].to_uppercase() + &raw[1..]
    }

    /// Components rather than a colour, so the ink is derived.
    pub fn components(self) -> [f64; 3] {
        match self {
            Tint::Slate => [0.38, 0.42, 0.48],
            Tint::Rust => [0.72, 0.34, 0.22],
            Tint::Amber => [0.87, 0.67, 0.20],
            Tint::Moss => [0.45, 0.58, 0.31],
            Tint::Teal => [0.20, 0.56, 0.56],
            Tint::Indigo => [0.35, 0.38, 0.72],
            Tint::Plum => [0.53, 0.31, 0.56],
            Tint::Rose => [0.83, 0.48, 0.55],
        }
    }

    /// Whether black reads better than white on the card, comparing the two contrasts.
    pub fn wants_dark_ink(self) -> bool {
        self.contrast_against(0.0) >= self.contrast_against(1.0)
    }

    fn contrast_against(self, text: f64) -> f64 {
        let fill = luminance(self.components());
        let (high, low) = if fill > text { (fill, text) } else { (text, fill) };
        (high + 0.05) / (low + 0.05)
    }

    /// The contrast of the ink the card writes in.
    pub fn contrast(self) -> f64 {
        self.contrast_against(0.0).max(self.contrast_against(1.0))
    }
}

/// Rec. 709 weights on linearised sRGB, for display legibility.
pub fn luminance(c: [f64; 3]) -> f64 {
    let linear = |v: f64| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
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

    /// The card's help: the name, then its description when it has one.
    pub fn help(&self) -> String {
        if self.note.is_empty() { self.name.clone() } else { format!("{} \u{2014} {}", self.name, self.note) }
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

/// The settings a hovered card shows on the canvas; never written, so a hover makes no step.
pub fn previewed(preset: Option<&Preset>, settings: &PipelineSettings) -> PipelineSettings {
    preset.map_or_else(|| settings.clone(), |p| p.apply(settings))
}

/// `CharacterSet.whitespaces`: tabs and space separators, never a line break.
fn is_blank(c: char) -> bool {
    c == '\t' || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

/// The first free `Preset NN`, two digits widening past 99, case and edge spaces folded so
/// near-duplicates count as taken.
pub fn default_name(names: &[&str]) -> String {
    let taken: BTreeSet<String> = names.iter().map(|n| n.trim_matches(is_blank).to_lowercase()).collect();
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
    let name = meta.get("name")?.as_str()?.trim_matches(is_blank).to_string();
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

/// The name trimmed, or the one on disk when it trims to nothing: `load` refuses a blank name,
/// so writing one would hide the preset and orphan its file.
pub fn writable_name(preset: &Preset, directory: &Path) -> String {
    let trimmed = preset.name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    files(directory)
        .iter()
        .filter_map(|f| decode(&std::fs::read(f).ok()?))
        .find(|p| p.id == preset.id)
        .map_or_else(|| "Preset".to_string(), |p| p.name)
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

// MARK: - The presets earlier versions laid down

/// The presets the Swift app once laid on every machine, kept only to be recognised.
pub const RETIRED: [&[u8]; 5] = [
    include_bytes!("../retired/Azure-3676f1b6.json"),
    include_bytes!("../retired/Cinestill-800T-16aa3e97.json"),
    include_bytes!("../retired/Gold-200-71bad116.json"),
    include_bytes!("../retired/Portra-400-795c6944.json"),
    include_bytes!("../retired/Redscale-b04c5354.json"),
];

/// The preference recording that shipped presets were taken back, so it runs once.
pub const RETIRED_KEY: &str = "retiredShippedPresets";

pub fn retired() -> Vec<Preset> {
    RETIRED.iter().filter_map(|bytes| decode(bytes)).collect()
}

/// Sends to the trash every held preset an earlier version laid down that still carries its name
/// and its look; renamed or retuned, a preset belongs to whoever has it. `done` is the record.
pub fn retire_shipped(
    shipped: &[Preset],
    directory: &Path,
    done: bool,
    discard: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Vec<Preset> {
    if done {
        return Vec::new();
    }
    let untouched: Vec<Preset> = load(directory).into_iter().filter(|held| shipped.iter().any(|s| s.same_look(held))).collect();
    for preset in &untouched {
        delete(preset, directory, discard);
    }
    untouched
}

// MARK: - A preset as one line of text

/// The share code's version and marker: a later format gets its own, and this one stays readable.
pub const SHARE_CODE_PREFIX: &str = "ONI1:";

/// The preset as one line: its carried rows laid on neutral settings, every key still at its
/// default dropped, raw deflate plus an FNV-1a check, then base64url; the codec restores the rest.
pub fn share_code(preset: &Preset) -> Result<String, json::Error> {
    use base64::Engine;
    use std::io::Write;
    let carried = Preset { settings: preset.apply(&PipelineSettings::default()), ..preset.clone() };
    let parse = |text: String| serde_json::from_str::<Value>(&text).map_err(|e| json::Error(e.to_string()));
    let mut object = parse(encode(&carried)?)?;
    let neutral = parse(codec::encode_settings(&PipelineSettings::default())?)?;
    let (Some(settings), Some(rest)) = (object.get("settings"), neutral.get("settings")) else {
        return Err(json::Error("a settings document is an object".into()));
    };
    let kept = departures(settings, rest).unwrap_or_else(|| Value::Object(Default::default()));
    object["settings"] = kept;
    let text = sorted_compact(&object);
    let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    deflater.write_all(text.as_bytes()).map_err(|e| json::Error(e.to_string()))?;
    let mut packed = deflater.finish().map_err(|e| json::Error(e.to_string()))?;
    packed.extend_from_slice(&checksum(text.as_bytes()));
    Ok(format!("{SHARE_CODE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed)))
}

/// The preset a code carries, `None` for anything else. Spaces and line breaks a messaging app
/// wraps a long line with are dropped first; a truncated or edited code fails as a whole.
pub fn from_share_code(text: &str) -> Option<Preset> {
    use base64::Engine;
    use std::io::Read;
    let bare: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let body = bare.strip_prefix(SHARE_CODE_PREFIX)?;
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        base64::engine::GeneralPurposeConfig::new().with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    );
    let packed = engine.decode(body).ok()?;
    if packed.len() <= 4 {
        return None;
    }
    let (deflated, check) = packed.split_at(packed.len() - 4);
    let mut json = Vec::new();
    flate2::read::DeflateDecoder::new(deflated).read_to_end(&mut json).ok()?;
    (checksum(&json) == check).then(|| decode(&json)).flatten()
}

/// FNV-1a over the JSON, big-endian: raw deflate carries no check of its own.
fn checksum(data: &[u8]) -> [u8; 4] {
    let mut hash: u32 = 2_166_136_261;
    for byte in data {
        hash = (hash ^ u32::from(*byte)).wrapping_mul(16_777_619);
    }
    hash.to_be_bytes()
}

/// What `value` holds that `rest` does not: objects key by key, anything else whole.
fn departures(value: &Value, rest: &Value) -> Option<Value> {
    if let (Value::Object(object), Value::Object(rest)) = (value, rest) {
        let mut kept = serde_json::Map::new();
        for (key, child) in object {
            match rest.get(key) {
                Some(rest_child) => {
                    if let Some(moved) = departures(child, rest_child) {
                        kept.insert(key.clone(), moved);
                    }
                }
                None => {
                    kept.insert(key.clone(), child.clone());
                }
            }
        }
        return (!kept.is_empty()).then_some(Value::Object(kept));
    }
    (value != rest).then(|| value.clone())
}

/// `JSONSerialization` with `.sortedKeys`: compact, every object's keys in order.
fn sorted_compact(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> =
                keys.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), sorted_compact(&map[*k]))).collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(sorted_compact).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
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
        let mark = |n: &str| Preset { name: n.into(), ..sample() }.abbreviation();
        assert_eq!([mark("Ektar"), mark("b&w"), mark("x"), mark("  ")], ["EKT", "BW", "X", "  "]);
        // Twin: the mark follows the name, so two names give two marks.
        assert_ne!(mark("Portra 400"), mark("Ektar"));
        assert_eq!(default_name(&[]), "Preset 01");
        assert_eq!(default_name(&["Preset 01", "Preset 03"]), "Preset 02", "the gap, not the highest plus one");
        let ninety_nine: Vec<String> = (1..=99).map(|n| format!("Preset {n:02}")).collect();
        let names: Vec<&str> = ninety_nine.iter().map(String::as_str).collect();
        assert_eq!(default_name(&names), "Preset 100");
        assert_eq!(default_name(&["Portra 400", "Ektar"]), "Preset 01");
        let hostile = Preset { name: "../../Kodak: 400/800".into(), ..sample() };
        let file = file_name(&hostile);
        assert!(!file.contains('/') && !file.contains(':') && !file.starts_with('.'), "{file}");
        let long = Preset { name: "x".repeat(300), ..sample() };
        assert_eq!(file_name(&long).len(), 64 + 1 + 8 + 5, "the stem stops at 64");
    }

    /// `PresetTint.selfCheck`: every card's text is readable.
    #[test]
    fn every_tint_carries_a_readable_ink() {
        let worst = Tint::ALL.iter().map(|t| t.contrast()).fold(f64::MAX, f64::min);
        assert!(worst >= 4.5, "the worst tint reads at {worst:.2}:1");
        let inks: BTreeSet<bool> = Tint::ALL.iter().map(|t| t.wants_dark_ink()).collect();
        assert_eq!(inks.len(), 2, "twin: both inks serve, so a constant ink would fail");
        assert_eq!(Tint::from_raw("chartreuse"), None);
        assert_eq!((Tint::default(), Tint::Slate.label()), (Tint::Slate, "Slate".to_string()));
    }

    #[test]
    fn a_share_code_is_one_line_and_reads_back_as_the_same_look() {
        let mut settings = PipelineSettings::default();
        settings.gains.stops = [0.3, -0.1, 0.2];
        settings.sharpen.amount = 1.25;
        settings.geometry.angle = 4.0;
        let looked = Preset {
            note: "Daylight, scanned on the light table".into(),
            ..Preset::new("Portra 400", ["gains", "sharpen", "saturation"].iter().map(|s| s.to_string()).collect(), Tint::Rust, settings)
        };
        let code = share_code(&looked).unwrap();
        assert!(code.starts_with(SHARE_CODE_PREFIX) && !code.chars().any(char::is_whitespace), "{code}");
        let back = from_share_code(&code).unwrap();
        assert!(back.same_look(&looked));
        assert_eq!(back.settings.geometry.angle, 0.0, "the code carries only the applied rows");
        let wrapped: String = code.chars().enumerate().map(|(i, c)| if i % 60 == 59 { format!("{c}\n ") } else { c.to_string() }).collect();
        assert!(from_share_code(&format!("  {wrapped}\n")).is_some_and(|p| p.same_look(&looked)));
        assert!(from_share_code(&code[..code.len() - 12]).is_none(), "a truncated code is refused whole");
        // Twin: one character of the payload changed is refused, not read as another look.
        let mut edited: Vec<char> = code.chars().collect();
        let middle = edited.len() / 2;
        edited[middle] = if edited[middle] == 'A' { 'B' } else { 'A' };
        assert!(from_share_code(&edited.into_iter().collect::<String>()).is_none());
        assert!(from_share_code("hello").is_none());
        let dir = folder("code");
        save(&looked, &dir).unwrap();
        assert!(adopting(vec![back], &dir).is_empty(), "one's own code lays nothing");
        assert_eq!(load(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A code written by the Swift app: raw deflate, sorted keys, FNV-1a, base64url.
    #[test]
    fn a_code_built_as_the_swift_app_builds_it_reads_back() {
        use base64::Engine;
        use std::io::Write;
        let json = br#"{"preset":{"id":"3676F1B6-B28F-4B26-9125-71C8039CD81B","name":"Azure","note":"","parameters":["sharpen"],"tint":"teal"},"settings":{"sharpen":{"amount":1.5}},"version":19}"#;
        let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflater.write_all(json).unwrap();
        let mut packed = deflater.finish().unwrap();
        packed.extend_from_slice(&checksum(json));
        let body = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&packed);
        let read = from_share_code(&format!("ONI1:{body}")).unwrap();
        assert_eq!((read.name.as_str(), read.tint, read.settings.sharpen.amount), ("Azure", Tint::Teal, 1.5));
        assert_eq!(read.settings.sharpen.radius, PipelineSettings::default().sharpen.radius, "the codec restores dropped keys");
        // Twin: the same body under a zlib header is no Swift code.
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(json).unwrap();
        let mut wrapped = zlib.finish().unwrap();
        wrapped.extend_from_slice(&checksum(json));
        assert!(from_share_code(&format!("ONI1:{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&wrapped))).is_none());
    }

    #[test]
    fn importing_never_overwrites_a_held_preset() {
        let dir = folder("import");
        let outside = folder("import-mail");
        std::fs::create_dir_all(&outside).unwrap();
        let held = Preset::new("Gold 200", BTreeSet::from(["gains".to_string()]), Tint::Amber, PipelineSettings::default());
        save(&held, &dir).unwrap();
        let sent = outside.join("gold.json");
        std::fs::write(&sent, shareable(&held).unwrap()).unwrap();
        assert!(importing(&[sent], &dir).is_empty() && load(&dir).len() == 1, "a held preset lays nothing");
        let mut retuned = held.clone();
        retuned.settings.gains.stops[0] += 0.5;
        let clash = outside.join("gold-retuned.json");
        std::fs::write(&clash, shareable(&retuned).unwrap()).unwrap();
        let garbage = outside.join("notes.json");
        std::fs::write(&garbage, br#"{"hello": 1}"#).unwrap();
        let taken = importing(&[clash, garbage], &dir);
        let after = load(&dir);
        assert_eq!((taken.len(), after.len()), (1, 2));
        assert!(after.contains(&held) && taken[0].id != held.id && taken[0].settings == retuned.settings);
        // Twin: saving the clashing file as it came replaces the held one.
        let naive = folder("import-naive");
        save(&held, &naive).unwrap();
        save(&retuned, &naive).unwrap();
        assert!(load(&naive).len() == 1 && !load(&naive).contains(&held));
        let copy = held.duplicated();
        assert!(copy.id != held.id && copy.name == "Gold 200 copy" && copy.settings == held.settings && copy.tint == held.tint);
        for d in [dir, outside, naive] {
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn untouched_shipped_presets_are_taken_back_once() {
        let dir = folder("retire");
        let shipped = retired();
        assert_eq!(shipped.len(), RETIRED.len(), "every retired file is a readable preset");
        for p in &shipped {
            save(p, &dir).unwrap();
        }
        let mut renamed = shipped[0].clone();
        renamed.name += " mine";
        save(&renamed, &dir).unwrap();
        let own = Preset::new("Own", BTreeSet::from(["gains".to_string()]), Tint::Slate, PipelineSettings::default());
        save(&own, &dir).unwrap();
        let mut trashed = Vec::new();
        let gone = retire_shipped(&shipped, &dir, false, &mut |f| {
            trashed.push(f.to_path_buf());
            std::fs::remove_file(f)
        });
        let left = load(&dir);
        assert_eq!((gone.len(), left.len(), trashed.len()), (shipped.len() - 1, 2, shipped.len() - 1));
        assert!(left.iter().any(|p| p.name == renamed.name) && left.iter().any(|p| p.id == own.id));
        // Twin: once recorded, the pass leaves a preset laid back afterwards alone.
        for p in &shipped {
            save(p, &dir).unwrap();
        }
        assert!(retire_shipped(&shipped, &dir, true, &mut |f| std::fs::remove_file(f)).is_empty());
        assert!(!retire_shipped(&shipped, &dir, false, &mut |f| std::fs::remove_file(f)).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_single_file_list_is_taken_over_once_then_archived() {
        let support = folder("migrate");
        std::fs::create_dir_all(&support).unwrap();
        let mut element: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
        element["name"] = "Old one".into();
        element["groups"] = serde_json::json!(["detail"]);
        std::fs::write(legacy_file(&support), serde_json::to_vec(&serde_json::json!([element, {"nothing": 1}])).unwrap()).unwrap();
        let shelf = load_shelf(&support);
        assert_eq!(shelf.len(), 1);
        assert_eq!(shelf[0].parameters, BTreeSet::from(["chroma".to_string(), "sharpen".to_string()]));
        assert!(!legacy_file(&support).exists() && support.join("presets.json.migrated").exists());
        // Twin: a second load finds no list and keeps the one file.
        assert_eq!(load_shelf(&support).len(), 1);
        assert_eq!(files(&directory(&support)).len(), 1);
        let _ = std::fs::remove_dir_all(&support);
    }

    #[test]
    fn a_blank_name_keeps_the_one_on_disk() {
        let dir = folder("blank");
        let mut p = sample();
        save(&p, &dir).unwrap();
        p.name = "   ".into();
        assert_eq!(writable_name(&p, &dir), "Portra / soft");
        save(&p, &dir).unwrap();
        assert_eq!(load(&dir)[0].name, "Portra / soft");
        // Twin: never on disk, a blank name falls back to the stem.
        let fresh = Preset { name: " ".into(), ..Preset::new("x", BTreeSet::new(), Tint::Slate, PipelineSettings::default()) };
        assert_eq!(writable_name(&fresh, &dir), "Preset");
        let mut trashed = 0;
        delete(&load(&dir)[0], &dir, &mut |f| {
            trashed += 1;
            std::fs::remove_file(f)
        });
        assert_eq!((trashed, files(&dir).len()), (1, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_preview_is_what_a_click_writes() {
        let preset = sample();
        let target = PipelineSettings::default();
        assert_eq!(previewed(None, &target), target);
        assert_eq!(previewed(Some(&preset), &target), preset.apply(&target));
        // Twin: the preset does change the destination, so an ignored preview fails above.
        assert_ne!(previewed(Some(&preset), &target), target);
    }
}
