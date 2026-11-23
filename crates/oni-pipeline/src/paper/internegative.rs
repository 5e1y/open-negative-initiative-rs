//! A slide on a colour paper as a lab printed one: copied on a virtual internegative, the light its scan holds exposing
//! that film as a scene does, then the negative printed as a digital photo's is. The copy is filtered as the film's own
//! reference reads a grey, so the slide's greys print neutral; its black is the film's bare base, as a negative's.

use super::printing::reference_grey;
use crate::conversion;
use crate::film::{self, GREY};
use oni_render::ColorMatrix;
use oni_settings::axis::density_axis;
use oni_settings::{ConversionMode, PipelineSettings, VirtualFilm, VirtualStock};

/// The film a slide is copied on: of the colour negatives, the one that kept the owner's slides' hues and chroma
/// closest to ONI's own render of them, its blues the least turned (#3300).
pub const STOCK: VirtualStock = VirtualStock::of("kodak-vision3-50d");

/// The internegative a slide printed on a paper is read from: the settings that read and print it, and the matrix that
/// turns the slide's decoded scan into the light exposing it.
#[derive(Clone, Debug, PartialEq)]
pub struct Copy {
    pub settings: PipelineSettings,
    pub light: ColorMatrix,
}

/// `s`'s copy where it is a slide printed on a paper, none for any other frame: its reference grey exposes the film at
/// the film's grey, each channel through the slide's gains; Lighting, the paper (or the neutral one) and the colour
/// density are the slide's.
pub fn copy(s: &PipelineSettings) -> Option<Copy> {
    if !s.mode.reads_positive() || s.paper().is_none() || s.print.bypassed {
        return None;
    }
    let grey = reference_grey(s);
    // The scan's light, read as stage 4 reads it, times this lands the reference grey on the film's grey.
    let gain: [f32; 3] = std::array::from_fn(|c| s.gains.stops[c].exp2() * GREY * 10f32.powf(grey[c]));
    let pedestal = density_axis::pedestal(conversion::rendered_ceiling(s));
    let mut settings = film::on_reference(STOCK, &STOCK.reference());
    settings.mode = ConversionMode::Digital;
    settings.virtual_film = Some(VirtualFilm { stock: STOCK, exposure: 0.0 });
    settings.decoded_from_mosaic = s.decoded_from_mosaic;
    settings.print = s.print;
    settings.print_paper = s.print_paper;
    settings.paperless = s.paperless;
    settings.density = s.density;
    let light = ColorMatrix {
        r_vector: [gain[0], 0.0, 0.0, 0.0],
        g_vector: [0.0, gain[1], 0.0, 0.0],
        b_vector: [0.0, 0.0, gain[2], 0.0],
        bias_vector: [gain[0] * pedestal, gain[1] * pedestal, gain[2] * pedestal, 0.0],
        ..ColorMatrix::default()
    };
    Some(Copy { settings, light })
}
