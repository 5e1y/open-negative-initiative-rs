//! The export plate's keys: each a complete state on the manual rows, hiding nothing.

use oni_colour::Space;
use serde::{Deserialize, Serialize};

use crate::resize::Resize;
use crate::settings::{Format, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preset {
    #[serde(rename = "Instagram")]
    Instagram,
    #[serde(rename = "Display")]
    FullDisplay,
    #[serde(rename = "Print")]
    FullPrint,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Instagram, Preset::FullDisplay, Preset::FullPrint];

    /// The key's name, also its identity.
    pub fn raw_value(self) -> &'static str {
        match self {
            Preset::Instagram => "Instagram",
            Preset::FullDisplay => "Display",
            Preset::FullPrint => "Print",
        }
    }

    /// Every field written, so where the hand started never shows.
    pub fn applied(self, current: &Settings) -> Settings {
        let mut out = current.clone();
        match self {
            Preset::Instagram => {
                out.set_format(Format::Jpeg);
                out.set_space(Space::Srgb);
                // 1440 across the narrow way clears the feed's 1080 re-encode on every ratio it takes.
                out.resize = Resize::ShortestEdge(1440);
                out.compress_tiff = true;
                out.jpeg_quality = 0.92;
                out.hdr = false;
            }
            Preset::FullDisplay => {
                out.set_format(Format::Jpeg);
                // Display P3 is what Apple panels render; a wider space shows on a phone what the canvas never did.
                out.set_space(Space::DisplayP3);
                out.resize = Resize::Full;
                out.compress_tiff = true;
                out.jpeg_quality = 1.0;
                out.hdr = false;
            }
            Preset::FullPrint => {
                out.set_format(Format::Tiff16);
                out.set_space(Space::AdobeRgb);
                out.resize = Resize::Full;
                out.compress_tiff = true;
                out.jpeg_quality = 1.0;
                out.hdr = false;
            }
        }
        out
    }

    /// The key the current state equals, read off the state itself and never off the last
    /// click, so a hand-edited state lights nothing and a state typed back lights its key again.
    pub fn matching(settings: &Settings) -> Option<Preset> {
        Self::ALL.into_iter().find(|p| p.applied(settings) == *settings)
    }
}
