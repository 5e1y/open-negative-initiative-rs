//! The export spaces: four matrix spaces, or an ICC profile for a paper or a lab's printer.

use std::path::Path;
use std::sync::LazyLock;

use moxcms::ColorProfile;

use crate::curve::Curve;
use crate::pcs::{self, Mat3};
use crate::profile::OutputProfile;
use crate::working::WORKING_TO_PCS;

/// A matrix space's profile as its file embeds it, read back so the conversion uses the very
/// colorants the file states, rounded to the ICC's 16-bit fractions.
fn write(made: ColorProfile) -> Option<(Vec<u8>, ColorProfile)> {
    let data = made.encode().ok()?;
    let read = ColorProfile::new_from_slice(&data).ok()?;
    Some((data, read))
}

static SRGB: LazyLock<Option<(Vec<u8>, ColorProfile)>> = LazyLock::new(|| write(ColorProfile::new_srgb()));
static DISPLAY_P3: LazyLock<Option<(Vec<u8>, ColorProfile)>> = LazyLock::new(|| write(ColorProfile::new_display_p3()));
static ADOBE_RGB: LazyLock<Option<(Vec<u8>, ColorProfile)>> = LazyLock::new(|| write(ColorProfile::new_adobe_rgb()));
static PRO_PHOTO: LazyLock<Option<(Vec<u8>, ColorProfile)>> = LazyLock::new(|| write(ColorProfile::new_pro_photo_rgb()));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Space {
    Srgb,
    DisplayP3,
    AdobeRgb,
    ProPhoto,
    Profile(OutputProfile),
}

impl Space {
    pub const BUILT_IN: [Space; 4] = [Space::Srgb, Space::DisplayP3, Space::AdobeRgb, Space::ProPhoto];

    /// The name on the space's tab, where five share a panel's width.
    pub fn tab_title(&self) -> String {
        if *self == Space::DisplayP3 { "DP3".into() } else { self.title() }
    }

    /// So wide that one 8-bit step is a visible band: written in TIFF 16-bit only.
    pub fn needs_sixteen_bits(&self) -> bool {
        *self == Space::ProPhoto
    }

    pub fn id(&self) -> String {
        self.key()
    }

    pub fn paper(&self) -> Option<&OutputProfile> {
        match self {
            Space::Profile(p) if OutputProfile::is_printer_at(Path::new(&p.path)) => Some(p),
            _ => None,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Space::Srgb => "sRGB".into(),
            Space::DisplayP3 => "Display P3".into(),
            Space::AdobeRgb => "Adobe RGB".into(),
            Space::ProPhoto => "ProPhoto".into(),
            Space::Profile(p) => p.name.clone(),
        }
    }

    /// What the space is for, under the name a lab or printer will ask for.
    pub fn purpose(&self) -> &'static str {
        match self {
            Space::Srgb => "Web and social networks: what every screen and app reads.",
            Space::DisplayP3 => "Recent phones, tablets and Apple screens: richer reds and greens.",
            Space::AdobeRgb => "Print labs and photo printers: richer cyans and greens.",
            Space::ProPhoto => {
                "Printing at home through the printer's own profile: holds nearly every color the \
                 conversion makes. TIFF 16-bit only."
            }
            Space::Profile(_) => "An ICC profile for a paper or a lab's printer.",
        }
    }

    /// The purpose in a few words, for the canvas chip; a profile's own name already says it.
    pub fn hint(&self) -> &'static str {
        match self {
            Space::Srgb => "web & social",
            Space::DisplayP3 => "phones & screens",
            Space::AdobeRgb => "print",
            Space::ProPhoto => "wide print",
            Space::Profile(_) => "paper",
        }
    }

    pub fn key(&self) -> String {
        match self {
            Space::Srgb => "srgb".into(),
            Space::DisplayP3 => "p3".into(),
            Space::AdobeRgb => "adobergb".into(),
            Space::ProPhoto => "prophoto".into(),
            Space::Profile(p) => format!("icc:{}", p.path),
        }
    }

    /// The stored key read back; a profile keeps its path even once the file is gone, so the
    /// export reports it missing instead of quietly writing sRGB.
    pub fn from_key(key: &str) -> Option<Space> {
        if let Some(path) = key.strip_prefix("icc:") {
            // A profile on disk but no print target reads as the default: Custom takes printers only.
            if Path::new(path).exists() && !OutputProfile::is_printer_at(Path::new(path)) {
                return None;
            }
            return Some(Space::Profile(OutputProfile::new(path)));
        }
        Self::BUILT_IN.into_iter().find(|s| s.key() == key)
    }

    pub(crate) fn written(&self) -> Option<&'static (Vec<u8>, ColorProfile)> {
        let held = match self {
            Space::Srgb => &SRGB,
            Space::DisplayP3 => &DISPLAY_P3,
            Space::AdobeRgb => &ADOBE_RGB,
            Space::ProPhoto => &PRO_PHOTO,
            Space::Profile(_) => return None,
        };
        held.as_ref()
    }

    /// The space's ICC profile; `None` only for a profile whose file cannot be read.
    pub fn icc_profile(&self) -> Option<ColorProfile> {
        match self {
            Space::Profile(p) => p.loaded().map(|l| l.profile.clone()),
            matrix => matrix.written().map(|w| w.1.clone()),
        }
    }

    /// The bytes a file in this space embeds.
    pub fn icc_data(&self) -> Option<Vec<u8>> {
        match self {
            Space::Profile(p) => p.loaded().map(|l| l.data.clone()),
            matrix => matrix.written().map(|w| w.0.clone()),
        }
    }

    /// Working light to the space's own linear light; `None` for a profile.
    pub fn from_working(&self) -> Option<Mat3> {
        if let Space::Profile(_) = self {
            return None;
        }
        let colorants = self.icc_profile()?.rgb_to_xyz_matrix().v;
        Some(pcs::matmul(&pcs::inverse(&colorants)?, &WORKING_TO_PCS))
    }

    /// Working-space RGBA as a matrix space writes it: clipped, then on the space's own curve.
    pub fn encoded(&self, pixels: &[f32]) -> Option<Vec<f32>> {
        let m = self.from_working()?;
        let profile = self.icc_profile()?;
        let curves: Vec<Curve> = [&profile.red_trc, &profile.green_trc, &profile.blue_trc]
            .iter()
            .map(|t| Curve::new(t.as_ref()?))
            .collect::<Option<_>>()?;
        let mut out = pixels.to_vec();
        for p in out.chunks_exact_mut(4) {
            let linear = pcs::mul_f32(&m, [p[0], p[1], p[2]]);
            for c in 0..3 {
                p[c] = (curves[c].inverse(linear[c].clamp(0.0, 1.0) as f64) as f32).clamp(0.0, 1.0);
            }
        }
        Some(out)
    }
}
