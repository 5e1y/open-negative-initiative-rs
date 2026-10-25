//! The Swift app's keycap, rasterised on the CPU so Slint can show what it cannot draw: the walls'
//! seam blur and the socket's blurred rim. Deterministic: the same style and size give the same bytes.

pub mod blur;
pub mod instances;
pub mod light;
pub mod look;
pub mod shape;
pub mod style;
pub mod surface;

#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests;

pub use style::{KeycapStyle, Motion, Rgba, Timing, Variant};
pub use surface::Frame;
pub use look::{Cache, Look, Stats};

/// A rendered key: premultiplied RGBA8, the key's top-left `margin` points in from the image's.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub margin: f64,
    pub pixels: Vec<u8>,
}

/// The room the cast shadow needs past the key on every side, in points.
pub fn margin(style: &KeycapStyle) -> f64 {
    let sigma = surface::SHADOW_SIGMA_PER_RADIUS * style.shadow_blur;
    let reach = style.shadow_y.abs().max(style.press_shadow_y.abs());
    (3.0 * sigma + reach + 1.0).ceil()
}

/// A key of `width` × `height` points at `depth` (0 rest, 1 pressed), at `scale` pixels per point.
pub fn render(style: &KeycapStyle, width: f64, height: f64, depth: f64, scale: f64) -> Image {
    render_in(style, Frame { width, height, scale, margin: margin(style) }, depth)
}

/// As `render`, with the margin chosen by the caller.
pub fn render_in(style: &KeycapStyle, frame: Frame, depth: f64) -> Image {
    let plane = surface::compose(style, frame, depth);
    let pixels = plane.data.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    Image { width: plane.width as u32, height: plane.height as u32, margin: frame.margin, pixels }
}
