//! The keys the Swift app builds from `KeycapStyle.hardened`: which variant, which albedo, which size.

use crate::style::{KeycapStyle, Rgba, Variant};

/// `DS.Palette.keycap`, the grey of a plain key.
pub const KEYCAP: Rgba = Rgba::new(0x31 as f64 / 255.0, 0x31 as f64 / 255.0, 0x31 as f64 / 255.0);
/// `DS.Palette.canvas`, the scanner's dark key and a pale key's legend.
pub const CANVAS: Rgba = Rgba::new(27.0 / 255.0, 27.0 / 255.0, 27.0 / 255.0);
/// `DS.Palette.accent`, `DSActionButton`'s fill.
pub const ACCENT: Rgba = Rgba::new(10.0 / 255.0, 132.0 / 255.0, 1.0);

/// A key of `variant` with its albedo, as `Keycap.style` composes the caller's colour after the variant.
pub fn key(variant: Variant, albedo: Rgba) -> KeycapStyle {
    let mut style = variant.applied(&KeycapStyle::hardened());
    style.albedo = albedo;
    style
}

/// `DSActionButton`, `AppBar.item` and `AppBar.word`: small keys at the bar's 40 pt.
pub fn bar_key(tint: Option<Rgba>) -> KeycapStyle {
    key(Variant::Small, tint.unwrap_or(KEYCAP))
}

