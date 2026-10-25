//! The keys the Swift app builds from `KeycapStyle.hardened`: which variant, which albedo, which size.

use crate::style::{KeycapStyle, Rgba, Variant};

/// `DS.Palette.keycap`, the grey of a plain key.
pub const KEYCAP: Rgba = Rgba::new(0x31 as f64 / 255.0, 0x31 as f64 / 255.0, 0x31 as f64 / 255.0);
/// `DS.Palette.canvas`, the scanner's dark key and a pale key's legend.
pub const CANVAS: Rgba = Rgba::new(27.0 / 255.0, 27.0 / 255.0, 27.0 / 255.0);
/// `DS.Palette.textOnFill`, a pale key and every legend on a plain one.
pub const TEXT_ON_FILL: Rgba = Rgba::white(1.0);
/// `DS.Palette.accent`, `DSActionButton`'s fill.
pub const ACCENT: Rgba = Rgba::new(10.0 / 255.0, 132.0 / 255.0, 1.0);

/// `DSPanelButton.Height`: the cap's outer height in points, and its variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelHeight {
    Standard,
    Plate,
    Wide,
    Mini,
}

impl PanelHeight {
    pub fn points(self) -> f64 {
        match self {
            PanelHeight::Standard => 40.0,
            PanelHeight::Plate => 52.0,
            PanelHeight::Wide => 60.0,
            PanelHeight::Mini => 26.0,
        }
    }

    pub fn variant(self) -> Variant {
        if self == PanelHeight::Mini { Variant::Mini } else { Variant::Small }
    }
}

/// A key of `variant` with its albedo, as `Keycap.style` composes the caller's colour after the variant.
pub fn key(variant: Variant, albedo: Rgba) -> KeycapStyle {
    let mut style = variant.applied(&KeycapStyle::hardened());
    style.albedo = albedo;
    style
}

/// `DSPanelButton`: a tint, else white when pale, else the keycap grey.
pub fn panel_button(height: PanelHeight, pale: bool, tint: Option<Rgba>) -> KeycapStyle {
    key(height.variant(), tint.unwrap_or(if pale { TEXT_ON_FILL } else { KEYCAP }))
}

/// `DSActionButton`, `AppBar.item` and `AppBar.word`: small keys at the bar's 40 pt.
pub fn bar_key(tint: Option<Rgba>) -> KeycapStyle {
    key(Variant::Small, tint.unwrap_or(KEYCAP))
}

/// The scanner keypad's keys: small, 46 pt high, coloured whole by their band.
pub fn scanner_key(albedo: Rgba) -> KeycapStyle {
    key(Variant::Small, albedo)
}

/// `KeycapPlate`'s radius: the cap's outer one, so the plate follows the contour.
pub fn plate_radius(variant: Variant) -> f64 {
    let style = variant.applied(&KeycapStyle::hardened());
    style.corner_radius + style.contour_width
}
