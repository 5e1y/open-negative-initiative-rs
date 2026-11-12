//! The former app's preferences taken once into `preferences.json` on macOS, the export's choices
//! foremost: read with `/usr/bin/plutil`, each value checked, never over one ONI holds already.

use crate::preferences::{self, Preferences};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The former app's preferences on this Mac; read, never written.
pub fn plist() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|_| cfg!(target_os = "macos"))?;
    Some(PathBuf::from(home).join("Library/Preferences/fr.courtillon.open-negative-initiative.plist"))
}

/// The keys taken, each with the type `plutil` must find.
const KEYS: [(&str, &str); 8] = [
    (oni_export::settings::PREFERENCE_KEY, "string"),
    (crate::app::export_card::ARTIST, "string"),
    (crate::app::export_card::COPYRIGHT, "string"),
    (crate::app::export_card::LAST_PROFILE, "string"),
    (preferences::GALLERY_COLUMNS, "integer"),
    (preferences::BRUSH_RADIUS, "float"),
    (preferences::SOUND_MUTED, "bool"),
    (crate::updates::LAST_LAUNCHED_KEY, "string"),
];

/// How many keys were taken; none where the file is absent or `plutil` refuses.
pub fn import(support: &Path, plist: &Path) -> usize {
    if !cfg!(target_os = "macos") || !plist.is_file() {
        return 0;
    }
    let mut prefs = Preferences::load(support);
    let mut taken = 0;
    for (key, kind) in KEYS {
        if prefs.value(key).is_some() {
            continue;
        }
        let Some(value) = extracted(plist, key, kind).filter(|v| fits(key, v)) else { continue };
        taken += usize::from(prefs.set(key, value).is_ok());
    }
    taken
}

/// `plutil -extract <key> raw`, which prints the value and a line's end; `None` for another type.
fn extracted(plist: &Path, key: &str, kind: &str) -> Option<Value> {
    let out = std::process::Command::new("/usr/bin/plutil").args(["-extract", key, "raw", "-expect", kind, "-o", "-"]).arg(plist).output().ok()?;
    let text = String::from_utf8(out.stdout).ok().filter(|_| out.status.success())?;
    let text = text.strip_suffix('\n').unwrap_or(&text);
    match kind {
        "integer" => text.parse::<i64>().ok().map(Value::from),
        "float" => text.parse::<f64>().ok().map(Value::from),
        "bool" => text.parse::<bool>().ok().map(Value::from),
        _ => Some(Value::from(text)),
    }
}

/// Whether a value is one ONI reads under its key: the export document, a rung's range, a text.
fn fits(key: &str, value: &Value) -> bool {
    let within = |(low, high): (f64, f64)| value.as_f64().is_some_and(|v| (low..=high).contains(&v));
    let steps = crate::gallery::COLUMN_STEPS;
    let rungs = (steps.iter().min().copied().unwrap_or(1) as f64, steps.iter().max().copied().unwrap_or(1) as f64);
    match key {
        oni_export::settings::PREFERENCE_KEY => value.as_str().and_then(oni_export::Settings::from_raw_value).is_some(),
        preferences::GALLERY_COLUMNS => value.is_i64() && within(rungs),
        preferences::BRUSH_RADIUS => within(oni_settings::frame::Correction::RADIUS_RANGE),
        preferences::SOUND_MUTED => value.is_boolean(),
        _ => value.as_str().is_some_and(|s| !s.is_empty()),
    }
}
