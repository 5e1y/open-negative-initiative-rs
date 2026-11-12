//! A document of format 21 and under, its chain done, read in the shape its version wrote and laid
//! into this format's: the side keys typed, the EXIF line renamed, the keypad in steps.

use super::draft::RollDraft;
use crate::date::Date;
use crate::document::{AutoBalance, Document, FORMAT, Failure, Original, VERSION};
use crate::modes::Medal;
use crate::scanner::ScannerClicks;
use crate::settings::PipelineSettings;
use crate::sidecar::ExifLine;
use crate::tolerant::{self, Object};
use serde::Deserialize;
use serde_json::Value;

/// The keypad's letters, each counted in whole units with the half step left over under `halves`.
const LETTERS: [&str; 5] = ["d", "c", "m", "y", "w"];

/// The envelope as format 21 and under typed it.
#[derive(Deserialize)]
struct Stored {
    #[serde(default)]
    #[allow(dead_code)]
    version: i64,
    #[serde(rename = "importedAt", default)]
    imported_at: Option<Date>,
    #[serde(default)]
    label: Option<Medal>,
    #[serde(default)]
    exif: Option<ExifLine>,
    #[serde(rename = "rollDraft", default)]
    roll_draft: Option<RollDraft>,
    #[serde(rename = "trashedAt", default)]
    trashed_at: Option<Date>,
    settings: PipelineSettings,
}

/// `completed`, the chain's document over the resting one, in this format's types, and the roll
/// draft it kept.
pub(super) fn reshaped(completed: Value) -> Result<(Document, Option<RollDraft>), Failure> {
    let Value::Object(mut map) = completed else { return Err(Failure::NotAnObject) };
    if let Some(Value::Object(settings)) = map.get_mut("settings")
        && let Some(Value::Object(clicks)) = settings.get_mut("scanner")
    {
        in_steps(clicks);
    }
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_owned);
    let original = text("originalPrint").map(|print| Original { print, tail: text("originalTail") });
    // A balance kept that does not read whole is read again rather than guessed.
    let auto_balance = map.get("autoBalance").and_then(|v| serde_json::from_value::<AutoBalance>(v.clone()).ok());
    let stored: Stored = serde_json::from_value(Value::Object(map)).map_err(|e| Failure::Malformed(e.to_string()))?;
    let document = Document {
        format: FORMAT.into(),
        version: VERSION,
        original,
        imported_at: stored.imported_at,
        label: stored.label.unwrap_or_default(),
        trashed_at: stored.trashed_at,
        exif_line: stored.exif.unwrap_or_default(),
        auto_balance,
        settings: stored.settings,
    };
    Ok((document, stored.roll_draft))
}

/// Each letter's whole units and its half step, read as the keypad of format 21 read them, in steps.
fn in_steps(clicks: &mut Object) {
    let halves = clicks.remove("halves");
    let halves = halves.as_ref().and_then(Value::as_object);
    for key in LETTERS {
        let half = halves.and_then(|h| tolerant::integer(h.get(key))).unwrap_or(0).clamp(-1, 1);
        let whole = tolerant::integer(clicks.get(key)).unwrap_or(0);
        clicks.insert(key.into(), Value::from(whole.saturating_mul(ScannerClicks::PER_UNIT).saturating_add(half)));
    }
}

/// `document` as a build of format 21 and under wrote it, stamped `version`: what the checks age a
/// document with, and what an earlier build is shown reading.
pub fn as_stored(document: &Document, version: i64) -> Value {
    let mut value = serde_json::to_value(document).unwrap_or(Value::Null);
    let Value::Object(map) = &mut value else { return value };
    map.remove("format");
    map.insert("version".into(), Value::from(version));
    if let Some(line) = map.remove("exifLine") {
        map.insert("exif".into(), line);
    }
    if let Some(Value::Object(mut original)) = map.remove("original") {
        map.extend(original.remove("print").map(|p| ("originalPrint".to_owned(), p)));
        map.extend(original.remove("tail").map(|t| ("originalTail".to_owned(), t)));
    }
    if let Some(Value::Object(settings)) = map.get_mut("settings")
        && let Some(Value::Object(clicks)) = settings.get_mut("scanner")
    {
        let mut halves = Object::new();
        for key in LETTERS {
            let steps = tolerant::integer(clicks.get(key)).unwrap_or(0);
            clicks.insert(key.into(), Value::from(steps / ScannerClicks::PER_UNIT));
            if steps % ScannerClicks::PER_UNIT != 0 {
                halves.insert(key.into(), Value::from(steps % ScannerClicks::PER_UNIT));
            }
        }
        if !halves.is_empty() {
            clicks.insert("halves".into(), Value::Object(halves));
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn steps(stored: Value) -> Value {
        let Value::Object(mut clicks) = stored else { unreachable!() };
        in_steps(&mut clicks);
        Value::Object(clicks)
    }

    #[test]
    fn whole_units_and_their_half_step_read_in_steps() {
        assert_eq!(steps(json!({"d": 3, "halves": {"d": 1}})), json!({"d": 7, "c": 0, "m": 0, "y": 0, "w": 0}));
        assert_eq!(steps(json!({"d": -3, "halves": {"d": -1}}))["d"], -7);
        // A half past one step is held to one; a count past the range lands on its stop as it is read.
        assert_eq!(steps(json!({"c": 1, "halves": {"c": 5}}))["c"], 3);
        let past: ScannerClicks = serde_json::from_value(steps(json!({"d": 30, "halves": {"d": 1}}))).unwrap();
        assert_eq!(past.d, ScannerClicks::DENSITY_RANGE.1);
        // Twin: a reshape that drops the half step reads 6 where the keypad stood on 7.
        let dropped = |stored: Value| {
            let Value::Object(mut clicks) = stored else { unreachable!() };
            clicks.remove("halves");
            steps(Value::Object(clicks))
        };
        assert_eq!(dropped(json!({"d": 3, "halves": {"d": 1}}))["d"], 6);
    }

    #[test]
    fn a_document_stored_the_old_way_reads_back_whole() {
        let mut document = Document::of(PipelineSettings::default());
        document.settings.scanner = ScannerClicks::new(7, -3, 2, 0, 1);
        document.original = Some(Original { print: "1-2-3".into(), tail: Some("4".into()) });
        document.exif_line.lens = Some("50mm".into());
        let stored = as_stored(&document, 21);
        assert_eq!(stored["settings"]["scanner"], json!({"d": 3, "c": -1, "m": 1, "y": 0, "w": 0, "halves": {"d": 1, "c": -1, "w": 1}}));
        assert_eq!((&stored["originalPrint"], &stored["exif"]["lens"]), (&json!("1-2-3"), &json!("50mm")));
        let completed = super::super::chain::merged(&stored, super::super::chain::neutral_document());
        assert_eq!(reshaped(completed).unwrap().0, document);
    }
}
