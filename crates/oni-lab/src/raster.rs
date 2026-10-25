//! `Raster.keycap`: the lab's `KeycapLook` turned into an oni-keycap render at the window's scale,
//! kept by its numbers so a still key is drawn once and a pressed one once per depth step.

use std::cell::RefCell;
use std::collections::HashMap;

use oni_keycap::{Frame, KeycapStyle, Rgba, render_in};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use slint_interpreter::{Struct, Value};

/// The numbers a look carries, in `KeycapLook`'s order; they and the flags make the cache key.
const NUMBERS: [&str; 23] = [
    "width", "height", "depth", "radius", "face-radius", "factor", "margin", "azimuth", "elevation",
    "ambient", "diffuse", "midtone", "wall-top", "wall-bottom", "wall-side", "rise", "dish", "seam-blur",
    "socket-rest", "socket-pressed", "socket-softness", "scale-pressed", "travel",
];
const SHADOW: [&str; 4] = ["shadow-opacity", "shadow-blur", "shadow-y", "bottom-lift"];
const FLAGS: [&str; 3] = ["level", "shadow", "rim"];

/// A press sweeps the depth continuously; 1/256 steps cannot be told apart and bound the cache.
const DEPTH_STEPS: f64 = 256.0;
/// Past this many renders the cache starts over: a lab session moves sliders for hours.
const KEPT: usize = 2048;

#[derive(Default)]
pub struct Raster {
    cache: RefCell<HashMap<Vec<u64>, Image>>,
}

fn number(look: &Struct, name: &str) -> f64 {
    match look.get_field(name) {
        Some(Value::Number(n)) => *n,
        _ => 0.0,
    }
}

fn flag(look: &Struct, name: &str) -> bool {
    matches!(look.get_field(name), Some(Value::Bool(true)))
}

fn colour(look: &Struct) -> slint::Color {
    match look.get_field("albedo") {
        Some(Value::Brush(brush)) => brush.color(),
        _ => slint::Color::from_rgb_u8(0x31, 0x31, 0x31),
    }
}

/// The Swift style a look asks for: the tunables at the large key's lengths, then the variant's factor.
pub fn style(look: &Struct) -> KeycapStyle {
    let n = |name| number(look, name);
    let base = KeycapStyle {
        light_azimuth: n("azimuth"),
        light_elevation: n("elevation"),
        ambient: n("ambient"),
        diffuse: n("diffuse"),
        midtone_match: n("midtone"),
        wall_top: n("wall-top"),
        wall_bottom: n("wall-bottom"),
        wall_left: n("wall-side"),
        wall_right: n("wall-side"),
        rise: n("rise"),
        dish_depth: n("dish"),
        seam_blur: n("seam-blur"),
        socket_shadow_rest: n("socket-rest"),
        socket_shadow_pressed: n("socket-pressed"),
        socket_shadow_softness: n("socket-softness"),
        shadow_opacity: n("shadow-opacity"),
        shadow_blur: n("shadow-blur"),
        shadow_y: n("shadow-y"),
        bottom_lift: n("bottom-lift"),
        scale_pressed: n("scale-pressed"),
        travel: n("travel"),
        ..KeycapStyle::hardened()
    };
    let mut style = base.scaled(n("factor"));
    style.corner_radius = n("radius");
    style.face_radius = n("face-radius");
    let c = colour(look);
    style.albedo = Rgba::from_u8(c.red(), c.green(), c.blue(), c.alpha());
    // `DSBarHandle`: seen head-on, equal walls; caught rather than pressed, no shadow and no rim.
    if flag(look, "level") {
        style.wall_bottom = style.wall_top;
    }
    if !flag(look, "shadow") {
        style.shadow_opacity = 0.0;
    }
    if !flag(look, "rim") {
        style.socket_shadow_rest = 0.0;
        style.socket_shadow_pressed = 0.0;
    }
    style
}

impl Raster {
    /// The key a `KeycapLook` describes, at `scale` pixels per point; an empty image for no size.
    pub fn keycap(&self, look: &Value, scale: f32) -> Value {
        let Value::Struct(look) = look else { return Value::Image(Image::default()) };
        let depth = (number(look, "depth").clamp(0.0, 1.0) * DEPTH_STEPS).round() / DEPTH_STEPS;
        let (width, height) = (number(look, "width"), number(look, "height"));
        if width <= 0.0 || height <= 0.0 || scale <= 0.0 {
            return Value::Image(Image::default());
        }
        let c = colour(look);
        let mut key: Vec<u64> = NUMBERS.iter().chain(&SHADOW).map(|f| number(look, f).to_bits()).collect();
        key.extend(FLAGS.iter().map(|f| flag(look, f) as u64));
        key.extend([depth.to_bits(), (scale as f64).to_bits(), u32::from_ne_bytes([c.red(), c.green(), c.blue(), c.alpha()]) as u64]);
        if let Some(image) = self.cache.borrow().get(&key) {
            return Value::Image(image.clone());
        }
        let frame = Frame { width, height, scale: scale as f64, margin: number(look, "margin") };
        let drawn = render_in(&style(look), frame, depth);
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&drawn.pixels, drawn.width, drawn.height);
        let image = Image::from_rgba8_premultiplied(buffer);
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= KEPT {
            cache.clear();
        }
        cache.insert(key, image.clone());
        Value::Image(image)
    }
}
