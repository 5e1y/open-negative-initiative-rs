//! How a frame's file is read before any stage: the decode's choices, which the decode and the dust
//! patches cut on it both read from this one value, so the two cannot drift apart.

use crate::frame::RgbPasses;
use crate::{ConversionMode, PipelineSettings, SourceEncoding};
use serde::Serialize;

/// How a frame's sidecar asks a file outside the RAW engine to be read.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Reading {
    /// Its windows were measured on its channels converted to the working primaries.
    pub primaries_converted: bool,
    /// The encoding the hand chose for a file naming no profile.
    pub source_encoding: Option<SourceEncoding>,
    /// The three passes a lead names, decoded and laid as one frame.
    pub passes: Option<RgbPasses>,
    /// A positive's RAW, balanced at `oni_decode::SLIDE_KELVIN`.
    pub slide: bool,
}

impl Reading {
    pub fn of(s: &PipelineSettings) -> Reading {
        let slide = s.mode == ConversionMode::Positive;
        Reading { primaries_converted: s.primaries_converted, source_encoding: s.source_encoding, passes: s.rgb_passes.clone(), slide }
    }

    /// FNV-1a over the reading: another reading stamps another decode.
    pub fn stamp(&self) -> u64 {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }
}
