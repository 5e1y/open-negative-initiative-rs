//! What an edit of the open frame changed, at the grain of a setting: the next edit of the same one
//! within the window continues its gesture.

use oni_settings::PipelineSettings;

/// `Several` never coalesces, and a mode flip has no class, so it always falls there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Geometry,
    Chroma,
    Gains,
    Levels,
    Print,
    Saturation,
    Sharpen,
    Correction,
    Several,
}

impl Class {
    pub fn between(from: &PipelineSettings, to: &PipelineSettings) -> Class {
        let notes = [
            (Class::Geometry, from.geometry != to.geometry),
            (Class::Chroma, from.chroma != to.chroma),
            (Class::Gains, from.gains != to.gains),
            (Class::Levels, from.levels != to.levels),
            (Class::Print, from.print != to.print),
            (Class::Saturation, from.saturation != to.saturation),
            (Class::Sharpen, from.sharpen != to.sharpen),
            (Class::Correction, from.corrections != to.corrections),
        ];
        let mut found = None;
        for (class, changed) in notes {
            if changed {
                found = Some(if found.is_none() { class } else { Class::Several });
            }
        }
        found.unwrap_or(Class::Several)
    }
}
