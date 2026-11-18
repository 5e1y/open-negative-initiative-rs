//! Each panel field's and block's Slint `Store` property, by its index on the Rust side.

use crate::Store;
use crate::fields::{Block, Field};
use slint::SharedString;

macro_rules! fields {
    ($($variant:ident => $fraction:ident, $get:ident, $readout:ident, $value:ident;)*) => {
        pub(super) fn set_fraction(g: &Store, f: Field, v: f32) {
            match f { $(Field::$variant => g.$fraction(v),)* }
        }
        /// The fraction the panel draws, read back by the checks.
        pub fn fraction(g: &Store, f: Field) -> f32 {
            match f { $(Field::$variant => g.$get(),)* }
        }
        pub(super) fn set_readout(g: &Store, f: Field, v: SharedString) {
            match f { $(Field::$variant => g.$readout(v),)* }
        }
        pub(super) fn set_value(g: &Store, f: Field, v: SharedString) {
            match f { $(Field::$variant => g.$value(v),)* }
        }
    };
}

fields! {
    Temperature => set_temperature_fraction, get_temperature_fraction, set_temperature_readout, set_temperature_value;
    Tint => set_tint_fraction, get_tint_fraction, set_tint_readout, set_tint_value;
    ColorDensity => set_color_density_fraction, get_color_density_fraction, set_color_density_readout, set_color_density_value;
    Exposure => set_exposure_fraction, get_exposure_fraction, set_exposure_readout, set_exposure_value;
    Highlights => set_highlights_fraction, get_highlights_fraction, set_highlights_readout, set_highlights_value;
    Shadows => set_shadows_fraction, get_shadows_fraction, set_shadows_readout, set_shadows_value;
    WhitePoint => set_white_point_fraction, get_white_point_fraction, set_white_point_readout, set_white_point_value;
    BlackPoint => set_black_point_fraction, get_black_point_fraction, set_black_point_readout, set_black_point_value;
    Contrast => set_contrast_fraction, get_contrast_fraction, set_contrast_readout, set_contrast_value;
    Vignette => set_vignette_fraction, get_vignette_fraction, set_vignette_readout, set_vignette_value;
    Chrome => set_chrome_fraction, get_chrome_fraction, set_chrome_readout, set_chrome_value;
    BalanceStrength => set_balance_strength_fraction, get_balance_strength_fraction, set_balance_strength_readout, set_balance_strength_value;
    DensityCeiling => set_density_ceiling_fraction, get_density_ceiling_fraction, set_density_ceiling_readout, set_density_ceiling_value;
    DyeSeparation => set_dye_separation_fraction, get_dye_separation_fraction, set_dye_separation_readout, set_dye_separation_value;
    Angle => set_angle_fraction, get_angle_fraction, set_angle_readout, set_angle_value;
    SharpenAmount => set_sharpen_amount_fraction, get_sharpen_amount_fraction, set_sharpen_amount_readout, set_sharpen_amount_value;
    SharpenRadius => set_sharpen_radius_fraction, get_sharpen_radius_fraction, set_sharpen_radius_readout, set_sharpen_radius_value;
    SharpenHalo => set_sharpen_halo_fraction, get_sharpen_halo_fraction, set_sharpen_halo_readout, set_sharpen_halo_value;
    ChromaAmount => set_chroma_amount_fraction, get_chroma_amount_fraction, set_chroma_amount_readout, set_chroma_amount_value;
    ChromaRadius => set_chroma_radius_fraction, get_chroma_radius_fraction, set_chroma_radius_readout, set_chroma_radius_value;
    Texture => set_texture_fraction, get_texture_fraction, set_texture_readout, set_texture_value;
    PaperRed => set_paper_red_fraction, get_paper_red_fraction, set_paper_red_readout, set_paper_red_value;
    PaperGreen => set_paper_green_fraction, get_paper_green_fraction, set_paper_green_readout, set_paper_green_value;
    PaperBlue => set_paper_blue_fraction, get_paper_blue_fraction, set_paper_blue_readout, set_paper_blue_value;
    GainRed => set_gain_red_fraction, get_gain_red_fraction, set_gain_red_readout, set_gain_red_value;
    GainGreen => set_gain_green_fraction, get_gain_green_fraction, set_gain_green_readout, set_gain_green_value;
    GainBlue => set_gain_blue_fraction, get_gain_blue_fraction, set_gain_blue_readout, set_gain_blue_value;
    FlatAmount => set_flat_amount_fraction, get_flat_amount_fraction, set_flat_amount_readout, set_flat_amount_value;
    FlatTemperature => set_flat_temperature_fraction, get_flat_temperature_fraction, set_flat_temperature_readout, set_flat_temperature_value;
    FlatTint => set_flat_tint_fraction, get_flat_tint_fraction, set_flat_tint_readout, set_flat_tint_value;
    GainTemperature => set_gain_temperature_fraction, get_gain_temperature_fraction, set_gain_temperature_readout, set_gain_temperature_value;
    GainTint => set_gain_tint_fraction, get_gain_tint_fraction, set_gain_tint_readout, set_gain_tint_value;
    RedWhitePoint => set_red_white_point_fraction, get_red_white_point_fraction, set_red_white_point_readout, set_red_white_point_value;
    GreenWhitePoint => set_green_white_point_fraction, get_green_white_point_fraction, set_green_white_point_readout, set_green_white_point_value;
    BlueWhitePoint => set_blue_white_point_fraction, get_blue_white_point_fraction, set_blue_white_point_readout, set_blue_white_point_value;
    RedBlackPoint => set_red_black_point_fraction, get_red_black_point_fraction, set_red_black_point_readout, set_red_black_point_value;
    GreenBlackPoint => set_green_black_point_fraction, get_green_black_point_fraction, set_green_black_point_readout, set_green_black_point_value;
    BlueBlackPoint => set_blue_black_point_fraction, get_blue_black_point_fraction, set_blue_black_point_readout, set_blue_black_point_value;
    WhiteKelvin => set_white_kelvin_fraction, get_white_kelvin_fraction, set_white_kelvin_readout, set_white_kelvin_value;
    WhiteTint => set_white_tint_fraction, get_white_tint_fraction, set_white_tint_readout, set_white_tint_value;
    GrainAmount => set_grain_amount_fraction, get_grain_amount_fraction, set_grain_amount_readout, set_grain_amount_value;
    GrainSize => set_grain_size_fraction, get_grain_size_fraction, set_grain_size_readout, set_grain_size_value;
    HalationAmount => set_halation_amount_fraction, get_halation_amount_fraction, set_halation_amount_readout, set_halation_amount_value;
    HalationSize => set_halation_size_fraction, get_halation_size_fraction, set_halation_size_readout, set_halation_size_value;
    Distortion => set_distortion_fraction, get_distortion_fraction, set_distortion_readout, set_distortion_value;
    FringeRed => set_fringe_red_fraction, get_fringe_red_fraction, set_fringe_red_readout, set_fringe_red_value;
    FringeBlue => set_fringe_blue_fraction, get_fringe_blue_fraction, set_fringe_blue_readout, set_fringe_blue_value;
    LensVignetting => set_lens_vignetting_fraction, get_lens_vignetting_fraction, set_lens_vignetting_readout, set_lens_vignetting_value;
}

pub(super) fn set_modified(g: &Store, b: Block, on: bool) {
    match b {
        Block::Color => g.set_color_modified(on),
        Block::Lighting => g.set_lighting_modified(on),
        Block::Effects => g.set_effects_modified(on),
        Block::DensityCeiling => g.set_density_ceiling_modified(on),
        Block::DyeSeparation => g.set_dye_separation_modified(on),
        Block::Sharpening => g.set_sharpening_modified(on),
        Block::ChromaDenoise => g.set_chroma_denoise_modified(on),
        Block::Texture => g.set_texture_modified(on),
        Block::PaperCorrection => g.set_paper_correction_modified(on),
        Block::SourceBalance => g.set_source_balance_modified(on),
        Block::FlatField => g.set_flat_field_modified(on),
        Block::ChannelPoints => g.set_channel_points_modified(on),
        Block::FilmEffects => g.set_film_effects_modified(on),
        Block::LensCorrections => g.set_lens_corrections_modified(on),
    }
}
