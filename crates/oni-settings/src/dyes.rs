//! The measured dye sets the dye separation unmixes: one per stock profile ONI ships, each written from its maker's
//! data sheet, and a generic colour-negative set, their mean, for a stock no profile measures; each defined by a stock's
//! file (`crate::stocks`).

use crate::stocks::{DyeEntry, DyeSource};

crate::stocks::keyed!(
    /// A dye set by the key a sidecar stores: its stock's spectral dye densities and base. A key no file defines is kept as
    /// stored and unmixes the generic set meanwhile.
    DyeSet, Dyes, "generic-c41"
);

impl DyeSet {
    /// Every set of the catalogue in use, the generic one first.
    pub fn all() -> Vec<DyeSet> {
        crate::stocks::current().dye_sets().iter().map(|d| d.set).collect()
    }

    /// Its file's entry; a key no file defines reads the generic set's.
    pub fn entry(self) -> DyeEntry {
        let catalogue = crate::stocks::current();
        catalogue.dye_set(self).or_else(|| catalogue.dye_set(DyeSet::default())).cloned().expect("the generic set is shipped")
    }

    /// The stock whose profile measured the dyes; a mean names none.
    pub fn stock_name(self) -> Option<&'static str> {
        self.entry().name
    }

    /// No profile measured these dyes: they stand for any colour negative, approximately.
    pub fn is_generic(self) -> bool {
        matches!(self.entry().source, DyeSource::Mean(_))
    }
}

/// The measured dyes `stock_id` writes on its roll's frames: none in black and white, on a slide or for a key the
/// catalogue lacks.
pub fn dye_set(stock_id: &str) -> Option<DyeSet> {
    crate::film::film_stock(Some(stock_id)).and_then(|k| k.dyes)
}

#[cfg(test)]
mod tests;
