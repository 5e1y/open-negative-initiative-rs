//! What the catalogue holds of each file: a virtual film, the paper reading it, a dye set, a printing paper, a canister.

use crate::dyes::DyeSet;
use crate::paper::PaperStock;
use crate::virtual_film::{Balance, FilmEffects, StockReference, VirtualStock};
use std::sync::Arc;

/// The four kinds of key a sidecar stores, each its own space: one key may name a roll's stock and a virtual film.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Space {
    Film,
    Virtual,
    Dyes,
    Paper,
}

/// A virtual film: its stock's name and key, its light, its dyes, its reference and its data sheet's profile.
#[derive(Debug, Clone)]
pub struct VirtualEntry {
    pub stock: VirtualStock,
    pub name: &'static str,
    pub catalogue: &'static str,
    pub balance: Balance,
    pub dyes: DyeSet,
    pub reference: StockReference,
    pub profile: Arc<str>,
    pub silver: bool,
    /// A colour reversal film: its curves fall, and its slide is projected.
    pub reversal: bool,
    /// A film of no colour of its own, its profile its density range and its tone curve.
    pub neutral: bool,
    /// Its own DIR coupler matrix, the common one where none.
    pub couplers: Option<[[f64; 3]; 3]>,
    /// Its layers' curves fitted with its couplers, its sheet's where none.
    pub curves: Option<Arc<[[f64; 3]]>>,
    /// The rests a digital photo's film effects take on it.
    pub effects: FilmEffects,
    /// Its file's content: a bake made of another is never served for it.
    pub revision: u64,
}

/// Where a dye set's dyes are read: a colour profile, or the mean of other sets.
#[derive(Debug, Clone)]
pub enum DyeSource {
    Profile(Arc<str>),
    Mean(Vec<DyeSet>),
}

#[derive(Debug, Clone)]
pub struct DyeEntry {
    pub set: DyeSet,
    /// The stock whose profile measured them; none for a mean.
    pub name: Option<&'static str>,
    pub source: DyeSource,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct PaperEntry {
    pub paper: PaperStock,
    pub name: &'static str,
    pub brand: &'static str,
    /// Its box's SVG beside its file, drawn as a stock's canister is.
    pub canister: Option<&'static str>,
    /// Its look as its file holds it, baked by `scripts/paper-looks.py`, which the pipeline reads.
    pub look: Arc<str>,
    /// The data sheet's profile it was baked from, which its file keeps; none for a machine measured (the Frontier's).
    pub profile: Option<Arc<str>>,
    /// Its colour compression scale, baked in its look; none where its file bakes none, which then prints as baked.
    pub compression: Option<Arc<crate::paper::CompressionScale>>,
    pub revision: u64,
}

/// A canister's SVG and its bytes' hash, which a drawn image is kept under.
#[derive(Debug, Clone)]
pub struct Canister {
    pub bytes: Arc<[u8]>,
    pub hash: u64,
}
