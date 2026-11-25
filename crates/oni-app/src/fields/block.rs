//! The panel's blocks, each the fields its reset key clears.

use super::Field;

/// The blocks a reset key clears, in the order of the Slint `Store`'s `*-modified` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Color,
    Lighting,
    Effects,
    DensityCeiling,
    Sharpening,
    ChromaDenoise,
    Texture,
    PaperCorrection,
    SourceBalance,
    FlatField,
    ChannelPoints,
    FilmEffects,
    LensCorrections,
    Film,
}

impl Block {
    pub const ALL: [Block; 14] = [
        Block::Color,
        Block::Lighting,
        Block::Effects,
        Block::DensityCeiling,
        Block::Sharpening,
        Block::ChromaDenoise,
        Block::Texture,
        Block::PaperCorrection,
        Block::SourceBalance,
        Block::FlatField,
        Block::ChannelPoints,
        Block::FilmEffects,
        Block::LensCorrections,
        Block::Film,
    ];

    pub fn from_index(i: i32) -> Option<Block> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub fn fields(self) -> &'static [Field] {
        use Field::*;
        match self {
            Block::Color => &[Temperature, Tint, ColorDensity],
            Block::Lighting => &[Exposure, Brightness, Contrast, Highlights, Shadows, WhitePoint, BlackPoint],
            Block::Effects => &[Vignette, Chrome],
            Block::DensityCeiling => &[DensityCeiling],
            Block::Sharpening => &[SharpenAmount, SharpenRadius, SharpenHalo],
            Block::ChromaDenoise => &[ChromaAmount, ChromaRadius],
            Block::Texture => &[Texture],
            Block::PaperCorrection => &[PaperRed, PaperGreen, PaperBlue],
            Block::SourceBalance => &[GainRed, GainGreen, GainBlue, GainTemperature, GainTint],
            Block::FlatField => &[FlatAmount, FlatTemperature, FlatTint],
            Block::ChannelPoints => &[
                RedWhitePoint,
                GreenWhitePoint,
                BlueWhitePoint,
                RedBlackPoint,
                GreenBlackPoint,
                BlueBlackPoint,
                RedMid,
                GreenMid,
                BlueMid,
                RedContrast,
                GreenContrast,
                BlueContrast,
            ],
            Block::FilmEffects => &[GrainAmount, GrainSize, HalationAmount, HalationSize, Bloom],
            Block::LensCorrections => &[Distortion, FringeRed, FringeBlue, LensVignetting],
            Block::Film => &[FilmExposure],
        }
    }
}
