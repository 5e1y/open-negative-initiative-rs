//! One file per kernel: its arguments and their uniform rows, and its scalar reference; the body is
//! `wgsl/<name>.wgsl`. A uniform block opens on a `vec4<u32>` head, then `vec4<f32>` rows.

use crate::Image;
use crate::prelude::*;
use detail::{Taps, detail_reach};
use grid::{Grid, mix4};
use bytemuck::{Pod, Zeroable};

pub(crate) mod affine;
pub(crate) mod color_cube;
pub(crate) mod color_matrix;
pub(crate) mod compositing;
pub(crate) mod curves;
pub(crate) mod detail;
pub(crate) mod exposure_adjust;
pub(crate) mod flat_field;
pub(crate) mod gaussian;
pub(crate) mod grid;
pub(crate) mod orient;
pub(crate) mod output_clip;
pub(crate) mod saturation_curve;
pub(crate) mod scanner;
pub(crate) mod spectrogram;
pub(crate) mod stage47;
pub(crate) mod stroke_mask;
pub(crate) mod texture;
pub(crate) mod vignette;
