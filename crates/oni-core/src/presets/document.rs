//! A preset as its file, `oni-preset`, written and read back; a file of an earlier version is read
//! through `migrate`.

use super::*;
use serde::{Deserialize, Serialize};

/// What a preset file names itself.
pub const FORMAT: &str = "oni-preset";

/// A preset's own block in its file; a file edited by hand may leave all but its name out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Block {
    #[serde(default)]
    id: Option<Id>,
    name: String,
    #[serde(default)]
    parameters: BTreeSet<String>,
    #[serde(default)]
    tint: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct File {
    format: String,
    version: i64,
    preset: Block,
    settings: PipelineSettings,
}

/// The preset's file, indented, a newline last.
pub fn encode(preset: &Preset) -> Result<String, String> {
    let block = Block { id: Some(preset.id), name: preset.name.clone(), parameters: preset.parameters.clone(), tint: preset.tint.raw_value().into(), note: preset.note.clone() };
    let file = File { format: FORMAT.into(), version: VERSION, preset: block, settings: preset.settings.clone() };
    serde_json::to_string_pretty(&file).map(|text| text + "\n").map_err(|e| e.to_string())
}

/// A preset from a file's bytes; a file from a newer version is skipped rather than half-applied.
pub fn decode(data: &[u8]) -> Option<Preset> {
    let object: Value = serde_json::from_slice(data).ok()?;
    if object.get("format").and_then(Value::as_str) == Some(FORMAT) && object.get("version").and_then(Value::as_i64) == Some(VERSION) {
        let file: File = serde_json::from_value(object).ok()?;
        let name = file.preset.name.trim_matches(is_blank).to_string();
        let b = file.preset;
        let tint = Tint::from_raw(&b.tint).unwrap_or_default();
        let id = b.id.unwrap_or_else(|| Id::of_bytes(data));
        return (!name.is_empty()).then_some(Preset { id, name, parameters: b.parameters, tint, note: b.note, settings: file.settings });
    }
    earlier(data, &object)
}

/// What `value` holds that `rest` does not: objects key by key, anything else whole.
pub(super) fn departures(value: &Value, rest: &Value) -> Option<Value> {
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

/// Compact, every object's keys in order, so one look gives one code.
pub(super) fn sorted_compact(value: &Value) -> String {
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
