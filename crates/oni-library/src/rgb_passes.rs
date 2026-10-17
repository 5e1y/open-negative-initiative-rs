//! The three files of a frame exposed under red, green and blue light, found through the lead's
//! sidecar, so a gesture on a frame moves or trashes the three together.

use crate::paths;
use oni_settings::frame::RgbPasses;
use oni_settings::sidecar;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

/// One key of a sidecar's settings, read only when its name is in the bytes: most frames carry none.
fn setting(path: &str, key: &str) -> Option<Value> {
    let data = std::fs::read(sidecar::url_for(Path::new(path))).ok()?;
    let quoted = format!("\"{key}\"");
    if !data.windows(quoted.len()).any(|w| w == quoted.as_bytes()) {
        return None;
    }
    let document: Value = serde_json::from_slice(&data).ok()?;
    document.get("settings")?.get(key).filter(|v| !v.is_null()).cloned()
}

/// The triplet a lead's sidecar names, `None` for a file that leads none.
pub fn named_by(lead: &str) -> Option<RgbPasses> {
    serde_json::from_value(setting(lead, "rgbPasses")?).ok()
}

/// The passes' paths beside the lead, red, green then blue.
pub fn paths_beside(passes: &RgbPasses, lead: &str) -> Vec<String> {
    let folder = paths::parent(lead);
    [&passes.red, &passes.green, &passes.blue].iter().map(|n| paths::join(&folder, n)).collect()
}

/// The three files of the frame `path` belongs to, lead or pass; `None` for a file standing alone.
pub fn group(path: &str) -> Option<Vec<String>> {
    if let Some(passes) = named_by(path) {
        return Some(paths_beside(&passes, path));
    }
    let lead = setting(path, "passOf")?.as_str()?.to_owned();
    let lead = paths::join(&paths::parent(path), &lead);
    named_by(&lead).map(|passes| paths_beside(&passes, &lead))
}

/// `paths` with every frame made of passes spelt out as its three files, in order, once each.
pub fn expanded(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        for file in group(path).unwrap_or_else(|| vec![path.clone()]) {
            if seen.insert(file.clone()) {
                out.push(file);
            }
        }
    }
    out
}
