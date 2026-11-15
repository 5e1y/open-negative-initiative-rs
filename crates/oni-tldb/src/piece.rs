//! A body or a lens as The Lens DB states it: the one shape its search and its Kit both answer with.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Camera,
    Lens,
}

/// A piece's identity: an id is unique within its kind only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub id: u64,
}

/// Shortest and longest focal length in mm, equal for a prime.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub min: f64,
    pub max: f64,
}

/// The widest aperture at the shortest and at the longest focal length, equal when constant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Aperture {
    pub at_min_focal: f64,
    pub at_max_focal: f64,
}

/// Every field but kind, id, name, model and url may be missing from the catalogue: a hand types it then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub id: u64,
    /// The full name, its brand included.
    pub name: String,
    #[serde(default)]
    pub brand: Option<String>,
    /// The name without its brand.
    pub model: String,
    pub url: String,
    #[serde(default)]
    pub mount: Option<String>,
    #[serde(default)]
    pub mounts: Option<Vec<String>>,
    #[serde(default)]
    pub year_introduced: Option<i32>,
    #[serde(default)]
    pub focal_length_mm: Option<Span>,
    #[serde(default)]
    pub max_aperture: Option<Aperture>,
    /// A compact's or a fixed-lens camera's own lens.
    #[serde(default)]
    pub built_in_lens: Option<Box<Piece>>,
}

impl Piece {
    pub fn key(&self) -> Key {
        Key { kind: self.kind, id: self.id }
    }
}
