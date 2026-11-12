//! Every stored document of an earlier format brought to today's reading: the only code that knows
//! what an older version meant. The original is asked a fact only when a step needs one.

use crate::codec::{Decoded, Failure, SettingsDocument};
use crate::curves::CurveSet;
use crate::geometry::Size;
use crate::settings::PipelineSettings;
use crate::tolerant::{self, Object};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub mod before_1_0;
pub mod chain;
pub mod density;
pub mod draft;
pub mod lighting;
pub mod look;
pub mod matrices;
pub mod strokes;

#[cfg(test)]
mod corpus;

pub use chain::{merged, neutral_document};
pub use matrices::M3;

/// What the file beside a sidecar says, asked only when a step needs it.
pub struct Facts<'a> {
    /// The file decodes through the RAW engine's mosaic.
    pub mosaic: bool,
    /// The retired camera matrix of the original's decode; `None` for a camera without one.
    pub matrix: &'a dyn Fn() -> Option<M3>,
    /// The whole decode's size for these settings; `None` when the file cannot say.
    pub scan: &'a dyn Fn(&PipelineSettings) -> Option<Size>,
}

impl Facts<'static> {
    /// A file that answers nothing beyond how it decodes.
    pub fn bare(mosaic: bool) -> Facts<'static> {
        Facts { mosaic, matrix: &|| None, scan: &|_| None }
    }
}

/// What every sidecar read asks of an original, given how the caller knows it decodes: the facts
/// handed to the closure it is given.
pub type Lookup = dyn Fn(&Path, bool, &mut dyn FnMut(&Facts)) + Send + Sync;

static LOOKUP: RwLock<Option<Arc<Lookup>>> = RwLock::new(None);

/// The facts every later sidecar read asks; `None` asks nothing (a check, a tool).
pub fn install(facts: Option<Arc<Lookup>>) {
    if let Ok(mut l) = LOOKUP.write() {
        *l = facts;
    }
}

/// `then` given the installed facts of `original`, or none beyond `mosaic` when nothing is installed.
pub(crate) fn asked<R>(original: &Path, mosaic: bool, then: impl Fn(&Facts) -> R) -> R {
    let mut out = None;
    if let Some(lookup) = LOOKUP.read().ok().and_then(|l| l.clone()) {
        lookup(original, mosaic, &mut |f: &Facts| {
            if out.is_none() {
                out = Some(then(f));
            }
        });
    }
    out.unwrap_or_else(|| then(&Facts::bare(mosaic)))
}

/// A stored document read to this format, in its types.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub decoded: Decoded,
    /// False when strokes waited for a scan size the original could not give: never written.
    pub placed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Not JSON, not an object, or a value the chain cannot read.
    Unreadable(Failure),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Unreadable(e) => write!(f, "{e}"),
        }
    }
}

/// The old bytes as this format reads them. Reads no file but through `facts`.
pub fn converted(bytes: &[u8], facts: &Facts) -> Result<Converted, Refusal> {
    reading(bytes, facts, true).map_err(Refusal::Unreadable)
}

/// `converted`; `catching_up` off reads metadata without fitting a former document's lighting.
pub(crate) fn reading(bytes: &[u8], facts: &Facts, catching_up: bool) -> Result<Converted, Failure> {
    let decoded = decoded(bytes, Some(facts.mosaic), catching_up)?;
    let settings = matrices::caught_up(decoded.version, decoded.settings.clone(), facts.matrix);
    let (settings, placed) = strokes::caught_up(decoded.version, settings, facts.scan);
    Ok(Converted { decoded: Decoded { settings, ..decoded }, placed })
}

/// A settings document outside a sidecar (a preset file, a share code, a paste): no file, no facts.
pub fn document(bytes: &[u8]) -> Result<Decoded, Failure> {
    decoded(bytes, None, true)
}

/// The chain alone, on the raw document then on its typed settings; `mosaic` is `None` for a
/// document of no file. The version is returned as stored, the caller deciding about a newer one.
pub fn decoded(bytes: &[u8], mosaic: Option<bool>, catching_up: bool) -> Result<Decoded, Failure> {
    let stored: Value = serde_json::from_slice(bytes).map_err(|_| Failure::NotAnObject)?;
    decoded_value(stored, mosaic, catching_up)
}

pub(crate) fn decoded_value(stored: Value, mosaic: Option<bool>, catching_up: bool) -> Result<Decoded, Failure> {
    let Value::Object(stored) = stored else { return Err(Failure::NotAnObject) };
    let version = tolerant::ns_integer(stored.get("version")).unwrap_or(0);
    // The unit changes run on the keys the document really carries, before the defaults are laid in.
    let aged = Value::Object(chain::aged(stored, version, mosaic));
    let completed = merged(&aged, neutral_document());
    let document: SettingsDocument = serde_json::from_value(completed).map_err(|e| Failure::Malformed(e.to_string()))?;
    let mut settings = document.settings;
    let raw = aged.get("settings").filter(|v| v.is_object());
    let colour = CurveSet::retired(raw, "curves");
    if catching_up && version < before_1_0::LIGHTING_VERSION {
        let empty = Value::Object(Object::new());
        if let Some(former) = lighting::former(raw.unwrap_or(&empty)) {
            settings = lighting::migrated(&settings, &former, &colour.clone().unwrap_or_default());
        }
    }
    if let Some(colour) = &colour {
        settings.manual_curves = settings.manual_curves.folding(colour);
    }
    if let Some(scanner) = CurveSet::retired(raw.and_then(|r| r.get("scannerFinish")), "curves") {
        settings.scanner_finish.manual_curves = settings.scanner_finish.manual_curves.folding(&scanner);
    }
    if version < chain::LOOK_FOLDED_VERSION
        && let Some(look) = look::ScannerLook::stored(raw)
    {
        settings = look.folded(&settings);
    }
    Ok(Decoded {
        settings,
        version,
        imported_at: document.imported_at,
        label: document.label.unwrap_or_default(),
        exif: document.exif,
        roll_draft: document.roll_draft,
        trashed_at: document.trashed_at,
    })
}
