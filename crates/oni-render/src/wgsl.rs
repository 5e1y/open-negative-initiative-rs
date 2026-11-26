//! Each kernel's body as a function a render-graph stage calls, reading a uniform `P: Params` and the
//! accessors its caller defines; the graph built-ins read `load`.

/// Shared by every kernel: luminance, the display gamma, the contrast family.
pub const PRELUDE: &str = include_str!("wgsl/prelude.wgsl");
/// The detail stage's Gaussian taps, which the blur built-in shares.
pub const DETAIL_TAPS: &str = include_str!("wgsl/detail_taps.wgsl");

/// `negative_px(s)`.
pub const NEGATIVE: &str = concat!(include_str!("wgsl/negative.wgsl"), include_str!("wgsl/negative_plain.wgsl"));
/// `negative_px(s)` with the colour tools on its scene.
pub const NEGATIVE_COLOURED: &str = concat!(include_str!("wgsl/negative.wgsl"), include_str!("wgsl/negative_colour.wgsl"));
/// `paper_px(t)`, on the window's shares.
pub const PAPER: &str = include_str!("wgsl/paper.wgsl");
/// `inversion_px(t)`, on the window's shares where no paper prints.
pub const INVERSION: &str = include_str!("wgsl/inversion.wgsl");
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
/// `vignette_px(s, at)`.
pub const VIGNETTE: &str = include_str!("wgsl/vignette.wgsl");
/// The grain's lattice, its helpers and `grain_field_px(p)`, the layers' field.
pub const GRAIN: &str = include_str!("wgsl/grain.wgsl");
/// `grain_px(p)`, the film grained, reading its layers' field through `load1`; after `GRAIN`.
pub const GRAIN_LAID: &str = include_str!("wgsl/grain_laid.wgsl");
/// `halation_source_px(e)`.
pub const HALATION_SOURCE: &str = include_str!("wgsl/halation_source.wgsl");
/// `halation_px(e, glow)`.
pub const HALATION: &str = include_str!("wgsl/halation.wgsl");
/// `halation_laid_px(p)`, the glow read off the film's plate through `load1`.
pub const HALATION_LAID: &str = include_str!("wgsl/halation_laid.wgsl");
/// `color_matrix_px(s)`.
pub const COLOR_MATRIX: &str = include_str!("wgsl/color_matrix.wgsl");
/// `color_cube_px(s)`, reading `cube_node(r, g, b, n)`.
pub const COLOR_CUBE: &str = include_str!("wgsl/color_cube.wgsl");
/// `blend_with_mask_px(picture, background, mask)`.
pub const BLEND_WITH_MASK: &str = include_str!("wgsl/blend_with_mask.wgsl");
/// `texture_moments_of` and `texture_coeff_of`, pure, for a blur's taps.
pub const TEXTURE_MATH: &str = include_str!("wgsl/texture_math.wgsl");
/// `texture_px(s, narrow, coeff)`.
pub const TEXTURE: &str = include_str!("wgsl/texture.wgsl");
/// `detail_rows_px(x, y)`, reading `detail_source(x, y)`; needs `DETAIL_TAPS`.
pub const DETAIL_ROWS: &str = include_str!("wgsl/detail_rows.wgsl");
/// `detail_columns_px(s, x, y)`, reading `detail_row(x, y)`; needs `DETAIL_TAPS`.
pub const DETAIL_COLUMNS: &str = include_str!("wgsl/detail_columns.wgsl");
/// `scanner_output_px(code, warmth)`.
pub const SCANNER_OUTPUT: &str = include_str!("wgsl/scanner_output.wgsl");
/// `scanner_encode_px(s)`.
pub const SCANNER_ENCODE: &str = include_str!("wgsl/scanner_encode.wgsl");
/// `dye_amounts_px(s)`, reading `dye_table(i, row)`.
pub const DYE_AMOUNTS: &str = include_str!("wgsl/dye_amounts.wgsl");
/// `held_black_px(lit, rest)`.
pub const HELD_BLACK: &str = include_str!("wgsl/held_black.wgsl");
/// `projection_px(s)`, reading `projection_table(i, row)`.
pub const PROJECTION: &str = include_str!("wgsl/projection.wgsl");
/// `neutral_negative_px(s)`.
pub const NEUTRAL_NEGATIVE: &str = include_str!("wgsl/neutral_negative.wgsl");
/// `virtual_negative_px(s)`, its `virtual_exposure_px(s)` and `virtual_development_px(e)`, reading `negative_table(i, j)`.
pub const VIRTUAL_NEGATIVE: &str = include_str!("wgsl/virtual_negative.wgsl");

/// `gaussian_px(p)`, `CIGaussianBlur` along one axis; needs `DETAIL_TAPS`.
pub const GAUSSIAN: &str = include_str!("wgsl/gaussian.wgsl");
/// `affine_px(p)`, a bilinear affine resampling.
pub const AFFINE: &str = include_str!("wgsl/affine.wgsl");
/// `warp_px(p)`, stage 0 and the lens corrections in one bilinear read per channel.
pub const WARP: &str = include_str!("wgsl/warp.wgsl");
/// `orient_px(p)`, quarter turns and mirrors.
pub const ORIENT: &str = include_str!("wgsl/orient.wgsl");
/// `stroke_mask_px(p)`, a stroke's hard mask.
pub const STROKE_MASK: &str = include_str!("wgsl/stroke_mask.wgsl");
