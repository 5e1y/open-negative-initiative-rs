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

mod tint;
mod naming;
mod document;
mod shelf;
mod retired;
mod share_code;

pub use tint::*;
pub use naming::*;
pub use document::*;
pub use shelf::*;
pub use retired::*;
pub use share_code::*;

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

/// The settings a hovered card shows on the canvas; never written, so a hover makes no step.
pub fn previewed(preset: Option<&Preset>, settings: &PipelineSettings) -> PipelineSettings {
    preset.map_or_else(|| settings.clone(), |p| p.apply(settings))
}

// MARK: - A preset as one line of text

#[cfg(test)]
mod tests;
