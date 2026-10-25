//! `Raster.keycap`: the design system's `KeycapLook` read from the interpreter's struct, drawn and kept
//! by oni-keycap's `Cache`, the mapping oni-app shares.

use std::cell::RefCell;

use oni_keycap::{Cache, Look};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use slint_interpreter::{Struct, Value};

/// A lab session moves sliders for hours: past this many bytes of renders the cache starts over.
const BUDGET: usize = 256 << 20;

pub struct Raster {
    cache: RefCell<Cache<Image>>,
}

impl Default for Raster {
    fn default() -> Self {
        Self { cache: RefCell::new(Cache::new(BUDGET)) }
    }
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

/// The interpreter's `KeycapLook`, field by field.
pub fn look(s: &Struct) -> Look {
    let n = |name| number(s, name);
    let c = match s.get_field("albedo") {
        Some(Value::Brush(brush)) => brush.color(),
        _ => slint::Color::from_rgb_u8(0x31, 0x31, 0x31),
    };
    Look {
        width: n("width"),
        height: n("height"),
        depth: n("depth"),
        albedo: [c.red(), c.green(), c.blue(), c.alpha()],
        radius: n("radius"),
        face_radius: n("face-radius"),
        factor: n("factor"),
        margin: n("margin"),
        scale: n("scale"),
        level: flag(s, "level"),
        shadow: flag(s, "shadow"),
        rim: flag(s, "rim"),
        azimuth: n("azimuth"),
        elevation: n("elevation"),
        ambient: n("ambient"),
        diffuse: n("diffuse"),
        midtone: n("midtone"),
        wall_top: n("wall-top"),
        wall_bottom: n("wall-bottom"),
        wall_side: n("wall-side"),
        rise: n("rise"),
        dish: n("dish"),
        seam_blur: n("seam-blur"),
        socket_rest: n("socket-rest"),
        socket_pressed: n("socket-pressed"),
        socket_softness: n("socket-softness"),
        scale_pressed: n("scale-pressed"),
        travel: n("travel"),
        shadow_opacity: n("shadow-opacity"),
        shadow_blur: n("shadow-blur"),
        shadow_y: n("shadow-y"),
        bottom_lift: n("bottom-lift"),
    }
}

impl Raster {
    /// The key a `KeycapLook` describes, at its own scale or else `scale`; an empty image for no size.
    pub fn keycap(&self, value: &Value, scale: f32) -> Value {
        let Value::Struct(s) = value else { return Value::Image(Image::default()) };
        let made = self.cache.borrow_mut().get(&look(s), scale as f64, |drawn| {
            let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&drawn.pixels, drawn.width, drawn.height);
            Image::from_rgba8_premultiplied(buffer)
        });
        Value::Image(made.unwrap_or_default())
    }
}
