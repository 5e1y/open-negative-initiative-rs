//! The migrations only documents from the builds before 1.0 need (version 13 and under), run in
//! their stored order by `from_before_1_0`; every release since writes version 14 or later.

use crate::axis::{density_axis, pipeline};
use crate::blocks::{ChromaDenoise, Gains, Sharpen, chroma_decompression, separation};
use crate::codec::{RADIUS_REFERENCE_SIDE, number, with_settings};
use crate::levels::Levels;
use crate::modes::ConversionMode;
use crate::tolerant::{Object, number_f32};
use serde_json::Value;

/// The first version a released build wrote.
pub const FIRST_RELEASED_VERSION: i64 = 14;
/// The version the per-channel windows changed unit, named so a later bump cannot rescale them.
pub const DENSITY_AXIS_VERSION: i64 = 3;
/// The version the two neighbourhood radii went from a fraction of the largest side to pixels.
pub const RADIUS_PIXELS_VERSION: i64 = 4;
/// The version the positive rest left a window keeping the pedestal's veil and a median at gamma 1.
pub const POSITIVE_REST_VERSION: i64 = 5;
/// The version the mosaic decode stopped assuming an orange base.
pub const NEUTRAL_DECODE_GAINS_VERSION: i64 = 6;
/// The version the dye separation's rest moved from a dose of one to `separation::FORMER_REST`.
pub const SEPARATION_REST_VERSION: i64 = 7;
/// The version the sharpening's rest moved from `Sharpen::FORMER_AMOUNT` to its new amount.
pub const SHARPEN_REST_VERSION: i64 = 8;
/// The version Lighting replaced the former windows and curves; older documents are fitted to it.
pub const LIGHTING_VERSION: i64 = 9;
/// The version the chroma denoise's rest moved from off to its neutral dose.
pub const CHROMA_REST_VERSION: i64 = 10;
/// The version Lighting's black point came to rest lifted.
pub const BLACK_POINT_REST_VERSION: i64 = 11;
/// The version the RAW decode lost Apple's shadow bias and a mosaic RAW came to rest knee-free.
pub const MOSAIC_AXIS_VERSION: i64 = 12;
/// The version Chroma Decompression came to rest on the film's colour, 0.
pub const DECOMPRESSION_REST_VERSION: i64 = 13;

/// The median earlier versions rested a positive on: the identity, hence no gamma at all.
pub const FORMER_POSITIVE_MID: f32 = 0.5;
/// What the former decode's shadow bias took off every channel, absorbed by the former pedestal.
pub const FORMER_SHADOW_BIAS: f32 = 0.0038;
/// The neutral decode (5000 K) against the retired mask-compensating one (2500 K) on the reference camera.
pub const NEUTRAL_DECODE_RATIO: [f32; 3] = [1.7683, 1.1011, 0.38227];
/// The per-channel windows' frozen names in the JSON.
pub const PER_CHANNEL_KEYS: [&str; 3] = ["red", "green", "blue"];
/// The handles that carry a density; the median is normalised between them and follows on its own.
pub const SCALED_HANDLES: [&str; 2] = ["black", "white"];

pub fn from_before_1_0(document: Object, stored_version: i64, mosaic_sourced: &dyn Fn() -> bool) -> Object {
    let mut aged = if stored_version < DENSITY_AXIS_VERSION { in_density(document) } else { document };
    // The radii's unit change reads the fractions the retirements are written in, so it runs first.
    if stored_version < RADIUS_PIXELS_VERSION {
        aged = in_pixels(aged, stored_version < 2);
    }
    if stored_version < POSITIVE_REST_VERSION {
        aged = off_former_positive_rest(aged);
    }
    if stored_version < NEUTRAL_DECODE_GAINS_VERSION && mosaic_sourced() {
        aged = neutral_decode_gains(aged);
    }
    if stored_version < SEPARATION_REST_VERSION {
        aged = off_former_separation_rest(aged);
    }
    if stored_version < SHARPEN_REST_VERSION {
        aged = off_former_sharpen_rest(aged);
    }
    if stored_version < CHROMA_REST_VERSION {
        aged = off_former_chroma_rest(aged);
    }
    if stored_version < BLACK_POINT_REST_VERSION {
        aged = keeping_former_black_point(aged);
    }
    if stored_version < MOSAIC_AXIS_VERSION && mosaic_sourced() {
        aged = off_former_mosaic_axis(aged);
    }
    if stored_version < DECOMPRESSION_REST_VERSION {
        aged = keeping_former_decompression(aged);
    }
    aged
}

/// The ceiling that reads the pinned decode as a former one read the biased decode.
pub fn former_axis(ceiling: f32) -> f32 {
    let lift = density_axis::pedestal(ceiling) - FORMER_SHADOW_BIAS;
    if lift <= density_axis::pedestal(density_axis::MOSAIC_REST) {
        return density_axis::MOSAIC_REST;
    }
    (-lift.log10()).min(*density_axis::mosaic_range().end())
}

fn mode_is(settings: &Object, mode: ConversionMode) -> bool {
    settings.get("mode").and_then(Value::as_str) == Some(mode.raw_value())
}

/// A frame from before renders on at the former rest; a dose off it was the hand's, and stays.
pub fn keeping_former_decompression(document: Object) -> Object {
    with_settings(document, |s| match number_f32(s.get("chromaDecompression")) {
        Some(stored) => {
            s.insert("chromaDecompressionByHand".into(), Value::Bool(stored != chroma_decompression::FORMER_REST));
        }
        None => {
            s.insert("chromaDecompression".into(), number(chroma_decompression::FORMER_REST));
        }
    })
}

/// A mosaic negative never balanced moves to the knee-free rest; one placed keeps its axis.
pub fn off_former_mosaic_axis(document: Object) -> Object {
    with_settings(document, |s| {
        s.insert("decodedFromMosaic".into(), Value::Bool(true));
        let stored = number_f32(s.get("densityCeiling")).unwrap_or_else(density_axis::base);
        let positive = mode_is(s, ConversionMode::Positive);
        let former_black = Levels::resting_black(
            if positive { ConversionMode::Positive } else { ConversionMode::Negative },
            density_axis::base(),
        );
        let empty = Object::new();
        let levels = s.get("levels").and_then(Value::as_object).unwrap_or(&empty);
        let windows_at_rest = PER_CHANNEL_KEYS.iter().all(|key| match levels.get(*key).and_then(Value::as_object) {
            None => true,
            Some(window) => {
                number_f32(window.get("black")).unwrap_or(former_black) == former_black
                    && number_f32(window.get("white")).unwrap_or(pipeline::DMAX) == pipeline::DMAX
            }
        });
        let stops: Vec<f32> = s
            .get("gains")
            .and_then(|g| g.get("stops"))
            .and_then(Value::as_array)
            .and_then(|a| a.iter().map(|v| number_f32(Some(v))).collect::<Option<Vec<_>>>())
            .unwrap_or_default();
        let gains_at_rest = stops.iter().all(|&v| v == 0.0);
        // Dust corrections were placed on the lens profile's geometry, which the pinned decode drops.
        if s.get("corrections").and_then(Value::as_array).is_some_and(|c| !c.is_empty()) {
            s.insert("lensCorrected".into(), Value::Bool(true));
        }
        let ceiling = if stored == density_axis::base() && windows_at_rest && gains_at_rest && !positive {
            density_axis::MOSAIC_REST
        } else {
            former_axis(stored)
        };
        s.insert("densityCeiling".into(), number(ceiling));
    })
}

/// Shifts a mosaic RAW's stored gain by the decode's own move, keeping each channel's density.
pub fn neutral_decode_gains(document: Object) -> Object {
    with_settings(document, |s| {
        let Some(Value::Object(gains)) = s.get_mut("gains") else { return };
        let Some(stored) = gains.get("stops").and_then(Value::as_array) else { return };
        let Some(stored) = stored.iter().map(|v| number_f32(Some(v))).collect::<Option<Vec<_>>>() else {
            return;
        };
        if stored.len() != 3 {
            return;
        }
        let shifted: Vec<Value> = (0..3)
            .map(|i| number((stored[i] - NEUTRAL_DECODE_RATIO[i].log2()).clamp(Gains::RANGE.0, Gains::RANGE.1)))
            .collect();
        gains.insert("stops".into(), Value::Array(shifted));
    })
}

/// Scales the per-channel windows onto the density axis; linked and luma read a dimensionless signal.
pub fn in_density(document: Object) -> Object {
    with_settings(document, |s| {
        let Some(Value::Object(levels)) = s.get_mut("levels") else { return };
        for key in PER_CHANNEL_KEYS {
            let Some(Value::Object(window)) = levels.get_mut(key) else { continue };
            for handle in SCALED_HANDLES {
                // Scaled at the width a handle is stored at, so the result matches the closed form.
                if let Some(carried) = number_f32(window.get(handle)) {
                    window.insert(handle.into(), number(carried * pipeline::DMAX));
                }
            }
        }
    })
}

/// A stored fraction as pixels, held inside the slider's track so every kept dose stays retouchable.
fn scaled(fraction: f32, range: (f32, f32)) -> Value {
    number(((f64::from(fraction) * RADIUS_REFERENCE_SIDE) as f32).clamp(range.0, range.1))
}

/// Turns the two dosed radii into full-resolution pixels, retiring the doses nobody chose first.
pub fn in_pixels(document: Object, retiring_chroma: bool) -> Object {
    with_settings(document, |s| {
        if let Some(Value::Object(chroma)) = s.get_mut("chroma") {
            let radius = number_f32(chroma.get("radius"));
            let force = number_f32(chroma.get("force"));
            match (radius, force) {
                (Some(r), Some(f)) if retiring_chroma && ChromaDenoise::retired(r, f) => {
                    let neutral = ChromaDenoise::default();
                    chroma.insert("radius".into(), number(neutral.radius));
                    chroma.insert("force".into(), number(neutral.force));
                }
                (Some(r), _) => {
                    chroma.insert("radius".into(), scaled(r, ChromaDenoise::RADIUS_RANGE));
                }
                _ => {}
            }
        }
        if let Some(Value::Object(sharpen)) = s.get_mut("sharpen")
            && let Some(radius) = number_f32(sharpen.get("radius"))
        {
            // The range's floor is a rest no gesture placed: it reads back as the anchor.
            let value = if radius == Sharpen::FORMER_REST {
                number(Sharpen::RADIUS_ANCHOR)
            } else {
                scaled(radius, Sharpen::radius_range())
            };
            sharpen.insert("radius".into(), value);
        }
    })
}

/// A dose written on the former rest moves to the new one; a hand's dose differs from it.
pub fn off_former_separation_rest(document: Object) -> Object {
    with_settings(document, |s| {
        if number_f32(s.get("separation")) == Some(separation::FORMER_BASE) {
            s.insert("separation".into(), number(separation::FORMER_REST));
        }
    })
}

/// A Lighting that never wrote a black point reads zero, so a hand's lifted black is never lifted twice.
pub fn keeping_former_black_point(document: Object) -> Object {
    with_settings(document, |s| {
        if let Some(Value::Object(print)) = s.get_mut("print")
            && !print.contains_key("blackPoint")
        {
            print.insert("blackPoint".into(), Value::from(0));
        }
    })
}

pub fn off_former_chroma_rest(document: Object) -> Object {
    with_settings(document, |s| {
        if let Some(Value::Object(chroma)) = s.get_mut("chroma")
            && number_f32(chroma.get("radius")) == Some(0.0)
            && number_f32(chroma.get("force")) == Some(0.0)
        {
            let neutral = ChromaDenoise::default();
            chroma.insert("radius".into(), number(neutral.radius));
            chroma.insert("force".into(), number(neutral.force));
        }
    })
}

pub fn off_former_sharpen_rest(document: Object) -> Object {
    with_settings(document, |s| {
        if let Some(Value::Object(sharpen)) = s.get_mut("sharpen")
            && number_f32(sharpen.get("amount")) == Some(Sharpen::FORMER_AMOUNT)
        {
            sharpen.insert("amount".into(), number(Sharpen::default().amount));
        }
    })
}

/// Takes a positive off the rest earlier versions left it on; any other mode passes key for key.
pub fn off_former_positive_rest(document: Object) -> Object {
    with_settings(document, |s| {
        if !mode_is(s, ConversionMode::Positive) {
            return;
        }
        let Some(Value::Object(levels)) = s.get_mut("levels") else { return };
        for key in PER_CHANNEL_KEYS {
            let Some(Value::Object(window)) = levels.get_mut(key) else { continue };
            if number_f32(window.get("black")) == Some(0.0) && number_f32(window.get("white")) == Some(pipeline::DMAX) {
                let black = Levels::resting_black(ConversionMode::Positive, density_axis::base());
                window.insert("black".into(), number(black));
            }
        }
        if let Some(Value::Object(linked)) = levels.get_mut("linked")
            && number_f32(linked.get("mid")) == Some(FORMER_POSITIVE_MID)
        {
            linked.insert("mid".into(), number(Levels::resting_mid(ConversionMode::Positive)));
        }
    })
}
