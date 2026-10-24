//! The keys a newer app added to a sidecar, carried through a rewrite: the format moves its version
//! only for a field whose meaning changes, so a medal set here must not erase a field Swift added.

use crate::codec::{self, Decoded, SettingsDocument, VERSION};
use crate::json::{self, Node};
use serde_json::Value;

/// The envelope's own keys, never carried: the document written says them, or their absence.
const ENVELOPE: [&str; 7] = ["version", "importedAt", "label", "exif", "rollDraft", "trashedAt", "settings"];

/// A key the stored document holds and this version does not write, by its path of object keys.
type Unknown = (Vec<String>, Value);

/// `document`'s text, with the keys `stored` holds that this version does not know.
pub(crate) fn encoded(document: &SettingsDocument, stored: Option<&(Value, Decoded)>) -> Result<String, json::Error> {
    let mut node = json::to_node(document)?;
    let bare = json::pretty_sorted(&node);
    // An older document's extra keys are the ones its migrations consumed, never to be read twice.
    let Some((stored, decoded)) = stored.filter(|(_, d)| d.version == VERSION) else { return Ok(bare) };
    let known = json::to_node(&codec::document(
        &decoded.settings,
        decoded.imported_at,
        Some(decoded.label),
        decoded.exif.clone(),
        decoded.roll_draft.clone(),
        decoded.trashed_at,
    ))?
    .to_value();
    let mut found = Vec::new();
    unknown(stored, &known, &mut Vec::new(), &mut found);
    if found.is_empty() {
        return Ok(bare);
    }
    for (path, value) in &found {
        graft(&mut node, path, value);
    }
    let text = json::pretty_sorted(&node);
    // A carried key must change nothing this version reads, as a retired key would; else none is.
    let same = matches!((codec::decode(bare.as_bytes()), codec::decode(text.as_bytes())), (Ok(a), Ok(b)) if a == b);
    Ok(if same { text } else { bare })
}

fn unknown(stored: &Value, known: &Value, path: &mut Vec<String>, found: &mut Vec<Unknown>) {
    let (Value::Object(stored), Value::Object(known)) = (stored, known) else { return };
    for (key, value) in stored {
        match known.get(key) {
            Some(under) => {
                path.push(key.clone());
                unknown(value, under, path, found);
                path.pop();
            }
            None if value.is_null() || (path.is_empty() && ENVELOPE.contains(&key.as_str())) => {}
            None => {
                let mut at = path.clone();
                at.push(key.clone());
                found.push((at, value.clone()));
            }
        }
    }
}

/// Lays `value` at `path`, under a parent the new document still has; the new document's own key wins.
fn graft(node: &mut Node, path: &[String], value: &Value) {
    let Node::Obj(entries) = node else { return };
    let Some((key, rest)) = path.split_first() else { return };
    match entries.iter_mut().find(|(k, _)| k == key) {
        Some((_, child)) if !rest.is_empty() => graft(child, rest, value),
        Some(_) => {}
        None if rest.is_empty() => entries.push((key.clone(), node_of(value))),
        None => {}
    }
}

fn node_of(value: &Value) -> Node {
    match value {
        Value::Null => Node::Null,
        Value::Bool(b) => Node::Bool(*b),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => Node::Int(i),
            (_, Some(u), _) => Node::UInt(u),
            (_, _, f) => Node::F64(f.unwrap_or(0.0)),
        },
        Value::String(s) => Node::Str(s.clone()),
        Value::Array(items) => Node::Arr(items.iter().map(node_of).collect()),
        Value::Object(map) => Node::Obj(map.iter().map(|(k, v)| (k.clone(), node_of(v))).collect()),
    }
}
