//! Each kernel's body as a function a render-graph stage calls, reading a uniform `P: Params` and the
//! accessors its caller defines; the graph built-ins read `load`.

/// Shared by every kernel: luminance, the display gamma, the contrast family.
pub const PRELUDE: &str = include_str!("wgsl/prelude.wgsl");
/// The detail stage's Gaussian taps, which the blur built-in shares.
pub const DETAIL_TAPS: &str = include_str!("wgsl/detail_taps.wgsl");

/// `stage47_px(s)`.
pub const STAGE47: &str = include_str!("wgsl/stage47.wgsl");
/// `output_clip_px(s)`.
pub const OUTPUT_CLIP: &str = include_str!("wgsl/output_clip.wgsl");
/// `exposure_adjust_px(s)`.
pub const EXPOSURE_ADJUST: &str = include_str!("wgsl/exposure_adjust.wgsl");
/// `reciprocal_px(s)`.
pub const RECIPROCAL: &str = include_str!("wgsl/reciprocal.wgsl");
/// `flat_field_px(s, mask)`.
pub const FLAT_FIELD: &str = include_str!("wgsl/flat_field.wgsl");
/// `curves_px(s)`, reading `curves_rgb_lut(k)` and `curves_luma_lut(k)`.
pub const CURVES: &str = include_str!("wgsl/curves.wgsl");
/// `saturation_curve_px(s)`.
pub const SATURATION_CURVE: &str = include_str!("wgsl/saturation_curve.wgsl");
/// `spectrogram_px(s)`.
pub const SPECTROGRAM: &str = include_str!("wgsl/spectrogram.wgsl");
/// `vignette_px(s, at)`.
pub const VIGNETTE: &str = include_str!("wgsl/vignette.wgsl");
/// `color_matrix_px(s)`.
pub const COLOR_MATRIX: &str = include_str!("wgsl/color_matrix.wgsl");
/// `color_cube_px(s)`, reading `cube_node(r, g, b, n)`.
pub const COLOR_CUBE: &str = include_str!("wgsl/color_cube.wgsl");
/// `blend_with_mask_px(picture, background, mask)`.
pub const BLEND_WITH_MASK: &str = include_str!("wgsl/blend_with_mask.wgsl");
/// `texture_moments_of` and `texture_coeff_of`, pure, for a pass or a blur's taps.
pub const TEXTURE_MATH: &str = include_str!("wgsl/texture_math.wgsl");
/// `texture_moments_px(luma)`; needs `TEXTURE_MATH`.
pub const TEXTURE_MOMENTS: &str = include_str!("wgsl/texture_moments.wgsl");
/// `texture_coeff_px(moments)`; needs `TEXTURE_MATH`.
pub const TEXTURE_COEFF: &str = include_str!("wgsl/texture_coeff.wgsl");
/// `texture_px(s, narrow, coeff)`.
pub const TEXTURE: &str = include_str!("wgsl/texture.wgsl");
/// `detail_rows_px(x, y)`, reading `detail_source(x, y)`; needs `DETAIL_TAPS`.
pub const DETAIL_ROWS: &str = include_str!("wgsl/detail_rows.wgsl");
/// `detail_columns_px(s, x, y)`, reading `detail_row(x, y)`; needs `DETAIL_TAPS`.
pub const DETAIL_COLUMNS: &str = include_str!("wgsl/detail_columns.wgsl");
/// `scanner_density_px(s)`.
pub const SCANNER_DENSITY: &str = include_str!("wgsl/scanner_density.wgsl");
/// `scanner_output_px(code, warmth)`.
pub const SCANNER_OUTPUT: &str = include_str!("wgsl/scanner_output.wgsl");
/// `scanner_encode_px(s)`.
pub const SCANNER_ENCODE: &str = include_str!("wgsl/scanner_encode.wgsl");

/// `gaussian_px(p)`, `CIGaussianBlur` along one axis; needs `DETAIL_TAPS`.
pub const GAUSSIAN: &str = include_str!("wgsl/gaussian.wgsl");
/// `affine_px(p)`, a bilinear affine resampling.
pub const AFFINE: &str = include_str!("wgsl/affine.wgsl");
/// `orient_px(p)`, quarter turns and mirrors.
pub const ORIENT: &str = include_str!("wgsl/orient.wgsl");
/// `stroke_mask_px(p)`, a stroke's hard mask.
pub const STROKE_MASK: &str = include_str!("wgsl/stroke_mask.wgsl");
