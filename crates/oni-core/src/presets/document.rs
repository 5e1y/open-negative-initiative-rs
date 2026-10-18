//! A preset as a settings document plus its `preset` block, written and read back.

use super::*;

/// The old per-tab groups, read off what applying them did then.
pub(super) fn legacy_rows(group: &str) -> &'static [&'static str] {
    match group {
        "convert" => &["gains", "levelsPerChannel"],
        "look" => &["levelsGlobal", "curves", "spectrogram", "saturation"],
        "detail" => &["chroma", "sharpen"],
        _ => &[],
    }
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

/// `JSONSerialization` with `.sortedKeys`: compact, every object's keys in order.
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
