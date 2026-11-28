//! The papers redone: a photo of an earlier format names a paper only where it chose one over the pure inversion, from
//! format 73, and that paper still ships; a legacy print laid takes none, printing on its own curve. Its print on a paper
//! is not kept (owner's call): those papers were never released.

use super::chain::{OPTIONAL_PAPER_VERSION, PAPERS_REDONE_VERSION};
use crate::PipelineSettings;

/// The keys of the papers that left: ONI Standard, and the keys the data sheets' papers once answered to.
const LEFT: [&str; 7] = [
    "oni-standard",
    "kodak-supra-endura",
    "kodak-ultra-endura",
    "kodak-endura-premier-datasheet",
    "kodak-portra-endura",
    "kodak-ektachrome-radiance-iii",
    "fujifilm-crystal-archive-type-ii",
];

/// `settings` of `version` on the paper its person chose, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings) -> PipelineSettings {
    if version >= PAPERS_REDONE_VERSION {
        return settings;
    }
    let chosen = version >= OPTIONAL_PAPER_VERSION && settings.legacy().is_none();
    let print_paper = settings.print_paper.filter(|p| chosen && !LEFT.contains(&p.key()));
    PipelineSettings { print_paper, ..settings }
}
