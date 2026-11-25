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

/// The paper reading a colour negative: its key, its spectral sensitivities as its file holds them, and the film's own
/// filtration through it, each layer's exposure offset in log exposure.
#[derive(Debug, Clone)]
pub struct Reader {
    pub paper: &'static str,
    pub sensitivity: Arc<str>,
    pub filtration: [f64; 3],
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
    /// A film of no character, its profile its density range alone.
    pub neutral: bool,
    /// The paper whose gaze reads its negative; none scans it as a camera does.
    pub reader: Option<Reader>,
    /// Its own DIR coupler matrix, the common one where none.
    pub couplers: Option<[[f64; 3]; 3]>,
    /// The rests a digital photo's film effects take on it.
    pub effects: FilmEffects,
    /// The look a `.cube` file gives it: a digital photo on it is read through these nodes, not its negative.
    pub look: Option<super::cube::Look>,
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

/// What a paper prints with: a data sheet's profile, or the Frontier SP-3000's fit.
#[derive(Debug, Clone)]
pub enum Print {
    Profile(Arc<str>),
    Frontier(Arc<str>),
}

#[derive(Debug, Clone)]
pub struct PaperEntry {
    pub paper: PaperStock,
    pub name: &'static str,
    pub brand: &'static str,
    pub print: Print,
    pub revision: u64,
}

/// A canister's SVG and its bytes' hash, which a drawn image is kept under.
#[derive(Debug, Clone)]
pub struct Canister {
    pub bytes: Arc<[u8]>,
    pub hash: u64,
}
