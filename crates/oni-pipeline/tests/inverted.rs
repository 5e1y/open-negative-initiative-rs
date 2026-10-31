//! A positive read as its own negative: its light inverted, then a negative's conversion, its tones
//! keeping their order as the positive's own conversion keeps them.

use oni_pipeline::conversion;
use oni_render::{Reciprocal, cpu};
use oni_settings::{ConversionMode, PipelineSettings};

fn converted(s: &PipelineSettings, light: f32) -> f32 {
    let k = conversion::stage47(s);
    let source = if s.inverted_source { cpu::reciprocal([light; 4], &Reciprocal { k: conversion::INVERSION, floor: conversion::INVERSION_FLOOR }) } else { [light; 4] };
    cpu::stage47(source, &k)[1]
}

#[test]
fn a_positive_read_as_its_own_negative_keeps_its_tones_order() {
    let mut inverted = PipelineSettings::rest(ConversionMode::Negative, true);
    inverted.inverted_source = true;
    let lights = [0.002f32, 0.01, 0.05, 0.2];
    let out: Vec<f32> = lights.iter().map(|&l| converted(&inverted, l)).collect();
    assert!(out.windows(2).all(|w| w[1] > w[0]), "more light through the slide, a lighter print: {out:?}");
    // Twin: the same light through a negative's own conversion runs the other way.
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    let plain: Vec<f32> = lights.iter().map(|&l| converted(&negative, l)).collect();
    assert!(plain.windows(2).all(|w| w[1] < w[0]), "{plain:?}");
}
