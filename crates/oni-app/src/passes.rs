//! A frame's three passes read alone, off the interface thread: each file's shares and picture for
//! the Pass Order's chips, and the bands the registration measures.

use crate::display::{Params, Screen};
use crate::render;
use crate::roll_previews::Bitmap;
use oni_library::rgb_passes;
use oni_measure::registration::{self, Plane};
use oni_settings::frame::{Channel, RgbPasses};
use std::path::Path;

/// One file of the first frame as the chips show it: its light per channel and its picture.
#[derive(Clone, Debug)]
pub struct PassRead {
    pub path: String,
    pub share: [f64; 3],
    pub picture: Bitmap,
}

/// The file decoded alone at `READ_SIDE`, its shares and its uninverted picture; `None` for a
/// file unreadable or black.
pub fn read(path: &str) -> Option<PassRead> {
    let d = render::decode_alone(Path::new(path), Some(rgb_passes::READ_SIDE)).ok()?;
    let share = rgb_passes::shares(&d.linear.pixels)?;
    let (w, h) = (d.linear.width, d.linear.height);
    let rgba = render::encoded(&d.linear, &Params::new(Screen::Srgb, w as u32, h as u32, 0), 1);
    Some(PassRead { path: path.to_string(), share, picture: Bitmap { rgba, width: w, height: h } })
}

/// `RGBRegistration.planes`: the three passes of a frame, each its own band, decoded alone.
pub fn planes(lead: &str, passes: &RgbPasses, side: Option<f64>) -> Option<[Plane; 3]> {
    let folder = oni_library::paths::parent(lead);
    let band = |c: Channel| {
        let d = render::decode_alone(Path::new(&oni_library::paths::join(&folder, passes.name(c))), side).ok()?;
        Plane::channel(d.linear.width, d.linear.height, &d.linear.pixels, c.index())
    };
    let (red, green, blue) = (band(Channel::Red)?, band(Channel::Green)?, band(Channel::Blue)?);
    Some([red, green, blue])
}

/// Each triplet's registration on the roll's one magnification; a frame it cannot read keeps none.
pub struct Registrar {
    magnification: Option<(f64, f64)>,
}

impl Registrar {
    /// The magnification read once, on the roll's first triplets at full resolution.
    pub fn new(triplets: &[(String, RgbPasses)]) -> Registrar {
        let magnification = if triplets.is_empty() { None } else { registration::magnification(triplets, &planes) };
        Registrar { magnification }
    }

    /// `passes` with its registration measured on its own preview, when the roll's magnification is known.
    pub fn registered(&self, lead: &str, mut passes: RgbPasses) -> RgbPasses {
        if let Some(m) = self.magnification
            && let Some(r) = registration::registration(lead, &passes, m, &planes)
        {
            passes.registration = Some(r);
        }
        passes
    }
}
