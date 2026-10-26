//! The one-slider demos: Details' Sharpening, Texture and Chroma Denoise on a detail of the photograph
//! framed, the Roll tab's Density Ceiling and Dye Separation on all of it.

use oni_app::fields::Field;
use oni_settings::PipelineSettings;
use oni_settings::blocks::ChromaDenoise;

use super::slider::{OneSlider, Spec, View};
use crate::encode::Tuning;
use crate::photo;

/// Sharpening from none to its most: the cap's brim, the grating and the sand crisp.
pub fn sharpening() -> OneSlider {
    OneSlider::new(Spec {
        topic: "sharpening",
        component: "OneRow",
        settings: photo::framed,
        view: View::Detail { at: (0.66, 0.5), zoom: 2 },
        field: Field::SharpenAmount,
        to: 1.0,
        levels: 6.0,
        tuning: Tuning { tolerance: 12, colours: 96 },
    })
}

/// Texture from its rest up to its most: the sand's grain and ridges stand out.
pub fn texture() -> OneSlider {
    OneSlider::new(Spec {
        topic: "texture",
        component: "OneRow",
        settings: photo::framed,
        view: View::Detail { at: (0.5, 0.42), zoom: 2 },
        field: Field::Texture,
        to: 1.0,
        levels: 6.0,
        tuning: Tuning { tolerance: 12, colours: 96 },
    })
}

/// Chroma Denoise from its rest to its most on the grating and the sand, its radius widest: the
/// colour mottling smooths, the grain stays.
pub fn chroma_denoise() -> OneSlider {
    OneSlider::new(Spec {
        topic: "chroma-denoise",
        component: "OneRow",
        settings: widest_chroma,
        view: View::Detail { at: (0.62, 0.6), zoom: 3 },
        field: Field::ChromaAmount,
        to: 1.0,
        levels: 4.0,
        tuning: Tuning { tolerance: 7, colours: 112 },
    })
}

/// The photograph framed, Chroma Denoise reaching its widest so its Amount is seen at work.
fn widest_chroma(s: PipelineSettings) -> PipelineSettings {
    let mut s = photo::framed(s);
    s.chroma.radius = ChromaDenoise::RADIUS_RANGE.1;
    s
}

/// Density Ceiling lowered from its rest to its least: the densest parts of the negative, the cream
/// sail and the sand, close to a flat light.
pub fn density_ceiling() -> OneSlider {
    OneSlider::new(Spec {
        topic: "density-ceiling",
        component: "OneRow",
        settings: photo::framed,
        view: View::Whole,
        field: Field::DensityCeiling,
        to: 0.0,
        levels: 16.0,
        tuning: Tuning { tolerance: 10, colours: 112 },
    })
}

/// Dye Separation from its rest to its most: the sails' colours part while the greys stay.
pub fn dye_separation() -> OneSlider {
    OneSlider::new(Spec {
        topic: "dye-separation",
        component: "OneRow",
        settings: photo::framed,
        view: View::Whole,
        field: Field::DyeSeparation,
        to: 1.0,
        levels: 5.0,
        tuning: Tuning { tolerance: 3, colours: 80 },
    })
}
