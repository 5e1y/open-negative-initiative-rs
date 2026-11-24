//! A film's look on a colour paper as a lab printed a positive: copied on a virtual internegative, the look's display
//! light exposing that film as a scene does, then the negative printed as a digital photo's is.

use crate::film;
use oni_settings::{ConversionMode, PipelineSettings, VirtualFilm, VirtualStock};

/// The film a look is copied on: of the colour negatives, the one that kept the owner's slides' hues and chroma closest
/// to ONI's own render of them, its blues the least turned (#3300).
pub const STOCK: VirtualStock = VirtualStock::of("kodak-vision3-50d");

/// The settings that read and print the internegative of `s`'s look on a paper, none for any other frame: Lighting, the
/// paper and the colour tools are the frame's.
pub fn copy(s: &PipelineSettings) -> Option<PipelineSettings> {
    let looked = s.film().is_some_and(|f| f.stock.look().is_some());
    if !looked || s.paper().is_none() || s.print.bypassed {
        return None;
    }
    let mut settings = film::on_reference(STOCK, &STOCK.reference());
    settings.mode = ConversionMode::Digital;
    settings.virtual_film = Some(VirtualFilm { stock: STOCK, exposure: 0.0 });
    settings.decoded_from_mosaic = s.decoded_from_mosaic;
    settings.print = s.print;
    settings.print_paper = s.print_paper;
    settings.density = s.density;
    settings.spectrogram = s.spectrogram;
    settings.saturation = s.saturation.clone();
    Some(settings)
}
