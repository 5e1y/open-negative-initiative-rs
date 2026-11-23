//! The reversal film a slide is projected as.

use super::PipelineSettings;
use crate::virtual_film::VirtualStock;

impl PipelineSettings {
    /// The reversal film a slide is projected as: a digital photo's on one; a scanned slide's, its roll's stock where
    /// its sheet is profiled, named in its dyes' key, else the default slide film. None on any other frame, and on a
    /// slide kept on a legacy print, whose render stays its own.
    pub fn projector(&self) -> Option<VirtualStock> {
        if let Some(f) = self.film() {
            return f.stock.entry().reversal.then_some(f.stock);
        }
        if !self.mode.is_slide() || self.legacy().is_some() {
            return None;
        }
        let named = crate::stocks::current().virtual_film(VirtualStock::of(self.dye_set.key())).filter(|e| e.reversal).map(|e| e.stock);
        Some(named.unwrap_or(VirtualStock::SLIDE))
    }
}
