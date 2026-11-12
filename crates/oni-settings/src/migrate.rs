//! Every stored document of an earlier format brought to this one: the only code that knows what an
//! older version meant. The original is asked a fact only when a step needs one.

use crate::curves::CurveSet;
use crate::document::{self, Document, Failure, VERSION};
use crate::geometry::Size;
use crate::settings::PipelineSettings;
use crate::tolerant::Object;
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
mod on_disk;
pub mod reshape;
pub mod strokes;

#[cfg(test)]
mod checks;
#[cfg(test)]
mod corpus;

pub use matrices::M3;
pub use on_disk::{Folder, Outcome, file, file_with, folder, marked, sidecar_of};
#[cfg(test)]
use on_disk::file_through;

/// What the file beside a sidecar says, asked only when a step needs it: asking reads the original.
pub struct Facts<'a> {
    /// Whether the file decodes through the RAW engine's mosaic, which only formats under 15 ask.
    pub mosaic: Mosaic<'a>,
    /// The retired camera matrix of the original's decode, `Ok(None)` for a camera without one.
    pub matrix: &'a dyn Fn() -> Result<Option<M3>, Refusal>,
    /// The whole decode's size for these settings.
    pub scan: &'a dyn Fn(&PipelineSettings) -> Result<Size, Refusal>,
}

/// How a file decodes: known by the caller, or asked of the original when a step needs it.
#[derive(Clone, Copy)]
pub enum Mosaic<'a> {
    Known(bool),
    Asked(&'a dyn Fn() -> Result<bool, Refusal>),
}

impl Mosaic<'_> {
    pub fn get(&self) -> Result<bool, Refusal> {
        match self {
            Mosaic::Known(m) => Ok(*m),
            Mosaic::Asked(ask) => ask(),
        }
    }
}

impl Facts<'static> {
    /// A file that answers nothing beyond how it decodes: a camera's former matrix stays unknown.
    pub fn bare(mosaic: bool) -> Facts<'static> {
        Facts { mosaic: Mosaic::Known(mosaic), matrix: &|| Err(Refusal::MatrixUnknown), scan: &|_| Err(Refusal::ScanUnknown) }
    }
}

/// What every sidecar read asks of an original, given how the caller knows it decodes (`None` when
/// it does not): the facts handed to the closure it is given.
pub type Lookup = dyn Fn(&Path, Option<bool>, &mut dyn FnMut(&Facts)) + Send + Sync;

static LOOKUP: RwLock<Option<Arc<Lookup>>> = RwLock::new(None);

/// The facts every later sidecar read asks; `None` asks nothing (a check, a tool).
pub fn install(facts: Option<Arc<Lookup>>) {
    if let Ok(mut l) = LOOKUP.write() {
        *l = facts;
    }
}

/// `then` given the installed facts of `original`; with nothing installed, none beyond `mosaic`.
pub(crate) fn asked<R>(original: &Path, mosaic: Option<bool>, then: impl Fn(&Facts) -> R) -> R {
    let mut out = None;
    if let Some(lookup) = LOOKUP.read().ok().and_then(|l| l.clone()) {
        lookup(original, mosaic, &mut |f: &Facts| {
            if out.is_none() {
                out = Some(then(f));
            }
        });
    }
    out.unwrap_or_else(|| match mosaic {
        Some(m) => then(&Facts::bare(m)),
        None => then(&Facts { mosaic: Mosaic::Asked(&|| Err(Refusal::CameraUnknown)), ..Facts::bare(false) }),
    })
}

/// Where a stored sidecar stands against this format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Current,
    Older(i64),
    Newer(i64),
    /// Not a JSON object.
    NotASidecar,
}

/// The version alone is read, the rest skipped unparsed.
pub fn standing(bytes: &[u8]) -> Standing {
    let Some(s) = document::sniffed(bytes) else { return Standing::NotASidecar };
    if s.version > VERSION {
        Standing::Newer(s.version)
    } else if s.named && s.version == VERSION {
        Standing::Current
    } else {
        Standing::Older(s.version)
    }
}

/// A stored document read to this format, in its types.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub document: Document,
    /// The version it was stored at.
    pub from: i64,
    /// The roll draft a frame of format 21 and under kept, which its folder's file keeps now.
    pub draft: Option<draft::RollDraft>,
}

/// Why a stored document was left as it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Not JSON, not an object, or a value the chain cannot read.
    Unreadable(Failure),
    Newer(i64),
    /// Dust strokes to move onto the scan, and no scan size: the original is away or unreadable.
    ScanUnknown,
    /// A camera matrix to keep in the gains, and the original could not say its camera.
    CameraUnknown,
    /// A camera matrix to keep in the gains, and this computer's table does not answer for the camera.
    MatrixUnknown,
    /// Written for another file of that name: its facts are not this file's.
    Foreign,
    /// The folder's Prepare file, which takes the frame's roll draft, could not be read or written.
    Drafts(String),
    /// The original's bytes are kept in the cloud, and a step would download them.
    Away,
    /// A read-only disk, an access refused, a disk full.
    Unwritable(String),
    /// The file read back did not say what was converted.
    Unverified,
    /// The old file changed while it was converted; taken again at the next pass.
    Moved,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Unreadable(e) => write!(f, "{e}"),
            Refusal::Newer(v) => write!(f, "written by a newer version (format {v})"),
            Refusal::ScanUnknown => f.write_str("its original gives no scan size to place its dust strokes"),
            Refusal::CameraUnknown => f.write_str("its original could not say its camera"),
            Refusal::MatrixUnknown => f.write_str("this computer does not know its camera's former colour matrix"),
            Refusal::Foreign => f.write_str("it was written for another file of that name"),
            Refusal::Drafts(e) => write!(f, "its folder's Prepare file could not be kept ({e})"),
            Refusal::Away => f.write_str("its original is not downloaded"),
            Refusal::Unwritable(e) => write!(f, "the disk refused the write ({e})"),
            Refusal::Unverified => f.write_str("the file read back did not say what was converted"),
            Refusal::Moved => f.write_str("the file changed while it was converted"),
        }
    }
}

impl Refusal {
    /// The reason in a few words that quote nothing of the file, for a log.
    pub fn kind(&self) -> &'static str {
        match self {
            Refusal::Unreadable(_) => "unreadable",
            Refusal::Newer(_) => "written by a newer version",
            Refusal::ScanUnknown => "no scan size for its dust strokes",
            Refusal::CameraUnknown => "its camera unknown",
            Refusal::MatrixUnknown => "its camera's former matrix unknown here",
            Refusal::Foreign => "written for another file",
            Refusal::Drafts(_) => "its folder's Prepare file refused",
            Refusal::Away => "its original not downloaded",
            Refusal::Unwritable(_) => "the disk refused the write",
            Refusal::Unverified => "not read back as converted",
            Refusal::Moved => "changed while converted",
        }
    }
}

/// The old bytes as this format reads them, every step done. Reads no file but through `facts`.
pub fn converted(bytes: &[u8], facts: &Facts) -> Result<Converted, Refusal> {
    let (converted, held) = reading(bytes, facts, false)?;
    match held {
        Some(why) => Err(why),
        None => Ok(converted),
    }
}

/// `converted`, with the step a missing fact stopped said apart, the document read without it; a
/// newer document read as far as these types read it when `newer` allows it.
pub fn reading(bytes: &[u8], facts: &Facts, newer: bool) -> Result<(Converted, Option<Refusal>), Refusal> {
    let stored = stored(bytes)?;
    let version = crate::tolerant::ns_integer(stored.get("version")).unwrap_or(0);
    if version > VERSION && !newer {
        return Err(Refusal::Newer(version));
    }
    if document::sniffed(bytes).is_some_and(|s| s.named) && version >= VERSION {
        let document = document::typed(bytes).map_err(Refusal::Unreadable)?;
        return Ok((Converted { document, from: version, draft: None }, None));
    }
    // Only the oldest formats ask how the file decodes, as asking reads the original.
    let mosaic = if version < chain::PRIMARIES_HELD_VERSION { Some(facts.mosaic.get()?) } else { None };
    let mut converted = earlier(stored, version, mosaic).map_err(Refusal::Unreadable)?;
    let mut held = None;
    let settings = std::mem::take(&mut converted.document.settings);
    let settings = match matrices::caught_up(version, settings.clone(), facts.matrix) {
        Ok(s) => s,
        Err(why) => {
            held = Some(why);
            settings
        }
    };
    converted.document.settings = match strokes::caught_up(version, settings.clone(), facts.scan) {
        Ok(s) => s,
        Err(why) => {
            held = held.or(Some(why));
            settings
        }
    };
    if version > VERSION {
        converted.document.version = version;
    }
    Ok((converted, held))
}

/// A settings document outside a sidecar (a preset file, a share code, a paste): no file, no facts.
/// Any `oni-` document of this version reads its settings as they stand.
pub fn document(bytes: &[u8]) -> Result<Converted, Refusal> {
    let stored = stored(bytes)?;
    let version = crate::tolerant::ns_integer(stored.get("version")).unwrap_or(0);
    if version > VERSION {
        return Err(Refusal::Newer(version));
    }
    let named = stored.get("format").and_then(Value::as_str).is_some_and(|f| f.starts_with("oni-"));
    if named && version == VERSION {
        let settings = stored.get("settings").cloned().unwrap_or(Value::Object(Object::new()));
        let settings: PipelineSettings = serde_json::from_value(settings).map_err(|e| Refusal::Unreadable(Failure::Malformed(e.to_string())))?;
        return Ok(Converted { document: Document::of(settings), from: version, draft: None });
    }
    earlier(stored, version, None).map_err(Refusal::Unreadable)
}

fn stored(bytes: &[u8]) -> Result<Object, Refusal> {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(stored)) => Ok(stored),
        _ => Err(Refusal::Unreadable(Failure::NotAnObject)),
    }
}

/// The frozen chain, on the raw document then on its typed settings, before any fact is asked;
/// `mosaic` is `None` for a document of no file, and for a format that never asks it.
fn earlier(stored: Object, version: i64, mosaic: Option<bool>) -> Result<Converted, Failure> {
    // The unit changes run on the keys the document really carries, before the defaults are laid in.
    let aged = Value::Object(chain::aged(stored, version, mosaic));
    let completed = chain::merged(&aged, chain::neutral_document());
    let (mut document, draft) = reshape::reshaped(completed)?;
    let mut settings = std::mem::take(&mut document.settings);
    let raw = aged.get("settings").filter(|v| v.is_object());
    let colour = CurveSet::retired(raw, "curves");
    if version < before_1_0::LIGHTING_VERSION {
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
    document.settings = settings;
    Ok(Converted { document, from: version, draft })
}
