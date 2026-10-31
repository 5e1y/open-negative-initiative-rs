//! The scalar references, each written beside its kernel under `kernel/`, gathered here by name.

pub use crate::kernel::affine::affine;
pub use crate::kernel::color_cube::color_cube;
pub use crate::kernel::color_matrix::color_matrix;
pub use crate::kernel::compositing::blend_with_mask;
pub use crate::kernel::curves::curves;
pub use crate::kernel::detail::{detail_rows, detail};
pub use crate::kernel::exposure_adjust::exposure_adjust;
pub use crate::kernel::reciprocal::reciprocal;
pub use crate::kernel::flat_field::flat_field;
pub use crate::kernel::gaussian::gaussian;
pub use crate::kernel::grid::Grid;
pub use crate::kernel::orient::orient;
pub use crate::kernel::output_clip::output_clip;
pub use crate::kernel::saturation_curve::saturation_curve;
pub use crate::kernel::scanner::{scanner_density, scanner_output, scanner_encode};
pub use crate::kernel::spectrogram::spectrogram;
pub use crate::kernel::stage47::stage47;
pub use crate::kernel::stroke_mask::stroke_mask;
pub use crate::kernel::texture::{texture_moments, texture_coeff, texture};
pub use crate::kernel::vignette::vignette;
