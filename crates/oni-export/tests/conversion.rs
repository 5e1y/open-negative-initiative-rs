//! The conversion into the export's space shared among the cores, against the same conversion run
//! whole on one thread.

use oni_colour::{Quantised, Space};
use oni_export::{Format, Image, Settings, converted};

fn settings(format: Format, space: Space) -> Settings {
    let mut s = Settings::default();
    s.set_format(format);
    s.set_space(space);
    s
}

/// `converted` as one thread runs it: the whole frame on the space's curve, premultiplied, rounded.
fn converted_alone(image: &Image, export: &Settings) -> Quantised {
    let scale = if export.format().bits() == 16 { 65535.0 } else { 255.0 };
    let mut encoded = export.space().encoded(&image.pixels).unwrap();
    for (p, s) in encoded.chunks_exact_mut(4).zip(image.pixels.chunks_exact(4)) {
        let a = s[3].clamp(0.0, 1.0);
        p[..3].iter_mut().for_each(|v| *v *= a);
        p[3] = a;
    }
    let quantised = encoded.iter().map(|v| (v.clamp(0.0, 1.0) * scale).round());
    if export.format().bits() == 16 { Quantised::Sixteen(quantised.map(|v| v as u16).collect()) } else { Quantised::Eight(quantised.map(|v| v as u8).collect()) }
}

#[test]
fn the_conversion_on_every_core_is_the_conversion_on_one() {
    // Odd sides, values past both ends and a varying alpha, so every piece and every clip is reached.
    let (w, h) = (333, 517);
    let pixels: Vec<f32> = (0..w * h * 4).map(|i| if i % 4 == 3 { (i % 7) as f32 / 6.0 } else { ((i * 7919) % 1201) as f32 / 1000.0 - 0.1 }).collect();
    let image = Image { width: w, height: h, pixels };
    for (format, space) in [(Format::Jpeg, Space::Srgb), (Format::Tiff16, Space::DisplayP3), (Format::Tiff16, Space::ProPhoto)] {
        let export = settings(format, space);
        let alone = converted_alone(&image, &export);
        assert!(converted(&image, &export).unwrap().pixels == alone, "{format:?}");
        // Twin: one sample one level off is caught.
        let mut off = alone.clone();
        match &mut off {
            Quantised::Eight(v) => v[w * 4 * 300 + 1] ^= 1,
            Quantised::Sixteen(v) => v[w * 4 * 300 + 1] ^= 1,
        }
        assert!(converted(&image, &export).unwrap().pixels != off);
    }
}
