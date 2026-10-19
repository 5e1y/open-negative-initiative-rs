//! The histograms the panel draws: the picture's, measured as Swift's canvas measures it, and every
//! plot traced as `DSHistogram` traces it, at the size it is shown.

mod measure;
mod trace;

pub use measure::*;
pub use trace::*;
